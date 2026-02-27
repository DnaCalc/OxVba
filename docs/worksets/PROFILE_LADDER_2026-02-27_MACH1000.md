# PROFILE_LADDER_2026-02-27_MACH1000.md

## Why This Exists
`mvp-int32-core-v1` is stabilized, but MACH1000 needs an execution queue that scales beyond incremental feature work.

This document defines the **next 25 concrete profile steps** and explicitly prioritizes formal verification earlier in the ladder.

## Replan Delta (Formal-First Revision)
Compared to the previous ladder version:

1. Formal verification is moved into early/mid profiles instead of late-only support.
2. External COM boundary and Forms runtime profiles are removed from this 20-step sequence.
3. Late profiles focus on proof-backed optimizer/JIT/performance graduation.
4. Every profile has explicit verification obligations, not just tests.

## How Many Profile Steps Are There?
There is no theoretical upper bound (full VBA parity and performance optimization are open-ended).

Concrete planning horizon in this document:

- Previously completed profile at planning time: `v1`
- Planned profiles in this ladder: `v2` through `v26`
- Total concrete future steps here: **25**
- Execution status now: **completed through `v26`**.

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

## 25-Profile Ladder (v2-v26)

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

## Global Quantitative Targets Across v2-v26

1. Corpus growth: minimum +10 conformance cases per profile.
2. Formal growth: minimum +3 formal obligations per profile from v3 onward.
3. Matrix breadth growth: at least one required-cell dimension expansion every 3 profiles.
4. Divergence hygiene: each open divergence has reproduction + impact + next action + evidence link.
5. Safety lanes:
   - no clippy warnings,
   - no test regressions,
   - formal lane status tracked and blocking where mandated by profile level.

## Immediate Operationalization

1. Execute `WS-CF-V2` as profile `v2`, adding its new formal obligations immediately.
2. Instantiate dedicated workset docs per profile from v3 onward using this ladder.
3. Keep this ladder as canonical forward queue and update only with explicit replanning commits.
