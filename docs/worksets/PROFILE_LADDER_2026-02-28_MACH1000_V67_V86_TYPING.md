# PROFILE_LADDER_2026-02-28_MACH1000_V67_V86_TYPING.md

## Why This Ladder Exists
The current runtime has meaningful semantics coverage, but full VBA type behavior is still incomplete end-to-end.

This ladder focuses on one objective:
- implement full internal language typing support aligned to MS-VBAL semantics, including early/late interaction, coercion rules, diagnostics, strings, and arrays.

Planning horizon in this document:
- Profiles: `v67` through `v86`
- Total planned steps: **20**
- Formal policy: `F3` with `Deferred Gate (DG)` for long-running async Kani obligations.

## Scope Anchors
- MS-VBAL root:
  - https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74
- MS-OAUT root (interop boundaries only):
  - https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/

Interpretation policy for this ladder:
- Internal language typing behavior follows MS-VBAL as primary source.
- Interop details follow MS-OAUT at host/COM boundaries.
- Implementation-defined behavior is allowed only when explicitly recorded in divergence/evidence docs.

## Simplifying Assumptions (Interop Boundary)
To keep this ladder focused on internal typing closure:
1. Canonical runtime boundary format remains COM-compatible `VARIANT`/`SAFEARRAY`.
2. External late binding is scoped to `IDispatch` Automation paths already in scope.
3. Type-library-driven compile-time external signature import is deferred.
4. COM `Option Compare Database` host-specific behavior is treated as implementation-defined unless host evidence is available.

## Typing Completion Targets
This ladder must close:
1. Full declared-type surface (`As <type>`, type characters, `Def*` default typing).
2. Full `Option Explicit` diagnostics and declaration-quality diagnostics.
3. Assignment/argument/operator coercion and conversion behavior per decision tables/spec.
4. String semantics beyond subset behavior (comparison/mutation/search and statement forms).
5. Array semantics for typed and `Variant` arrays, non-zero lower bounds, and multi-dim behavior.
6. Early/late call interaction under typed and untyped call sites.

## Deferred Gate (DG) Model For Formal
Long-running Kani obligations are started during profile execution and tracked as deferred gates:

- `Gate-pass` requirements for a profile:
  - unit/conformance/matrix gates pass for declared scope,
  - async Kani run is started with reproducible state/log paths,
  - deferred-gate register entry is updated.
- `Gate-pass` does not require immediate Kani completion.
- Completed async runs are folded back in scheduled reconciliation profiles.

DG status lifecycle:
- `dg-started` -> `dg-running` -> `dg-pass|dg-fail` -> `dg-folded`.

Artifacts:
- Register: `docs/evidence/formal/DEFERRED_GATES.md`
- Async logs/state: `temp/async/formal-kani/<run-name>/`
- Latest strict result merge: `docs/evidence/formal/latest_run.md`

## 20-Profile Ladder (v67-v86)

### Track A: Type Model + Diagnostics (`v67..v72`)

### v67 - `mvp-typing-type-lattice-v67` (F3 + DG)
Scope:
- Introduce full VBA value-type lattice in binder/typecheck layers.
- Include scalar numeric families, `String`, `Date`, `Currency`, `Decimal`, `Object`, `Variant`, enums, UDT references, arrays.
Formal obligations:
- Type lattice partial-order and join totality checks on in-scope type pairs.
- Kani harnesses for type-tag transition validity in assignment/temporary storage.
Gate:
- Type lattice model exercised by compiler tests and typed corpus baseline.

### v68 - `mvp-typing-option-explicit-diagnostics-v68` (F3 + DG)
Scope:
- Full `Option Explicit` enforcement.
- Declaration diagnostics: undeclared symbol, duplicate declaration, scope collision, illegal redefinition.
Formal obligations:
- Symbol-table determinism checks for repeated compile runs.
- Diagnostic stability checks (same input -> same diagnostic set ordering).
Gate:
- Diagnostic conformance corpus green with deterministic error snapshots.

### v69 - `mvp-typing-default-type-rules-v69` (F3 + DG)
Scope:
- `DefBool/DefByte/DefInt/DefLng/DefLngLng/DefLngPtr/DefSng/DefDbl/DefDec/DefCur/DefDate/DefStr/DefObj/DefVar`.
- Type-declaration characters and precedence against explicit `As <type>`.
Formal obligations:
- Default-type resolution function totality and precedence correctness.
- Reduced-domain proof of no ambiguous default typing.
Gate:
- Default typing corpus green with per-letter-range coverage.

