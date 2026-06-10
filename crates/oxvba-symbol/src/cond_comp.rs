//! VBA conditional compilation (`#If` / `#ElseIf` / `#Else` / `#End If` / `#Const`).
//!
//! This is a **pre-parse** pass: VBA evaluates `#If` at compile time against the
//! conditional-compilation constants and only the active branch is compiled — an
//! inactive branch is not even syntax-checked. So [`preprocess`] walks the source a
//! logical line at a time, evaluates the directives against the constant
//! environment, and blanks every directive line and inactive-branch line (replacing
//! content with spaces, preserving byte offsets + line numbers for diagnostics)
//! before the source reaches `oxvba_syntax::parse`.
//!
//! The constant environment layers, lowest first: the **predefined** host constants
//! ([`predefined_cc_constants`] — `Win64`/`VBA7`/`Win32` true on a 64-bit Windows
//! runtime), the project `DefineConstants`, and module-level `#Const`s accumulated
//! top-to-bottom. `#If` conditions reuse the constant-expression evaluator
//! ([`crate::const_eval`]) so their semantics match VBA `Const` semantics exactly.

use std::collections::{BTreeMap, HashMap};

use oxvba_bundle::StringCompareMode;
use oxvba_bundle::coreir::CoreConst;
use oxvba_syntax::SyntaxNode;

use crate::const_eval::{
    core_binop, fold_const_binary, fold_const_literal, negate_const, not_const,
};
use crate::model::fold_identifier;

/// Conditional-compilation constants, keyed by folded (case-insensitive) name.
pub type CcConstants = HashMap<String, CoreConst>;

/// The predefined conditional-compilation constants for a 64-bit Windows runtime.
/// VBA `True` is `-1`; `Win32` is true on **both** 32- and 64-bit Windows (it marks
/// the Win32 API base), while `Win64` is true only on 64-bit.
pub fn predefined_cc_constants() -> CcConstants {
    [
        ("vba7", true),
        ("win64", true),
        ("win32", true),
        ("vba6", false),
        ("win16", false),
        ("mac", false),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), CoreConst::Bool(value)))
    .collect()
}

/// The base conditional-compilation environment: predefined host constants plus the
/// project's `DefineConstants`. Module-level `#Const`s are layered on per module
/// inside [`preprocess`].
pub fn base_cc_constants(project: &BTreeMap<String, i32>) -> CcConstants {
    let mut cc = predefined_cc_constants();
    for (name, value) in project {
        cc.insert(fold_identifier(name), CoreConst::I32(*value));
    }
    cc
}

/// Strip inactive `#If` branches and all directive lines from `source`, evaluating
/// the directives against `base` (+ any module `#Const`s). The result has the same
/// byte length as `source` (stripped content → spaces), so parse-error offsets stay
/// aligned with the original. Errors on unbalanced directives.
pub fn preprocess(source: &str, base: &CcConstants) -> Result<String, String> {
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut cc = base.clone();
    let mut stack: Vec<Frame> = Vec::new();
    let mut out = String::with_capacity(source.len());

    let mut i = 0;
    while i < lines.len() {
        // Gather one logical line (joining `_` continuations) for directive parsing.
        let start = i;
        let mut logical = String::new();
        loop {
            let line = lines[i];
            logical.push_str(line_body(line, continues(line)));
            i += 1;
            if i >= lines.len() || !continues(lines[i - 1]) {
                break;
            }
            logical.push(' ');
        }
        let physical = &lines[start..i];

        let is_directive = process_logical(logical.trim_start(), &mut stack, &mut cc)?;
        let keep = !is_directive && emitting(&stack);
        for line in physical {
            if keep {
                out.push_str(line);
            } else {
                out.push_str(&blank_line(line));
            }
        }
    }

    if !stack.is_empty() {
        return Err("conditional compilation: `#If` without a matching `#End If`".to_string());
    }
    Ok(out)
}

/// One `#If` block frame. `parent_active` is whether the enclosing context emitted
/// when this `#If` opened; `taken` records whether any branch has matched yet;
/// `branch_active` is whether the *current* branch emits.
struct Frame {
    parent_active: bool,
    taken: bool,
    branch_active: bool,
}

