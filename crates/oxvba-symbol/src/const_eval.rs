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

use oxvba_bundle::StringCompareMode;
use oxvba_bundle::coreir::{CoreBinOp, CoreConst};
use oxvba_runtime::CurrencyValue;
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::model::{
    ScopeId, ScopeKind, SymbolId, SymbolImpl, SymbolKind, SymbolNamespace, SymbolTable, Visibility,
};
use crate::providers::vba_library;
use crate::scanner::parameter_name_token;
use crate::signature::{BuiltinType, VarTypeRef};

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
    // The string comparison/`Like` regime is the declaring module's `Option Compare`.
    let module_modes: HashMap<ScopeId, StringCompareMode> = module_roots
        .iter()
        .map(|(scope, root)| (*scope, module_compare_mode(*root)))
        .collect();
    // 1) Collect plain `Const` declarators (module- and proc-level).
    let mut pending: Vec<(ScopeId, SymbolId, SyntaxNode<'_>)> = Vec::new();
    let mut const_syms: HashSet<SymbolId> = HashSet::new();
    for (module_scope, root) in module_roots {
        collect_consts(symbols, *module_scope, *root, &mut pending, &mut const_syms);
    }
    // 2) Fold them by fixed point (forward + cross-const references).
    let mut values = resolve_const_worklist(symbols, pending, &const_syms, &module_modes);
    // 3) Fold `Enum` members (sequential auto-increment, reading earlier values).
    for (module_scope, root) in module_roots {
        let mode = module_modes
            .get(module_scope)
            .copied()
            .unwrap_or(StringCompareMode::Binary);
        fold_enums(symbols, *module_scope, *root, &mut values, mode);
    }
    values
}

/// A module's `Option Compare` mode (`Text` → case-insensitive string ops; default
/// `Binary`). Mirrors the binder's `module_compare_mode` so const-folded string
/// comparisons/`Like` match run-time behaviour.
fn module_compare_mode(module: SyntaxNode<'_>) -> StringCompareMode {
    for node in module.child_nodes() {
        if node.kind() != SyntaxKind::OptionStmt {
            continue;
        }
        let toks = node.child_tokens();
        let is_compare = toks.iter().any(|t| t.kind == SyntaxKind::KwCompare);
        let is_text = toks
            .iter()
            .any(|t| t.kind == SyntaxKind::Ident && t.text.eq_ignore_ascii_case("Text"));
        if is_compare && is_text {
            return StringCompareMode::Text;
        }
    }
    StringCompareMode::Binary
}

/// The `Option Compare` mode in effect for `scope` — its module's mode (a proc scope
/// inherits its module's).
fn mode_for_scope(
    symbols: &SymbolTable,
    scope: ScopeId,
    module_modes: &HashMap<ScopeId, StringCompareMode>,
) -> StringCompareMode {
    let mut cur = Some(scope);
    while let Some(s) = cur {
        if let Some(m) = module_modes.get(&s) {
            return *m;
        }
        cur = symbols
            .scopes()
            .iter()
            .find(|sc| sc.id == s)
            .and_then(|sc| sc.parent);
    }
    StringCompareMode::Binary
}