### v70 - `mvp-typing-procedure-signatures-v70` (F3 + DG)
Scope:
- Typed params/returns across `Sub`/`Function`/`Property`.
- ByRef legality rules under typed arguments and temporary values.
Formal obligations:
- ByRef admissibility invariants (lvalue-only and coercion legality).
- Call-frame type-shape consistency checks.
Gate:
- Procedure typing suite green; illegal ByRef patterns produce stable diagnostics.

### v71 - `mvp-typing-early-late-classification-v71` (F3 + DG)
Scope:
- Call-site classification: early-bound, late-bound, or mixed fallback path.
- Default-member binding behavior for known object types in scope.
Formal obligations:
- Call-mode selection determinism for fixed symbol/type environments.
- No-invalid-lowering checks between call-mode tags and emitted ops.
Gate:
- Early/late classification conformance corpus green with explicit traces.

### v72 - `mvp-typing-diagnostic-rollup-v72` (F3)
Scope:
- Consolidate and stabilize the typing diagnostic surface from `v67..v71`.
- Add user-facing error taxonomy table in docs.
Formal obligations:
- Manifest completeness for Track A obligations.
- Async DG foldback for `v67..v71`.
Gate:
- Track A matrix cells green and deferred gates reconciled.

### Track B: Coercion + Operators + Conversion (`v73..v76`)

### v73 - `mvp-typing-coercion-matrix-v73` (F3 + DG)
Scope:
- Full assignment and argument coercion matrix implementation for in-scope VBA types.
- Overflow/type-mismatch/error-surface alignment.
Formal obligations:
- Decision-table completeness checks (`tables/coercion.csv` mapped to runtime paths).
- Kani harnesses for conversion safety on bounded numeric/string domains.
Gate:
- Coercion conformance matrix green; unsupported edges explicitly classified.

### v74 - `mvp-typing-operator-result-rules-v74` (F3 + DG)
Scope:
- Arithmetic/comparison/logical/concatenation result typing and coercion behavior.
- `Null`, `Empty`, boolean and numeric cross-family interaction handling for supported domains.
Formal obligations:
- Operator result-type determinism against `tables/arithmetic.csv` + `tables/comparison.csv`.
- VM/JIT observable equivalence for typed operator subsets.
Gate:
- Operator corpus and parity checks green.

### v75 - `mvp-typing-call-coercion-early-late-v75` (F3 + DG)
Scope:
- Apply coercion semantics uniformly across early-bound and late-bound invocation routes.
- Named/optional/default-property call interactions under typed signatures.
Formal obligations:
- Equivalence checks: early vs late route for semantically equivalent call shapes.
- Argument-pack correctness checks at boundary marshalling points.
Gate:
- Call coercion corpus green across VM/JIT and binder modes.

### v76 - `mvp-typing-conversion-intrinsics-v76` (F3)
Scope:
- Conversion intrinsic parity (`C*` family, `Val`, `Str`) for full in-scope type surface.
- Integrate with shared coercion engine to avoid split semantics.
Formal obligations:
- Roundtrip and monotonicity checks for supported conversion families.
- Async DG foldback for `v73..v75`.
Gate:
- Conversion intrinsic corpus green and no split-path regressions.

### Track C: Full String Semantics (`v77..v79`)

### v77 - `mvp-string-storage-semantics-v77` (F3 + DG)
Scope:
- String value semantics over canonical BSTR-compatible representation.
- Distinguish `""`, `Empty`, `Null`, and `vbNullString` behavior where in scope.
Formal obligations:
- String storage invariants (length/content/ownership) with Kani harnesses.
- Coercion-to/from-string consistency checks.
Gate:
- Storage and coercion string corpus green.

### v78 - `mvp-string-compare-search-v78` (F3 + DG)
Scope:
- `Option Compare` (`Binary`, `Text`; `Database` tracked as implementation-defined unless host evidence exists).
- `InStr`, `InStrRev`, `StrComp`, `Like` subset completion.
Formal obligations:
- Comparator law checks (symmetry/consistency where defined).
- Deterministic behavior checks under compare mode switches.
Gate:
- Compare/search corpus green with mode-specific snapshots.