/// Whether the current point emits code (all enclosing branches active).
fn emitting(stack: &[Frame]) -> bool {
    stack.last().is_none_or(|f| f.branch_active)
}

/// Process a logical line's leading text. Returns `Ok(true)` if it was a CC
/// directive (the line should be blanked), `Ok(false)` for ordinary code.
fn process_logical(
    text: &str,
    stack: &mut Vec<Frame>,
    cc: &mut CcConstants,
) -> Result<bool, String> {
    let lower = text.to_ascii_lowercase();

    if directive_at(&lower, "#const") {
        if emitting(stack)
            && let Some((name, expr)) = split_const(&text["#const".len()..])
        {
            let value = evaluate(expr, cc);
            cc.insert(fold_identifier(name), value);
        }
        return Ok(true);
    }
    // `#ElseIf` must be tested before `#Else` (it has the `#else` prefix).
    if directive_at(&lower, "#elseif") {
        let condition = directive_condition(&text["#elseif".len()..]);
        match stack.last_mut() {
            Some(frame) => {
                let active =
                    frame.parent_active && !frame.taken && truthy(&evaluate(condition, cc));
                frame.branch_active = active;
                frame.taken |= active;
            }
            None => return Err("conditional compilation: `#ElseIf` without `#If`".to_string()),
        }
        return Ok(true);
    }
    if directive_at(&lower, "#else") {
        match stack.last_mut() {
            Some(frame) => {
                frame.branch_active = frame.parent_active && !frame.taken;
                frame.taken = true;
            }
            None => return Err("conditional compilation: `#Else` without `#If`".to_string()),
        }
        return Ok(true);
    }
    if directive_at(&lower, "#end") && lower["#end".len()..].trim_start().starts_with("if") {
        if stack.pop().is_none() {
            return Err("conditional compilation: `#End If` without `#If`".to_string());
        }
        return Ok(true);
    }
    if directive_at(&lower, "#if") {
        let condition = directive_condition(&text["#if".len()..]);
        let parent_active = emitting(stack);
        let active = parent_active && truthy(&evaluate(condition, cc));
        stack.push(Frame {
            parent_active,
            taken: active,
            branch_active: active,
        });
        return Ok(true);
    }
    Ok(false)
}

/// True if `lower` (already lowercased + left-trimmed) starts the directive `kw`
/// followed by a boundary (whitespace, `(`, `=`, or end) — so `#ifx`/`#consts`
/// don't match.
fn directive_at(lower: &str, kw: &str) -> bool {
    lower.strip_prefix(kw).is_some_and(|rest| {
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == '(' || c == '=')
    })
}

/// Evaluate a `#If`/`#ElseIf` condition (the text between the directive and `Then`)
/// or a `#Const` initializer against the constant environment. Reuses the real VBA
/// expression grammar + the constant-expression folder; an undefined constant or an
/// unevaluable expression is `Empty` (false).
fn evaluate(expr_text: &str, cc: &CcConstants) -> CoreConst {
    let parse = oxvba_syntax::parse_expression(expr_text);
    match parse
        .syntax()
        .child_nodes()
        .into_iter()
        .find(|n| n.kind().is_expr())
    {
        Some(expr) => eval_cc(expr, cc),
        None => CoreConst::Empty,
    }
}

