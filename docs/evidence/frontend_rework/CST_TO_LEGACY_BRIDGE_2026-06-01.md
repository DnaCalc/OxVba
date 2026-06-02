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

FE-4.2 then extended the same CST lowerer for scoped postfix proof: simple calls become
`BoundExpr::ProcCall`, member/bang chains become `BoundExpr::Member`, and member calls attach
arguments from the CST `ArgList`.

FE-4.3 added statement coverage bridge validation. The CST parser accepts attributes, inline
statement separators, `On Error`/`Resume`, `With`, `Property`, `Declare`, `Type`, and `Enum`
fixtures. FE-4.4 then added a selected legacy bridge for colon statement separators: after CST
validation, `compile_source_via_syntax_bridge` lowers `Colon` tokens to line breaks before calling
the legacy compiler.

FE-4.5 added diagnostic-route proof: recovered CST parse errors stop
`compile_source_via_syntax_bridge` before legacy lowering, while the partial tree remains lossless
and contains an `ErrorNode`.

FE-4.4 second reopen, after the FE-9.6 audit, removed the hidden production fallback from
`compile_source_with_runtime_metadata_via_syntax_bridge`: after CST validation the route now tries
HIR production lowering, returns compiled bytecode on success, and returns `Unsupported` on HIR
unsupported shapes instead of calling `compile_with_runtime_metadata` itself. The CST expression
bridge remains as an explicit compatibility helper for focused tests, but the bridge is no longer
an internal "CST validation then legacy compile" production path. Inline statement separator and
bare object identity fixtures still compile because they now route through HIR production.

FE-9 retirement continuation then made `syntax_bridge` crate-private and moved the CST-to-legacy
expression/source bridge helpers behind `#[cfg(test)]`. Those helpers remain available to internal
compatibility tests, but they are no longer a public compiler API surface or part of ordinary
production builds. The route-audit classifier remains compiled and calls direct HIR lowering.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-compiler syntax_bridge --quiet`
  - First-run result: passed, 2 tests.
  - FE-4.1 reopen result: passed, 3 tests after adding CST expression lowering coverage.
  - FE-4.2 reopen result: passed, 4 tests after adding CST postfix lowering coverage.
  - FE-4.3/FE-4.4 reopen result: passed, 7 tests after adding statement coverage validation and
    selected colon-separator bridge lowering.
  - FE-4.5 reopen result: passed, 8 tests after adding recovered-syntax diagnostic route proof.
  - FE-4.4 second reopen result: passed, 9 tests after removing the hidden legacy compile fallback
    from the bridge.
  - FE-9 retirement continuation result: passed, 9 tests after making the bridge module
    crate-private and test-gating the CST-to-legacy helpers.
- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
  - FE-4.4 second reopen result: passed, proving route classification now distinguishes HIR
    production from HIR-unsupported residuals.
- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
  - FE-4.4 second reopen result: passed; the audit still fails the terminal gate because unsupported
    constructs remain owned by later HIR/project/language-service beads, not by the CST bridge
    fallback itself.
- `cargo test -p oxvba-compiler --quiet`
  - FE-4.4 second reopen result: passed after updating the frontend corpus expectation to count the
    call/coercion conformance row as a known FE-8.5 HIR-production bug instead of silently treating
    it as equivalent through the removed bridge fallback.
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
parser. FE-4.2 postfix forms are also lowered from CST for the scoped bridge tests. FE-4.3
statement forms are validated by CST first. The FE-9.6 audit showed that the bridge was still
acting as a hidden production fallback, so this second reopen removed that fallback. Full
statement/source compilation gaps now surface as HIR `Unsupported` residuals for FE-8.5/project
beads instead of being handled inside the CST bridge. The FE-9 retirement continuation further
quarantines the bridge by removing it from the public compiler module surface and compiling its
CST-to-legacy lowering helpers only for tests.

The test assertion initially referenced a nonexistent `StoreSlot` bytecode instruction. The final
test checks the actual instruction family emitted by this compiler for the assignment/arithmetic
fixture.

Residuals left for later beads:

- full statement and expression lowering should move to HIR rather than expanding this bridge
  indefinitely;
- production HIR coverage gaps belong to FE-8.5 and project semantics beads, not to FE-4.4;
- outer production fallback policy belongs to FE-9/terminal audit;
- semantic/differential corpus classification belongs to FE-5.2 and FE-5.3;
- bridge fallback/error policy needs the FE-4.5 diagnostic fixtures and FE-5 harness.