### v79 - `mvp-string-mutation-and-slices-v79` (F3)
Scope:
- `Mid` statement semantics and slice mutation paths.
- `Left$/Right$/Mid$`, `Replace`, `Split`, `Join`, trim family completion.
Formal obligations:
- Slice-bound and mutation-preservation invariants.
- Async DG foldback for `v77..v78`.
Gate:
- Full string-surface-in-scope corpus green.

### Track D: Full Array Semantics (`v80..v84`)

### v80 - `mvp-array-type-model-v80` (F3 + DG)
Scope:
- Typed arrays and `Variant` arrays under one descriptor model.
- Fixed/dynamic descriptors and rank metadata.
Formal obligations:
- Descriptor structural invariants and type-tag coherence checks.
- Memory-layout safety checks for descriptor transitions.
Gate:
- Array descriptor tests and compiler typing tests green.

### v81 - `mvp-array-bounds-and-indexing-v81` (F3 + DG)
Scope:
- Non-standard lower bounds and `Option Base` interactions.
- Multi-dimensional indexing and linearization semantics.
Formal obligations:
- Index mapping correctness for rank `1..N` reduced domains.
- Kani bounds-safety harnesses for index computations.
Gate:
- Bounds/indexing corpus green including non-zero lower-bound fixtures.

### v82 - `mvp-array-redim-full-v82` (F3 + DG)
Scope:
- `ReDim` / `ReDim Preserve` full in-scope behavior.
- Preserve constraints (last-dimension rules) and typed-array restrictions.
Formal obligations:
- Shape transformation invariants.
- Data preservation proofs for legal `Preserve` paths.
Gate:
- ReDim conformance corpus green including multi-dim preserve cases.

### v83 - `mvp-array-call-and-paramarray-v83` (F3 + DG)
Scope:
- Array passing semantics (`ByRef`/`ByVal`) with typed vs variant arrays.
- `ParamArray` packing into `Variant` arrays and introspection correctness.
Formal obligations:
- Alias visibility invariants for array arguments.
- ParamArray packing/unpacking consistency checks.
Gate:
- Procedure-array interaction corpus green.

### v84 - `mvp-array-boundary-and-dispatch-v84` (F3)
Scope:
- Array marshalling at call/dispatch boundaries for in-scope Automation shapes.
- Early/late dispatch behavior when array arguments are present.
Formal obligations:
- SAFEARRAY roundtrip checks for in-scope element types.
- Async DG foldback for `v80..v83`.
Gate:
- Array boundary corpus green; unsupported shapes classified deterministically.

### Track E: Typed Runtime Consolidation + Perf (`v85..v86`)

### v85 - `mvp-typed-execution-fastpaths-v85` (F3 + DG)
Scope:
- Introduce typed hot-path execution specializations while preserving canonical semantics.
- Keep fallback to generic Variant path for unsupported edges.
Formal obligations:
- Typed-fastpath vs baseline execution equivalence on typed corpus.
- JIT/VM parity over newly specialized operations.
Gate:
- Typed corpus parity green with measurable no-regression baseline.

### v86 - `mvp-full-typing-conformance-gate-v86` (F3)
Scope:
- Consolidated gate for full typing ladder (`v67..v86`).
- Coverage, formal, and performance evidence rollup.
Formal obligations:
- Manifest completeness and unresolved-DG audit.
- Final foldback of all DG results started during ladder.
Gate:
- Required type/coercion/string/array matrix cells green.
- No uncategorized typing divergences in declared scope.
- Deferred-gate register reconciled (all entries `dg-folded` or explicitly deferred with unblock steps).

## Execution Pattern For This Ladder
Per profile:
1. Implement pass-pack deltas (`P0..P6`).
2. Add/extend conformance fixtures (`P7`).
3. Update evidence/coverage/divergence artifacts (`P8`).
4. Run formal lane (`P9`) and start async Kani DG run:
   - `./scripts/run-formal.ps1 -ProfileScope <profile>`
   - `./scripts/run-formal-kani-async.ps1 -Action Start -Name <profile>-kani -ProfileScope <profile> -StartWatcher $true -WatchPollSeconds 600`
   - `./scripts/run-formal-kani-async.ps1 -Action Status -Name <profile>-kani`
