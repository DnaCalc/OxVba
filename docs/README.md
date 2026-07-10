# OxVba Documentation

## Start here

Read these documents in order:

1. [`../CHARTER.md`](../CHARTER.md) — mission, values and project scope.
2. [`../OPERATIONS.md`](../OPERATIONS.md) — execution, evidence and completion doctrine.
3. [`spec/OXVBA_SYSTEM_CONTRACT_V1.md`](spec/OXVBA_SYSTEM_CONTRACT_V1.md) — durable destination architecture and capability clauses.
4. [`ARCHITECTURE.md`](ARCHITECTURE.md) — current implementation realization and gaps.
5. [`OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`](OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md) — evidence-backed review that initiated the current program.

`AUTORUN_STATE.md` is the sole volatile execution-control surface. It is not architecture authority.

## Proposed capability program

| profile | destination clauses | current workset |
|---|---|---|
| Core VBA toolchain and dual runtimes | `PROFILE-CORE-001`, `SRC-*`, `COMP-*`, `IR-*`, `RUNTIME-*`, `VM3-*`, `JIT-*` | [`worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md`](worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md) |
| Windows VBA/COM/native compatibility and outputs | `PROFILE-WIN-001`, `WIN-*`, `COM-*`, `NATIVE-*`, `BUILD-*` | [`worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md`](worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md) |
| IDE language-service foundation | `PROFILE-IDE-001`, `LS-*` | [`worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md`](worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md) |

The three worksets are proposed and bead-ready; acceptance and rollout are separate execution steps. Older worksets and profile ladders are historical execution records unless an accepted current workset explicitly imports a residual.

## Current subsystem architecture contracts

- [`spec/OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md`](spec/OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md)
- [`spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md)
- [`spec/OXVBA_JIT_ARCHITECTURE_V1.md`](spec/OXVBA_JIT_ARCHITECTURE_V1.md)
- [`spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`](spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md)
- [`spec/OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md`](spec/OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md)
- [`spec/OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md`](spec/OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md)

The full classification of current and historical specs is in [`spec/README.md`](spec/README.md). Superseded-document mapping is in [`spec/DEPRECATION_LEDGER_2026-07-10.md`](spec/DEPRECATION_LEDGER_2026-07-10.md).

## VBA semantic references

These organize the clean-room VBA target. They refine language semantics; they do not claim implementation completeness:

- [`spec/VBA_GRAMMAR_V1.md`](spec/VBA_GRAMMAR_V1.md)
- [`spec/VBA_TYPE_SYSTEM_V1.md`](spec/VBA_TYPE_SYSTEM_V1.md)
- [`spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md)
- [`spec/VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](spec/VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md)
- [`spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`](spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md)
- [`spec/BASPROJ_SPEC_V1.md`](spec/BASPROJ_SPEC_V1.md)

External authority/source policy is summarized in [`FOUNDATION_SPEC_REFERENCE.md`](FOUNDATION_SPEC_REFERENCE.md).

## Operational documentation

- [`BUILDING.md`](BUILDING.md) — build commands and prerequisites.
- [`TESTING.md`](TESTING.md) — test lanes and harnesses.
- [`CONFORMANCE.md`](CONFORMANCE.md) — compatibility evidence model.
- [`DIAGNOSTICS.md`](DIAGNOSTICS.md) — diagnostic conventions.
- [`FORMAL.md`](FORMAL.md) — formal verification lanes.
- [`LOCAL_EXECUTION_DOCTRINE.md`](LOCAL_EXECUTION_DOCTRINE.md) — local execution hygiene.
- [`IMPLEMENTATION_LOG.md`](IMPLEMENTATION_LOG.md) — chronological implementation record; not current architecture authority.
- [`../CURRENT_BLOCKERS.md`](../CURRENT_BLOCKERS.md) — current documented blockers.
- [`operations/OPERATIONAL_INCIDENT_LOG.md`](operations/OPERATIONAL_INCIDENT_LOG.md) — execution/evidence incidents.

## Validation and evidence

Canonical completion truth belongs in independently closable matrices under `validation/` and reproducible artifacts under `evidence/`. Narrative reports and status tours provide context but do not override matrices, tests or current architecture.

The current worksets require new consolidated matrices for:

- core compiler/VM3/JIT readiness;
- VBA base-library parity;
- OxIR backend support;
- OxImage package/verification;
- current-stack Excel/VBA oracle evidence;
- Windows COM/native/import/export profiles;
- language-service features, reference kinds, LSP methods and performance.

Until those matrices are delivered, the 2026-07-10 review and executable tests are the honest entry evidence.

## Historical material

The following directories contain useful provenance but are not current authority by location alone:

- `archive/`
- `reviews/`
- `status-tours/`
- `evidence/`
- older files in `worksets/`
- superseded files listed in the deprecation ledger.

A historical document may explain why a design existed or preserve old evidence. It must not be cited as proof that the current clean stack implements that design.
