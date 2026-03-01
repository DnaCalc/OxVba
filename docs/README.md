# OxVba Documentation

## Authoritative Documents

| Document | Location | Description |
|---|---|---|
| **MACH-1000 Plan** | [`MACH1000_PLAN.md`](../MACH1000_PLAN.md) | The definitive OxVba project plan. Charter, architecture, formal approach, testing strategy, implementation sequencing. |
| **AutoRun State** | [`AUTORUN_STATE.md`](AUTORUN_STATE.md) | Active execution guardrail for continuous runs. Current target is non-HAL completion/hardening ladders through terminal gate `v186`; blocker handling follows `CURRENT_BLOCKERS.md`. |
| Implementation Log | [`IMPLEMENTATION_LOG.md`](IMPLEMENTATION_LOG.md) | Rolling execution log for implementation progress. |
| Building | [`BUILDING.md`](BUILDING.md) | Build and local verification instructions. |
| Contributing | [`CONTRIBUTING.md`](CONTRIBUTING.md) | Contribution workflow and compatibility evidence expectations. |
| Testing | [`TESTING.md`](TESTING.md) | Test lanes, current coverage, and next testing milestones. |
| Architecture | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Current architecture snapshot and near-term evolution notes. |
| IR Design | [`IR_DESIGN.md`](IR_DESIGN.md) | Multi-level IR status and next implementation targets. |
| Bytecode Format | [`BYTECODE_FORMAT.md`](BYTECODE_FORMAT.md) | Bytecode representation status and planned evolution. |
| VM Architecture | [`VM_ARCHITECTURE.md`](VM_ARCHITECTURE.md) | VM implementation status and next milestones. |
| Conformance | [`CONFORMANCE.md`](CONFORMANCE.md) | Conformance assets, commands, and comparison policy. |
| Spec Drafts Index | [`spec/README.md`](spec/README.md) | Early-stage design/contract drafts that prepare future normative specs. |
| HAL Design Draft | [`spec/HAL_DESIGN_DRAFT.md`](spec/HAL_DESIGN_DRAFT.md) | Host Abstraction Layer scope, principles, profile targets, and staged spec-run plan. |
| HAL Interface Draft | [`spec/HAL_INTERFACE_DRAFT.md`](spec/HAL_INTERFACE_DRAFT.md) | Proposed HAL contracts, capability descriptors, maturity levels, and policy gates. |
| HAL Conformance Draft | [`spec/HAL_CONFORMANCE_DRAFT.md`](spec/HAL_CONFORMANCE_DRAFT.md) | Proposed HAL conformance levels, test obligations, and evidence model. |
| HAL Profile Matrix Draft | [`spec/HAL_PROFILE_MATRIX_DRAFT.md`](spec/HAL_PROFILE_MATRIX_DRAFT.md) | Initial five-profile (Windows/Linux/macOS/WASM/Null) capability+maturity planning matrix. |
| Spec Source Sets | [`spec/sources/README.md`](spec/sources/README.md) | Local source manifests for language/runtime specification references. |
| Diagnostic Taxonomy | [`DIAGNOSTIC_TAXONOMY.md`](DIAGNOSTIC_TAXONOMY.md) | Consolidated user-facing compiler/typecheck diagnostic categories and message mapping. |
| Profile v2 Status | [`PROFILE_STATUS_V2.md`](profile-status/PROFILE_STATUS_V2.md) | Current gate status contract for `mvp-controlflow-v2`. |
| Profile v3 Status | [`PROFILE_STATUS_V3.md`](profile-status/PROFILE_STATUS_V3.md) | Current gate status contract for `mvp-formal-foundation-v3`. |
| Profile v4 Status | [`PROFILE_STATUS_V4.md`](profile-status/PROFILE_STATUS_V4.md) | Current gate status contract for `mvp-boolean-logic-v4`. |
| Profile v5 Status | [`PROFILE_STATUS_V5.md`](profile-status/PROFILE_STATUS_V5.md) | Current gate status contract for `mvp-else-paths-v5`. |
| Profile v6 Status | [`PROFILE_STATUS_V6.md`](profile-status/PROFILE_STATUS_V6.md) | Current gate status contract for `mvp-while-loop-v6`. |
| Profile v7 Status | [`PROFILE_STATUS_V7.md`](profile-status/PROFILE_STATUS_V7.md) | Current gate status contract for `mvp-select-case-v7`. |
| Profile v8 Status | [`PROFILE_STATUS_V8.md`](profile-status/PROFILE_STATUS_V8.md) | Current gate status contract for `mvp-procedures-v8`. |
| Profile v9 Status | [`PROFILE_STATUS_V9.md`](profile-status/PROFILE_STATUS_V9.md) | Current gate status contract for `mvp-params-v9`. |
| Profile v10 Status | [`PROFILE_STATUS_V10.md`](profile-status/PROFILE_STATUS_V10.md) | Current gate status contract for `mvp-arrays-v10`. |
| Profile v11 Status | [`PROFILE_STATUS_V11.md`](profile-status/PROFILE_STATUS_V11.md) | Current gate status contract for `mvp-error-state-v11`. |
| Profile v12 Status | [`PROFILE_STATUS_V12.md`](profile-status/PROFILE_STATUS_V12.md) | Current gate status contract for `mvp-resume-goto-v12`. |
| Profile v13 Status | [`PROFILE_STATUS_V13.md`](profile-status/PROFILE_STATUS_V13.md) | Current gate status contract for `mvp-variant-numeric-v13`. |
| Profile v14 Status | [`PROFILE_STATUS_V14.md`](profile-status/PROFILE_STATUS_V14.md) | Current gate status contract for `mvp-string-bstr-v14`. |
| Profile v15 Status | [`PROFILE_STATUS_V15.md`](profile-status/PROFILE_STATUS_V15.md) | Current gate status contract for `mvp-date-currency-v15`. |
| Profile v16 Status | [`PROFILE_STATUS_V16.md`](profile-status/PROFILE_STATUS_V16.md) | Current gate status contract for `mvp-semantics-model-v16`. |
| Profile v17 Status | [`PROFILE_STATUS_V17.md`](profile-status/PROFILE_STATUS_V17.md) | Current gate status contract for `mvp-proof-integration-v17`. |
| Profile v18 Status | [`PROFILE_STATUS_V18.md`](profile-status/PROFILE_STATUS_V18.md) | Current gate status contract for `mvp-divergence-proof-closure-v18`. |
| Profile v19 Status | [`PROFILE_STATUS_V19.md`](profile-status/PROFILE_STATUS_V19.md) | Current gate status contract for `mvp-ir-optimizer-v19`. |
| Profile v20 Status | [`PROFILE_STATUS_V20.md`](profile-status/PROFILE_STATUS_V20.md) | Current gate status contract for `mvp-jit-exec-v20`. |
| Profile v21 Status | [`PROFILE_STATUS_V21.md`](profile-status/PROFILE_STATUS_V21.md) | Current gate status contract for `mvp-perf-stabilization-v21`. |
| Profile v22 Status | [`PROFILE_STATUS_V22.md`](profile-status/PROFILE_STATUS_V22.md) | Current gate status contract for `mvp-jit-loops-v22`. |
| Profile v23 Status | [`PROFILE_STATUS_V23.md`](profile-status/PROFILE_STATUS_V23.md) | Current gate status contract for `mvp-formal-strict-kani-v23`. |
| Profile v24 Status | [`PROFILE_STATUS_V24.md`](profile-status/PROFILE_STATUS_V24.md) | Current gate status contract for `mvp-jit-calls-v24`. |
| Profile v25 Status | [`PROFILE_STATUS_V25.md`](profile-status/PROFILE_STATUS_V25.md) | Current gate status contract for `mvp-optimizer-pack2-v25`. |
| Profile v26 Status | [`PROFILE_STATUS_V26.md`](profile-status/PROFILE_STATUS_V26.md) | Current gate status contract for `mvp-perf-shape-v26`. |
| Profile v27 Status | [`PROFILE_STATUS_V27.md`](profile-status/PROFILE_STATUS_V27.md) | Current gate status contract for `mvp-formal-async-ops-v27`. |
| Profile v28 Status | [`PROFILE_STATUS_V28.md`](profile-status/PROFILE_STATUS_V28.md) | Current gate status contract for `mvp-kani-unblock-v28`. |
| Profile v29 Status | [`PROFILE_STATUS_V29.md`](profile-status/PROFILE_STATUS_V29.md) | Current gate status contract for `mvp-kani-capacity-v29`. |
| Profile v30 Status | [`PROFILE_STATUS_V30.md`](profile-status/PROFILE_STATUS_V30.md) | Current gate status contract for `mvp-com-variant-conformance-v30`. |
| Profile v31 Status | [`PROFILE_STATUS_V31.md`](profile-status/PROFILE_STATUS_V31.md) | Current gate status contract for `mvp-boundary-marshalling-v31`. |
| Profile v32 Status | [`PROFILE_STATUS_V32.md`](profile-status/PROFILE_STATUS_V32.md) | Current gate status contract for `mvp-language-coverage-audit-v32`. |
| Profile v33 Status | [`PROFILE_STATUS_V33.md`](profile-status/PROFILE_STATUS_V33.md) | Current gate status contract for `mvp-language-coverage-core-v33`. |
| Profile v34 Status | [`PROFILE_STATUS_V34.md`](profile-status/PROFILE_STATUS_V34.md) | Current gate status contract for `mvp-language-coverage-objects-v34`. |
| Profile v35 Status | [`PROFILE_STATUS_V35.md`](profile-status/PROFILE_STATUS_V35.md) | Current gate status contract for `mvp-jit-optimizer-hotpaths-v35`. |
| Profile v36 Status | [`PROFILE_STATUS_V36.md`](profile-status/PROFILE_STATUS_V36.md) | Current gate status contract for `mvp-full-coverage-perf-gate-v36`. |
| Profile v37 Status | [`PROFILE_STATUS_V37.md`](profile-status/PROFILE_STATUS_V37.md) | Current gate status contract for `mvp-lang-optional-params-v37`. |
| Profile v38 Status | [`PROFILE_STATUS_V38.md`](profile-status/PROFILE_STATUS_V38.md) | Current gate status contract for `mvp-lang-named-args-v38`. |
| Profile v40 Status | [`PROFILE_STATUS_V40.md`](profile-status/PROFILE_STATUS_V40.md) | Current gate status contract for `mvp-lang-gosub-return-v40`. |
| Profile v41 Status | [`PROFILE_STATUS_V41.md`](profile-status/PROFILE_STATUS_V41.md) | Current gate status contract for `mvp-lang-on-error-goto-label-v41`. |
| Profile v42 Status | [`PROFILE_STATUS_V42.md`](profile-status/PROFILE_STATUS_V42.md) | Current gate status contract for `mvp-lang-redim-preserve-v42`. |
| Profile v43 Status | [`PROFILE_STATUS_V43.md`](profile-status/PROFILE_STATUS_V43.md) | Current gate status contract for `mvp-lang-udt-enum-const-v43`. |
| Profile v44 Status | [`PROFILE_STATUS_V44.md`](profile-status/PROFILE_STATUS_V44.md) | Current gate status contract for `mvp-lang-property-procedures-v44`. |
| Profile v45 Status | [`PROFILE_STATUS_V45.md`](profile-status/PROFILE_STATUS_V45.md) | Current gate status contract for `mvp-stdlib-conversion-core-v45`. |
| Profile v46 Status | [`PROFILE_STATUS_V46.md`](profile-status/PROFILE_STATUS_V46.md) | Current gate status contract for `mvp-stdlib-string-core-v46`. |
| Profile v47 Status | [`PROFILE_STATUS_V47.md`](profile-status/PROFILE_STATUS_V47.md) | Current gate status contract for `mvp-stdlib-string-advanced-v47`. |
| Profile v48 Status | [`PROFILE_STATUS_V48.md`](profile-status/PROFILE_STATUS_V48.md) | Current gate status contract for `mvp-stdlib-date-time-core-v48`. |
| Profile v49 Status | [`PROFILE_STATUS_V49.md`](profile-status/PROFILE_STATUS_V49.md) | Current gate status contract for `mvp-stdlib-math-financial-core-v49`. |
| Profile v50 Status | [`PROFILE_STATUS_V50.md`](profile-status/PROFILE_STATUS_V50.md) | Current gate status contract for `mvp-stdlib-array-variant-introspection-v50`. |
| Profile v51 Status | [`PROFILE_STATUS_V51.md`](profile-status/PROFILE_STATUS_V51.md) | Current gate status contract for `mvp-stdlib-error-surface-v51`. |
| Profile v52 Status | [`PROFILE_STATUS_V52.md`](profile-status/PROFILE_STATUS_V52.md) | Current gate status contract for `mvp-stdlib-host-sensitive-v52`. |
| Profile v53 Status | [`PROFILE_STATUS_V53.md`](profile-status/PROFILE_STATUS_V53.md) | Current gate status contract for `mvp-object-collection-core-v53`. |
| Profile v54 Status | [`PROFILE_STATUS_V54.md`](profile-status/PROFILE_STATUS_V54.md) | Current gate status contract for `mvp-class-lifecycle-v54`. |
| Profile v55 Status | [`PROFILE_STATUS_V55.md`](profile-status/PROFILE_STATUS_V55.md) | Current gate status contract for `mvp-com-dispatch-boundary-v55`. |
| Profile v56 Status | [`PROFILE_STATUS_V56.md`](profile-status/PROFILE_STATUS_V56.md) | Current gate status contract for `mvp-language-stdlib-consolidation-gate-v56`. |
| Profile v57 Status | [`PROFILE_STATUS_V57.md`](profile-status/PROFILE_STATUS_V57.md) | Current gate status contract for `mvp-formal-async-hardening-v57`. |
| Profile v58 Status | [`PROFILE_STATUS_V58.md`](profile-status/PROFILE_STATUS_V58.md) | Current gate status contract for `mvp-kani-harness-expansion-v58`. |
| Profile v59 Status | [`PROFILE_STATUS_V59.md`](profile-status/PROFILE_STATUS_V59.md) | Current gate status contract for `mvp-lang-line-continuation-v59`. |
| Profile v60 Status | [`PROFILE_STATUS_V60.md`](profile-status/PROFILE_STATUS_V60.md) | Current gate status contract for `mvp-lang-with-block-v60`. |
| Profile v61 Status | [`PROFILE_STATUS_V61.md`](profile-status/PROFILE_STATUS_V61.md) | Current gate status contract for `mvp-lang-conditional-compilation-v61`. |
| Profile v62 Status | [`PROFILE_STATUS_V62.md`](profile-status/PROFILE_STATUS_V62.md) | Current gate status contract for `mvp-stdlib-surface-architecture-v62`. |
| Profile v63 Status | [`PROFILE_STATUS_V63.md`](profile-status/PROFILE_STATUS_V63.md) | Current gate status contract for `mvp-jit-surface-expansion-v63`. |
| Profile v64 Status | [`PROFILE_STATUS_V64.md`](profile-status/PROFILE_STATUS_V64.md) | Current gate status contract for `mvp-perf-hotpath-baselines-v64`. |
| Profile v65 Status | [`PROFILE_STATUS_V65.md`](profile-status/PROFILE_STATUS_V65.md) | Current gate status contract for `mvp-integrated-correctness-perf-gate-v65`. |
| Profile v66 Status | [`PROFILE_STATUS_V66.md`](profile-status/PROFILE_STATUS_V66.md) | Current gate status contract for `mvp-stabilization-rollup-v66`. |
| Profile v67 Status | [`PROFILE_STATUS_V67.md`](profile-status/PROFILE_STATUS_V67.md) | Current gate status contract for `mvp-typing-type-lattice-v67`. |
| Profile v68 Status | [`PROFILE_STATUS_V68.md`](profile-status/PROFILE_STATUS_V68.md) | Current gate status contract for `mvp-typing-option-explicit-diagnostics-v68`. |
| Profile v69 Status | [`PROFILE_STATUS_V69.md`](profile-status/PROFILE_STATUS_V69.md) | Current gate status contract for `mvp-typing-default-type-rules-v69`. |
| Profile v70 Status | [`PROFILE_STATUS_V70.md`](profile-status/PROFILE_STATUS_V70.md) | Current gate status contract for `mvp-typing-procedure-signatures-v70`. |
| Profile v71 Status | [`PROFILE_STATUS_V71.md`](profile-status/PROFILE_STATUS_V71.md) | Current gate status contract for `mvp-typing-early-late-classification-v71`. |
| Profile v72 Status | [`PROFILE_STATUS_V72.md`](profile-status/PROFILE_STATUS_V72.md) | Current gate status contract for `mvp-typing-diagnostic-rollup-v72`. |
| Profile v73 Status | [`PROFILE_STATUS_V73.md`](profile-status/PROFILE_STATUS_V73.md) | Current gate status contract for `mvp-typing-coercion-matrix-v73`. |
| Profile v74 Status | [`PROFILE_STATUS_V74.md`](profile-status/PROFILE_STATUS_V74.md) | Current gate status contract for `mvp-typing-operator-result-rules-v74`. |
| Profile v75 Status | [`PROFILE_STATUS_V75.md`](profile-status/PROFILE_STATUS_V75.md) | Current gate status contract for `mvp-typing-call-coercion-early-late-v75`. |
| Profile v76 Status | [`PROFILE_STATUS_V76.md`](profile-status/PROFILE_STATUS_V76.md) | Current gate status contract for `mvp-typing-conversion-intrinsics-v76`. |
| Profile v77 Status | [`PROFILE_STATUS_V77.md`](profile-status/PROFILE_STATUS_V77.md) | Current gate status contract for `mvp-string-storage-semantics-v77`. |
| Profile v78 Status | [`PROFILE_STATUS_V78.md`](profile-status/PROFILE_STATUS_V78.md) | Current gate status contract for `mvp-string-compare-search-v78`. |
| Profile v79 Status | [`PROFILE_STATUS_V79.md`](profile-status/PROFILE_STATUS_V79.md) | Current gate status contract for `mvp-string-mutation-and-slices-v79`. |
| Profile v80 Status | [`PROFILE_STATUS_V80.md`](profile-status/PROFILE_STATUS_V80.md) | Current gate status contract for `mvp-array-type-model-v80`. |
| Profile v81 Status | [`PROFILE_STATUS_V81.md`](profile-status/PROFILE_STATUS_V81.md) | Current gate status contract for `mvp-array-bounds-and-indexing-v81`. |
| Profile v82 Status | [`PROFILE_STATUS_V82.md`](profile-status/PROFILE_STATUS_V82.md) | Current gate status contract for `mvp-array-redim-full-v82`. |
| Profile v83 Status | [`PROFILE_STATUS_V83.md`](profile-status/PROFILE_STATUS_V83.md) | Current gate status contract for `mvp-array-call-and-paramarray-v83`. |
| Profile v84 Status | [`PROFILE_STATUS_V84.md`](profile-status/PROFILE_STATUS_V84.md) | Current gate status contract for `mvp-array-boundary-and-dispatch-v84`. |
| Profile v85 Status | [`PROFILE_STATUS_V85.md`](profile-status/PROFILE_STATUS_V85.md) | Current gate status contract for `mvp-typed-execution-fastpaths-v85`. |
| Profile v86 Status | [`PROFILE_STATUS_V86.md`](profile-status/PROFILE_STATUS_V86.md) | Typing-ladder terminal gate status contract for `mvp-full-typing-conformance-gate-v86` (latest published profile status file before `v87..v106` ladder execution). |
| Profile v147 Status | [`PROFILE_STATUS_V147.md`](profile-status/PROFILE_STATUS_V147.md) | Current gate status contract for `mvp-profile-v147` (non-HAL gap baseline lock). |
| Profile v148 Status | [`PROFILE_STATUS_V148.md`](profile-status/PROFILE_STATUS_V148.md) | Current gate status contract for `mvp-profile-v148` (`Err` surface expansion I). |
| Profile v149 Status | [`PROFILE_STATUS_V149.md`](profile-status/PROFILE_STATUS_V149.md) | Current gate status contract for `mvp-profile-v149` (`Err` lifecycle transitions). |
| Profile v150 Status | [`PROFILE_STATUS_V150.md`](profile-status/PROFILE_STATUS_V150.md) | Current gate status contract for `mvp-profile-v150` (string runtime completion I: array-tag-aware `Join`). |
| Profile v151 Status | [`PROFILE_STATUS_V151.md`](profile-status/PROFILE_STATUS_V151.md) | Current gate status contract for `mvp-profile-v151` (`vbNullString` non-boundary typing guard tightening). |
| Profile v152 Status | [`PROFILE_STATUS_V152.md`](profile-status/PROFILE_STATUS_V152.md) | Current gate status contract for `mvp-profile-v152` (deterministic whole-UDT assignment lowering). |
| Profile v153 Status | [`PROFILE_STATUS_V153.md`](profile-status/PROFILE_STATUS_V153.md) | Current gate status contract for `mvp-profile-v153` (Null/Empty/Error coercion-edge normalization). |
| Profile v154 Status | [`PROFILE_STATUS_V154.md`](profile-status/PROFILE_STATUS_V154.md) | Current gate status contract for `mvp-profile-v154` (algorithmic `NPV`/`IRR`/`MIRR` financial execution subset). |
| Profile v155 Status | [`PROFILE_STATUS_V155.md`](profile-status/PROFILE_STATUS_V155.md) | Current gate status contract for `mvp-profile-v155` (algorithmic `Rate`/`NPer` financial execution subset). |
| Profile v107 Status | [`PROFILE_STATUS_V107.md`](profile-status/PROFILE_STATUS_V107.md) | Current gate status contract for `mvp-lang-with-member-target-v107`. |
| Profile v108-v146 Statuses | [`profile-status/`](profile-status/README.md) | AutoRun ladder status records are published through `PROFILE_STATUS_V146.md` for the active full-language/built-ins ladder range. |
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
| Work Set Plan (v22) | [`worksets/WORKSET_2026-02-27_JIT_LOOPS_V22.md`](worksets/WORKSET_2026-02-27_JIT_LOOPS_V22.md) | JIT loop/backedge parity work set. |
| Work Set Plan (v23) | [`worksets/WORKSET_2026-02-27_FORMAL_STRICT_KANI_V23.md`](worksets/WORKSET_2026-02-27_FORMAL_STRICT_KANI_V23.md) | Strict formal/Kani activation work set. |
| Work Set Plan (v24) | [`worksets/WORKSET_2026-02-27_JIT_CALLS_V24.md`](worksets/WORKSET_2026-02-27_JIT_CALLS_V24.md) | JIT call subset parity work set. |
| Work Set Plan (v25) | [`worksets/WORKSET_2026-02-27_OPTIMIZER_PACK2_V25.md`](worksets/WORKSET_2026-02-27_OPTIMIZER_PACK2_V25.md) | Optimizer pack2 correctness/parity work set. |
| Work Set Plan (v26) | [`worksets/WORKSET_2026-02-27_PERF_SHAPE_V26.md`](worksets/WORKSET_2026-02-27_PERF_SHAPE_V26.md) | Perf-shape stabilization and v26 closure work set. |
| Work Set Plan (v27) | [`worksets/WORKSET_2026-02-27_FORMAL_ASYNC_OPS_V27.md`](worksets/WORKSET_2026-02-27_FORMAL_ASYNC_OPS_V27.md) | Async formal/Kani operations stabilization work set. |
| Work Set Plan (v28) | [`worksets/WORKSET_2026-02-27_KANI_UNBLOCK_V28.md`](worksets/WORKSET_2026-02-27_KANI_UNBLOCK_V28.md) | Kani unblock and bounded harness hardening work set. |
| Work Set Plan (v29) | [`worksets/WORKSET_2026-02-27_KANI_CAPACITY_V29.md`](worksets/WORKSET_2026-02-27_KANI_CAPACITY_V29.md) | Kani capacity and reproducibility work set. |
| Work Set Plan (v30) | [`worksets/WORKSET_2026-02-27_COM_VARIANT_CONFORMANCE_V30.md`](worksets/WORKSET_2026-02-27_COM_VARIANT_CONFORMANCE_V30.md) | COM VARIANT conformance work set. |
| Work Set Plan (v31) | [`worksets/WORKSET_2026-02-27_BOUNDARY_MARSHALLING_V31.md`](worksets/WORKSET_2026-02-27_BOUNDARY_MARSHALLING_V31.md) | Boundary marshalling correctness work set. |
| Work Set Plan (v32) | [`worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_AUDIT_V32.md`](worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_AUDIT_V32.md) | Language coverage audit/index work set. |
| Work Set Plan (v33) | [`worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_CORE_V33.md`](worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_CORE_V33.md) | Core language coverage closure work set. |
| Work Set Plan (v34) | [`worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_OBJECTS_V34.md`](worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_OBJECTS_V34.md) | Object/class/module coverage closure work set. |
| Work Set Plan (v35) | [`worksets/WORKSET_2026-02-27_JIT_OPT_HOTPATHS_V35.md`](worksets/WORKSET_2026-02-27_JIT_OPT_HOTPATHS_V35.md) | JIT/optimizer hotpath parity/performance work set. |
| Work Set Plan (v36) | [`worksets/WORKSET_2026-02-27_FULL_COVERAGE_PERF_GATE_V36.md`](worksets/WORKSET_2026-02-27_FULL_COVERAGE_PERF_GATE_V36.md) | Coverage/performance consolidation gate work set. |
| Work Set Plan (v37) | [`worksets/WORKSET_2026-02-27_OPTIONAL_PARAMS_V37.md`](worksets/WORKSET_2026-02-27_OPTIONAL_PARAMS_V37.md) | Optional trailing parameter/default materialization work set. |
| Work Set Plan (v38) | [`worksets/WORKSET_2026-02-27_NAMED_ARGS_V38.md`](worksets/WORKSET_2026-02-27_NAMED_ARGS_V38.md) | Named-argument call binding and validation work set. |
| Work Set Plan (v40) | [`worksets/WORKSET_2026-02-27_GOSUB_RETURN_V40.md`](worksets/WORKSET_2026-02-27_GOSUB_RETURN_V40.md) | GoSub/Return label-flow subset work set. |
| Work Set Plan (v41) | [`worksets/WORKSET_2026-02-27_ON_ERROR_GOTO_LABEL_V41.md`](worksets/WORKSET_2026-02-27_ON_ERROR_GOTO_LABEL_V41.md) | On Error GoTo label handler-transfer subset work set. |
| Work Set Plan (v42) | [`worksets/WORKSET_2026-02-27_REDIM_PRESERVE_V42.md`](worksets/WORKSET_2026-02-27_REDIM_PRESERVE_V42.md) | ReDim/ReDim Preserve (1D literal-bound) subset work set. |
| Work Set Plan (v43) | [`worksets/WORKSET_2026-02-27_UDT_ENUM_CONST_V43.md`](worksets/WORKSET_2026-02-27_UDT_ENUM_CONST_V43.md) | Module-level Const/Enum and UDT declaration-baseline work set. |
| Work Set Plan (v44) | [`worksets/WORKSET_2026-02-27_PROPERTY_PROCEDURES_V44.md`](worksets/WORKSET_2026-02-27_PROPERTY_PROCEDURES_V44.md) | Property Get/Let/Set parsing and assignment-routing subset work set. |
| Work Set Plan (v45) | [`worksets/WORKSET_2026-02-27_STDLIB_CONVERSION_CORE_V45.md`](worksets/WORKSET_2026-02-27_STDLIB_CONVERSION_CORE_V45.md) | Intrinsic conversion subset baseline work set. |
| Work Set Plan (v46) | [`worksets/WORKSET_2026-02-27_STDLIB_STRING_CORE_V46.md`](worksets/WORKSET_2026-02-27_STDLIB_STRING_CORE_V46.md) | String-core intrinsic subset over current runtime model. |
| Work Set Plan (v47) | [`worksets/WORKSET_2026-02-27_STDLIB_STRING_ADVANCED_V47.md`](worksets/WORKSET_2026-02-27_STDLIB_STRING_ADVANCED_V47.md) | Advanced string intrinsic subset over current runtime model. |
| Work Set Plan (v48) | [`worksets/WORKSET_2026-02-27_STDLIB_DATE_TIME_CORE_V48.md`](worksets/WORKSET_2026-02-27_STDLIB_DATE_TIME_CORE_V48.md) | Date/time intrinsic subset work set. |
| Work Set Plan (v49) | [`worksets/WORKSET_2026-02-27_STDLIB_MATH_FINANCIAL_CORE_V49.md`](worksets/WORKSET_2026-02-27_STDLIB_MATH_FINANCIAL_CORE_V49.md) | Math and financial intrinsic subset work set. |
| Work Set Plan (v50) | [`worksets/WORKSET_2026-02-27_STDLIB_ARRAY_VARIANT_INTROSPECTION_V50.md`](worksets/WORKSET_2026-02-27_STDLIB_ARRAY_VARIANT_INTROSPECTION_V50.md) | Array/variant introspection intrinsic subset work set. |
| Work Set Plan (v51) | [`worksets/WORKSET_2026-02-27_STDLIB_ERROR_SURFACE_V51.md`](worksets/WORKSET_2026-02-27_STDLIB_ERROR_SURFACE_V51.md) | Error-surface intrinsic subset work set. |
| Work Set Plan (v52) | [`worksets/WORKSET_2026-02-27_STDLIB_HOST_SENSITIVE_V52.md`](worksets/WORKSET_2026-02-27_STDLIB_HOST_SENSITIVE_V52.md) | Host-sensitive intrinsic subset work set. |
| Work Set Plan (v53) | [`worksets/WORKSET_2026-02-27_OBJECT_COLLECTION_CORE_V53.md`](worksets/WORKSET_2026-02-27_OBJECT_COLLECTION_CORE_V53.md) | Collection-core object subset work set. |
| Work Set Plan (v54) | [`worksets/WORKSET_2026-02-27_CLASS_LIFECYCLE_V54.md`](worksets/WORKSET_2026-02-27_CLASS_LIFECYCLE_V54.md) | Class lifecycle subset work set. |
| Work Set Plan (v55) | [`worksets/WORKSET_2026-02-27_COM_DISPATCH_BOUNDARY_V55.md`](worksets/WORKSET_2026-02-27_COM_DISPATCH_BOUNDARY_V55.md) | COM/dispatch boundary subset work set. |
| Work Set Plan (v56) | [`worksets/WORKSET_2026-02-27_LANGUAGE_STDLIB_CONSOLIDATION_GATE_V56.md`](worksets/WORKSET_2026-02-27_LANGUAGE_STDLIB_CONSOLIDATION_GATE_V56.md) | Consolidation gate work set for language+stdlib+interop closure. |
| Work Set Plan (v57) | [`worksets/WORKSET_2026-02-28_FORMAL_ASYNC_HARDENING_V57.md`](worksets/WORKSET_2026-02-28_FORMAL_ASYNC_HARDENING_V57.md) | Async formal orchestration hardening and liveness control work set. |
| Work Set Plan (v58) | [`worksets/WORKSET_2026-02-28_KANI_HARNESS_EXPANSION_V58.md`](worksets/WORKSET_2026-02-28_KANI_HARNESS_EXPANSION_V58.md) | Kani harness expansion across syntax/parser/optimizer work set. |
| Work Set Plan (v59) | [`worksets/WORKSET_2026-02-28_LINE_CONTINUATION_V59.md`](worksets/WORKSET_2026-02-28_LINE_CONTINUATION_V59.md) | Line continuation language semantics work set. |
| Work Set Plan (v60) | [`worksets/WORKSET_2026-02-28_WITH_BLOCK_V60.md`](worksets/WORKSET_2026-02-28_WITH_BLOCK_V60.md) | `With ... End With` language subset work set. |
| Work Set Plan (v61) | [`worksets/WORKSET_2026-02-28_CONDITIONAL_COMPILATION_V61.md`](worksets/WORKSET_2026-02-28_CONDITIONAL_COMPILATION_V61.md) | Conditional compilation directive subset work set. |
| Work Set Plan (v62) | [`worksets/WORKSET_2026-02-28_STDLIB_SURFACE_ARCH_V62.md`](worksets/WORKSET_2026-02-28_STDLIB_SURFACE_ARCH_V62.md) | Intrinsic surface architecture split work set. |
| Work Set Plan (v63) | [`worksets/WORKSET_2026-02-28_JIT_SURFACE_EXPANSION_V63.md`](worksets/WORKSET_2026-02-28_JIT_SURFACE_EXPANSION_V63.md) | Cranelift JIT supported-op surface expansion work set. |
| Work Set Plan (v64) | [`worksets/WORKSET_2026-02-28_PERF_HOTPATH_BASELINES_V64.md`](worksets/WORKSET_2026-02-28_PERF_HOTPATH_BASELINES_V64.md) | Mixed VM/JIT hotpath baseline benchmarking work set. |
| Work Set Plan (v65) | [`worksets/WORKSET_2026-02-28_INTEGRATED_GATE_V65.md`](worksets/WORKSET_2026-02-28_INTEGRATED_GATE_V65.md) | Integrated correctness/performance gate orchestration work set. |
| Work Set Plan (v66) | [`worksets/WORKSET_2026-02-28_STABILIZATION_ROLLUP_V66.md`](worksets/WORKSET_2026-02-28_STABILIZATION_ROLLUP_V66.md) | Ladder stabilization rollup and closure work set. |
| Work Set Plan (v67) | [`worksets/WORKSET_2026-02-28_TYPING_TYPE_LATTICE_V67.md`](worksets/WORKSET_2026-02-28_TYPING_TYPE_LATTICE_V67.md) | Type lattice and initial typed semantic checks work set. |
| Work Set Plan (v68) | [`worksets/WORKSET_2026-02-28_OPTION_EXPLICIT_DIAGNOSTICS_V68.md`](worksets/WORKSET_2026-02-28_OPTION_EXPLICIT_DIAGNOSTICS_V68.md) | Option Explicit and declaration diagnostics expansion work set. |
| Work Set Plan (v69) | [`worksets/WORKSET_2026-02-28_DEFAULT_TYPE_RULES_V69.md`](worksets/WORKSET_2026-02-28_DEFAULT_TYPE_RULES_V69.md) | Def* default typing and type-character precedence work set. |
| Work Set Plan (v70) | [`worksets/WORKSET_2026-02-28_PROCEDURE_SIGNATURES_V70.md`](worksets/WORKSET_2026-02-28_PROCEDURE_SIGNATURES_V70.md) | Typed procedure signatures, return typing, and typed ByRef legality work set. |
| Work Set Plan (v71) | [`worksets/WORKSET_2026-02-28_EARLY_LATE_CLASSIFICATION_V71.md`](worksets/WORKSET_2026-02-28_EARLY_LATE_CLASSIFICATION_V71.md) | Deterministic early/mixed/late call-mode classification work set. |
| Work Set Plan (v72) | [`worksets/WORKSET_2026-02-28_TYPING_DIAGNOSTIC_ROLLUP_V72.md`](worksets/WORKSET_2026-02-28_TYPING_DIAGNOSTIC_ROLLUP_V72.md) | Typing diagnostic taxonomy and Track-A deferred-gate reconciliation work set. |
| Work Set Plan (v73) | [`worksets/WORKSET_2026-02-28_COERCION_MATRIX_V73.md`](worksets/WORKSET_2026-02-28_COERCION_MATRIX_V73.md) | Table-backed coercion matrix alignment for assignment/argument typing work set. |
| Work Set Plan (v74) | [`worksets/WORKSET_2026-02-28_OPERATOR_RESULT_RULES_V74.md`](worksets/WORKSET_2026-02-28_OPERATOR_RESULT_RULES_V74.md) | Operator result typing and comparison compatibility table-alignment work set. |
| Work Set Plan (v75) | [`worksets/WORKSET_2026-02-28_CALL_COERCION_EARLY_LATE_V75.md`](worksets/WORKSET_2026-02-28_CALL_COERCION_EARLY_LATE_V75.md) | Mode-aware call coercion alignment across early/mixed/late call paths. |
| Work Set Plan (v76) | [`worksets/WORKSET_2026-02-28_CONVERSION_INTRINSICS_V76.md`](worksets/WORKSET_2026-02-28_CONVERSION_INTRINSICS_V76.md) | Conversion intrinsic typing parity with shared coercion integration and DG reconciliation poll. |
| Work Set Plan (v77) | [`worksets/WORKSET_2026-02-28_STRING_STORAGE_SEMANTICS_V77.md`](worksets/WORKSET_2026-02-28_STRING_STORAGE_SEMANTICS_V77.md) | String sentinel storage semantics (`vbNullString`) in current executable subset. |
| Work Set Plan (v78) | [`worksets/WORKSET_2026-02-28_STRING_COMPARE_SEARCH_V78.md`](worksets/WORKSET_2026-02-28_STRING_COMPARE_SEARCH_V78.md) | Option Compare + compare/search intrinsic subset (`InStrRev`, `Like`) in current executable model. |
| Work Set Plan (v79) | [`worksets/WORKSET_2026-02-28_STRING_MUTATION_SLICES_V79.md`](worksets/WORKSET_2026-02-28_STRING_MUTATION_SLICES_V79.md) | String mutation/slice subset: `Mid` statement mutation and slice intrinsic completion coverage. |
| Work Set Plan (v80) | [`worksets/WORKSET_2026-02-28_ARRAY_TYPE_MODEL_V80.md`](worksets/WORKSET_2026-02-28_ARRAY_TYPE_MODEL_V80.md) | Unified array descriptor model for typed/variant arrays and rank/bounds metadata. |
| Work Set Plan (v81) | [`worksets/WORKSET_2026-02-28_ARRAY_BOUNDS_INDEXING_V81.md`](worksets/WORKSET_2026-02-28_ARRAY_BOUNDS_INDEXING_V81.md) | Lower-bound aware and multidimensional indexing semantics for array declarations/references. |
| Work Set Plan (v82) | [`worksets/WORKSET_2026-02-28_ARRAY_REDIM_FULL_V82.md`](worksets/WORKSET_2026-02-28_ARRAY_REDIM_FULL_V82.md) | Full in-scope `ReDim`/`ReDim Preserve` legality and tail-clearing semantics. |
| Work Set Plan (v83) | [`worksets/WORKSET_2026-02-28_ARRAY_CALL_PARAMARRAY_V83.md`](worksets/WORKSET_2026-02-28_ARRAY_CALL_PARAMARRAY_V83.md) | Array call semantics and initial `ParamArray` packing support in current subset. |
| Work Set Plan (v84) | [`worksets/WORKSET_2026-02-28_ARRAY_BOUNDARY_DISPATCH_V84.md`](worksets/WORKSET_2026-02-28_ARRAY_BOUNDARY_DISPATCH_V84.md) | Array dispatch-boundary marshalling subset and deferred-gate reconciliation checkpoint for array track DG runs. |
| Work Set Plan (v85) | [`worksets/WORKSET_2026-02-28_TYPED_EXEC_FASTPATHS_V85.md`](worksets/WORKSET_2026-02-28_TYPED_EXEC_FASTPATHS_V85.md) | Typed VM execution fast-paths with fallback parity checks and typed hot-loop benchmark capture. |
| Work Set Plan (v86) | [`worksets/WORKSET_2026-02-28_FULL_TYPING_CONFORMANCE_GATE_V86.md`](worksets/WORKSET_2026-02-28_FULL_TYPING_CONFORMANCE_GATE_V86.md) | Final typing ladder gate: integrated evidence rollup, deferred-gate audit, and Phase 12 status consolidation. |
| Work Set Plan (v27-v36) | [`worksets/WORKSET_2026-02-27_BATCH_V27_V36.md`](worksets/WORKSET_2026-02-27_BATCH_V27_V36.md) | Next long batch: formal reliability, language coverage closure, and hotspot performance work. |
| Profile Ladder | [`worksets/PROFILE_LADDER_2026-02-27_MACH1000.md`](worksets/PROFILE_LADDER_2026-02-27_MACH1000.md) | MACH1000 profile roadmap and execution history. |
| Profile Ladder (v37-v56) | [`worksets/PROFILE_LADDER_2026-02-27_MACH1000_V37_V56.md`](worksets/PROFILE_LADDER_2026-02-27_MACH1000_V37_V56.md) | Next horizon ladder split into language core, intrinsic runtime, and host/interop tracks. |
| Profile Ladder (v57-v66) | [`worksets/PROFILE_LADDER_2026-02-28_MACH1000_V57_V66.md`](worksets/PROFILE_LADDER_2026-02-28_MACH1000_V57_V66.md) | Language closure + formal depth + JIT throughput ladder for the next 10 profiles. |
| Profile Ladder (v67-v86) | [`worksets/PROFILE_LADDER_2026-02-28_MACH1000_V67_V86_TYPING.md`](worksets/PROFILE_LADDER_2026-02-28_MACH1000_V67_V86_TYPING.md) | Full VBA typing semantics ladder: diagnostics, coercion, strings, arrays, and early/late interaction with deferred formal gates. |
| Profile Ladder (v87-v106) | [`worksets/PROFILE_LADDER_2026-02-28_MACH1000_V87_V106_LANGUAGE_COMPLETION.md`](worksets/PROFILE_LADDER_2026-02-28_MACH1000_V87_V106_LANGUAGE_COMPLETION.md) | Outstanding language-feature closure ladder: loops, unstructured flow, resume/error semantics, UDT/property/late binding, and external declare binding. |
| Profile Ladder (v107-v146) | [`worksets/PROFILE_LADDER_2026-02-28_MACH1000_V107_V146_FULL_VBA_LANGUAGE_BUILTINS.md`](worksets/PROFILE_LADDER_2026-02-28_MACH1000_V107_V146_FULL_VBA_LANGUAGE_BUILTINS.md) | Full VBA closure ladder: semantic completion, full built-in expansion, interop hardening, oracle conformance, formal foldback, and terminal integrated gate. |
| Profile Ladder (v147-v166) | [`worksets/PROFILE_LADDER_2026-03-01_MACH1000_V147_V166_NON_HAL_COMPLETION.md`](worksets/PROFILE_LADDER_2026-03-01_MACH1000_V147_V166_NON_HAL_COMPLETION.md) | Non-HAL language/runtime/library completion ladder with deferred-oracle gate policy. |
| Profile Ladder (v167-v186) | [`worksets/PROFILE_LADDER_2026-03-01_MACH1000_V167_V186_NON_HAL_HARDENING.md`](worksets/PROFILE_LADDER_2026-03-01_MACH1000_V167_V186_NON_HAL_HARDENING.md) | Follow-on non-HAL hardening/perf/formal ladder after completion gate. |
| Work Set Plan (v147) | [`worksets/WORKSET_2026-03-01_NON_HAL_GAP_BASELINE_LOCK_V147.md`](worksets/WORKSET_2026-03-01_NON_HAL_GAP_BASELINE_LOCK_V147.md) | Baseline lock workset for non-HAL gap classification and scope freeze. |
| Work Set Plan (v148) | [`worksets/WORKSET_2026-03-01_ERR_SURFACE_EXPANSION_V148.md`](worksets/WORKSET_2026-03-01_ERR_SURFACE_EXPANSION_V148.md) | `Err` member-surface expansion subset workset for deterministic non-HAL execution. |
| Work Set Plan (v149) | [`worksets/WORKSET_2026-03-01_ERR_LIFECYCLE_TRANSITIONS_V149.md`](worksets/WORKSET_2026-03-01_ERR_LIFECYCLE_TRANSITIONS_V149.md) | Deterministic `Err` lifecycle transitions for `Resume*` and procedure-boundary clearing in non-HAL execution. |
| Work Set Plan (v150) | [`worksets/WORKSET_2026-03-01_STRING_RUNTIME_COMPLETION_I_V150.md`](worksets/WORKSET_2026-03-01_STRING_RUNTIME_COMPLETION_I_V150.md) | String runtime completion step replacing `Join` projection behavior with concrete array-tag-aware semantics. |
| Work Set Plan (v151) | [`worksets/WORKSET_2026-03-01_STRING_SENTINEL_TIGHTENING_V151.md`](worksets/WORKSET_2026-03-01_STRING_SENTINEL_TIGHTENING_V151.md) | String sentinel tightening pass for deterministic `vbNullString` usage rules in compile-time assignment/call flows. |
| Work Set Plan (v152) | [`worksets/WORKSET_2026-03-01_UDT_VALUE_SEMANTICS_V152.md`](worksets/WORKSET_2026-03-01_UDT_VALUE_SEMANTICS_V152.md) | UDT value-semantics hardening pass for whole-value assignment lowering into deterministic field-copy behavior. |
| Work Set Plan (v153) | [`worksets/WORKSET_2026-03-01_COERCION_EDGE_NORMALIZATION_V153.md`](worksets/WORKSET_2026-03-01_COERCION_EDGE_NORMALIZATION_V153.md) | Coercion-edge normalization pass for deterministic `Null`/`Empty`/`CVErr` tag behavior and predicate consistency. |
| Work Set Plan (v154) | [`worksets/WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_I_V154.md`](worksets/WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_I_V154.md) | Financial functions pass replacing `NPV`/`IRR`/`MIRR` projection behavior with deterministic algorithmic runtime execution. |
| Work Set Plan (v155) | [`worksets/WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_II_V155.md`](worksets/WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_II_V155.md) | Financial functions pass replacing `Rate`/`NPer` projection behavior with deterministic algorithmic runtime execution. |
| Status Tours | [`status-tours/`](status-tours/) | Date-stamped orientation/showcase docs for implemented project state. |
| Formal | [`FORMAL.md`](FORMAL.md) | Lean/Kani formal scaffold status and structure. |
| Spec Checklist | [`evidence/SPEC_CHECKLIST.md`](evidence/SPEC_CHECKLIST.md) | Structured language + built-in/library checklist aligned to current evidence and planned gaps. |
| Conformance Check Topics | [`evidence/conformance/CONFORMANCE_CHECK_TOPICS.md`](evidence/conformance/CONFORMANCE_CHECK_TOPICS.md) | Oracle-driven backlog for semantically uncertain VBA behaviors to differential-check after implementation. |
| Deferred Oracle Gates | [`evidence/conformance/DEFERRED_ORACLE_GATES.md`](evidence/conformance/DEFERRED_ORACLE_GATES.md) | Deferred gate register for oracle-dependent semantics (parallel to deferred formal gates). |
| Non-HAL Completion Backlog | [`evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md`](evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md) | Remaining non-HAL implementation targets and explicit exclusions. |
| Deferred Formal Gates | [`evidence/formal/DEFERRED_GATES.md`](evidence/formal/DEFERRED_GATES.md) | Async Kani deferred-gate register and reconciliation status. |
| Remote Kani Runner | [`evidence/formal/REMOTE_KANI_RUNNER.md`](evidence/formal/REMOTE_KANI_RUNNER.md) | Remote Linux Kani orchestration model, constraints, commands, and artifact retrieval flow. |

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