/// Fold a conditional-compilation expression CST node to a constant. Mirrors
/// `const_eval::eval_inner` but resolves identifiers against the CC constant map
/// (undefined → `Empty`) instead of the symbol table.
fn eval_cc(node: SyntaxNode<'_>, cc: &CcConstants) -> CoreConst {
    use oxvba_syntax::SyntaxKind;
    match node.kind() {
        SyntaxKind::LiteralExpr => fold_const_literal(node).unwrap_or(CoreConst::Empty),
        SyntaxKind::ParenExpr => node
            .paren_inner()
            .map_or(CoreConst::Empty, |inner| eval_cc(inner, cc)),
        SyntaxKind::IdentExpr => node
            .ident_name_token()
            .and_then(|tok| cc.get(&fold_identifier(tok.text)).cloned())
            .unwrap_or(CoreConst::Empty),
        SyntaxKind::UnaryExpr => {
            let Some(operand) = node.unary_operand() else {
                return CoreConst::Empty;
            };
            let inner = eval_cc(operand, cc);
            match node.unary_op_token().map(|t| t.kind) {
                Some(SyntaxKind::Plus) => inner,
                Some(SyntaxKind::Minus) => negate_const(inner).unwrap_or(CoreConst::Empty),
                Some(SyntaxKind::KwNot) => not_const(inner).unwrap_or(CoreConst::Empty),
                _ => CoreConst::Empty,
            }
        }
        SyntaxKind::BinaryExpr => {
            let (Some(op), Some(lhs), Some(rhs)) =
                (node.binary_op_token(), node.binary_lhs(), node.binary_rhs())
            else {
                return CoreConst::Empty;
            };
            let Some(binop) = core_binop(op.kind) else {
                return CoreConst::Empty;
            };
            let lv = eval_cc(lhs, cc);
            let rv = eval_cc(rhs, cc);
            // CC expressions are not under an `Option Compare`; use Binary.
            fold_const_binary(binop, &lv, &rv, StringCompareMode::Binary)
                .unwrap_or(CoreConst::Empty)
        }
        _ => CoreConst::Empty,
    }
}

/// VBA truthiness of a `#If` condition value: a non-zero number / `True`; an
/// undefined constant (`Empty`) and `Null`/`Nothing`/empty string are false.
fn truthy(value: &CoreConst) -> bool {
    match value {
        CoreConst::Bool(b) => *b,
        CoreConst::I32(n) => *n != 0,
        CoreConst::I64(n) | CoreConst::Currency(n) => *n != 0,
        CoreConst::F64(bits) | CoreConst::Date(bits) => f64::from_bits(*bits) != 0.0,
        CoreConst::F32(bits) => f32::from_bits(*bits) != 0.0,
        CoreConst::Str(s) => !s.is_empty(),
        CoreConst::Empty | CoreConst::Null | CoreConst::Nothing => false,
    }
}

/// The condition text of `#If`/`#ElseIf <cond> Then`: the text after the directive
/// keyword, with a trailing line comment and the trailing `Then` removed.
fn directive_condition(rest: &str) -> &str {
    let s = strip_trailing_comment(rest).trim();
    let bytes = s.as_bytes();
    if s.len() >= 4 && s[s.len() - 4..].eq_ignore_ascii_case("then") {
        let boundary_ok = s.len() == 4 || matches!(bytes[s.len() - 5], b' ' | b'\t' | b')');
        if boundary_ok {
            return s[..s.len() - 4].trim_end();
        }
    }
    s
}

/// Split a `#Const` body `name = expr` into its name and initializer (comment
/// stripped); `None` if there is no `=`.
fn split_const(rest: &str) -> Option<(&str, &str)> {
    let body = strip_trailing_comment(rest);
    let (name, expr) = body.split_once('=')?;
    Some((name.trim(), expr.trim()))
}

/// Drop a trailing `'…` line comment, respecting `"…"` string literals.
fn strip_trailing_comment(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_string = !in_string,
            b'\'' if !in_string => return &s[..i],
            _ => {}
        }
        i += 1;
    }
    s
}

/// Whether a physical line ends with a VBA line continuation (` _` before the EOL).
fn continues(line: &str) -> bool {
    let body = line.trim_end_matches(['\r', '\n']);
    let trimmed = body.trim_end();
    trimmed.len() < body.len() && trimmed.ends_with('_') // whitespace then `_` at end
}

/// A physical line's content for logical-line joining: without its EOL, and without
/// a trailing ` _` continuation marker when `is_continuation`.
fn line_body(line: &str, is_continuation: bool) -> &str {
    let body = line.trim_end_matches(['\r', '\n']);
    if is_continuation {
        body.trim_end().trim_end_matches('_')
    } else {
        body
    }
}

