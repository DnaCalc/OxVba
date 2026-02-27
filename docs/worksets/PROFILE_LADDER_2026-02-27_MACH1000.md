# PROFILE_LADDER_2026-02-27_MACH1000.md

## Why This Exists
`mvp-int32-core-v1` is stabilized, but MACH1000 needs an execution queue that scales beyond incremental feature work.

This document defines the **next 35 concrete profile steps** and explicitly prioritizes formal verification early, then language coverage and performance closure.

## Replan Delta (Formal-First Revision)
Compared to the previous ladder version:

1. Formal verification is moved into early/mid profiles instead of late-only support.
2. External COM boundary and Forms runtime profiles are removed from this core sequence.
3. Late profiles focus on proof-backed optimizer/JIT/performance graduation.
4. Every profile has explicit verification obligations, not just tests.
5. Extension batch (`v27`-`v36`) pivots to formal reliability, full language coverage closure, and hotspot performance work instead of alternative Variant-representation design.

## How Many Profile Steps Are There?
There is no theoretical upper bound (full VBA parity and performance optimization are open-ended).

Concrete planning horizon in this document:

- Previously completed profile at planning time: `v1`
- Planned profiles in this ladder: `v2` through `v36`
- Total concrete future steps here: **35**
- Execution status now: **completed through `v26`; queued batch is `v27` through `v36`**.

## Naming Convention
- `mvp-<capability>-vN`
- Each profile declares:
  - capability scope,
  - pass-pack delta,
  - formal obligations,
  - matrix/gate criteria,
  - divergence delta.

## Pass Packs
All profiles execute with this pass-pack structure.

1. `P0 Syntax`: lexer/parser/CST changes.
2. `P1 Bind`: symbol resolution and typed bound model.
3. `P2 HIR`: high-level semantic representation.
4. `P3 MIR`: explicit operation lowering.
5. `P4 CFG`: control-flow/SSA/legalization.
6. `P5 Emit`: bytecode/JIT IR emission.
7. `P6 Runtime`: VM/JIT execution semantics.
8. `P7 Conformance`: corpus/golden/matrix updates.
9. `P8 Evidence`: divergence and status docs.
10. `P9 Formal`: Kani/Lean/spec checks and proof artifacts.

## Formal Obligation Levels
Each profile declares a minimum formal level.

- `F0`: none beyond ordinary tests.
- `F1`: Kani harnesses for local safety invariants.
- `F2`: Lean or machine-checked spec lemmas for semantics.
- `F3`: translation/equivalence validation between stages.

Target progression across this ladder:
- v2-v4: establish repeatable `F1` cadence.
- v5-v11: mixed `F1/F2` on semantics-critical behavior.
- v12-v18: sustained `F2` with selective `F3`.
- v19-v26: `F3` mandatory for optimizer/JIT/perf graduation.
- v27-v31: `F3` formal reliability and proof-capacity scaling.
- v32-v36: `F3` full-language coverage closure + performance shaping.

## 35-Profile Ladder (v2-v36)

### v2 — `mvp-controlflow-v2` (F1)
Scope:
- `If...Then...End If` (no `Else`).
- `For...Next` ascending step 1.
- variable copy assignment.
Formal obligations:
- Kani harnesses for jump target bounds and `pc` progression invariants.
- Kani harness for temporary slot allocator non-overlap with declared slots.
Gate:
- close `DIV-0001`/`DIV-0002` (or split into narrower residuals).

### v3 — `mvp-formal-foundation-v3` (F1)
Scope:
- Formal infrastructure profile (no large language-surface expansion).
Formal obligations:
- Establish repeatable Kani lane for compiler/VM invariants in CI script flow.
- Add formal artifact index format (obligation id, artifact path, status).
- Add first proof inventory in `docs/evidence/formal/`.
Gate:
- CI lane executes selected Kani obligations successfully and reproducibly.

### v4 — `mvp-boolean-logic-v4` (F1)
Scope:
- relational ops `= <> < <= > >=` on integer subset.
- `And/Or/Not` truthiness subset.
Formal obligations:
- Kani harnesses for comparator totality over in-scope integer domain slices.
Gate:
- condition corpus green + formal obligations pass.

