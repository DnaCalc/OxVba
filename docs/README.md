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
| Phase 12 Status | [`PHASE12_STATUS.md`](PHASE12_STATUS.md) | Declared profile scope and final conformance/stabilization gate artifacts. |
| Work Set Plan (v2) | [`worksets/WORKSET_2026-02-27_CONTROLFLOW_V2.md`](worksets/WORKSET_2026-02-27_CONTROLFLOW_V2.md) | Detailed execution-grade plan for control-flow expansion beyond `mvp-int32-core-v1`. |
| Work Set Plan (v3) | [`worksets/WORKSET_2026-02-27_FORMAL_FOUNDATION_V3.md`](worksets/WORKSET_2026-02-27_FORMAL_FOUNDATION_V3.md) | Formal infrastructure work set for manifest-driven obligations and reporting. |
| Work Set Plan (v4) | [`worksets/WORKSET_2026-02-27_BOOLEAN_LOGIC_V4.md`](worksets/WORKSET_2026-02-27_BOOLEAN_LOGIC_V4.md) | Relational + boolean condition semantics expansion work set. |
| Work Set Plan (v5) | [`worksets/WORKSET_2026-02-27_ELSE_PATHS_V5.md`](worksets/WORKSET_2026-02-27_ELSE_PATHS_V5.md) | Branch-chain completion work set (`Else`/`ElseIf`) with formal branch determinism checks. |
| Work Set Plan (v6) | [`worksets/WORKSET_2026-02-27_WHILE_LOOP_V6.md`](worksets/WORKSET_2026-02-27_WHILE_LOOP_V6.md) | `Do`-loop semantics work set with pre/post condition flow and `Exit Do`. |
| Work Set Plan (v7) | [`worksets/WORKSET_2026-02-27_SELECT_CASE_V7.md`](worksets/WORKSET_2026-02-27_SELECT_CASE_V7.md) | Select-case dispatch work set with first-match and fallback semantics. |
| Work Set Plan (v8) | [`worksets/WORKSET_2026-02-27_PROCEDURES_V8.md`](worksets/WORKSET_2026-02-27_PROCEDURES_V8.md) | Procedure/call-frame baseline work set with `Call` dispatch and return semantics. |
| Work Set Plan (v9) | [`worksets/WORKSET_2026-02-27_PARAMS_V9.md`](worksets/WORKSET_2026-02-27_PARAMS_V9.md) | Parameter passing work set for `ByVal`/`ByRef` subset semantics. |
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
