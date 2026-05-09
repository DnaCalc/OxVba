# Validation Derived Summary

Generated from:
- `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv`
- `docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv`
- `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv`
- `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv`

## Totals

| Domain | Rows | In Progress | Implemented Subset | Implemented Full | Verified | Planned |
|---|---|---|---|---|---|---|
| language | 4 | 0 | 4 | 0 | 0 | 0 |
| com_external | 10 | 1 | 2 | 0 | 5 | 2 |
| project_hosting | 11 | 1 | 9 | 0 | 0 | 1 |
| language_services_formalization | 8 | 3 | 5 | 0 | 0 | 0 |

## Open Focus

| Feature ID | Domain | Feature | Truth State | Matrix |
|---|---|---|---|---|
| COM-0008 | com_external | WrappedComServer generated TLB and dispatch-backed early-bound client calls | in-progress | docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv |
| COM-0009 | com_external | WrappedComServer Automation-safe dual-interface vtable publication | planned | docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv |
| COM-0010 | com_external | WrappedComServer source dispinterface and connection-point event publication | planned | docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv |
| PH-0010 | project_hosting | MS-OVBA storage roundtrip | in-progress | docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv |
| PH-0011 | project_hosting | Host UDF catalog and invocation for DnaOneCalc/OxIde-style hosts | planned | docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv |
| LSF-0001 | language_services_formalization | Language-service core workspace/query substrate | in-progress | docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv |
| LSF-0105 | language_services_formalization | Thin transport and embedding boundary | in-progress | docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv |
| LSF-0002 | language_services_formalization | Formal compiler/language representation coverage | in-progress | docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv |
