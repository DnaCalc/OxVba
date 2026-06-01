# CST-to-Legacy Bridge Evidence

Date: 2026-06-01
Bead: `bd-aprs.5.4`
Workset lane: FE-4.4 CST-to-legacy bridge

## Outcome

Added a temporary `oxvba-compiler::syntax_bridge` module:

- `lower_expression_to_legacy_bound_expr` parses a wrapped expression through `oxvba-syntax`,
  verifies the expected CST assignment shape, then lowers the selected expression from the CST
  node shape into the existing `BoundExpr` representation;
- `compile_source_via_syntax_bridge` parses full source through `oxvba-syntax` and, when accepted,
  compiles through the existing compiler/lowering path;
- bridge tests cover expression precedence lowering (`1 + 2 * 3`) and a real assignment family
  compiling to bytecode after CST validation.

The bridge is intentionally transitional. It does not replace the future HIR binder. After the
workset reopen, the FE-4.1 expression bridge no longer calls the special legacy
`parse_expr_for_syntax_bridge` hook; unsupported expression shapes fail explicitly instead of
falling back silently.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-compiler syntax_bridge --quiet`
  - First-run result: passed, 2 tests.
  - Reopen result: passed, 3 tests after adding CST expression lowering coverage.
- `cargo test -p oxvba-syntax --quiet`
  - First-run result: passed, 78 unit tests plus 2 integration tests.
  - Reopen result: passed, 79 unit tests plus 2 integration tests.
- `cargo fmt --check -p oxvba-compiler -p oxvba-syntax`
  - Result: passed after formatting.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The bridge deliberately does not pretend to be a full CST lowerer. Its value is a concrete,
checked handoff point: the new CST parser must accept the source first, and FE-4.1 expression
forms are now lowered from the CST rather than reparsed from source text by the legacy expression
parser. Full statement/source compilation still uses the legacy compiler after CST validation until
later HIR/binder/lowering beads replace that path.

The test assertion initially referenced a nonexistent `StoreSlot` bytecode instruction. The final
test checks the actual instruction family emitted by this compiler for the assignment/arithmetic
fixture.

Residuals left for later beads:

- full statement and expression lowering should move to HIR rather than expanding this bridge
  indefinitely;
- production gating belongs to FE-5.1;
- semantic/differential corpus classification belongs to FE-5.2 and FE-5.3;
- bridge fallback/error policy needs the FE-4.5 diagnostic fixtures and FE-5 harness.
