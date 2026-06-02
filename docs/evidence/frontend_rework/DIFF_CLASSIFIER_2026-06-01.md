# Diff Classifier Evidence

Date: 2026-06-01
Bead: `bd-aprs.6.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added a classification policy layer to `crates/oxvba-compiler/src/frontend_diff.rs`.

The classifier turns a `FrontendDiffReport` plus fixture policy into one of:

- `Equivalent`: diagnostics, bytecode summary, metadata, execution trace, and output all match.
- `Bug`: diagnostics, metadata, trace, or output differ without an explicit fixture policy;
  bytecode differs without an explicit fixture policy; diagnostic/acceptance behavior differs
  without an explicit fixture policy; or a policy row is missing rationale/close condition.
- `HarmlessDrift`: only bytecode summary differs, and the fixture row documents why the drift is
  acceptable plus the condition that keeps it acceptable.
- `IntentionalImprovement`: bytecode, documented metadata fields, or one-sided diagnostic/
  acceptance behavior differs because the new front-end/lowering intentionally fixes a documented
  legacy divergence, with evidence and close condition supplied by the fixture.

## Fixture Rows

### fixture-1: synthetic slot reuse drift

- Fixture link: `inline:frontend_diff::bytecode_drift_report`
- Classification: `HarmlessDrift`
- Rationale: alternate lowering reuses temporaries while preserving diagnostics, metadata,
  execution trace status, and observable output status.
- Close condition: keep as harmless only while diagnostics, metadata, execution, and output match.

This is a classifier fixture, not a claim that the current frontend-v2 route naturally emits
different bytecode. The frontend-v2 observer now calls HIR production lowering directly, so
bytecode identity is not required; equivalence is judged by diagnostics, metadata policy, execution
trace status, and observable output status.

### fixture-2: synthetic legacy divergence fix

- Fixture link: `inline:frontend_diff::bytecode_drift_report`
- Classification: `IntentionalImprovement`
- Rationale: new lowering fixes a documented legacy divergence.
- Close condition: requires fixture evidence linking the divergence and expected VBA behavior.

This row proves that the classifier has an intentional-improvement lane without weakening the
default rule: undocumented drift is still classified as `Bug`.

### fixture-3: inline statement separator acceptance improvement

- Fixture link: `inline:frontend_diff::inline_statement_improvement_report`
- Classification: `IntentionalImprovement`
- Rationale: v2 accepts a CST-valid inline statement sequence that the legacy-default compiler
  rejects.
- Close condition: keep as improvement only while v2 compiles and FE-5.4 adds execution evidence.

This is a real route-backed fixture, not a synthetic bytecode mutation. It compares:

```vb
Sub Main()
    Dim x As Long
    x = 1: x = x + 1
End Sub
```

The left side records the legacy diagnostic and no bytecode/metadata. The right side records
frontend-v2 bytecode and metadata through
`frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir`. Without
`ExpectedDiagnosticDrift::IntentionalImprovement`, the same report is classified as `Bug`.

### fixture-4: source-map metadata improvement

- Fixture link: `conformance/tests/call_coercion_mixed_variant_to_long.bas`
- Classification: `IntentionalImprovement`
- Rationale: HIR source spans preserve the blank line before the second procedure, while legacy
  metadata maps the second procedure one line early.
- Close condition: keep as improvement only while bytecode, diagnostics, call descriptors, and
  non-source-map metadata match.

This row proves that metadata differences are not automatically treated as equivalent, but also are
not forced into `Bug` when the fixture documents a field-level source-map improvement. The reported
metadata paths are `procedures.use.source_line_start`, `procedures.use.source_line_end`, and
`procedures.use.statement_line_numbers`.

## Checks

- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The classifier does not infer harmlessness from bytecode-only differences. A fixture row must
  supply both rationale and close condition.
- Diagnostic/acceptance drift is still a bug unless the fixture explicitly marks it as an
  intentional improvement, supplies a rationale plus close condition, and has one-sided bytecode
  availability. This matters because a production replacement must be allowed to fix legacy false
  negatives without treating legacy acceptance as the source of truth, while ordinary diagnostic
  drift between two successful compiles remains a bug.
- Metadata mismatches are still bugs unless the fixture explicitly marks the field-level delta as
  harmless or an intentional improvement with a rationale and close condition.
- Execution trace and observable output mismatches remain bugs in the compiler-layer classifier;
  FE-5.4 must add higher-layer execution observations so improvement rows can be validated beyond
  compiler bytecode/metadata availability.
- Bytecode non-identity evidence remains synthetic. The diagnostic and metadata improvement
  fixtures are real route-backed evidence from the reopened harness.