### v5 — `mvp-else-paths-v5` (F2)
Scope:
- `Else` and `ElseIf` chains.
Formal obligations:
- Lean lemma set for branch determinism and single-path selection semantics.
Gate:
- branch conformance suite green + Lean proofs checked.

### v6 — `mvp-while-loop-v6` (F2)
Scope:
- `Do While...Loop`, `Do...Loop While`, `Exit Do`.
Formal obligations:
- Lean model for loop-step semantics on finite traces.
- Kani loop-state safety harnesses for VM interpreter transitions.
Gate:
- loop corpus green + proof artifacts linked.

### v7 — `mvp-select-case-v7` (F2)
Scope:
- `Select Case` constants + `Case Else`.
Formal obligations:
- proof/validation artifact for first-match case dispatch determinism.
Gate:
- select-case parity green + determinism artifact present.

### v8 — `mvp-procedures-v8` (F1)
Scope:
- `Sub`/`Function` calls, local scopes, call frames.
Formal obligations:
- Kani frame-window bounds and return-address integrity harnesses.
Gate:
- procedure corpus green + frame-safety obligations pass.

### v9 — `mvp-params-v9` (F2)
Scope:
- `ByVal` and `ByRef` subset.
Formal obligations:
- aliasing semantics model for `ByRef` mutation visibility.
- machine-checked obligations for pass-by-mode behavior in reduced domain.
Gate:
- byref corpus green + alias obligations green.

### v10 — `mvp-arrays-v10` (F1)
Scope:
- fixed-size arrays, index load/store, bounds errors.
Formal obligations:
- Kani bounds harnesses for indexed ops.
Gate:
- array corpus green + bounds obligations pass.

### v11 — `mvp-error-state-v11` (F2)
Scope:
- `On Error Resume Next`, minimal `Err.Number` semantics.
Formal obligations:
- formal state-machine spec for error-mode transitions.
Gate:
- error-state corpus green + transition invariants validated.

### v12 — `mvp-resume-goto-v12` (F2)
Scope:
- `On Error GoTo label`, `Resume`, `Resume Next`.
Formal obligations:
- control-flow correctness obligations for resumption edges.
Gate:
- explicit err-edge conformance + formal edge obligations green.

### v13 — `mvp-variant-numeric-v13` (F2)
Scope:
- numeric variant coercion core (`Integer`, `Long`, `Double`, `Boolean`).
Formal obligations:
- Lean/Kani-backed consistency checks against coercion table slices.
Gate:
- decision-table coverage >=95% in scope + formal checks linked.

### v14 — `mvp-string-bstr-v14` (F1)
Scope:
- BSTR strings and core functions subset.
Formal obligations:
- Kani invariants for string length/capacity and operation safety paths.
Gate:
- string corpus green + safety harnesses pass.

### v15 — `mvp-date-currency-v15` (F2)
Scope:
- Date/Currency conversion and arithmetic subset.
Formal obligations:
- formalized conversion-law checks for selected operation set.
Gate:
- conversion corpus green + formal law checks pass.

### v16 — `mvp-semantics-model-v16` (F3)
Scope:
- define small-step executable spec for supported subset and trace format.
Formal obligations:
- translation/equivalence checks: runtime trace vs spec trace on corpus.
Gate:
- spec-vs-runtime trace equivalence green for required suite.

### v17 — `mvp-proof-integration-v17` (F3)
Scope:
- integrate formal obligation runner into standard quality gate workflow.
Formal obligations:
- enforce proof obligation manifest and failure policy in scripts/CI.
Gate:
- `meta-check` includes formal lane with deterministic pass/fail behavior.

### v18 — `mvp-divergence-proof-closure-v18` (F3)
Scope:
- move divergence handling from narrative-only to proof-linked classification where possible.
Formal obligations:
- each open divergence in in-scope behavior links to either:
  - failing proof obligation, or
  - failing reproducible conformance artifact.
Gate:
- divergence index audit passes (no ungrounded entries).

### v19 — `mvp-ir-optimizer-v19` (F3)
Scope:
- first real optimization pack:
  - constant fold,
  - dead branch elimination,
  - redundant coercion elimination.
