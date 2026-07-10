# OxVba Specifications

## Authority model

Specifications in this directory have one of four roles:

- **system contract** — durable destination and capability clauses;
- **current architecture contract** — current accepted subsystem design;
- **semantic reference** — clean-room VBA semantics refined by evidence;
- **historical/supporting** — prior design, planning, conformance mechanics or provenance that cannot override current contracts.

File location does not grant authority. A document not listed as current below is supporting or historical unless a current contract explicitly adopts it.

Normative external authorities are public specifications, the real VBA type library and reproducible Office/VBA observations under the clean-room rule. See [`../FOUNDATION_SPEC_REFERENCE.md`](../FOUNDATION_SPEC_REFERENCE.md).

## System destination

- [`OXVBA_SYSTEM_CONTRACT_V1.md`](OXVBA_SYSTEM_CONTRACT_V1.md) — authoritative OxVba destination, capability profiles, ownership and completion clauses.

## Current subsystem architecture

- [`OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md`](OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md)
- [`OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md)
- [`OXVBA_JIT_ARCHITECTURE_V1.md`](OXVBA_JIT_ARCHITECTURE_V1.md)
- [`OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`](OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md)
- [`OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md`](OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md)
- [`OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md`](OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md)

These contracts describe the intended architecture. Current implementation truth and gaps remain in [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

## Current VBA semantic references

- [`VBA_GRAMMAR_V1.md`](VBA_GRAMMAR_V1.md) — grammar and recovery anchor.
- [`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md) — declared/runtime type model.
- [`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md) — expressions, coercion, assignment and calls.
- [`VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md) — machine-readable semantic-table targets.
- [`PROJECT_MODULE_REFERENCE_SPEC_V1.md`](PROJECT_MODULE_REFERENCE_SPEC_V1.md) — project/module/reference semantics.
- [`PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`](PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md) — stable PMR clauses.
- [`BASPROJ_SPEC_V1.md`](BASPROJ_SPEC_V1.md) — `.basproj` format.
- [`OXVBA_POINTER_HELPERS_CONTRACT_V1.md`](OXVBA_POINTER_HELPERS_CONTRACT_V1.md) — pointer-helper semantic target, subordinate to Windows interop architecture.
- [`DEBUG_PRINT_HOST_OWNERSHIP_V1.md`](DEBUG_PRINT_HOST_OWNERSHIP_V1.md) — accepted Debug.Print ownership rule.

These references do not use `implemented` language unless current matrices/tests prove the complete scoped behavior.

## Retained supporting specifications

The following families remain useful subordinate detail but are not system/current-status authority:

- COM scope/conformance and reference-selection documents, where consistent with `OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`;
- HAL clause, conformance, runtime-profile and host-policy documents, where consistent with `HOST-HAL-001`;
- project/reference conformance and source-crosswalk documents;
- build/wrapper/BASPROJ documents, where consistent with `BUILD-*` and current `.oxi` architecture;
- browser/web-host documents as extended-profile design inputs only;
- debugger, immediate-evaluator and embedded-host documents as future design inputs only.

Their implementation snapshots and historical workset/gate references are not current capability claims.

## Superseded architecture families

The following are explicitly superseded for current architecture:

- `OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md` and `HIR_RESOLUTION_ENVIRONMENT_V1.md`;
- `EXECUTABLE_SEMANTIC_PACKAGE_V1.md`, its completion map, bytecode/VM contract and VMR-06 plan;
- the `JIT_V2_*` planning/design family and `docs/OXVBA_JIT_PLAN.md`;
- `LANGUAGE_SERVICE_SPEC_V1.md`, `LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md` and root language-service guidance;
- VM2/Bundle-era execution plans and handoffs;
- debugger/direct-host documents that claim deleted active crates or implemented surfaces.

They remain provenance and may contain reusable tests or design insights. They cannot override the system contract, current architecture or successor subsystem contracts.

The exact successor map and handling rule is in [`DEPRECATION_LEDGER_2026-07-10.md`](DEPRECATION_LEDGER_2026-07-10.md).

## Status vocabulary

- `normative destination contract` — accepted target; not an implementation claim.
- `current architecture contract` — accepted subsystem design; status still comes from architecture/matrices.
- `working semantic reference` — authoritative semantic organization with open evidence/revision.
- `supporting` — useful detail subordinate to current contracts.
- `historical/superseded` — provenance only.

Avoid labels such as `active`, `implemented`, `complete` or `design-locked` when they conflate document maturity with current product capability.
