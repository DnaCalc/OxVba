//! Compile-time `Const` / `Enum`-member value evaluation — part of the **published
//! type system**, not the binder.
//!
//! A constant's value is metadata a project publishes (like a typelib's enum/const
//! values), so it is folded here, where the surface is built, and read uniformly by
//! both the project's own binder and any referrer. Relocated from `oxvba-bind`
//! (`expr.rs`/`ids.rs`/`date.rs`) and re-expressed against the `SymbolTable` (the
//! `ResolutionEnvironment` does not exist yet when the surface is synthesized): name
//! references resolve through the scope chain + the static VBA-library constant
//! table, not through providers.

use std::collections::{HashMap, HashSet};

use oxvba_bundle::coreir::{CoreBinOp, CoreConst};
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::model::{LibraryConstValue, ScopeId, SymbolId, SymbolNamespace, SymbolTable};
use crate::providers::vba_library;

/// Fold every `Const` and `Enum` member reachable from the given module roots into
/// a `SymbolId → value` map. Module- *and* proc-level consts are included (the
/// binder reads proc-level consts too); failures are simply absent (a referrer/
/// binder reading an absent value reports the error). One call per resolution
/// environment covers the active project + every referenced project, since all are
/// scanned into the one `SymbolTable`.
pub fn fold_const_values(
    symbols: &SymbolTable,
    module_roots: &[(ScopeId, SyntaxNode<'_>)],
) -> HashMap<SymbolId, CoreConst> {
    // 1) Collect plain `Const` declarators (module- and proc-level).
    let mut pending: Vec<(ScopeId, SymbolId, SyntaxNode<'_>)> = Vec::new();
    let mut const_syms: HashSet<SymbolId> = HashSet::new();
    for (module_scope, root) in module_roots {
        collect_consts(symbols, *module_scope, *root, &mut pending, &mut const_syms);
    }
    // 2) Fold them by fixed point (forward + cross-const references).
    let mut values = resolve_const_worklist(symbols, pending, &const_syms);
    // 3) Fold `Enum` members (sequential auto-increment, reading earlier values).
    for (module_scope, root) in module_roots {
        fold_enums(symbols, *module_scope, *root, &mut values);
    }
    values
}

/// Walk for `ConstStmt`s under `scope`; proc bodies open their own `Procedure`
/// scope, so recurse into proc decls with that scope. (Mirrors the binder's two
/// collection sites in one walk — block scoping is flattened to the proc scope.)
fn collect_consts<'a>(
    symbols: &SymbolTable,
    scope: ScopeId,
    node: SyntaxNode<'a>,
    pending: &mut Vec<(ScopeId, SymbolId, SyntaxNode<'a>)>,
    const_syms: &mut HashSet<SymbolId>,
) {
    if node.kind() == SyntaxKind::ConstStmt {
        for declarator in node.declarators() {
            let Some(name) = declarator.declarator_name() else { continue };
            let Some(init) = declarator.first_expr_child() else { continue };
            if let Ok(Some(sym)) = symbols.find_in_scope(scope, SymbolNamespace::Local, name.text) {
                const_syms.insert(sym);
                pending.push((scope, sym, init));
            }
        }
    }
    for child in node.child_nodes() {
        let child_scope = match child.kind() {
            SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl => child
                .proc_name_token()
                .and_then(|t| proc_scope_under(symbols, scope, t.text))
                .unwrap_or(scope),
            _ => scope,
        };
        collect_consts(symbols, child_scope, child, pending, const_syms);
    }
}

/// The `Procedure` scope under `module_scope` whose name folds to `name` — for
/// resolving proc-level const declarators in the right scope.
fn proc_scope_under(symbols: &SymbolTable, module_scope: ScopeId, name: &str) -> Option<ScopeId> {
    let folded = crate::model::fold_identifier(name.trim_start_matches('[').trim_end_matches(']'));
    symbols.scopes().iter().find_map(|s| {
        if s.parent != Some(module_scope) || s.kind != crate::model::ScopeKind::Procedure {
            return None;
        }
        let n = s.name.and_then(|id| symbols.name(id))?;
        (n.folded == folded).then_some(s.id)
    })
}

/// Fold `Enum` members in source order: a member with an explicit initializer takes
/// that folded value (and resets the running counter); an implicit member is
/// `previous + 1` (first implicit = 0). Enum members are `Long` → `CoreConst::I32`.
fn fold_enums(
    symbols: &SymbolTable,
    module_scope: ScopeId,
    node: SyntaxNode<'_>,
    values: &mut HashMap<SymbolId, CoreConst>,
) {
    if node.kind() == SyntaxKind::EnumBlock {
        let mut next = 0i32;
        for member in node.enum_members() {
            let Some(name_tok) = member.declarator_name() else { continue };
            let Ok(Some(sym)) =
                symbols.find_in_scope(module_scope, SymbolNamespace::Local, name_tok.text)
            else {
                continue;
            };
            let value = match member.first_expr_child() {
                Some(init) => match eval_const_expr(symbols, module_scope, init, values) {
                    ConstEval::Value(c) => as_i32(&c).unwrap_or(next),
                    _ => next,
                },
                None => next,
            };
            values.insert(sym, CoreConst::I32(value));
            next = value.wrapping_add(1);
        }
    }
    for child in node.child_nodes() {
        fold_enums(symbols, module_scope, child, values);
    }
}

fn as_i32(c: &CoreConst) -> Option<i32> {
    match c {
        CoreConst::I32(n) => Some(*n),
        CoreConst::I64(n) => i32::try_from(*n).ok(),
        CoreConst::Bool(b) => Some(if *b { -1 } else { 0 }),
        _ => None,
    }
}

/// Result of folding one `Const` initializer: a value, a not-yet-resolved
/// dependency (`Pending`, retried by the worklist), or unresolvable (dropped).
enum ConstEval {
    Value(CoreConst),
    Pending,
    Unresolvable,
}

/// Fixed point: resolve consts whose dependencies are known, retry the rest, stop
/// when a pass makes no progress (a cycle / unresolvable ref — those are simply
/// left absent). Distinguishing `Pending` from `Unresolvable` stops an overflow
/// being misread as a cycle.
fn resolve_const_worklist(
    symbols: &SymbolTable,
    pending: Vec<(ScopeId, SymbolId, SyntaxNode<'_>)>,
    const_syms: &HashSet<SymbolId>,
) -> HashMap<SymbolId, CoreConst> {
    let mut values: HashMap<SymbolId, CoreConst> = HashMap::new();
    let mut remaining = pending;
    loop {
        let mut progress = false;
        let mut still = Vec::new();
        for (scope, sym, init) in remaining {
            if values.contains_key(&sym) {
                continue;
            }
            match eval_const_expr_syms(symbols, scope, init, &values, const_syms) {
                ConstEval::Value(v) => {
                    values.insert(sym, v);
                    progress = true;
                }
                ConstEval::Pending => still.push((scope, sym, init)),
                ConstEval::Unresolvable => {} // dropped → absent
            }
        }
        if still.is_empty() || !progress {
            return values;
        }
        remaining = still;
    }
}

/// Evaluate against the worklist's `const_syms` (a name that is a yet-unfolded
/// project const → `Pending`).
fn eval_const_expr_syms(
    symbols: &SymbolTable,
    scope: ScopeId,
    node: SyntaxNode<'_>,
    values: &HashMap<SymbolId, CoreConst>,
    const_syms: &HashSet<SymbolId>,
) -> ConstEval {
    eval_inner(symbols, scope, node, values, Some(const_syms))
}

/// Evaluate with no pending set (enum initializers, read after consts are folded).
fn eval_const_expr(
    symbols: &SymbolTable,
    scope: ScopeId,
    node: SyntaxNode<'_>,
    values: &HashMap<SymbolId, CoreConst>,
) -> ConstEval {
    eval_inner(symbols, scope, node, values, None)
}

fn eval_inner(
    symbols: &SymbolTable,
    scope: ScopeId,
    node: SyntaxNode<'_>,
    values: &HashMap<SymbolId, CoreConst>,
    const_syms: Option<&HashSet<SymbolId>>,
) -> ConstEval {
    match node.kind() {
        SyntaxKind::LiteralExpr => match fold_const_literal(node) {
            Some(c) => ConstEval::Value(c),
            None => ConstEval::Unresolvable,
        },
        SyntaxKind::ParenExpr => match node.paren_inner() {
            Some(inner) => eval_inner(symbols, scope, inner, values, const_syms),
            None => ConstEval::Unresolvable,
        },
        SyntaxKind::UnaryExpr => {
            let Some(operand) = node.unary_operand() else { return ConstEval::Unresolvable };
            let inner = match eval_inner(symbols, scope, operand, values, const_syms) {
                ConstEval::Value(c) => c,
                other => return other,
            };
            match node.unary_op_token().map(|t| t.kind) {
                Some(SyntaxKind::Plus) => ConstEval::Value(inner),
                Some(SyntaxKind::Minus) => opt(negate_const(inner)),
                Some(SyntaxKind::KwNot) => opt(not_const(inner)),
                _ => ConstEval::Unresolvable,
            }
        }
        SyntaxKind::IdentExpr => {
            let Some(tok) = node.ident_name_token() else { return ConstEval::Unresolvable };
            // A project const in scope; else a `vb*` library constant.
            if let Ok(Some(sym)) =
                symbols.resolve_in_scope_chain(scope, SymbolNamespace::Local, tok.text)
            {
                if let Some(v) = values.get(&sym) {
                    return ConstEval::Value(v.clone());
                }
                if const_syms.is_some_and(|s| s.contains(&sym)) {
                    return ConstEval::Pending;
                }
            }
            match vba_library::library_constant(tok.text) {
                Some(v) => ConstEval::Value(library_const_value(&v)),
                None => ConstEval::Unresolvable,
            }
        }
        SyntaxKind::BinaryExpr => {
            let (Some(op_tok), Some(lhs_n), Some(rhs_n)) =
                (node.binary_op_token(), node.binary_lhs(), node.binary_rhs())
            else {
                return ConstEval::Unresolvable;
            };
            let Some(op) = core_binop(op_tok.kind) else { return ConstEval::Unresolvable };
            match (
                eval_inner(symbols, scope, lhs_n, values, const_syms),
                eval_inner(symbols, scope, rhs_n, values, const_syms),
            ) {
                (ConstEval::Pending, _) | (_, ConstEval::Pending) => ConstEval::Pending,
                (ConstEval::Value(l), ConstEval::Value(r)) => opt(fold_const_binary(op, &l, &r)),
                _ => ConstEval::Unresolvable,
            }
        }
        _ => ConstEval::Unresolvable,
    }
}

fn opt(c: Option<CoreConst>) -> ConstEval {
    match c {
        Some(c) => ConstEval::Value(c),
        None => ConstEval::Unresolvable,
    }
}

/// Fold a literal/sign/paren initializer to a value.
fn fold_const_literal(node: SyntaxNode<'_>) -> Option<CoreConst> {
    match node.kind() {
        SyntaxKind::LiteralExpr => {
            let tok = node.first_significant_token()?;
            match tok.kind {
                SyntaxKind::IntLiteral => parse_int(tok.text),
                SyntaxKind::HexLiteral => parse_radix(tok.text, 16),
                SyntaxKind::OctLiteral => parse_radix(tok.text, 8),
                SyntaxKind::FloatLiteral => Some(CoreConst::F64(parse_float(tok.text)?.to_bits())),
                SyntaxKind::StringLiteral => Some(CoreConst::Str(unquote(tok.text))),
                SyntaxKind::KwTrue => Some(CoreConst::Bool(true)),
                SyntaxKind::KwFalse => Some(CoreConst::Bool(false)),
                SyntaxKind::DateLiteral => date::parse_date_literal_serial_bits(tok.text).map(CoreConst::Date),
                _ => None,
            }
        }
        SyntaxKind::ParenExpr => fold_const_literal(node.paren_inner()?),
        SyntaxKind::UnaryExpr => {
            let inner = fold_const_literal(node.unary_operand()?)?;
            match node.unary_op_token()?.kind {
                SyntaxKind::Plus => Some(inner),
                SyntaxKind::Minus => negate_const(inner),
                _ => None,
            }
        }
        _ => None,
    }
}

fn parse_int(text: &str) -> Option<CoreConst> {
    let digits = text.trim_end_matches(['&', '%', '@', '!', '#', '$', '^']);
    let n: i64 = digits.parse().ok()?;
    Some(if i32::try_from(n).is_ok() { CoreConst::I32(n as i32) } else { CoreConst::I64(n) })
}

fn parse_radix(text: &str, radix: u32) -> Option<CoreConst> {
    let body = text
        .trim_start_matches(['&'])
        .trim_start_matches(['h', 'H', 'o', 'O'])
        .trim_end_matches(['&', '%', '^']);
    let n = i64::from_str_radix(body, radix).ok()?;
    Some(if i32::try_from(n).is_ok() { CoreConst::I32(n as i32) } else { CoreConst::I64(n) })
}

fn parse_float(text: &str) -> Option<f64> {
    text.trim_end_matches(['!', '#', '@']).parse().ok()
}

fn unquote(text: &str) -> String {
    let inner = text.strip_prefix('"').unwrap_or(text);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    inner.replace("\"\"", "\"")
}

fn negate_const(c: CoreConst) -> Option<CoreConst> {
    Some(match c {
        CoreConst::I32(n) => CoreConst::I32(n.checked_neg()?),
        CoreConst::I64(n) => CoreConst::I64(n.checked_neg()?),
        CoreConst::F64(bits) => CoreConst::F64((-f64::from_bits(bits)).to_bits()),
        _ => return None,
    })
}

fn not_const(c: CoreConst) -> Option<CoreConst> {
    Some(match c {
        CoreConst::Bool(b) => CoreConst::Bool(!b),
        CoreConst::I32(n) => CoreConst::I32(!n),
        CoreConst::I64(n) => CoreConst::I64(!n),
        _ => return None,
    })
}

fn library_const_value(v: &LibraryConstValue) -> CoreConst {
    match v {
        LibraryConstValue::Str(s) => CoreConst::Str(s.clone()),
        LibraryConstValue::Int(i) => CoreConst::I32(*i),
    }
}

enum ConstNum {
    Int(i64),
    Float(f64),
}

fn const_num(c: &CoreConst) -> Option<ConstNum> {
    Some(match c {
        CoreConst::I32(n) => ConstNum::Int(i64::from(*n)),
        CoreConst::I64(n) => ConstNum::Int(*n),
        CoreConst::Bool(b) => ConstNum::Int(if *b { -1 } else { 0 }),
        CoreConst::F64(bits) => ConstNum::Float(f64::from_bits(*bits)),
        CoreConst::Date(bits) => ConstNum::Float(f64::from_bits(*bits)),
        _ => return None,
    })
}

fn int_const(n: i64) -> CoreConst {
    match i32::try_from(n) {
        Ok(v) => CoreConst::I32(v),
        Err(_) => CoreConst::I64(n),
    }
}

fn f64_const(v: f64) -> CoreConst {
    CoreConst::F64(v.to_bits())
}

fn const_to_string(c: &CoreConst) -> Option<String> {
    Some(match c {
        CoreConst::Str(s) => s.clone(),
        CoreConst::I32(n) => n.to_string(),
        CoreConst::I64(n) => n.to_string(),
        CoreConst::Bool(b) => if *b { "True".into() } else { "False".into() },
        CoreConst::F64(bits) => f64::from_bits(*bits).to_string(),
        _ => return None,
    })
}

fn fold_const_binary(op: CoreBinOp, lhs: &CoreConst, rhs: &CoreConst) -> Option<CoreConst> {
    use CoreBinOp::*;
    if matches!(op, Concat) {
        return Some(CoreConst::Str(const_to_string(lhs)? + &const_to_string(rhs)?));
    }
    let (l, r) = (const_num(lhs)?, const_num(rhs)?);
    let both_int = matches!((&l, &r), (ConstNum::Int(_), ConstNum::Int(_)));
    let li = match &l { ConstNum::Int(v) => *v, ConstNum::Float(v) => v.round() as i64 };
    let ri = match &r { ConstNum::Int(v) => *v, ConstNum::Float(v) => v.round() as i64 };
    let lf = match &l { ConstNum::Int(v) => *v as f64, ConstNum::Float(v) => *v };
    let rf = match &r { ConstNum::Int(v) => *v as f64, ConstNum::Float(v) => *v };
    let bool_const = |b: bool| CoreConst::Bool(b);
    Some(match op {
        Add if both_int => int_const(li.checked_add(ri)?),
        Sub if both_int => int_const(li.checked_sub(ri)?),
        Mul if both_int => int_const(li.checked_mul(ri)?),
        Add => f64_const(lf + rf),
        Sub => f64_const(lf - rf),
        Mul => f64_const(lf * rf),
        Div => f64_const(lf / rf),
        IntDiv => int_const(li.checked_div(ri)?),
        Mod => int_const(li.checked_rem(ri)?),
        Pow => f64_const(lf.powf(rf)),
        And => int_const(li & ri),
        Or => int_const(li | ri),
        Xor => int_const(li ^ ri),
        Eqv => int_const(!(li ^ ri)),
        Imp => int_const(!li | ri),
        Eq => bool_const(lf == rf),
        Ne => bool_const(lf != rf),
        Lt => bool_const(lf < rf),
        Le => bool_const(lf <= rf),
        Gt => bool_const(lf > rf),
        Ge => bool_const(lf >= rf),
        Concat | Is | Like => return None,
    })
}

fn core_binop(kind: SyntaxKind) -> Option<CoreBinOp> {
    Some(match kind {
        SyntaxKind::Plus => CoreBinOp::Add,
        SyntaxKind::Minus => CoreBinOp::Sub,
        SyntaxKind::Star => CoreBinOp::Mul,
        SyntaxKind::Slash => CoreBinOp::Div,
        SyntaxKind::Backslash => CoreBinOp::IntDiv,
        SyntaxKind::Caret => CoreBinOp::Pow,
        SyntaxKind::Ampersand => CoreBinOp::Concat,
        SyntaxKind::Eq => CoreBinOp::Eq,
        SyntaxKind::LtGt => CoreBinOp::Ne,
        SyntaxKind::Lt => CoreBinOp::Lt,
        SyntaxKind::LtEq => CoreBinOp::Le,
        SyntaxKind::Gt => CoreBinOp::Gt,
        SyntaxKind::GtEq => CoreBinOp::Ge,
        SyntaxKind::KwMod => CoreBinOp::Mod,
        SyntaxKind::KwAnd => CoreBinOp::And,
        SyntaxKind::KwOr => CoreBinOp::Or,
        SyntaxKind::KwXor => CoreBinOp::Xor,
        SyntaxKind::KwEqv => CoreBinOp::Eqv,
        SyntaxKind::KwImp => CoreBinOp::Imp,
        SyntaxKind::KwIs => CoreBinOp::Is,
        SyntaxKind::KwLike => CoreBinOp::Like,
        _ => return None,
    })
}

/// Date-literal parsing (pure text → OLE serial bits) — the single canonical
/// implementation, used both here (date `Const`s) and by the binder's literal
/// lowering (`#…#` date literals).
pub mod date {
    pub fn parse_date_literal_serial_bits(text: &str) -> Option<u64> {
        let inner = text.strip_prefix('#')?.strip_suffix('#')?.trim();
        let packed = parse_date_literal_to_packed(inner)?;
        Some(packed_date_to_ole_serial(packed)?.to_bits())
    }

    fn parse_date_literal_to_packed(text: &str) -> Option<i32> {
        let normalized = text.trim().replace([',', '.', '-', '/'], " ");
        let parts: Vec<&str> = normalized.split_whitespace().collect();
        let packed = match parts.as_slice() {
            [year, month, day] if year.len() == 4 => {
                let year = year.parse::<i32>().ok()?;
                let month = parse_month_token(month).or_else(|| month.parse::<i32>().ok())?;
                let day = day.parse::<i32>().ok()?;
                year.saturating_mul(10_000) + month.saturating_mul(100) + day
            }
            [month, day, year] if parse_month_token(month).is_some() => {
                let month = parse_month_token(month)?;
                let day = day.parse::<i32>().ok()?;
                let year = year.parse::<i32>().ok()?;
                year.saturating_mul(10_000) + month.saturating_mul(100) + day
            }
            [month, day, year] if is_unambiguous_numeric_month_day(month, day) => {
                let month = month.parse::<i32>().ok()?;
                let day = day.parse::<i32>().ok()?;
                let year = year.parse::<i32>().ok()?;
                year.saturating_mul(10_000) + month.saturating_mul(100) + day
            }
            [day, month, year] => {
                let day = day.parse::<i32>().ok()?;
                let month = parse_month_token(month).or_else(|| month.parse::<i32>().ok())?;
                let year = year.parse::<i32>().ok()?;
                year.saturating_mul(10_000) + month.saturating_mul(100) + day
            }
            _ => return None,
        };
        packed_date_components(packed)?;
        Some(packed)
    }

    fn is_unambiguous_numeric_month_day(month: &str, day: &str) -> bool {
        let (Ok(month), Ok(day)) = (month.parse::<i32>(), day.parse::<i32>()) else {
            return false;
        };
        (1..=12).contains(&month) && day > 12
    }

    fn parse_month_token(text: &str) -> Option<i32> {
        match text.trim().to_ascii_lowercase().as_str() {
            "jan" | "january" => Some(1),
            "feb" | "february" => Some(2),
            "mar" | "march" => Some(3),
            "apr" | "april" => Some(4),
            "may" => Some(5),
            "jun" | "june" => Some(6),
            "jul" | "july" => Some(7),
            "aug" | "august" => Some(8),
            "sep" | "sept" | "september" => Some(9),
            "oct" | "october" => Some(10),
            "nov" | "november" => Some(11),
            "dec" | "december" => Some(12),
            _ => None,
        }
    }

    fn packed_date_components(packed: i32) -> Option<(i32, u32, u32)> {
        let year = packed / 10_000;
        let month = ((packed / 100) % 100) as u32;
        let day = (packed % 100) as u32;
        if !(1..=12).contains(&month) {
            return None;
        }
        let max_day = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if is_leap_year(year) => 29,
            2 => 28,
            _ => 0,
        };
        if max_day == 0 || day == 0 || day > max_day {
            return None;
        }
        Some((year, month, day))
    }

    fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    fn packed_date_to_ole_serial(packed: i32) -> Option<f64> {
        let (year, month, day) = packed_date_components(packed)?;
        Some((days_from_civil(year, month, day) + 25_569) as f64)
    }

    fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
        let year = i64::from(year) - i64::from((month <= 2) as i32);
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let month_index = i64::from(month) + if month > 2 { -3 } else { 9 };
        let day_of_year = (153 * month_index + 2) / 5 + i64::from(day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }
}
