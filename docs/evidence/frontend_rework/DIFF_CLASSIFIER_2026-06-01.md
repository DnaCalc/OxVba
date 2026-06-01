# Diff Classifier Evidence

Date: 2026-06-01
Bead: `bd-aprs.6.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added a classification policy layer to `crates/oxvba-compiler/src/frontend_diff.rs`.

The classifier turns a `FrontendDiffReport` plus fixture policy into one of:

- `Equivalent`: diagnostics, bytecode summary, metadata, execution trace, and output all match.
- `Bug`: diagnostics, metadata, trace, or output differ; bytecode differs without an explicit
  fixture policy; or a policy row is missing rationale/close condition.
- `HarmlessDrift`: only bytecode summary differs, and the fixture row documents why the drift is
  acceptable plus the condition that keeps it acceptable.
- `IntentionalImprovement`: bytecode differs because the new front-end/lowering intentionally
  fixes a documented legacy divergence, with evidence and close condition supplied by the fixture.

## Fixture Rows

### fixture-1: synthetic slot reuse drift

- Fixture link: `inline:frontend_diff::bytecode_drift_report`
- Classification: `HarmlessDrift`
- Rationale: alternate lowering reuses temporaries while preserving diagnostics, metadata,
  execution trace status, and observable output status.
- Close condition: keep as harmless only while diagnostics, metadata, execution, and output match.

This is a classifier fixture, not a claim that the current v2 bridge naturally emits different
bytecode. The current bridge validates with the CST parser and then uses legacy lowering, so the
smoke compiler fixture is expected to remain byte-identical today.

### fixture-2: synthetic legacy divergence fix

- Fixture link: `inline:frontend_diff::bytecode_drift_report`
- Classification: `IntentionalImprovement`
- Rationale: new lowering fixes a documented legacy divergence.
- Close condition: requires fixture evidence linking the divergence and expected VBA behavior.

This row proves that the classifier has an intentional-improvement lane without weakening the
default rule: undocumented drift is still classified as `Bug`.

## Checks

- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The classifier does not infer harmlessness from bytecode-only differences. A fixture row must
  supply both rationale and close condition.
- Diagnostics, metadata, execution trace, and observable output mismatches are always bugs in this
  layer. Later beads may add richer fixture metadata, but they should not silently downgrade these
  mismatches.
- The non-byte-identical evidence is synthetic because the current v2 path intentionally reuses
  legacy lowering after CST validation. This is acceptable for FE-5.3 because the bead creates the
  classification mechanism; FE-5.4 is responsible for broader corpus runner integration.