Formal obligations:
- optimization translation validation (pre/post observable equivalence) for in-scope subset.
Gate:
- optimization-on/off parity green with formal equivalence obligations green.

### v20 — `mvp-jit-exec-v20` (F3)
Scope:
- real CLIF emission + executable JIT for supported subset.
Formal obligations:
- VM-vs-JIT equivalence checks on conformance corpus and generated property corpus.
Gate:
- required corpus semantic identity across VM/JIT + formal equivalence artifacts.

### v21 — `mvp-perf-stabilization-v21` (F3)
Scope:
- correctness-preserving performance graduation for:
  - broadword dispatch path,
  - zero-copy bytecode path,
  - selected register-window heuristics.
Formal obligations:
- guardrail proofs/checks that perf flags preserve in-scope semantics.
Gate:
- measurable benchmark gain with zero new semantic divergences and no failing formal obligations.

### v22 — `mvp-jit-loops-v22` (F3)
Scope:
- enable CLIF execution for loop backedges in the supported subset.
Formal obligations:
- VM-vs-JIT equivalence checks for `For` and `Do` loop patterns.
Gate:
- loop conformance parity green on required VM/JIT cells.

### v23 — `mvp-formal-strict-kani-v23` (F3)
Scope:
- activate strict formal lane option for Kani-backed obligations.
Formal obligations:
- enforce `-RequireKani` behavior and optional CI Kani lane wiring.
Gate:
- strict formal mode path is reproducible and documented.

### v24 — `mvp-jit-calls-v24` (F3)
Scope:
- support JIT execution of static call subset while preserving fallback for unsupported runtime features.
Formal obligations:
- VM-vs-JIT parity checks for call-flow subset.
Gate:
- call conformance/parity cases green.

### v25 — `mvp-optimizer-pack2-v25` (F3)
Scope:
- expand optimizer with safe branch/select folding and straight-line cleanup.
Formal obligations:
- parity checks between optimized and non-optimized bytecode on representative cases.
Gate:
- optimizer parity and conformance remain green.

### v26 — `mvp-perf-shape-v26` (F3)
Scope:
- stabilize profile defaults, evidence paths, and benchmark flow on `v26`.
Formal obligations:
- profile-default/script/evidence consistency checks.
Gate:
- `v26` matrix gate pass + formal report updated + benchmark artifact generated.

### v27 — `mvp-formal-async-ops-v27` (F3)
Scope:
- make async execution the default operational path for long-running formal/Kani profile steps.
- persist run-state, logs, and completion evidence conventions for unattended profile execution.
Formal obligations:
- executable checks for async orchestration contract (`Start`/`Status`/`Tail`/`Wait`/`Stop`).
- evidence-path stability checks for async logs and completion artifacts.
Gate:
- at least one profile-scoped formal run is executed and recovered fully through async workflow.

### v28 — `mvp-kani-unblock-v28` (F3)
Scope:
- unblock failing/unstable Kani obligations (starting with VM `pc` progression harness) via harness decomposition, bounded domains, and targeted stubbing.
Formal obligations:
- convert current Kani `todo`/OOM failure into either:
  - passing bounded harness set, or
  - explicit split obligations with reproducible failure taxonomy and next actions.
Gate:
- no opaque Kani failures in active required obligations; each non-pass has structured classification and bounded reproduction.

### v29 — `mvp-kani-capacity-v29` (F3)
Scope:
- add verification capacity controls (bounds, unwinding guidance, optional memory/time budget profiles) for reproducible Kani operation.
Formal obligations:
- harness-level configuration checks and run-manifest integrity checks.
- deterministic re-run obligations for selected Kani harness subsets.
Gate:
- strict formal lane can be re-run with reproducible status under documented capacity profiles.

### v30 — `mvp-com-variant-conformance-v30` (F3)
Scope:
- deepen semantic checks around canonical COM `VARIANT` layout/flags semantics and VBA compatibility behavior.
Formal obligations:
- layout invariants, `VARENUM` compatibility, and reserved-field handling checks.
- executable equivalence checks for representative coercion/arithmetic behaviors crossing runtime boundaries.
Gate:
- COM `VARIANT` conformance obligations green for required subset.