5. Record DG entry and foldback target in `docs/evidence/formal/DEFERRED_GATES.md`.
6. Run matrix lane:
   - `./scripts/run-matrix.ps1 -ProfileScope <profile> -OutputDir docs/evidence/profiles/v<nn>`

## Deferred-Gate Reconciliation Cadence
- Reconciliation points: `v72`, `v76`, `v79`, `v84`, `v86`.
- At each point:
  - poll all active DG runs,
  - merge completed results into `latest_run.md/csv`,
  - triage failures with moderate effort,
  - move unresolved issues into `docs/evidence/formal/EXTENDED_TODO.md` with unblock steps.

## Success Criteria At Ladder End (`v86`)
1. Full internal typing surface in scope is executable and covered by matrix-backed conformance.
2. `Option Explicit` and declaration diagnostics are stable and spec-aligned.
3. Coercion/conversion behavior is table-backed and formally checked.
4. String and array semantics in declared scope are complete, including multi-dim and lower-bound behavior.
5. Async Kani deferred-gate flow is operational, reproducible, and reconciled into formal evidence artifacts.

## Execution Status (2026-02-28)
- `v67` (`mvp-typing-type-lattice-v67`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v67/gate_report.md`),
  - formal obligations `FO-V67-001..003` pass (`docs/evidence/formal/latest_run.md`).
- Strict WSL Kani lane for `v67` started async as deferred gate:
  - run: `v67-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v68` (`mvp-typing-option-explicit-diagnostics-v68`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v68/gate_report.md`),
  - formal obligations `FO-V68-001..003` pass (`docs/evidence/formal/latest_run.md`).
- Strict WSL Kani lane for `v68` started async as deferred gate:
  - run: `v68-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v69` (`mvp-typing-default-type-rules-v69`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v69/gate_report.md`),
  - formal obligations `FO-V69-001..003` pass (`docs/evidence/formal/latest_run.md`).
- Strict WSL Kani lane for `v69` started async as deferred gate:
  - run: `v69-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v70` (`mvp-typing-procedure-signatures-v70`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v70/gate_report.md`),
  - formal obligations `FO-V70-001..003` pass (`docs/evidence/formal/latest_run.md`).
