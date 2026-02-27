# OxVba Documentation

## Authoritative Documents

| Document | Location | Description |
|---|---|---|
| **MACH-1000 Plan** | [`MACH1000_PLAN.md`](../MACH1000_PLAN.md) | The definitive OxVba project plan. Charter, architecture, formal approach, testing strategy, implementation sequencing. |
| Implementation Log | [`IMPLEMENTATION_LOG.md`](IMPLEMENTATION_LOG.md) | Rolling execution log for implementation progress. |
| Building | [`BUILDING.md`](BUILDING.md) | Build and local verification instructions. |
| Contributing | [`CONTRIBUTING.md`](CONTRIBUTING.md) | Contribution workflow and compatibility evidence expectations. |
| Testing | [`TESTING.md`](TESTING.md) | Test lanes, current coverage, and next testing milestones. |
| Architecture | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Current architecture snapshot and near-term evolution notes. |
| IR Design | [`IR_DESIGN.md`](IR_DESIGN.md) | Multi-level IR status and next implementation targets. |
| Bytecode Format | [`BYTECODE_FORMAT.md`](BYTECODE_FORMAT.md) | Bytecode representation status and planned evolution. |
| VM Architecture | [`VM_ARCHITECTURE.md`](VM_ARCHITECTURE.md) | VM implementation status and next milestones. |
| Conformance | [`CONFORMANCE.md`](CONFORMANCE.md) | Conformance assets, commands, and comparison policy. |
| Profile v2 Status | [`PROFILE_STATUS_V2.md`](PROFILE_STATUS_V2.md) | Current gate status contract for `mvp-controlflow-v2`. |
| Profile v3 Status | [`PROFILE_STATUS_V3.md`](PROFILE_STATUS_V3.md) | Current gate status contract for `mvp-formal-foundation-v3`. |
| Profile v4 Status | [`PROFILE_STATUS_V4.md`](PROFILE_STATUS_V4.md) | Current gate status contract for `mvp-boolean-logic-v4`. |
| Profile v5 Status | [`PROFILE_STATUS_V5.md`](PROFILE_STATUS_V5.md) | Current gate status contract for `mvp-else-paths-v5`. |
| Profile v6 Status | [`PROFILE_STATUS_V6.md`](PROFILE_STATUS_V6.md) | Current gate status contract for `mvp-while-loop-v6`. |
| Profile v7 Status | [`PROFILE_STATUS_V7.md`](PROFILE_STATUS_V7.md) | Current gate status contract for `mvp-select-case-v7`. |
| Profile v8 Status | [`PROFILE_STATUS_V8.md`](PROFILE_STATUS_V8.md) | Current gate status contract for `mvp-procedures-v8`. |
| Profile v9 Status | [`PROFILE_STATUS_V9.md`](PROFILE_STATUS_V9.md) | Current gate status contract for `mvp-params-v9`. |
| Profile v10 Status | [`PROFILE_STATUS_V10.md`](PROFILE_STATUS_V10.md) | Current gate status contract for `mvp-arrays-v10`. |
| Profile v11 Status | [`PROFILE_STATUS_V11.md`](PROFILE_STATUS_V11.md) | Current gate status contract for `mvp-error-state-v11`. |
| Profile v12 Status | [`PROFILE_STATUS_V12.md`](PROFILE_STATUS_V12.md) | Current gate status contract for `mvp-resume-goto-v12`. |
| Profile v13 Status | [`PROFILE_STATUS_V13.md`](PROFILE_STATUS_V13.md) | Current gate status contract for `mvp-variant-numeric-v13`. |
| Profile v14 Status | [`PROFILE_STATUS_V14.md`](PROFILE_STATUS_V14.md) | Current gate status contract for `mvp-string-bstr-v14`. |
| Profile v15 Status | [`PROFILE_STATUS_V15.md`](PROFILE_STATUS_V15.md) | Current gate status contract for `mvp-date-currency-v15`. |
| Profile v16 Status | [`PROFILE_STATUS_V16.md`](PROFILE_STATUS_V16.md) | Current gate status contract for `mvp-semantics-model-v16`. |
| Profile v17 Status | [`PROFILE_STATUS_V17.md`](PROFILE_STATUS_V17.md) | Current gate status contract for `mvp-proof-integration-v17`. |
| Profile v18 Status | [`PROFILE_STATUS_V18.md`](PROFILE_STATUS_V18.md) | Current gate status contract for `mvp-divergence-proof-closure-v18`. |
| Profile v19 Status | [`PROFILE_STATUS_V19.md`](PROFILE_STATUS_V19.md) | Current gate status contract for `mvp-ir-optimizer-v19`. |
| Profile v20 Status | [`PROFILE_STATUS_V20.md`](PROFILE_STATUS_V20.md) | Current gate status contract for `mvp-jit-exec-v20`. |
| Profile v21 Status | [`PROFILE_STATUS_V21.md`](PROFILE_STATUS_V21.md) | Current gate status contract for `mvp-perf-stabilization-v21`. |
| Phase 12 Status | [`PHASE12_STATUS.md`](PHASE12_STATUS.md) | Declared profile scope and final conformance/stabilization gate artifacts. |
| Work Set Plan (v2) | [`worksets/WORKSET_2026-02-27_CONTROLFLOW_V2.md`](worksets/WORKSET_2026-02-27_CONTROLFLOW_V2.md) | Detailed execution-grade plan for control-flow expansion beyond `mvp-int32-core-v1`. |
| Work Set Plan (v3) | [`worksets/WORKSET_2026-02-27_FORMAL_FOUNDATION_V3.md`](worksets/WORKSET_2026-02-27_FORMAL_FOUNDATION_V3.md) | Formal infrastructure work set for manifest-driven obligations and reporting. |
| Work Set Plan (v4) | [`worksets/WORKSET_2026-02-27_BOOLEAN_LOGIC_V4.md`](worksets/WORKSET_2026-02-27_BOOLEAN_LOGIC_V4.md) | Relational + boolean condition semantics expansion work set. |
| Work Set Plan (v5) | [`worksets/WORKSET_2026-02-27_ELSE_PATHS_V5.md`](worksets/WORKSET_2026-02-27_ELSE_PATHS_V5.md) | Branch-chain completion work set (`Else`/`ElseIf`) with formal branch determinism checks. |
| Work Set Plan (v6) | [`worksets/WORKSET_2026-02-27_WHILE_LOOP_V6.md`](worksets/WORKSET_2026-02-27_WHILE_LOOP_V6.md) | `Do`-loop semantics work set with pre/post condition flow and `Exit Do`. |
| Work Set Plan (v7) | [`worksets/WORKSET_2026-02-27_SELECT_CASE_V7.md`](worksets/WORKSET_2026-02-27_SELECT_CASE_V7.md) | Select-case dispatch work set with first-match and fallback semantics. |
| Work Set Plan (v8) | [`worksets/WORKSET_2026-02-27_PROCEDURES_V8.md`](worksets/WORKSET_2026-02-27_PROCEDURES_V8.md) | Procedure/call-frame baseline work set with `Call` dispatch and return semantics. |
| Work Set Plan (v9) | [`worksets/WORKSET_2026-02-27_PARAMS_V9.md`](worksets/WORKSET_2026-02-27_PARAMS_V9.md) | Parameter passing work set for `ByVal`/`ByRef` subset semantics. |
| Work Set Plan (v10) | [`worksets/WORKSET_2026-02-27_ARRAYS_V10.md`](worksets/WORKSET_2026-02-27_ARRAYS_V10.md) | Fixed-size array work set with indexed load/store and bounds checks. |
| Work Set Plan (v11) | [`worksets/WORKSET_2026-02-27_ERROR_STATE_V11.md`](worksets/WORKSET_2026-02-27_ERROR_STATE_V11.md) | Error-state work set for `On Error Resume Next` + `Err.Number` subset. |
| Work Set Plan (v12) | [`worksets/WORKSET_2026-02-27_RESUME_GOTO_V12.md`](worksets/WORKSET_2026-02-27_RESUME_GOTO_V12.md) | Error-control work set for `On Error GoTo 0` and `Resume Next`. |
| Work Set Plan (v13) | [`worksets/WORKSET_2026-02-27_VARIANT_NUMERIC_V13.md`](worksets/WORKSET_2026-02-27_VARIANT_NUMERIC_V13.md) | Numeric variant/coercion proof-oriented work set. |
| Work Set Plan (v14) | [`worksets/WORKSET_2026-02-27_STRING_BSTR_V14.md`](worksets/WORKSET_2026-02-27_STRING_BSTR_V14.md) | String/BSTR semantics work set. |
| Work Set Plan (v15) | [`worksets/WORKSET_2026-02-27_DATE_CURRENCY_V15.md`](worksets/WORKSET_2026-02-27_DATE_CURRENCY_V15.md) | Date/currency semantics work set. |
| Work Set Plan (v16) | [`worksets/WORKSET_2026-02-27_SEMANTICS_MODEL_V16.md`](worksets/WORKSET_2026-02-27_SEMANTICS_MODEL_V16.md) | Small-step/spec-trace modeling work set. |
| Work Set Plan (v17) | [`worksets/WORKSET_2026-02-27_PROOF_INTEGRATION_V17.md`](worksets/WORKSET_2026-02-27_PROOF_INTEGRATION_V17.md) | Formal runner/gate integration work set. |
| Work Set Plan (v18) | [`worksets/WORKSET_2026-02-27_DIVERGENCE_PROOF_CLOSURE_V18.md`](worksets/WORKSET_2026-02-27_DIVERGENCE_PROOF_CLOSURE_V18.md) | Divergence/proof-closure evidence work set. |
| Work Set Plan (v19) | [`worksets/WORKSET_2026-02-27_IR_OPTIMIZER_V19.md`](worksets/WORKSET_2026-02-27_IR_OPTIMIZER_V19.md) | IR optimizer correctness/parity work set. |
| Work Set Plan (v20) | [`worksets/WORKSET_2026-02-27_JIT_EXEC_V20.md`](worksets/WORKSET_2026-02-27_JIT_EXEC_V20.md) | JIT execution parity work set. |
| Work Set Plan (v21) | [`worksets/WORKSET_2026-02-27_PERF_STABILIZATION_V21.md`](worksets/WORKSET_2026-02-27_PERF_STABILIZATION_V21.md) | Performance guardrail and benchmark evidence work set. |
| Profile Ladder | [`worksets/PROFILE_LADDER_2026-02-27_MACH1000.md`](worksets/PROFILE_LADDER_2026-02-27_MACH1000.md) | MACH1000-scale 20-step forward profile roadmap (`v2`..`v21`). |
| Status Tours | [`status-tours/`](status-tours/) | Date-stamped orientation/showcase docs for implemented project state. |
| Formal | [`FORMAL.md`](FORMAL.md) | Lean/Kani formal scaffold status and structure. |

## Synthesis Records

| Record | Location | Description |
|---|---|---|
| Synthesis process | [`synthesis/README.md`](../synthesis/README.md) | How synthesis runs work |
| MACH-1000 synthesis | [`synthesis/runs/20260226-mach1000-synthesis/`](../synthesis/runs/20260226-mach1000-synthesis/README.md) | Decision log and report for the synthesis that produced the MACH-1000 plan |

## Archive

Superseded planning documents retained for provenance:

| Document | Date | Status |
|---|---|---|
| [PLAN v1](archive/PLAN_v1_20260226.md) | 2026-02-26 | Superseded by MACH-1000 Plan |
| [MACH-1000 Brainstorm](archive/BRAINSTORM_MACH1000_20260226.md) | 2026-02-26 | Consumed by synthesis into MACH-1000 Plan |