/// Replace a line's content with spaces (preserving its EOL and byte length) so that
/// stripped directive/inactive lines keep parse offsets aligned with the original.
fn blank_line(line: &str) -> String {
    let eol_len = line.len() - line.trim_end_matches(['\r', '\n']).len();
    let content_len = line.len() - eol_len;
    let mut blanked = " ".repeat(content_len);
    blanked.push_str(&line[content_len..]);
    blanked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cc() -> CcConstants {
        base_cc_constants(&BTreeMap::new())
    }

    /// Active lines survive; directive + inactive lines become blank, same length.
    fn run(src: &str) -> String {
        preprocess(src, &cc()).expect("preprocess")
    }

    #[test]
    fn win64_branch_is_kept_else_dropped() {
        let out =
            run("#If Win64 Then\r\nDim p As LongLong\r\n#Else\r\nDim p As Long\r\n#End If\r\n");
        assert!(out.contains("LongLong"), "{out:?}");
        assert!(
            !out.contains("As Long\r"),
            "Win32 branch should be stripped: {out:?}"
        );
        assert_eq!(
            out.len(),
            "#If Win64 Then\r\nDim p As LongLong\r\n#Else\r\nDim p As Long\r\n#End If\r\n".len()
        );
    }

    #[test]
    fn false_if_keeps_else() {
        let out = run("#If Win16 Then\nDim a\n#Else\nDim b\n#End If\n");
        assert!(!out.contains("Dim a"), "{out:?}");
        assert!(out.contains("Dim b"), "{out:?}");
    }

    #[test]
    fn elseif_chain_picks_first_true() {
        let out = run(
            "#If Mac Then\nx1\n#ElseIf Win64 Then\nx2\n#ElseIf VBA7 Then\nx3\n#Else\nx4\n#End If\n",
        );
        assert!(
            out.contains("x2") && !out.contains("x1") && !out.contains("x3") && !out.contains("x4"),
            "{out:?}"
        );
    }

    #[test]
    fn nested_if_in_inactive_branch_stays_inactive() {
        let out = run("#If Mac Then\n#If Win64 Then\ninner\n#End If\n#Else\nouter\n#End If\n");
        assert!(!out.contains("inner"), "{out:?}");
        assert!(out.contains("outer"), "{out:?}");
    }

    #[test]
    fn module_const_drives_if() {
        let out = run("#Const DEBUGMODE = 1\n#If DEBUGMODE Then\nlogit\n#Else\nquiet\n#End If\n");
        assert!(out.contains("logit") && !out.contains("quiet"), "{out:?}");
    }

    #[test]
    fn project_constant_visible() {
        let mut project = BTreeMap::new();
        project.insert("FEATURE".to_string(), 1);
        let out = preprocess(
            "#If FEATURE Then\non\n#Else\noff\n#End If\n",
            &base_cc_constants(&project),
        )
        .expect("preprocess");
        assert!(out.contains("on") && !out.contains("off"), "{out:?}");
    }

    #[test]
    fn undefined_constant_is_false() {
        let out = run("#If NotDefined Then\nyes\n#Else\nno\n#End If\n");
        assert!(out.contains("no") && !out.contains("yes"), "{out:?}");
    }

    #[test]
    fn boolean_expression_and_or_not() {
        let out = run("#If Win64 And Not Mac Then\nok\n#Else\nbad\n#End If\n");
        assert!(out.contains("ok") && !out.contains("bad"), "{out:?}");
    }

    #[test]
    fn directive_with_trailing_comment() {
        let out = run("#If Win64 Then ' 64-bit only\nok\n#End If\n");
        assert!(out.contains("ok"), "{out:?}");
    }

    #[test]
    fn unbalanced_if_errors() {
        assert!(preprocess("#If Win64 Then\nx\n", &cc()).is_err());
        assert!(preprocess("x\n#End If\n", &cc()).is_err());
    }

    #[test]
    fn non_directive_hash_untouched() {
        // A date literal mid-expression is not a directive and must survive.
        let out = run("d = #1/2/2020#\n");
        assert!(out.contains("#1/2/2020#"), "{out:?}");
    }
}