- Strict WSL Kani lane for `v70` started async as deferred gate:
  - run: `v70-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v71` (`mvp-typing-early-late-classification-v71`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v71/gate_report.md`),
  - formal obligations `FO-V71-001..003` pass (`docs/evidence/formal/latest_run.md`).
- Strict WSL Kani lane for `v71` started async as deferred gate:
  - run: `v71-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v72` (`mvp-typing-diagnostic-rollup-v72`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v72/gate_report.md`),
  - formal obligations `FO-V72-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - diagnostic taxonomy published (`docs/DIAGNOSTIC_TAXONOMY.md`).
- Track A DG reconciliation (`v67..v71`) checkpoint:
  - all tracked runs currently `dg-running` at poll time,
  - unresolved foldback recorded in `docs/evidence/formal/EXTENDED_TODO.md` (`FTODO-V72-001`).
- Strict WSL Kani lane for `v72` started async as deferred gate:
  - run: `v72-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v73` (`mvp-typing-coercion-matrix-v73`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v73/gate_report.md`),
  - formal obligations `FO-V73-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - coercion decision table alignment checks enabled (`tables/coercion.csv` vs typecheck rules).
- Strict WSL Kani lane for `v73` started async as deferred gate:
  - run: `v73-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v74` (`mvp-typing-operator-result-rules-v74`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v74/gate_report.md`),
  - formal obligations `FO-V74-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - arithmetic/comparison decision-table alignment checks enabled (`tables/arithmetic.csv`, `tables/comparison.csv` vs typecheck rules).
- Strict WSL Kani lane for `v74` started async as deferred gate:
  - run: `v74-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v75` (`mvp-typing-call-coercion-early-late-v75`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v75/gate_report.md`),
  - formal obligations `FO-V75-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - call coercion decision table alignment checks enabled (`tables/call_coercion.csv` vs typecheck rules).
- Strict WSL Kani lane for `v75` started async as deferred gate:
  - run: `v75-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v76` (`mvp-typing-conversion-intrinsics-v76`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v76/gate_report.md`),
  - formal obligations `FO-V76-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - conversion intrinsic decision-table alignment checks enabled (`tables/conversion_intrinsics.csv` vs typecheck rules).
- Track B DG reconciliation (`v73..v75`) checkpoint:
  - all tracked runs currently `dg-running` at poll time,
  - unresolved foldback recorded in `docs/evidence/formal/EXTENDED_TODO.md` (`FTODO-V76-001`).
- `v77` (`mvp-string-storage-semantics-v77`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v77/gate_report.md`),
  - formal obligations `FO-V77-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - `vbNullString` string-sentinel path wired through resolver/typecheck/emitter and conformance fixtures.
- Strict WSL Kani lane for `v77` started async as deferred gate:
  - run: `v77-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v78` (`mvp-string-compare-search-v78`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v78/gate_report.md`),
  - formal obligations `FO-V78-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - added `Option Compare` mode capture in resolver and mode-aware compare/search bytecode lowering for `InStr`, `InStrRev`, `StrComp`, and `Like` subset execution.
- Strict WSL Kani lane for `v78` started async as deferred gate:
  - run: `v78-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v79` (`mvp-string-mutation-and-slices-v79`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v79/gate_report.md`),
  - formal obligations `FO-V79-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - added executable `Mid` statement mutation subset and expanded slice coverage with type-character forms (`Left$`, `Right$`, `Mid$`).
- Track C DG reconciliation (`v77..v78`) checkpoint:
  - tracked runs `v77-kani` and `v78-kani` currently `dg-running` at poll time,
  - unresolved foldback recorded in `docs/evidence/formal/EXTENDED_TODO.md` (`FTODO-V79-001`).
- Strict WSL Kani lane for `v79` started async as deferred gate:
  - run: `v79-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v80` (`mvp-array-type-model-v80`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v80/gate_report.md`),
  - formal obligations `FO-V80-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - added unified array descriptor metadata (`element_type`, `rank`, `bounds`, `dynamic`) for typed/variant arrays in resolver-bound module/procedure artifacts.
- Strict WSL Kani lane for `v80` started async as deferred gate:
  - run: `v80-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v81` (`mvp-array-bounds-and-indexing-v81`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v81/gate_report.md`),
  - formal obligations `FO-V81-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - added `Option Base` + explicit lower-bound parsing and multi-dimensional index linearization in resolver alias mapping.
- Strict WSL Kani lane for `v81` started async as deferred gate:
  - run: `v81-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v82` (`mvp-array-redim-full-v82`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v82/gate_report.md`),
  - formal obligations `FO-V82-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - added preserve legality diagnostics and tail-clearing semantics for shrink/expand `ReDim Preserve` transitions.
- Strict WSL Kani lane for `v82` started async as deferred gate:
  - run: `v82-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v83` (`mvp-array-call-and-paramarray-v83`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v83/gate_report.md`),
  - formal obligations `FO-V83-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - added `ParamArray` signature parsing + trailing positional pack behavior in call lowering with explicit current-subset diagnostic for named `ParamArray` arguments.
- Strict WSL Kani lane for `v83` started async as deferred gate:
  - run: `v83-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
- `v84` (`mvp-array-boundary-and-dispatch-v84`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v84/gate_report.md`),
  - formal obligations `FO-V84-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - added array-tag boundary marshalling projection for dispatch invocation path (`DispatchInvoke`) with scalar-path stability retained.
- Track D DG reconciliation (`v80..v83`) checkpoint:
  - tracked strict runs `v80-kani`, `v81-kani`, `v82-kani`, and `v83-kani` are currently `dg-running` at poll time,
  - unresolved foldback recorded in `docs/evidence/formal/EXTENDED_TODO.md` (`FTODO-V84-001`).
- `v85` (`mvp-typed-execution-fastpaths-v85`) completed:
  - matrix gate `PASS` (`docs/evidence/profiles/v85/gate_report.md`),
  - formal obligations `FO-V85-001..003` pass (`docs/evidence/formal/latest_run.md`),
  - benchmark artifacts recorded (`docs/evidence/profiles/v85/benchmark_latest.md`, aggregate gain `0.31%`),
  - added typed VM fast-path helpers for core integer slot ops with baseline fallback parity checks.
- Strict WSL Kani lane for `v85` started async as deferred gate:
  - run: `v85-kani`,
  - status: `dg-running`,
  - register: `docs/evidence/formal/DEFERRED_GATES.md`.
