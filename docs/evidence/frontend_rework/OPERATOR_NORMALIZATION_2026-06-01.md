# Operator Normalization Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Reopened production-route update: `parse_expr` no longer produces `BoundExpr::AddConst` or
`BoundExpr::SubConst` for simple `var + const` / `var - const` source forms. Those source forms now
bind as uniform `BoundExpr::BinaryOp` trees, including line-continuation expressions, `With` member
assignments, and runtime `ReDim` bounds.

The backend fast path is retained as an optimizer/lowering concern. `optimize_module` now walks
statement and expression trees and rewrites eligible uniform `BinaryOp(Add/Sub, Var, IntConst)`
nodes into `AddConst`/`SubConst` before the existing emitter consumes them. This keeps the current
bytecode optimization available without baking the optimization into parser/resolver shape.

Uniform arithmetic checking was updated with the route move: object/array arithmetic rejection now
lives on `BoundExpr::BinaryOp` instead of depending on the old parser-produced AddConst/SubConst
shape. A frontend diagnostic bridge formatting bug was also corrected so Set-target type diagnostics
continue to match the production typecheck wording.

The earlier scaffold remains in `crates/oxvba-compiler/src/frontend_operator_normalization.rs` as a
small HIR-facing target-shape helper, but closure for this bead is based on the production
`resolve.rs`/`optimize.rs` route change above.

## Checks

- `cargo test -p oxvba-compiler resolve_line_continuation_assignment_expression_uses_uniform_binary_op --quiet`
- `cargo test -p oxvba-compiler resolve_with_block_member_assignments --quiet`
- `cargo test -p oxvba-compiler resolve_runtime_redim_expression_bounds_on_dynamic_array --quiet`
- `cargo test -p oxvba-compiler resolve_ --quiet`
- `cargo test -p oxvba-compiler optimize --quiet`
- `cargo test -p oxvba-compiler frontend_operator_normalization --quiet`
- `cargo test -p oxvba-compiler parse_expr_add_sub_is_left_associative --quiet`
- `cargo test -p oxvba-compiler arithmetic_object_plus_const_is_rejected --quiet`
- `cargo test -p oxvba-compiler set_keyword_rejects_scalar_target_for_scalar_source --quiet`
- `cargo test -p oxvba-compiler variant_source_scalar_payload_assignment_rejects_compile_time_mismatch_lanes --quiet`
- `cargo test -p oxvba-compiler compile_project_ --quiet`
- `cargo test -p oxvba-compiler emit --quiet`
- `cargo test -p oxvba-compiler --quiet`

## Fresh-Eyes Review

- `BoundExpr::AddConst`/`SubConst` still exist because the current bytecode emitter and VM have
  explicit fast-path instructions for them. That is now a compatibility/optimization boundary, not
  a parser-produced IR shape.
- The production resolver route was audited for remaining direct expectations. Remaining
  resolver-side references are the enum definition, module-constant typing compatibility, and test
  coverage for non-parser fast-path contexts; the old parser branch that constructed AddConst/SubConst
  was removed.
- The optimizer now rewrites expression trees recursively for assignments, conditions, loop bounds,
  `For Each` iterable expressions, `Select Case` expressions, calls, member receivers, and nested
  intrinsic/call arguments before existing dead-store/no-op elimination and emitter fast paths run.