/// Fold every `Optional` parameter's **default expression** to a constant, keyed by
/// `(procedure symbol, parameter index)`. VBA Optional defaults are constant
/// expressions (they may reference module/global `Const`s), so they fold here in the
/// published type system, after `fold_const_values`. A parameter with no default, or
/// one that does not fold, is simply absent (the binder supplies the declared-type
/// zero / `Missing`).
pub fn fold_optional_defaults(
    symbols: &SymbolTable,
    module_roots: &[(ScopeId, SyntaxNode<'_>)],
    values: &HashMap<SymbolId, CoreConst>,
) -> HashMap<(SymbolId, usize), CoreConst> {
    let module_modes: HashMap<ScopeId, StringCompareMode> = module_roots
        .iter()
        .map(|(scope, root)| (*scope, module_compare_mode(*root)))
        .collect();
    let mut out = HashMap::new();
    for (module_scope, root) in module_roots {
        let mode = mode_for_scope(symbols, *module_scope, &module_modes);
        collect_proc_defaults(symbols, *module_scope, *root, values, mode, &mut out);
    }
    out
}

fn collect_proc_defaults(
    symbols: &SymbolTable,
    module_scope: ScopeId,
    node: SyntaxNode<'_>,
    values: &HashMap<SymbolId, CoreConst>,
    mode: StringCompareMode,
    out: &mut HashMap<(SymbolId, usize), CoreConst>,
) {
    if matches!(
        node.kind(),
        SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl
    ) && let Some(name) = node.proc_name_token()
        && let Ok(Some(proc_sym)) =
            symbols.find_in_scope(module_scope, SymbolNamespace::Procedure, name.text)
        && let Some(param_list) = node.param_list()
    {
        let proc_scope = proc_scope_under(symbols, module_scope, name.text);
        for (i, param) in param_list.params().iter().enumerate() {
            if let Some(def) = param.param_default().and_then(|d| d.first_expr_child())
                && let ConstEval::Value(c) =
                    eval_const_expr(symbols, module_scope, def, values, mode)
            {
                out.insert(
                    (proc_sym, i),
                    coerce_param_default_value(symbols, proc_scope, *param, c),
                );
            }
        }
    }
    for child in node.child_nodes() {
        collect_proc_defaults(symbols, module_scope, child, values, mode, out);
    }
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
            let Some(name) = declarator.declarator_name() else {
                continue;
            };
            let Some(init) = declarator.first_expr_child() else {
                continue;
            };
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
    let folded = crate::model::fold_identifier(normalize_identifier_text(name));
    symbols.scopes().iter().find_map(|s| {
        if s.parent != Some(module_scope) || s.kind != crate::model::ScopeKind::Procedure {
            return None;
        }
        let n = s.name.and_then(|id| symbols.name(id))?;
        (n.folded == folded).then_some(s.id)
    })
}

fn coerce_param_default_value(
    symbols: &SymbolTable,
    proc_scope: Option<ScopeId>,
    param: SyntaxNode<'_>,
    value: CoreConst,
) -> CoreConst {
    let Some(scope) = proc_scope else {
        return value;
    };
    let Some(name) = parameter_name_token(param) else {
        return value;
    };
    let Ok(Some(sym)) = symbols.find_in_scope(scope, SymbolNamespace::Parameter, name.text) else {
        return value;
    };
    coerce_declared_const_value(symbols, sym, value.clone()).unwrap_or(value)
}

/// Fold `Enum` members in source order: a member with an explicit initializer takes
/// that folded value (and resets the running counter); an implicit member is
/// `previous + 1` (first implicit = 0). Enum members are `Long` → `CoreConst::I32`.
fn fold_enums(
    symbols: &SymbolTable,
    module_scope: ScopeId,
    node: SyntaxNode<'_>,
    values: &mut HashMap<SymbolId, CoreConst>,
    mode: StringCompareMode,
) {
    if node.kind() == SyntaxKind::EnumBlock {
        let mut next = 0i32;
        for member in node.enum_members() {
            let Some(name_tok) = member.declarator_name() else {
                continue;
            };
            let Ok(Some(sym)) =
                symbols.find_in_scope(module_scope, SymbolNamespace::Local, name_tok.text)
            else {
                continue;
            };
            let value = match member.first_expr_child() {
                Some(init) => match eval_const_expr(symbols, module_scope, init, values, mode) {
                    ConstEval::Value(c) => match as_i32(&c) {
                        Some(value) => value,
                        None => break,
                    },
                    _ => break,
                },
                None => next,
            };
            values.insert(sym, CoreConst::I32(value));
            next = value.wrapping_add(1);
        }
    }
    for child in node.child_nodes() {
        fold_enums(symbols, module_scope, child, values, mode);
    }
}

fn as_i32(c: &CoreConst) -> Option<i32> {
    match c {
        CoreConst::I16(n) => Some(i32::from(*n)),
        CoreConst::I32(n) => Some(*n),
        // An `Enum` member is a `Long`. Radix Long bit-pattern literals such as
        // `&HFFFFFFFF` are already folded as signed `I32`; LongLong carriers are
        // valid only when their numeric value still fits a VBA Long.
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
    module_modes: &HashMap<ScopeId, StringCompareMode>,
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
            let mode = mode_for_scope(symbols, scope, module_modes);
            match eval_const_expr_syms(symbols, scope, init, &values, const_syms, mode) {
                ConstEval::Value(v) => {
                    let Some(v) = coerce_declared_const_value(symbols, sym, v) else {
                        continue;
                    };
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

fn coerce_declared_const_value(
    symbols: &SymbolTable,
    sym: SymbolId,
    value: CoreConst,
) -> Option<CoreConst> {
    let Some(symbol) = symbols.symbol(sym) else {
        return Some(value);
    };
    let SymbolImpl::DeclaredType(ty) = &symbol.imp else {
        return Some(value);
    };
    coerce_const_to_declared_type(value, ty)
}

pub(crate) fn coerce_const_to_declared_type(
    value: CoreConst,
    ty: &VarTypeRef,
) -> Option<CoreConst> {
    match ty {
        VarTypeRef::Variant => Some(value),
        VarTypeRef::FixedString(_) | VarTypeRef::Builtin(BuiltinType::String) => {
            const_to_string(&value).map(CoreConst::Str)
        }
        VarTypeRef::Builtin(BuiltinType::Boolean) => const_to_bool(&value).map(CoreConst::Bool),
        VarTypeRef::Builtin(BuiltinType::Byte) => {
            let n = const_to_i64(&value)?;
            (0..=255).contains(&n).then_some(CoreConst::I32(n as i32))
        }
        VarTypeRef::Builtin(BuiltinType::Integer) => {
            let n = const_to_i64(&value)?;
            (i64::from(i16::MIN)..=i64::from(i16::MAX))
                .contains(&n)
                .then_some(CoreConst::I16(n as i16))
        }
        VarTypeRef::Builtin(BuiltinType::Long) => {
            let n = const_to_i64(&value)?;
            i32::try_from(n).ok().map(CoreConst::I32)
        }
        VarTypeRef::Builtin(BuiltinType::LongLong | BuiltinType::LongPtr) => {
            const_to_i64(&value).map(CoreConst::I64)
        }
        VarTypeRef::Builtin(BuiltinType::Single) => {
            let n = const_to_f64(&value)?;
            (n.is_finite() && n.abs() <= f64::from(f32::MAX))
                .then_some(CoreConst::F32((n as f32).to_bits()))
        }
        VarTypeRef::Builtin(BuiltinType::Double) => {
            let n = const_to_f64(&value)?;
            n.is_finite().then_some(CoreConst::F64(n.to_bits()))
        }
        VarTypeRef::Builtin(BuiltinType::Currency) => {
            let n = const_to_f64(&value)?;
            let scaled = (n * 10_000.0).round_ties_even();
            (scaled.is_finite() && scaled >= i64::MIN as f64 && scaled <= i64::MAX as f64)
                .then_some(CoreConst::Currency(scaled as i64))
        }
        VarTypeRef::Builtin(BuiltinType::Date) => const_to_date_bits(&value).map(CoreConst::Date),
        VarTypeRef::Object(_)
        | VarTypeRef::Udt(_)
        | VarTypeRef::Array(_)
        | VarTypeRef::FixedArray { .. } => None,
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
    mode: StringCompareMode,
) -> ConstEval {
    eval_inner(symbols, scope, node, values, Some(const_syms), mode)
}

/// Evaluate with no pending set (enum initializers, read after consts are folded).
fn eval_const_expr(
    symbols: &SymbolTable,
    scope: ScopeId,
    node: SyntaxNode<'_>,
    values: &HashMap<SymbolId, CoreConst>,
    mode: StringCompareMode,
) -> ConstEval {
    eval_inner(symbols, scope, node, values, None, mode)
}

fn eval_inner(
    symbols: &SymbolTable,
    scope: ScopeId,
    node: SyntaxNode<'_>,
    values: &HashMap<SymbolId, CoreConst>,
    const_syms: Option<&HashSet<SymbolId>>,
    mode: StringCompareMode,
) -> ConstEval {
    match node.kind() {
        SyntaxKind::LiteralExpr => match fold_const_literal(node) {
            Some(c) => ConstEval::Value(c),
            None => ConstEval::Unresolvable,
        },
        SyntaxKind::ParenExpr => match node.paren_inner() {
            Some(inner) => eval_inner(symbols, scope, inner, values, const_syms, mode),
            None => ConstEval::Unresolvable,
        },
        SyntaxKind::UnaryExpr => {
            let Some(operand) = node.unary_operand() else {
                return ConstEval::Unresolvable;
            };
            let inner = match eval_inner(symbols, scope, operand, values, const_syms, mode) {
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
            let Some(tok) = node.ident_name_token() else {
                return ConstEval::Unresolvable;
            };
            // A project const in scope; else a `vb*` library constant.
            if let Ok(Some(sym)) =
                symbols.resolve_in_scope_chain(scope, SymbolNamespace::Local, tok.text)
            {
                match eval_resolved_const(sym, values, const_syms) {
                    ConstEval::Unresolvable => {}
                    resolved => return resolved,
                }
            }
            match vba_library::library_constant(tok.text) {
                Some(v) => ConstEval::Value(v),
                None => ConstEval::Unresolvable,
            }
        }
        SyntaxKind::MemberExpr => {
            let Some(sym) = resolve_qualified_const_symbol(symbols, scope, node) else {
                return ConstEval::Unresolvable;
            };
            eval_resolved_const(sym, values, const_syms)
        }
        SyntaxKind::BinaryExpr => {
            let (Some(op_tok), Some(lhs_n), Some(rhs_n)) =
                (node.binary_op_token(), node.binary_lhs(), node.binary_rhs())
            else {
                return ConstEval::Unresolvable;
            };
            let Some(op) = core_binop(op_tok.kind) else {
                return ConstEval::Unresolvable;
            };
            match (
                eval_inner(symbols, scope, lhs_n, values, const_syms, mode),
                eval_inner(symbols, scope, rhs_n, values, const_syms, mode),
            ) {
                (ConstEval::Pending, _) | (_, ConstEval::Pending) => ConstEval::Pending,
                (ConstEval::Value(l), ConstEval::Value(r)) => {
                    opt(fold_const_binary(op, &l, &r, mode))
                }
                _ => ConstEval::Unresolvable,
            }
        }
        _ => ConstEval::Unresolvable,
    }
}

fn eval_resolved_const(
    sym: SymbolId,
    values: &HashMap<SymbolId, CoreConst>,
    const_syms: Option<&HashSet<SymbolId>>,
) -> ConstEval {
    if let Some(v) = values.get(&sym) {
        return ConstEval::Value(v.clone());
    }
    if const_syms.is_some_and(|s| s.contains(&sym)) {
        return ConstEval::Pending;
    }
    ConstEval::Unresolvable
}

/// Resolve `Module.Const` / `Project.Module.Const` in a constant expression.
///
/// The provider chain is not available while constants are folded, so this mirrors
/// the active-project qualified-member rule against the symbol table: the module
/// qualifier is looked up among siblings of the declaring module's project scope,
/// preserving VBA visibility (Private constants are module-local).
/// Referenced projects fold in their own parent scope; cross-project publication
/// still flows through export surfaces after this pass.
fn resolve_qualified_const_symbol(
    symbols: &SymbolTable,
    scope: ScopeId,
    node: SyntaxNode<'_>,
) -> Option<SymbolId> {
    let parts = qualified_ident_parts(node)?;
    let module_scope = enclosing_module_scope(symbols, scope)?;
    let project_scope = symbols.scope(module_scope).ok()?.parent?;
    if let [enum_name, member_name] = parts.as_slice()
        && !module_qualifier_shadowed(symbols, scope, enum_name)
        && let Some(sym) = enum_member_symbol(symbols, module_scope, enum_name, member_name)
    {
        return Some(sym);
    }

    let (module_name, member_name) = match parts.as_slice() {
        [module_name, member_name] if !module_qualifier_shadowed(symbols, scope, module_name) => {
            (*module_name, *member_name)
        }
        [project_name, module_name, member_name]
            if scope_name_matches(symbols, project_scope, project_name) =>
        {
            (*module_name, *member_name)
        }
        _ => return None,
    };
    let target_module = sibling_module_scope(symbols, project_scope, module_name)?;
    let sym = symbols
        .find_in_scope(target_module, SymbolNamespace::Local, member_name)
        .ok()
        .flatten()?;
    qualified_const_visible(symbols, module_scope, target_module, sym).then_some(sym)
}

fn qualified_const_visible(
    symbols: &SymbolTable,
    declaring_module: ScopeId,
    target_module: ScopeId,
    sym: SymbolId,
) -> bool {
    let Some(symbol) = symbols.symbol(sym) else {
        return false;
    };
    if !matches!(symbol.kind, SymbolKind::Const | SymbolKind::EnumMember) {
        return false;
    }
    target_module == declaring_module || symbol.visibility == Some(Visibility::Public)
}

fn enum_member_symbol(
    symbols: &SymbolTable,
    module_scope: ScopeId,
    enum_name: &str,
    member_name: &str,
) -> Option<SymbolId> {
    let enum_sym = symbols
        .find_in_scope(module_scope, SymbolNamespace::Type, enum_name)
        .ok()
        .flatten()?;
    if symbols.symbol(enum_sym)?.kind != crate::model::SymbolKind::Enum {
        return None;
    }
    symbols
        .find_in_scope(module_scope, SymbolNamespace::Local, member_name)
        .ok()
        .flatten()
        .filter(|sym| {
            symbols
                .symbol(*sym)
                .is_some_and(|s| s.kind == crate::model::SymbolKind::EnumMember)
        })
}

fn module_qualifier_shadowed(symbols: &SymbolTable, scope: ScopeId, name: &str) -> bool {
    [SymbolNamespace::Local, SymbolNamespace::Parameter]
        .iter()
        .any(|namespace| {
            symbols
                .resolve_in_scope_chain(scope, *namespace, name)
                .ok()
                .flatten()
                .is_some()
        })
}

fn qualified_ident_parts(node: SyntaxNode<'_>) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    collect_qualified_ident_parts(node, &mut parts).then_some(parts)
}

fn collect_qualified_ident_parts<'a>(node: SyntaxNode<'a>, parts: &mut Vec<&'a str>) -> bool {
    match node.kind() {
        SyntaxKind::IdentExpr => {
            let Some(tok) = node.ident_name_token() else {
                return false;
            };
            parts.push(normalize_identifier_text(tok.text));
            true
        }
        SyntaxKind::MemberExpr => {
            if node.member_has_leading_dot() || node.member_is_bang() {
                return false;
            }
            let (Some(receiver), Some(member)) = (node.member_receiver(), node.member_name_token())
            else {
                return false;
            };
            if !collect_qualified_ident_parts(receiver, parts) {
                return false;
            }
            parts.push(normalize_identifier_text(member.text));
            true
        }
        _ => false,
    }
}

fn enclosing_module_scope(symbols: &SymbolTable, scope: ScopeId) -> Option<ScopeId> {
    let mut current = Some(scope);
    while let Some(scope_id) = current {
        let scope = symbols.scope(scope_id).ok()?;
        if scope.kind == ScopeKind::Module {
            return Some(scope_id);
        }
        current = scope.parent;
    }
    None
}

fn scope_name_matches(symbols: &SymbolTable, scope: ScopeId, name: &str) -> bool {
    let Some(scope_name) = symbols
        .scope(scope)
        .ok()
        .and_then(|s| s.name)
        .and_then(|id| symbols.name(id))
    else {
        return false;
    };
    scope_name.folded == crate::model::fold_identifier(name)
}

fn sibling_module_scope(
    symbols: &SymbolTable,
    project_scope: ScopeId,
    module_name: &str,
) -> Option<ScopeId> {
    let target = crate::model::fold_identifier(module_name);
    symbols.scopes().iter().find_map(|scope| {
        if scope.kind != ScopeKind::Module || scope.parent != Some(project_scope) {
            return None;
        }
        let name = scope.name.and_then(|id| symbols.name(id))?;
        (name.folded == target).then_some(scope.id)
    })
}

fn normalize_identifier_text(text: &str) -> &str {
    text.strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(text)
}

fn opt(c: Option<CoreConst>) -> ConstEval {
    match c {
        Some(c) => ConstEval::Value(c),
        None => ConstEval::Unresolvable,
    }
}

/// Fold a literal/sign/paren initializer to a value.
pub(crate) fn fold_const_literal(node: SyntaxNode<'_>) -> Option<CoreConst> {
    match node.kind() {
        SyntaxKind::LiteralExpr => {
            let tok = node.first_significant_token()?;
            match tok.kind {
                SyntaxKind::IntLiteral => parse_int(tok.text),
                SyntaxKind::HexLiteral => parse_radix(tok.text, 16),
                SyntaxKind::OctLiteral => parse_radix(tok.text, 8),
                SyntaxKind::FloatLiteral => CoreConst::from_float_literal(tok.text),
                SyntaxKind::StringLiteral => Some(CoreConst::Str(unquote(tok.text))),
                SyntaxKind::KwTrue => Some(CoreConst::Bool(true)),
                SyntaxKind::KwFalse => Some(CoreConst::Bool(false)),
                SyntaxKind::DateLiteral => {
                    date::parse_date_literal_serial_bits(tok.text).map(CoreConst::Date)
                }
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
    CoreConst::from_int_literal(text)
}

fn parse_radix(text: &str, radix: u32) -> Option<CoreConst> {
    // Width-based two's-complement sign + optional `%`/`&`/`^` type character,
    // shared with the binder (MS-VBAL §3.3.2).
    CoreConst::from_vba_radix(text, radix)
}

fn unquote(text: &str) -> String {
    let inner = text.strip_prefix('"').unwrap_or(text);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    inner.replace("\"\"", "\"")
}

pub(crate) fn negate_const(c: CoreConst) -> Option<CoreConst> {
    Some(match c {
        CoreConst::I16(n) => CoreConst::I16(n.checked_neg()?),
        CoreConst::I32(n) => CoreConst::I32(n.checked_neg()?),
        CoreConst::I64(n) => CoreConst::I64(n.checked_neg()?),
        CoreConst::F64(bits) => CoreConst::F64((-f64::from_bits(bits)).to_bits()),
        CoreConst::F32(bits) => CoreConst::F32((-f32::from_bits(bits)).to_bits()),
        CoreConst::Currency(scaled) => CoreConst::Currency(scaled.checked_neg()?),
        CoreConst::Date(bits) => CoreConst::Date((-f64::from_bits(bits)).to_bits()),
        _ => return None,
    })
}

pub(crate) fn not_const(c: CoreConst) -> Option<CoreConst> {
    Some(match c {
        CoreConst::Bool(b) => CoreConst::Bool(!b),
        CoreConst::I16(n) => CoreConst::I16(!n),
        CoreConst::I32(n) => CoreConst::I32(!n),
        CoreConst::I64(n) => CoreConst::I64(!n),
        _ => return None,
    })
}

enum ConstNum {
    Int(i64),
    Float(f64),
}

fn const_num(c: &CoreConst) -> Option<ConstNum> {
    Some(match c {
        CoreConst::I16(n) => ConstNum::Int(i64::from(*n)),
        CoreConst::I32(n) => ConstNum::Int(i64::from(*n)),
        CoreConst::I64(n) => ConstNum::Int(*n),
        CoreConst::Bool(b) => ConstNum::Int(if *b { -1 } else { 0 }),
        CoreConst::F64(bits) => ConstNum::Float(f64::from_bits(*bits)),
        CoreConst::F32(bits) => ConstNum::Float(f64::from(f32::from_bits(*bits))),
        CoreConst::Currency(scaled) => ConstNum::Float(*scaled as f64 / 10_000.0),
        CoreConst::Date(bits) => ConstNum::Float(f64::from_bits(*bits)),
        _ => return None,
    })
}

fn const_to_f64(c: &CoreConst) -> Option<f64> {
    if let CoreConst::Str(s) = c {
        let n = s.trim().parse::<f64>().ok()?;
        return n.is_finite().then_some(n);
    }
    Some(match const_num(c)? {
        ConstNum::Int(n) => n as f64,
        ConstNum::Float(n) => n,
    })
}

fn const_to_bool(c: &CoreConst) -> Option<bool> {
    match c {
        CoreConst::Bool(b) => Some(*b),
        CoreConst::Str(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => const_to_f64(c).map(|n| n != 0.0),
        },
        _ => const_to_f64(c).map(|n| n != 0.0),
    }
}

fn const_to_date_bits(c: &CoreConst) -> Option<u64> {
    match c {
        CoreConst::Date(bits) => Some(*bits),
        CoreConst::Str(s) => date::parse_date_text_serial_bits(s).or_else(|| {
            let n = const_to_f64(c)?;
            n.is_finite().then_some(n.to_bits())
        }),
        _ => {
            let n = const_to_f64(c)?;
            n.is_finite().then_some(n.to_bits())
        }
    }
}

fn const_to_i64(c: &CoreConst) -> Option<i64> {
    match c {
        CoreConst::I16(n) => Some(i64::from(*n)),
        CoreConst::I32(n) => Some(i64::from(*n)),
        CoreConst::I64(n) => Some(*n),
        _ => {
            let n = const_to_f64(c)?;
            if !n.is_finite() || n.abs() >= 9.223_372_036_854_775e18 {
                return None;
            }
            Some(n.round_ties_even() as i64)
        }
    }
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
        CoreConst::I16(n) => n.to_string(),
        CoreConst::I32(n) => n.to_string(),
        CoreConst::I64(n) => n.to_string(),
        CoreConst::Bool(b) => {
            if *b {
                "True".into()
            } else {
                "False".into()
            }
        }
        CoreConst::F64(bits) => f64::from_bits(*bits).to_string(),
        CoreConst::F32(bits) => f32::from_bits(*bits).to_string(),
        CoreConst::Currency(scaled) => CurrencyValue::from_scaled_i64(*scaled).to_string(),
        CoreConst::Date(bits) => f64::from_bits(*bits).to_string(),
        _ => return None,
    })
}

pub(crate) fn fold_const_binary(
    op: CoreBinOp,
    lhs: &CoreConst,
    rhs: &CoreConst,
    mode: StringCompareMode,
) -> Option<CoreConst> {
    use CoreBinOp::*;
    if matches!(op, Concat) {
        return Some(CoreConst::Str(
            const_to_string(lhs)? + &const_to_string(rhs)?,
        ));
    }
    // String relational / `Like`: when both operands are strings, compare them under
    // the module's `Option Compare` (Text → case-insensitive) rather than numerically.
    if let (CoreConst::Str(ls), CoreConst::Str(rs)) = (lhs, rhs)
        && let Some(b) = fold_string_relational(op, ls, rs, mode)
    {
        return Some(CoreConst::Bool(b));
    }
    let (l, r) = (const_num(lhs)?, const_num(rhs)?);
    let both_int = matches!((&l, &r), (ConstNum::Int(_), ConstNum::Int(_)));
    let li = match &l {
        ConstNum::Int(v) => *v,
        ConstNum::Float(v) => v.round() as i64,
    };
    let ri = match &r {
        ConstNum::Int(v) => *v,
        ConstNum::Float(v) => v.round() as i64,
    };
    let lf = match &l {
        ConstNum::Int(v) => *v as f64,
        ConstNum::Float(v) => *v,
    };
    let rf = match &r {
        ConstNum::Int(v) => *v as f64,
        ConstNum::Float(v) => *v,
    };
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

/// Fold a relational / `Like` operator on two string operands under `Option Compare`
/// (`Text` → case-insensitive). Returns `None` for non-relational operators.
fn fold_string_relational(
    op: CoreBinOp,
    lhs: &str,
    rhs: &str,
    mode: StringCompareMode,
) -> Option<bool> {
    let norm = |s: &str| match mode {
        StringCompareMode::Text => s.to_lowercase(),
        StringCompareMode::Binary => s.to_string(),
    };
    let (l, r) = (norm(lhs), norm(rhs));
    use CoreBinOp::*;
    Some(match op {
        Eq => l == r,
        Ne => l != r,
        Lt => l < r,
        Le => l <= r,
        Gt => l > r,
        Ge => l >= r,
        Like => like_match(
            &l.chars().collect::<Vec<_>>(),
            &r.chars().collect::<Vec<_>>(),
        ),
        _ => return None,
    })
}

/// VBA `Like` pattern match: `?` any char, `*` any run, `#` any digit,
/// `[chars]`/`[!chars]` a (negated) set. Case is already normalised by the caller.
fn like_match(s: &[char], p: &[char]) -> bool {
    match p.first() {
        None => s.is_empty(),
        Some('*') => (0..=s.len()).any(|i| like_match(&s[i..], &p[1..])),
        Some('?') => !s.is_empty() && like_match(&s[1..], &p[1..]),
        Some('#') => !s.is_empty() && s[0].is_ascii_digit() && like_match(&s[1..], &p[1..]),
        Some('[') => match p.iter().position(|&c| c == ']') {
            Some(end) => {
                let set = &p[1..end];
                let (negate, set) = if set.first() == Some(&'!') {
                    (true, &set[1..])
                } else {
                    (false, set)
                };
                !s.is_empty()
                    && (set.contains(&s[0]) != negate)
                    && like_match(&s[1..], &p[end + 1..])
            }
            None => !s.is_empty() && s[0] == '[' && like_match(&s[1..], &p[1..]),
        },
        Some(&c) => !s.is_empty() && s[0] == c && like_match(&s[1..], &p[1..]),
    }
}

pub(crate) fn core_binop(kind: SyntaxKind) -> Option<CoreBinOp> {
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

/// Compatibility path for date-literal parsing. The canonical implementation now
/// lives in `oxvba-runtime` so compile-time constants and VM Date coercion share
/// the same deterministic parser.
pub mod date {
    pub use oxvba_runtime::date::{parse_date_literal_serial_bits, parse_date_text_serial_bits};
}
