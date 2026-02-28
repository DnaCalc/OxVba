# PROFILE_LADDER_2026-02-28_MACH1000_V87_V106_LANGUAGE_COMPLETION.md

## Goal
Close all currently outstanding **language** items from `docs/evidence/language/COVERAGE_INDEX.csv`, and leave the repo in a state where only explicitly non-language scope remains.

Profiles in this ladder: **20** (`v87..v106`).

Primary tracking artifacts:
- Language checklist: `docs/evidence/language/COVERAGE_INDEX.csv`
- Combined checklist: `docs/evidence/SPEC_CHECKLIST.md`
- Oracle backlog for uncertain semantics: `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`

Policy:
- Keep normal gates non-blocking for formal failures.
- Start strict Kani as async deferred lanes for each profile where meaningful.
- For semantically uncertain items, land implementation + local tests first, then mark oracle-check topics `in-progress` for post-implementation differential validation.

## Scope Closure Targets
Items to close from language checklist (planned/partial):
- `For Step`, `For Each`, `Do Until`, `Loop Until`, `While Wend`, `Exit For`
- `Select Case Is` / range clauses
- `GoTo <label>`, line-number labels, numeric `GoTo`
- `Resume` (full forms)
- `Erase`
- `Property Get` expression semantics
- `Late-bound default-member` execution
- `UDT field access/assignment`
- `Declare` and external signature binding (language side)

## 20-Profile Execution Plan

### Track A: Loop and Branch Completion (`v87..v92`)

### v87 - `mvp-lang-for-step-v87`
- Implement `For ... Step ... Next` semantics, including negative step and termination behavior.
- Add conformance fixtures for step sign, zero-step diagnostics, and bound mutation edge cases.
- Oracle topic links: `CCT-006`.

### v88 - `mvp-lang-exit-for-v88`
- Implement `Exit For` for nested loops with correct innermost loop unwinding.
- Add VM/JIT parity tests for nested control-flow with early loop exits.
- Oracle topic links: `CCT-007`.

### v89 - `mvp-lang-do-until-v89`
- Implement `Do Until ... Loop` and `Do ... Loop Until` (pre/post check forms).
- Ensure lowering parity with existing DoWhile machinery.
- Oracle topic links: `CCT-009`.

### v90 - `mvp-lang-while-wend-v90`
- Implement `While ... Wend` by normalization to loop IR with parser-level diagnostics.
- Add malformed-block diagnostics and conformance fixtures.
- Oracle topic links: `CCT-010`.

### v91 - `mvp-lang-select-case-is-range-v91`
- Implement `Case Is <op> ...` and range clauses (`a To b`) with first-match precedence.
- Add mixed clause tests and optimizer/JIT parity checks.
- Oracle topic links: `CCT-011`, `CCT-012`.

### v92 - `mvp-lang-loop-branch-rollup-v92`
- Consolidation pass for Track A; stabilize diagnostics and matrix gates.
- Fold async formal lanes for `v87..v91`.

### Track B: Unstructured Control Flow + Error Semantics (`v93..v98`)

### v93 - `mvp-lang-goto-label-v93`
- Implement `GoTo <label>` with procedure-local label integrity checks.
- Add forward/backward label transfer tests through nested blocks.
- Oracle topic links: `CCT-001`.

### v94 - `mvp-lang-line-number-goto-v94`
- Add line-number labels and numeric target resolution.
- Add duplicate/missing target diagnostics and parser precedence coverage.
- Oracle topic links: `CCT-002`.

### v95 - `mvp-lang-resume-full-v95`
- Implement `Resume` and `Resume <label>` semantics with error-site tracking.
- Extend error-state runtime model to support resume target legality.
- Oracle topic links: `CCT-003`, `CCT-004`.

### v96 - `mvp-lang-err-surface-v96`
- Expand `Err` object subset to full in-scope surface used by runtime/error flow.
- Add lifecycle/clear-point tests across procedure boundaries and handlers.
- Oracle topic links: `CCT-005`.

### v97 - `mvp-lang-on-error-resume-rollup-v97`
- Consolidate error-handling semantics across all `On Error` modes + resume forms.
- Add deterministic state-transition trace artifact.
- Fold async formal lanes for `v93..v96`.

### v98 - `mvp-lang-unstructured-error-gate-v98`
- Final Track B gate: matrix + conformance + formal rollup.
- Mark covered oracle topics as `in-progress` with probe scaffolds committed.

### Track C: Arrays, UDT, Property, Late Binding (`v99..v103`)

### v99 - `mvp-lang-erase-v99`
- Implement `Erase` semantics for dynamic/fixed arrays and Variant array containers.
- Add side-effect and post-erase shape tests.
- Oracle topic links: `CCT-022`.

### v100 - `mvp-lang-udt-fields-v100`
- Implement UDT field access, assignment, copy semantics in current runtime model.
- Add declaration + field read/write conformance fixtures.
- Oracle topic links: `CCT-019`.

### v101 - `mvp-lang-property-get-expr-v101`
- Complete executable expression semantics for `Property Get` read paths.
- Verify Let/Set/Get interaction ordering and side effects.
- Oracle topic links: `CCT-024`.

### v102 - `mvp-lang-late-default-member-v102`
- Upgrade late-bound default-member calls from diagnostic-only to executable subset.
- Add explicit fallback diagnostics where host metadata is missing.
- Oracle topic links: `CCT-023`, `CCT-013`.

### v103 - `mvp-lang-array-type-edge-rollup-v103`
- Consolidate array + UDT + property + late-bound execution interactions.
- Focus on ByRef/copy-back + Option Base/Array constructor edge behavior.
- Oracle topic links: `CCT-013`, `CCT-020`, `CCT-021`.

### Track D: Declare/External Binding + Final Closure (`v104..v106`)

### v104 - `mvp-lang-declare-binding-v104`
- Implement language-side `Declare` syntax/binding and signature metadata ingestion.
- Add compile-time diagnostics for unsupported calling conventions/signatures.
- Oracle topic links: `CCT-027`.

### v105 - `mvp-lang-external-call-subset-v105`
- Implement minimal executable external-call subset through existing host boundary model.
- Add guarded host-policy behavior and deterministic fallback paths.
- Oracle topic links: `CCT-026`, `CCT-035`.

### v106 - `mvp-lang-full-closure-gate-v106`
- Final language closure gate:
  - all language rows in `COVERAGE_INDEX.csv` are `implemented` or explicitly `partial` with rationale accepted by policy,
  - matrix/conformance/formal gates green,
  - async strict formal lanes reconciled (`dg-folded` or explicit deferred with unblock steps),
  - conformance topics backlog updated with probe readiness status.

## Cross-Profile Standard Work Per Step
1. Implement parser/resolver/typecheck/emit/vm/jit changes for scoped feature.
2. Add conformance fixtures under `conformance/tests`.
3. Add compiler/runtime regression tests.
4. Add/refresh formal obligations for profile scope.
5. Start strict async Kani lane:
   - `./scripts/run-formal-kani-async.ps1 -Action Start -Name <profile>-kani -ProfileScope <profile-scope> -WatchPollSeconds 600`
6. Update evidence docs:
   - `docs/profile-status/PROFILE_STATUS_V<nn>.md`
   - `docs/evidence/profiles/v<nn>/...`
   - `docs/evidence/formal/DEFERRED_GATES.md`
   - `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` (topic status/notes)

## Exit Definition
This ladder is complete when `v106` passes and the outstanding language feature set is closed according to `COVERAGE_INDEX.csv`, with oracle conformance topics queued as actionable differential checks for post-implementation parity confirmation.
