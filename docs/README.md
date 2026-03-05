# OxVba Documentation

## Authoritative Documents

| Document | Location | Description |
|---|---|---|
| **MACH-1000 Plan** | [`MACH1000_PLAN.md`](../MACH1000_PLAN.md) | The definitive OxVba project plan. Charter, architecture, formal approach, testing strategy, implementation sequencing. |
| **AutoRun State** | [`AUTORUN_STATE.md`](AUTORUN_STATE.md) | Active execution guardrail for continuous runs. Current target is the COM early-binding/type-library planning gate `v416`; blocker handling follows `CURRENT_BLOCKERS.md`. |
| **Local Execution Doctrine** | [`LOCAL_EXECUTION_DOCTRINE.md`](LOCAL_EXECUTION_DOCTRINE.md) | Local process hardening rules learned from ladder execution, including scaffold integrity gates and required local checks. |
| Implementation Log | [`IMPLEMENTATION_LOG.md`](IMPLEMENTATION_LOG.md) | Rolling execution log for implementation progress. |
| Building | [`BUILDING.md`](BUILDING.md) | Build and local verification instructions. |
| Contributing | [`CONTRIBUTING.md`](CONTRIBUTING.md) | Contribution workflow and compatibility evidence expectations. |
| Testing | [`TESTING.md`](TESTING.md) | Test lanes, current coverage, and next testing milestones. |
| Architecture | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Current architecture snapshot and near-term evolution notes. |
| IR Design | [`IR_DESIGN.md`](IR_DESIGN.md) | Multi-level IR status and next implementation targets. |
| Bytecode Format | [`BYTECODE_FORMAT.md`](BYTECODE_FORMAT.md) | Bytecode representation status and planned evolution. |
| VM Architecture | [`VM_ARCHITECTURE.md`](VM_ARCHITECTURE.md) | VM implementation status and next milestones. |
| Conformance | [`CONFORMANCE.md`](CONFORMANCE.md) | Conformance assets, commands, and comparison policy. |
| Integration Suite Strategy | [`evidence/conformance/PROJECT_INTEGRATION_SUITE_STRATEGY_V1.md`](evidence/conformance/PROJECT_INTEGRATION_SUITE_STRATEGY_V1.md) | Data-driven multi-project integration test strategy, deterministic policy contract, and growth plan. |
| Integration Deferred Notes | [`evidence/conformance/PROJECT_INTEGRATION_DEFERRED_UNCERTAINTIES_V1.md`](evidence/conformance/PROJECT_INTEGRATION_DEFERRED_UNCERTAINTIES_V1.md) | Deferred/unclear integration topics linked to `ODG`/`CCT` tracking and active-limit coverage. |
| Foundation Spec Reference | [`FOUNDATION_SPEC_REFERENCE.md`](FOUNDATION_SPEC_REFERENCE.md) | Canonical external specification source map in `../Foundation/reference` (no local vendored VBA spec snapshots). |
| Spec Drafts Index | [`spec/README.md`](spec/README.md) | Early-stage design/contract drafts that prepare future normative specs. |
| HAL Design Draft | [`spec/HAL_DESIGN_DRAFT.md`](spec/HAL_DESIGN_DRAFT.md) | Host Abstraction Layer scope, principles, profile targets, and staged spec-run plan. |
| HAL Interface Draft | [`spec/HAL_INTERFACE_DRAFT.md`](spec/HAL_INTERFACE_DRAFT.md) | Proposed HAL contracts, capability descriptors, maturity levels, and policy gates. |
| HAL Conformance Draft | [`spec/HAL_CONFORMANCE_DRAFT.md`](spec/HAL_CONFORMANCE_DRAFT.md) | Proposed HAL conformance levels, test obligations, and evidence model. |
| HAL Profile Matrix Draft | [`spec/HAL_PROFILE_MATRIX_DRAFT.md`](spec/HAL_PROFILE_MATRIX_DRAFT.md) | Initial five-profile (Windows/Linux/macOS/WASM/Null) capability+maturity planning matrix. |
| HAL Spec Working Draft | [`spec/HAL_SPEC_WORKING_DRAFT.md`](spec/HAL_SPEC_WORKING_DRAFT.md) | Implementation-linked HAL contract, deterministic error model, unsupported-mode semantics, and current Windows-only COM scope decision. |
| HAL Spec Crosswalk | [`spec/HAL_SPEC_CROSSWALK.md`](spec/HAL_SPEC_CROSSWALK.md) | Capability/intrinsic mapping to Foundation conformance anchors and extraction-quality gaps. |
| HAL Conformance Suite | [`spec/HAL_CONFORMANCE_SUITE.md`](spec/HAL_CONFORMANCE_SUITE.md) | Runnable HAL verification lanes, artifact outputs, and profile expectations. |
| HAL Formalization Program | [`spec/HAL_FORMALIZATION_PROGRAM.md`](spec/HAL_FORMALIZATION_PROGRAM.md) | Charter-driven HAL formalization ladder (5-step program + H1/H2/H3 execution tracks). |
| HAL Contract Clause Catalog v1 | [`spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md`](spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md) | Clause-level HAL contract baseline with pre/postconditions, failure semantics, and verification links. |
| HAL Contract Clause Catalog v1 (CSV) | [`spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.csv`](spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.csv) | Machine-readable clause schema used for conformance coverage and markdown drift-guard checks. |
| HAL Policy Presets | [`spec/HAL_POLICY_PRESETS.md`](spec/HAL_POLICY_PRESETS.md) | Explicit policy preset table for reproducible host behavior across CI/runtime/dev lanes. |
| HAL Contract Assertion Hardening | [`spec/HAL_CONTRACT_ASSERTION_HARDENING.md`](spec/HAL_CONTRACT_ASSERTION_HARDENING.md) | Build-gated debug/checked pre-post assertion scaffold and staged hardening plan for in-code contract enforcement. |
| HAL Operating Envelope v1 | [`spec/HAL_OPERATING_ENVELOPE_V1.md`](spec/HAL_OPERATING_ENVELOPE_V1.md) | Explicit HAL boundary guarantees/non-guarantees for safe runtime/compiler optimization assumptions. |
| HAL Runtime Bootstrap Impl v2 | [`spec/HAL_RUNTIME_PROFILE_BOOTSTRAP_IMPLEMENTATION_V2.md`](spec/HAL_RUNTIME_PROFILE_BOOTSTRAP_IMPLEMENTATION_V2.md) | Implemented runtime profile bootstrap resolver (`CLI > ENV > config > defaults`) and CLI surface snapshot. |
| HAL UI Platform Impl v2 | [`spec/HAL_UI_PLATFORM_IMPLEMENTATION_V2.md`](spec/HAL_UI_PLATFORM_IMPLEMENTATION_V2.md) | Implemented Windows GUI and Linux stdio interaction lanes with runtime-class-aware `DoEvents` behavior. |
| HAL Declare Execution Impl v2 | [`spec/HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md`](spec/HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md) | Implemented `Declare` metadata/lowering/VM/HAL dynamic-link execution subset and error model. |
| HAL Evidence Artifacts | [`evidence/hal/README.md`](evidence/hal/README.md) | Generated HAL conformance result bundles (`md` + `jsonl`) and lane semantics. |
| HAL Block B-D Summary | [`evidence/hal/HAL_BLOCK_BCD_IMPLEMENTATION_2026-03-02.md`](evidence/hal/HAL_BLOCK_BCD_IMPLEMENTATION_2026-03-02.md) | Consolidated implementation + verification summary for host-platform expansion blocks `v197..v226`. |
| HAL Phase-1 Baseline Audit | [`evidence/hal/HAL_PHASE1_BASELINE_AUDIT_2026-03-02.md`](evidence/hal/HAL_PHASE1_BASELINE_AUDIT_2026-03-02.md) | Capability/domain audit baseline for HAL formalization phase 1. |
| HAL Phase-2 Contract Checks | [`evidence/hal/HAL_PHASE2_CONTRACT_CHECKS_2026-03-02.md`](evidence/hal/HAL_PHASE2_CONTRACT_CHECKS_2026-03-02.md) | Clause-mapped executable contract check expansion and verification outcomes for HAL phase 2. |
| HAL Phase-3 Adapter Refinement | [`evidence/hal/HAL_PHASE3_ADAPTER_REFINEMENT_2026-03-02.md`](evidence/hal/HAL_PHASE3_ADAPTER_REFINEMENT_2026-03-02.md) | Adapter invariant/property hardening and operating-envelope closure evidence for HAL phase 3. |
| HAL Uncertainty Register | [`evidence/hal/HAL_UNCERTAINTY_REGISTER.md`](evidence/hal/HAL_UNCERTAINTY_REGISTER.md) | Open HAL contract-shape uncertainties and planned resolution phases. |
| HAL Implementation-Defined Register | [`evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`](evidence/hal/HAL_IMPLEMENTATION_DEFINED.md) | HAL-specific implementation-defined behaviors that are explicitly tracked. |
| MS-VBAL Module/Project Requirements | [`evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.md`](evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.md) | Full-scope module/project backlog for MS-VBAL closure beyond current single-source execution model. |
| PMR Spec v1 | [`spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`](spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md) | Formal Project/Module/Reference state model, invariants, operation contracts, and deterministic error model. |
| PMR Clause Catalog v1 | [`spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`](spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md) | Clause IDs and verification mapping for PMR semantics. |
| PMR Conformance v1 | [`spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md`](spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md) | Planned executable conformance lanes for PMR static semantics, references, class/event semantics, and storage. |
| PMR HAL Integration v1 | [`spec/PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md`](spec/PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md) | HAL-adjacent capability and contract planning for host projects, references, and storage. |
| PMR Typelib/Importlib HAL Draft v1 | [`spec/PROJECT_MODULE_REFERENCE_TYPELIB_IMPORTLIB_HAL_DRAFT_V1.md`](spec/PROJECT_MODULE_REFERENCE_TYPELIB_IMPORTLIB_HAL_DRAFT_V1.md) | Deterministic first-pass contract for type-library importlib resolution and planned HAL resolver boundary. |
| PMR Source Crosswalk v1 | [`spec/PROJECT_MODULE_REFERENCE_SOURCE_CROSSWALK_V1.md`](spec/PROJECT_MODULE_REFERENCE_SOURCE_CROSSWALK_V1.md) | Anchor-level source traceability across MS-VBAL, MS-OAUT, and current MS-OVBA extraction status. |
| Class/COM Alignment Plan v1 | [`spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md`](spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md) | Staged plan to align class-module semantics with COM behavior now while explicitly deferring full ABI/interop mechanics. |
| COM Client/Server Scope v1 | [`spec/COM_CLIENT_SERVER_SCOPE_V1.md`](spec/COM_CLIENT_SERVER_SCOPE_V1.md) | Windows COM client+server scope baseline with formal contract boundaries, tier model, and apartment/lifecycle policy decisions. |
| COM Client/Server Conformance v1 | [`spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md`](spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md) | Executable lane plan and artifact schema for COM client/server verification and deferred-oracle foldback. |
| COM Early Binding + Typelib Scope v1 | [`spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`](spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md) | Full design baseline for COM early binding and type-library consumption across PMR/HAL/binder/IR/runtime, with deterministic error and cache models. |
| COM Early Binding + Typelib Conformance v1 | [`spec/COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md`](spec/COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md) | Early-binding conformance lane plan (`E0..E6`), formal/deferred gate structure, and artifact schema. |
| HAL COM Bridge Scope v1 | [`spec/HAL_COM_BRIDGE_SCOPE_V1.md`](spec/HAL_COM_BRIDGE_SCOPE_V1.md) | HAL boundary scope for COM transport, including C1 floor and C2 late-bound contract transition requirements. |
| COM Late-Bound Bridge v1 | [`spec/COM_CLIENT_LATEBOUND_BRIDGE_V1.md`](spec/COM_CLIENT_LATEBOUND_BRIDGE_V1.md) | Explicit bridge contract from VBA late-bound semantics to compiler/VM/HAL COM transport layers for C2 implementation lanes. |
| PMR Class/COM A1-A5 Evidence | [`evidence/language/PMR_CLASS_COM_ALIGNMENT_A1_A5_2026-03-03.md`](evidence/language/PMR_CLASS_COM_ALIGNMENT_A1_A5_2026-03-03.md) | Execution evidence for A1-A5: project-graph scaffold, PMR diagnostics, class semantic boundary alignment, and claim-tier gating. |
| PMR ProjectGraph P0-P10 Rollup | [`evidence/language/PMR_PROJECTGRAPH_P0_P10_ROLLUP_2026-03-03.md`](evidence/language/PMR_PROJECTGRAPH_P0_P10_ROLLUP_2026-03-03.md) | End-to-end execution rollup for parser+binder ProjectGraph integration master workset through oracle/deferred-gate setup. |
| PMR Fixture Matrix v1 | [`evidence/conformance/PMR_PROJECT_MODEL_FIXTURE_MATRIX_V1.md`](evidence/conformance/PMR_PROJECT_MODEL_FIXTURE_MATRIX_V1.md) | Deterministic executable fixture mapping for PMR project-model scenarios required by P9. |
| PMR Oracle Templates v1 | [`evidence/conformance/PMR_PROJECT_MODEL_ORACLE_TEMPLATES_V1.md`](evidence/conformance/PMR_PROJECT_MODEL_ORACLE_TEMPLATES_V1.md) | Structured Excel probe templates for deferred-oracle topics `CCT-037..CCT-041` (P10). |
| PMR Oracle Runner | [`../scripts/run-pmr-project-model-oracle.ps1`](../scripts/run-pmr-project-model-oracle.ps1) | Automated Excel oracle capture runner for PMR topics (`CCT-037..CCT-041`) with CSV+summary artifacts. |
| PMR Follow-up Queue | [`worksets/WORKSET_2026-03-03_PMR_FOLLOWUP_QUEUE_FROM_OBSERVATIONS.md`](worksets/WORKSET_2026-03-03_PMR_FOLLOWUP_QUEUE_FROM_OBSERVATIONS.md) | Queue generated from P10 observations and parity foldback, including divergence-linked implementation backlog. |
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
| Profile v156 Status | [`PROFILE_STATUS_V156.md`](profile-status/PROFILE_STATUS_V156.md) | Current gate status contract for `mvp-profile-v156` (deterministic financial tolerance/non-convergence error-tag model). |
| Profile v157 Status | [`PROFILE_STATUS_V157.md`](profile-status/PROFILE_STATUS_V157.md) | Current gate status contract for `mvp-profile-v157` (compile-time vs runtime diagnostic timing classification pass). |
| Profile v158 Status | [`PROFILE_STATUS_V158.md`](profile-status/PROFILE_STATUS_V158.md) | Current gate status contract for `mvp-profile-v158` (VM parity expansion for financial/tag introspection runtime behavior). |
| Profile v159 Status | [`PROFILE_STATUS_V159.md`](profile-status/PROFILE_STATUS_V159.md) | Current gate status contract for `mvp-profile-v159` (JIT fallback parity expansion for financial/tag introspection behavior). |
| Profile v160 Status | [`PROFILE_STATUS_V160.md`](profile-status/PROFILE_STATUS_V160.md) | Current gate status contract for `mvp-profile-v160` (conformance corpus expansion across Err/string/UDT/coercion edges). |
| Profile v161 Status | [`PROFILE_STATUS_V161.md`](profile-status/PROFILE_STATUS_V161.md) | Current gate status contract for `mvp-profile-v161` (financial algorithm/tolerance corpus expansion). |
| Profile v162 Status | [`PROFILE_STATUS_V162.md`](profile-status/PROFILE_STATUS_V162.md) | Current gate status contract for `mvp-profile-v162` (formal/Kani obligation expansion for financial/tag runtime paths). |
| Profile v163 Status | [`PROFILE_STATUS_V163.md`](profile-status/PROFILE_STATUS_V163.md) | Current gate status contract for `mvp-profile-v163` (non-HAL evidence index reconciliation). |
| Profile v164 Status | [`PROFILE_STATUS_V164.md`](profile-status/PROFILE_STATUS_V164.md) | Current gate status contract for `mvp-profile-v164` (deferred-oracle gate synchronization with foldback notes). |
| Profile v165 Status | [`PROFILE_STATUS_V165.md`](profile-status/PROFILE_STATUS_V165.md) | Current gate status contract for `mvp-profile-v165` (integrated non-HAL completion gate evidence run). |
| Profile v166 Status | [`PROFILE_STATUS_V166.md`](profile-status/PROFILE_STATUS_V166.md) | Current gate status contract for `mvp-profile-v166` (terminal closure of the `v147..v166` non-HAL completion ladder). |
| Profile v167 Status | [`PROFILE_STATUS_V167.md`](profile-status/PROFILE_STATUS_V167.md) | Current gate status contract for `mvp-profile-v167` (post-completion non-HAL residual audit and classification). |
| Profile v168 Status | [`PROFILE_STATUS_V168.md`](profile-status/PROFILE_STATUS_V168.md) | Current gate status contract for `mvp-profile-v168` (runtime benchmark instrumentation expansion for focused non-HAL subsets). |
| Profile v169 Status | [`PROFILE_STATUS_V169.md`](profile-status/PROFILE_STATUS_V169.md) | Current gate status contract for `mvp-profile-v169` (financial hot-path derivative optimization pass). |
| Profile v170 Status | [`PROFILE_STATUS_V170.md`](profile-status/PROFILE_STATUS_V170.md) | Current gate status contract for `mvp-profile-v170` (string-digit path slice-based optimization pass). |
| Profile v171 Status | [`PROFILE_STATUS_V171.md`](profile-status/PROFILE_STATUS_V171.md) | Current gate status contract for `mvp-profile-v171` (coercion matrix hardening for `CVErr` range/predicate edges). |
| Profile v172 Status | [`PROFILE_STATUS_V172.md`](profile-status/PROFILE_STATUS_V172.md) | Current gate status contract for `mvp-profile-v172` (nested error-mode transition hardening). |
| Profile v173 Status | [`PROFILE_STATUS_V173.md`](profile-status/PROFILE_STATUS_V173.md) | Current gate status contract for `mvp-profile-v173` (JIT fallback robustness expansion for hardened coercion/error regressions). |
| Profile v174 Status | [`PROFILE_STATUS_V174.md`](profile-status/PROFILE_STATUS_V174.md) | Current gate status contract for `mvp-profile-v174` (deferred oracle probe scaffolding preparation). |
| Profile v175 Status | [`PROFILE_STATUS_V175.md`](profile-status/PROFILE_STATUS_V175.md) | Current gate status contract for `mvp-profile-v175` (formal lane expansion I). |
| Profile v176 Status | [`PROFILE_STATUS_V176.md`](profile-status/PROFILE_STATUS_V176.md) | Current gate status contract for `mvp-profile-v176` (formal lane expansion II and deferred strict-lane tracking). |
| Profile v177 Status | [`PROFILE_STATUS_V177.md`](profile-status/PROFILE_STATUS_V177.md) | Current gate status contract for `mvp-profile-v177` (conformance/formal documentation normalization). |
| Profile v178 Status | [`PROFILE_STATUS_V178.md`](profile-status/PROFILE_STATUS_V178.md) | Current gate status contract for `mvp-profile-v178` (coverage matrix normalization audit). |
| Profile v179 Status | [`PROFILE_STATUS_V179.md`](profile-status/PROFILE_STATUS_V179.md) | Current gate status contract for `mvp-profile-v179` (regression corpus growth for hardened non-HAL semantics). |
| Profile v180 Status | [`PROFILE_STATUS_V180.md`](profile-status/PROFILE_STATUS_V180.md) | Current gate status contract for `mvp-profile-v180` (integrated performance gate and trend publication). |
| Profile v181 Status | [`PROFILE_STATUS_V181.md`](profile-status/PROFILE_STATUS_V181.md) | Current gate status contract for `mvp-profile-v181` (integrated correctness gate sweep). |
| Profile v182 Status | [`PROFILE_STATUS_V182.md`](profile-status/PROFILE_STATUS_V182.md) | Current gate status contract for `mvp-profile-v182` (deferred-oracle hygiene audit). |
| Profile v183 Status | [`PROFILE_STATUS_V183.md`](profile-status/PROFILE_STATUS_V183.md) | Current gate status contract for `mvp-profile-v183` (divergence hygiene audit). |
| Profile v184 Status | [`PROFILE_STATUS_V184.md`](profile-status/PROFILE_STATUS_V184.md) | Current gate status contract for `mvp-profile-v184` (terminal stabilization pass). |
| Profile v185 Status | [`PROFILE_STATUS_V185.md`](profile-status/PROFILE_STATUS_V185.md) | Current gate status contract for `mvp-profile-v185` (release-candidate integrated gate). |
| Profile v186 Status | [`PROFILE_STATUS_V186.md`](profile-status/PROFILE_STATUS_V186.md) | Current gate status contract for `mvp-profile-v186` (batch-2 terminal closure). |
| Profile v226 Status | [`PROFILE_STATUS_V226.md`](profile-status/PROFILE_STATUS_V226.md) | Current gate status contract for `mvp-profile-v226` (host-platform expansion terminal closure). |
| Profile v286 Status | [`PROFILE_STATUS_V286.md`](profile-status/PROFILE_STATUS_V286.md) | Current gate status contract for `mvp-profile-v286` (declare/marshaling full-scope terminal closure). |
| Profile v287-v416 Statuses | [`profile-status/`](profile-status/README.md) | COM client/server + early-binding planning status records are published through `PROFILE_STATUS_V416.md`. |
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
| Profile Ladder (v187-v226) | [`worksets/PROFILE_LADDER_2026-03-02_MACH1000_V187_V226_HOST_PLATFORM_EXPANSION.md`](worksets/PROFILE_LADDER_2026-03-02_MACH1000_V187_V226_HOST_PLATFORM_EXPANSION.md) | Host/HAL platform expansion ladder covering runtime profile bootstrap, UI/DoEvents host lanes, Declare dynlink integration, and terminal closure gates. |
| Profile Series (COM Windows) | [`worksets/PROFILE_SERIES_2026-03-04_MACH1000_COM_WINDOWS_CLIENT_SERVER.md`](worksets/PROFILE_SERIES_2026-03-04_MACH1000_COM_WINDOWS_CLIENT_SERVER.md) | Multi-round COM roadmap for Windows client+server support (`v287..v406`). |
| Profile Ladder (v287-v306) | [`worksets/PROFILE_LADDER_2026-03-04_MACH1000_V287_V306_COM_FORMAL_SCAFFOLD.md`](worksets/PROFILE_LADDER_2026-03-04_MACH1000_V287_V306_COM_FORMAL_SCAFFOLD.md) | First COM ladder: formal baseline, clause/conformance scaffolding, and native smoke-path bring-up. |
| Profile Ladder (v307-v336) | [`worksets/PROFILE_LADDER_2026-03-04_MACH1000_V307_V336_COM_CLIENT_DEPTH.md`](worksets/PROFILE_LADDER_2026-03-04_MACH1000_V307_V336_COM_CLIENT_DEPTH.md) | COM client-depth ladder for native activation/invoke behavior, fallback policy contracts, and executable evidence. |
| Profile Ladder (v337-v366) | [`worksets/PROFILE_LADDER_2026-03-04_MACH1000_V337_V366_COM_SERVER_DEPTH.md`](worksets/PROFILE_LADDER_2026-03-04_MACH1000_V337_V366_COM_SERVER_DEPTH.md) | COM server-depth ladder for class-factory/dispatch scaffolding, policy controls, and harness coverage. |
| Profile Ladder (v367-v386) | [`worksets/PROFILE_LADDER_2026-03-04_MACH1000_V367_V386_COM_STABILIZATION.md`](worksets/PROFILE_LADDER_2026-03-04_MACH1000_V367_V386_COM_STABILIZATION.md) | COM stabilization ladder for regression hardening, formal/deferred gate sync, and terminal closure gate `v386`. |
| Profile Ladder (v387-v406) | [`worksets/PROFILE_LADDER_2026-03-04_MACH1000_V387_V406_COM_CLIENT_LATEBOUND_C2.md`](worksets/PROFILE_LADDER_2026-03-04_MACH1000_V387_V406_COM_CLIENT_LATEBOUND_C2.md) | COM late-bound client C2 ladder covering contract closure, implementation runway, and terminal closure gate `v406` (completed). |
| Profile Ladder (v407-v466) | [`worksets/PROFILE_LADDER_2026-03-05_MACH1000_V407_V466_COM_EARLY_BINDING_TYPELIB.md`](worksets/PROFILE_LADDER_2026-03-05_MACH1000_V407_V466_COM_EARLY_BINDING_TYPELIB.md) | COM early-binding and type-library consumption ladder covering planning (`v407..v416`) through integrated implementation/conformance closure (`v466`). |
| Work Set Plan (v147) | [`worksets/WORKSET_2026-03-01_NON_HAL_GAP_BASELINE_LOCK_V147.md`](worksets/WORKSET_2026-03-01_NON_HAL_GAP_BASELINE_LOCK_V147.md) | Baseline lock workset for non-HAL gap classification and scope freeze. |
| Work Set Plan (v148) | [`worksets/WORKSET_2026-03-01_ERR_SURFACE_EXPANSION_V148.md`](worksets/WORKSET_2026-03-01_ERR_SURFACE_EXPANSION_V148.md) | `Err` member-surface expansion subset workset for deterministic non-HAL execution. |
| Work Set Plan (v149) | [`worksets/WORKSET_2026-03-01_ERR_LIFECYCLE_TRANSITIONS_V149.md`](worksets/WORKSET_2026-03-01_ERR_LIFECYCLE_TRANSITIONS_V149.md) | Deterministic `Err` lifecycle transitions for `Resume*` and procedure-boundary clearing in non-HAL execution. |
| Work Set Plan (v150) | [`worksets/WORKSET_2026-03-01_STRING_RUNTIME_COMPLETION_I_V150.md`](worksets/WORKSET_2026-03-01_STRING_RUNTIME_COMPLETION_I_V150.md) | String runtime completion step replacing `Join` projection behavior with concrete array-tag-aware semantics. |
| Work Set Plan (v151) | [`worksets/WORKSET_2026-03-01_STRING_SENTINEL_TIGHTENING_V151.md`](worksets/WORKSET_2026-03-01_STRING_SENTINEL_TIGHTENING_V151.md) | String sentinel tightening pass for deterministic `vbNullString` usage rules in compile-time assignment/call flows. |
| Work Set Plan (v152) | [`worksets/WORKSET_2026-03-01_UDT_VALUE_SEMANTICS_V152.md`](worksets/WORKSET_2026-03-01_UDT_VALUE_SEMANTICS_V152.md) | UDT value-semantics hardening pass for whole-value assignment lowering into deterministic field-copy behavior. |
| Work Set Plan (v153) | [`worksets/WORKSET_2026-03-01_COERCION_EDGE_NORMALIZATION_V153.md`](worksets/WORKSET_2026-03-01_COERCION_EDGE_NORMALIZATION_V153.md) | Coercion-edge normalization pass for deterministic `Null`/`Empty`/`CVErr` tag behavior and predicate consistency. |
| Work Set Plan (v154) | [`worksets/WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_I_V154.md`](worksets/WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_I_V154.md) | Financial functions pass replacing `NPV`/`IRR`/`MIRR` projection behavior with deterministic algorithmic runtime execution. |
| Work Set Plan (v155) | [`worksets/WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_II_V155.md`](worksets/WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_II_V155.md) | Financial functions pass replacing `Rate`/`NPer` projection behavior with deterministic algorithmic runtime execution. |
| Work Set Plan (v156) | [`worksets/WORKSET_2026-03-01_FINANCIAL_TOLERANCE_MODEL_V156.md`](worksets/WORKSET_2026-03-01_FINANCIAL_TOLERANCE_MODEL_V156.md) | Financial tolerance policy pass with bounded-iteration solver behavior and deterministic error-tag signaling for invalid/non-convergent cases. |
| Work Set Plan (v157) | [`worksets/WORKSET_2026-03-01_DIAGNOSTICS_TIMING_PASS_V157.md`](worksets/WORKSET_2026-03-01_DIAGNOSTICS_TIMING_PASS_V157.md) | Diagnostic phase-timing pass with explicit compile-time/runtime classification and precedence tests. |
| Work Set Plan (v158) | [`worksets/WORKSET_2026-03-01_VM_PARITY_EXPANSION_V158.md`](worksets/WORKSET_2026-03-01_VM_PARITY_EXPANSION_V158.md) | VM parity coverage expansion for newly concrete financial and sentinel-tag introspection behavior. |
| Work Set Plan (v159) | [`worksets/WORKSET_2026-03-01_JIT_PARITY_EXPANSION_V159.md`](worksets/WORKSET_2026-03-01_JIT_PARITY_EXPANSION_V159.md) | JIT fallback parity expansion with explicit VM-equivalence checks for unsupported financial/tag introspection surfaces. |
| Work Set Plan (v160) | [`worksets/WORKSET_2026-03-01_CORPUS_EXPANSION_I_V160.md`](worksets/WORKSET_2026-03-01_CORPUS_EXPANSION_I_V160.md) | Conformance corpus expansion for Err lifecycle reset, string sentinel flows, UDT overwrite-copy, and CVErr normalization edges. |
| Work Set Plan (v161) | [`worksets/WORKSET_2026-03-01_CORPUS_EXPANSION_II_V161.md`](worksets/WORKSET_2026-03-01_CORPUS_EXPANSION_II_V161.md) | Financial algorithm/tolerance corpus expansion with dedicated success/failure fixture coverage. |
| Work Set Plan (v162) | [`worksets/WORKSET_2026-03-01_FORMAL_OBLIGATIONS_UPDATE_V162.md`](worksets/WORKSET_2026-03-01_FORMAL_OBLIGATIONS_UPDATE_V162.md) | Formal/Kani obligations expansion for newly added financial tolerance and `VarType` runtime paths. |
| Work Set Plan (v163) | [`worksets/WORKSET_2026-03-01_EVIDENCE_RECONCILIATION_V163.md`](worksets/WORKSET_2026-03-01_EVIDENCE_RECONCILIATION_V163.md) | Evidence index reconciliation of achieved non-HAL implementation status across language/runtime/spec checklists. |
| Work Set Plan (v164) | [`worksets/WORKSET_2026-03-01_DEFERRED_ORACLE_SYNC_V164.md`](worksets/WORKSET_2026-03-01_DEFERRED_ORACLE_SYNC_V164.md) | Deferred-oracle synchronization pass with explicit foldback notes and implementation-defined follow-up tracking registration. |
| Work Set Plan (v165) | [`worksets/WORKSET_2026-03-01_INTEGRATED_NON_HAL_GATE_V165.md`](worksets/WORKSET_2026-03-01_INTEGRATED_NON_HAL_GATE_V165.md) | Integrated gate run for the non-HAL completion ladder before terminal closure. |
| Work Set Plan (v166) | [`worksets/WORKSET_2026-03-01_TERMINAL_CLOSURE_V166.md`](worksets/WORKSET_2026-03-01_TERMINAL_CLOSURE_V166.md) | Terminal closure workset for non-HAL completion ladder `v147..v166` with explicit exit-criteria evidence. |
| Work Set Plan (v167) | [`worksets/WORKSET_2026-03-01_POST_COMPLETION_AUDIT_V167.md`](worksets/WORKSET_2026-03-01_POST_COMPLETION_AUDIT_V167.md) | Post-completion audit workset to verify no residual non-HAL partial/planned items after `v166` closure. |
| Work Set Plan (v168) | [`worksets/WORKSET_2026-03-01_RUNTIME_PERF_INSTRUMENTATION_V168.md`](worksets/WORKSET_2026-03-01_RUNTIME_PERF_INSTRUMENTATION_V168.md) | Runtime instrumentation workset for focused Err/string/financial benchmark subsets. |
| Work Set Plan (v169) | [`worksets/WORKSET_2026-03-01_FINANCIAL_HOTPATH_PERF_V169.md`](worksets/WORKSET_2026-03-01_FINANCIAL_HOTPATH_PERF_V169.md) | Financial intrinsic hot-path optimization workset (`Rate` derivative path). |
| Work Set Plan (v170) | [`worksets/WORKSET_2026-03-01_STRING_PATH_PERF_V170.md`](worksets/WORKSET_2026-03-01_STRING_PATH_PERF_V170.md) | String-digit intrinsic path optimization workset (slice-based helper flow). |
| Work Set Plan (v171) | [`worksets/WORKSET_2026-03-01_COERCION_MATRIX_HARDENING_V171.md`](worksets/WORKSET_2026-03-01_COERCION_MATRIX_HARDENING_V171.md) | Coercion matrix hardening workset for `CVErr` range and predicate regression coverage. |
| Work Set Plan (v172) | [`worksets/WORKSET_2026-03-01_ERROR_MODEL_HARDENING_V172.md`](worksets/WORKSET_2026-03-01_ERROR_MODEL_HARDENING_V172.md) | Error-model hardening workset for nested mode-transition regression coverage. |
| Work Set Plan (v173) | [`worksets/WORKSET_2026-03-01_JIT_LOWERING_ROBUSTNESS_V173.md`](worksets/WORKSET_2026-03-01_JIT_LOWERING_ROBUSTNESS_V173.md) | JIT lowering/fallback robustness workset for hardened non-HAL regression surfaces. |
| Work Set Plan (v174) | [`worksets/WORKSET_2026-03-01_DIFFERENTIAL_SCAFFOLD_PREP_V174.md`](worksets/WORKSET_2026-03-01_DIFFERENTIAL_SCAFFOLD_PREP_V174.md) | Deferred oracle differential scaffold workset (non-blocking queue generation). |
| Work Set Plan (v175) | [`worksets/WORKSET_2026-03-01_FORMAL_LANE_EXPANSION_I_V175.md`](worksets/WORKSET_2026-03-01_FORMAL_LANE_EXPANSION_I_V175.md) | Formal lane expansion I with new strict Kani harness obligations. |
| Work Set Plan (v176) | [`worksets/WORKSET_2026-03-01_FORMAL_LANE_EXPANSION_II_V176.md`](worksets/WORKSET_2026-03-01_FORMAL_LANE_EXPANSION_II_V176.md) | Formal lane expansion II with deferred strict-lane reconciliation tracking. |
| Work Set Plan (v177) | [`worksets/WORKSET_2026-03-01_DOCUMENTATION_NORMALIZATION_V177.md`](worksets/WORKSET_2026-03-01_DOCUMENTATION_NORMALIZATION_V177.md) | Conformance/formal documentation normalization workset. |
| Work Set Plan (v178) | [`worksets/WORKSET_2026-03-01_COVERAGE_MATRIX_NORMALIZATION_V178.md`](worksets/WORKSET_2026-03-01_COVERAGE_MATRIX_NORMALIZATION_V178.md) | Coverage matrix normalization/audit workset. |
| Work Set Plan (v179) | [`worksets/WORKSET_2026-03-01_REGRESSION_CORPUS_GROWTH_V179.md`](worksets/WORKSET_2026-03-01_REGRESSION_CORPUS_GROWTH_V179.md) | Regression corpus growth workset for hardened non-HAL semantics. |
| Work Set Plan (v180) | [`worksets/WORKSET_2026-03-01_INTEGRATED_PERF_GATE_V180.md`](worksets/WORKSET_2026-03-01_INTEGRATED_PERF_GATE_V180.md) | Integrated performance gate workset with v166 trend comparison. |
| Work Set Plan (v181) | [`worksets/WORKSET_2026-03-01_INTEGRATED_CORRECTNESS_GATE_V181.md`](worksets/WORKSET_2026-03-01_INTEGRATED_CORRECTNESS_GATE_V181.md) | Integrated correctness gate workset. |
| Work Set Plan (v182) | [`worksets/WORKSET_2026-03-01_DEFERRED_ORACLE_HYGIENE_V182.md`](worksets/WORKSET_2026-03-01_DEFERRED_ORACLE_HYGIENE_V182.md) | Deferred-oracle hygiene workset. |
| Work Set Plan (v183) | [`worksets/WORKSET_2026-03-01_DIVERGENCE_HYGIENE_V183.md`](worksets/WORKSET_2026-03-01_DIVERGENCE_HYGIENE_V183.md) | Divergence evidence hygiene workset. |
| Work Set Plan (v184) | [`worksets/WORKSET_2026-03-01_TERMINAL_STABILIZATION_V184.md`](worksets/WORKSET_2026-03-01_TERMINAL_STABILIZATION_V184.md) | Terminal stabilization pass workset. |
| Work Set Plan (v185) | [`worksets/WORKSET_2026-03-01_RELEASE_CANDIDATE_GATE_V185.md`](worksets/WORKSET_2026-03-01_RELEASE_CANDIDATE_GATE_V185.md) | Release-candidate integrated gate workset. |
| Work Set Plan (v186) | [`worksets/WORKSET_2026-03-01_BATCH2_CLOSURE_V186.md`](worksets/WORKSET_2026-03-01_BATCH2_CLOSURE_V186.md) | Batch-2 closure and handoff workset. |
| Work Set Plan (v226) | [`worksets/WORKSET_2026-03-02_TERMINAL_INTEGRATED_CLOSURE_GATE_V226.md`](worksets/WORKSET_2026-03-02_TERMINAL_INTEGRATED_CLOSURE_GATE_V226.md) | Terminal integrated closure gate workset for host-platform expansion ladder `v187..v226`. |
| Work Set Plan (v386) | [`worksets/WORKSET_2026-03-04_TERMINAL_INTEGRATED_CLOSURE_GATE_V386.md`](worksets/WORKSET_2026-03-04_TERMINAL_INTEGRATED_CLOSURE_GATE_V386.md) | Terminal integrated closure gate workset for COM client/server series ladder `v287..v386`. |
| Work Set Plan (v387-v392) | [`worksets/WORKSET_2026-03-04_COM_CLIENT_LATEBOUND_SPEC_CLOSURE_V387_V392.md`](worksets/WORKSET_2026-03-04_COM_CLIENT_LATEBOUND_SPEC_CLOSURE_V387_V392.md) | Spec-closure workset for COM late-bound client C2 runway (`v387..v392`). |
| Work Set Plan (v393-v396) | [`worksets/WORKSET_2026-03-04_COM_CLIENT_LATEBOUND_IMPLEMENTATION_V393_V396.md`](worksets/WORKSET_2026-03-04_COM_CLIENT_LATEBOUND_IMPLEMENTATION_V393_V396.md) | First implementation block for COM late-bound client C2 runway: bridge/lifetime hardening, error taxonomy, and conformance/process scaffolding. |
| Work Set Plan (v397-v400) | [`worksets/WORKSET_2026-03-05_COM_CLIENT_LATEBOUND_IMPLEMENTATION_V397_V400.md`](worksets/WORKSET_2026-03-05_COM_CLIENT_LATEBOUND_IMPLEMENTATION_V397_V400.md) | Second implementation block for COM late-bound client C2 runway: ProgID/member-name literal lowering, invoke packing phase-I, and failure-path fixtures. |
| Work Set Plan (v401-v406) | [`worksets/WORKSET_2026-03-05_COM_CLIENT_LATEBOUND_EXECUTION_V401_V406.md`](worksets/WORKSET_2026-03-05_COM_CLIENT_LATEBOUND_EXECUTION_V401_V406.md) | C2 execution/closure workset: lane scaffolding, registrationless+registered evidence runs, VM/JIT parity sweep, and terminal closure gate. |
| Work Set Plan (v407-v416) | [`worksets/WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_PLANNING_V407_V416.md`](worksets/WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_PLANNING_V407_V416.md) | Planning closure workset for COM early binding and type-library support baseline before implementation-heavy phases. |
| COM Early Open Questions (v416) | [`evidence/conformance/com_early/COM_EARLY_OPEN_QUESTIONS_V416.md`](evidence/conformance/com_early/COM_EARLY_OPEN_QUESTIONS_V416.md) | Planning-stage open-question register for early-binding/type-library behavior to close via source anchors or explicit implementation-defined policy. |
| Status Tours | [`status-tours/`](status-tours/) | Date-stamped orientation/showcase docs for implemented project state. |
| Formal | [`FORMAL.md`](FORMAL.md) | Lean/Kani formal scaffold status and structure. |
| Spec Checklist | [`evidence/SPEC_CHECKLIST.md`](evidence/SPEC_CHECKLIST.md) | Structured language + built-in/library checklist aligned to current evidence and planned gaps. |
| Conformance Check Topics | [`evidence/conformance/CONFORMANCE_CHECK_TOPICS.md`](evidence/conformance/CONFORMANCE_CHECK_TOPICS.md) | Oracle-driven backlog for semantically uncertain VBA behaviors to differential-check after implementation. |
| Deferred Oracle Gates | [`evidence/conformance/DEFERRED_ORACLE_GATES.md`](evidence/conformance/DEFERRED_ORACLE_GATES.md) | Deferred gate register for oracle-dependent semantics (parallel to deferred formal gates). |
| Implementation-Defined Register | [`evidence/conformance/IMPLEMENTATION_DEFINED.md`](evidence/conformance/IMPLEMENTATION_DEFINED.md) | Explicit catalog of implementation-defined behavior choices and conformance impact links. |
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
