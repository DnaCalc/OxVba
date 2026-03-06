# OxVba Spec Drafts

This directory contains early-stage OxVba internal design drafts.

Normative external specification sources are maintained in `../Foundation/reference`
(see `docs/FOUNDATION_SPEC_REFERENCE.md`).

Status model:
- `design-draft`: directional, incomplete, expected to change quickly.
- `working-draft`: structured and testable, still open for significant revision.
- `stable-draft`: implementation-linked and evidence-backed; still not final normative text.

Current draft set:
- [`HAL_DESIGN_DRAFT.md`](HAL_DESIGN_DRAFT.md) (`design-draft`): scope, principles, profile targets, and staged design plan for the Host Abstraction Layer.
- [`HAL_INTERFACE_DRAFT.md`](HAL_INTERFACE_DRAFT.md) (`design-draft`): proposed HAL contracts, capability schema, and maturity model.
- [`HAL_CONFORMANCE_DRAFT.md`](HAL_CONFORMANCE_DRAFT.md) (`design-draft`): proposed conformance classes, test obligations, and evidence model.
- [`HAL_SPEC_WORKING_DRAFT.md`](HAL_SPEC_WORKING_DRAFT.md) (`working-draft`): implementation-linked HAL contract, deterministic error model, unsupported-mode semantics, and Windows-only COM decision.
- [`HAL_SPEC_CROSSWALK.md`](HAL_SPEC_CROSSWALK.md) (`working-draft`): capability/intrinsic to Foundation anchor mapping plus known extraction gaps.
- [`HAL_CONFORMANCE_SUITE.md`](HAL_CONFORMANCE_SUITE.md) (`working-draft`): runnable HAL harness layers, commands, artifact schema, and expectations.
- [`HAL_FORMALIZATION_PROGRAM.md`](HAL_FORMALIZATION_PROGRAM.md) (`working-draft`): charter-driven HAL formalization program with 5-step execution ladder and H1/H2/H3 tracks.
- [`HAL_CONTRACT_CLAUSE_CATALOG_V1.md`](HAL_CONTRACT_CLAUSE_CATALOG_V1.md) (`working-draft`): explicit clause ID catalog with pre/postconditions, failure obligations, and verification mapping.
- [`HAL_CONTRACT_CLAUSE_CATALOG_V1.csv`](HAL_CONTRACT_CLAUSE_CATALOG_V1.csv) (`working-draft`): machine-readable clause schema for coverage computation and drift-guard checks.
- [`HAL_POLICY_PRESETS.md`](HAL_POLICY_PRESETS.md) (`working-draft`): named host-policy preset table (`strict-ci`, deterministic modes, interactive-dev) and intended usage.
- [`HAL_CONTRACT_ASSERTION_HARDENING.md`](HAL_CONTRACT_ASSERTION_HARDENING.md) (`working-draft`): debug/checked build assertion scaffold and staged hardening path for in-code contract checks.
- [`HAL_OPERATING_ENVELOPE_V1.md`](HAL_OPERATING_ENVELOPE_V1.md) (`working-draft`): explicit v1 host-boundary guarantees, non-guarantees, and optimization-safe operating constraints.
- [`HAL_RUNTIME_PROFILE_BOOTSTRAP_IMPLEMENTATION_V2.md`](HAL_RUNTIME_PROFILE_BOOTSTRAP_IMPLEMENTATION_V2.md) (`working-draft`): implemented runtime bootstrap resolver and CLI integration snapshot (`v198..v201`).
- [`HAL_UI_PLATFORM_IMPLEMENTATION_V2.md`](HAL_UI_PLATFORM_IMPLEMENTATION_V2.md) (`working-draft`): implemented Windows GUI/Linux stdio UI + DoEvents runtime-class behavior snapshot (`v207..v211`).
- [`HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md`](HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md) (`working-draft`): implemented Declare metadata/lowering/VM/HAL dynamic-link subset snapshot (`v212..v218`).
- [`HAL_DECLARE_ABI_SPEC_V1.md`](HAL_DECLARE_ABI_SPEC_V1.md) (`working-draft`): formalized external declaration + marshaling contract with source-anchor mapping and implementation-defined boundaries.
- [`HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`](HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md) (`working-draft`): clause-mapped conformance lanes for declaration parsing, runtime gating, marshaling, and deferred oracle checks.
- [`PROJECT_MODULE_REFERENCE_SPEC_V1.md`](PROJECT_MODULE_REFERENCE_SPEC_V1.md) (`working-draft`): formal state model, invariants, pre/postconditions, and deterministic error semantics for project/module/reference behavior.
- [`PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`](PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md) (`working-draft`): clause IDs and verification mappings for PMR semantics.
- [`PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv`](PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv) (`working-draft`): machine-readable PMR clause coverage map.
- [`PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md`](PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md) (`working-draft`): executable lane design for PMR static semantics, multi-module resolution, references, and storage.
- [`PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md`](PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md) (`working-draft`): HAL-adjacent contract and capability planning for host projects, references, and storage.
- [`PROJECT_MODULE_REFERENCE_TYPELIB_IMPORTLIB_HAL_DRAFT_V1.md`](PROJECT_MODULE_REFERENCE_TYPELIB_IMPORTLIB_HAL_DRAFT_V1.md) (`working-draft`): deterministic importlib/type-library binding contract draft and HAL interaction shape for PMR reference resolution.
- [`PROJECT_MODULE_REFERENCE_SOURCE_CROSSWALK_V1.md`](PROJECT_MODULE_REFERENCE_SOURCE_CROSSWALK_V1.md) (`working-draft`): PMR source-anchor crosswalk across MS-VBAL, MS-OAUT, and MS-OVBA extraction status.
- [`VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md`](VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md) (`design-draft`): `.vbp` subset support strategy, wrapper EXE/DLL packaging model, and lateral artifact options for loose vs compiled project execution.
- [`CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md`](CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md) (`working-draft`): staged class-module/COM alignment plan with explicit near-term semantic steps and deferred interop boundaries.
- [`COM_CLIENT_SERVER_SCOPE_V1.md`](COM_CLIENT_SERVER_SCOPE_V1.md) (`working-draft`): Windows COM client/server support scope, contract boundaries, tier model, apartment policy stance, and C2 late-bound client runway.
- [`COM_CLIENT_SERVER_CONFORMANCE_V1.md`](COM_CLIENT_SERVER_CONFORMANCE_V1.md) (`working-draft`): COM-specific conformance lane architecture, artifact model, and C2 late-bound client lane planning with formal/deferred-oracle integration.
- [`COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`](COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md) (`working-draft`): comprehensive design for COM early binding and type-library consumption across PMR, HAL, binder, IR/runtime, caching, diagnostics, and formalized verification planning.
- [`COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md`](COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md) (`working-draft`): executable lane plan (`E0..E6`) for early-binding/type-library conformance, formal lanes, and deferred-oracle tracking.
- [`HAL_COM_BRIDGE_SCOPE_V1.md`](HAL_COM_BRIDGE_SCOPE_V1.md) (`working-draft`): HAL-owned COM boundary scope and C1->C2 transition contract for tokenized/native late-bound client behavior.
- [`COM_CLIENT_LATEBOUND_BRIDGE_V1.md`](COM_CLIENT_LATEBOUND_BRIDGE_V1.md) (`working-draft`): explicit cross-layer bridge contract (VBA semantics -> compiler/VM transport -> HAL COM transport -> native adapter).

These files intentionally optimize for design velocity and clarity of open decisions rather than immediate lock-in.