### v31 — `mvp-boundary-marshalling-v31` (F3)
Scope:
- formalize and test runtime boundary marshalling rules (host/COM <-> runtime canonical form).
Formal obligations:
- roundtrip and no-loss guarantees for in-scope Variant subtypes.
- deterministic failure-surface checks for unsupported boundary values.
Gate:
- boundary marshalling parity and formal roundtrip checks green.

### v32 — `mvp-language-coverage-audit-v32` (F3)
Scope:
- produce executable language coverage map against full VBA7 target surface.
- classify all uncovered constructs by complexity/risk and define closure order.
Formal obligations:
- coverage ledger integrity checks (no unknown/uncategorized parser/binder/runtime gaps).
Gate:
- published coverage index with actionable backlog and profile-linked closure tasks.

### v33 — `mvp-language-coverage-core-v33` (F3)
Scope:
- implement and stabilize highest-impact missing language constructs from coverage audit (control flow/type/coercion semantics first).
Formal obligations:
- executable semantic/parity checks for each newly covered construct group.
Gate:
- coverage index materially reduced with new conformance corpus and green required cells.

### v34 — `mvp-language-coverage-objects-v34` (F3)
Scope:
- expand coverage for object/class/module interaction semantics in core engine scope (excluding Forms and external COM library breadth work).
Formal obligations:
- lifecycle/dispatch/state-transition invariants for newly covered object behavior.
Gate:
- object-semantics coverage milestones green with formal linkage.

### v35 — `mvp-jit-optimizer-hotpaths-v35` (F3)
Scope:
- expand JIT/optimizer support breadth on proven-safe language subsets and hotspot instruction paths.
Formal obligations:
- VM-vs-JIT and opt-on/off equivalence for all newly enabled hot paths.
Gate:
- parity remains green while benchmark harness shows measurable improvements on selected workloads.

### v36 — `mvp-full-coverage-perf-gate-v36` (F3)
Scope:
- consolidate language coverage and performance evidence into a new stabilization gate.
Formal obligations:
- required coverage closure checks + formal manifest completeness checks for declared scope.
Gate:
- declared full-language-in-scope matrix cells green, formal obligations current, and performance guardrails met.

## Execution Rhythm Per Profile

1. Implement pass-pack deltas.
2. Add/expand conformance corpus.
3. Add/expand formal obligations for profile level.
4. Update matrix required cells for profile scope.
5. Update divergence/evidence records.
6. Run `meta-check -Fast -Conformance -Matrix` plus formal lane.
   - For long Kani profiles, run formal as an async step using `scripts/run-formal-kani-async.ps1` (`Start`/`Status`/`Tail`/`Wait`).
7. Commit/push and generate status tour.

## Parallelization Template (Per Profile)

1. Worker A: `P0/P1` (syntax/bind).
2. Worker B: `P2/P3/P4` (IR/lowering/control flow).
3. Worker C: `P5/P6` (emitter/runtime).
4. Worker D: `P7/P8` (conformance/evidence/docs).
5. Worker E: `P9` (formal obligations and proof artifact integration).

Merge order:
1. A+B lock semantic contract.
2. C implements executable behavior.
3. E lands formal obligations.
4. D finalizes gates/docs from observed outcomes.

## Global Quantitative Targets Across v2-v36

1. Corpus growth: minimum +10 conformance cases per profile.
2. Formal growth: minimum +3 formal obligations per profile from v3 onward.
3. Matrix breadth growth: at least one required-cell dimension expansion every 3 profiles.
4. Divergence hygiene: each open divergence has reproduction + impact + next action + evidence link.
5. Safety lanes:
   - no clippy warnings,
   - no test regressions,
   - formal lane status tracked and blocking where mandated by profile level.
6. Coverage closure:
   - maintain a language-coverage index from `v32` onward,
   - each profile must reduce uncovered in-scope language surface or close a coverage-class blocker.

## Immediate Operationalization

1. Execute `WS-CF-V2` as profile `v2`, adding its new formal obligations immediately.
2. Instantiate dedicated workset docs per profile from v3 onward using this ladder.
3. Keep this ladder as canonical forward queue and update only with explicit replanning commits.
