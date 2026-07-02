# VM3 IsArray Unallocated Excel Oracle

- Run ID: vm3_isarray_unallocated_oracle_20260702T040452Z
- Captured: 2026-07-02T04:11:20Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-isarray-unallocated-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value | Error |
|---|---|---|---|---|---|---|
| DYNAMIC-UNALLOCATED-LONG | ok |  |  | ok | True |  |
| DYNAMIC-UNALLOCATED-LONG-LBOUND | ok |  |  | ok | True:9 |  |
| DYNAMIC-UNALLOCATED-LONG-UBOUND | ok |  |  | ok | True:9 |  |
| DYNAMIC-UNALLOCATED-LONG-INTROSPECTION | ok |  |  | ok | True:8195:Long() |  |
| DYNAMIC-ALLOCATED-LONG | ok |  |  | ok | True |  |
| DYNAMIC-ERASED-LONG | ok |  |  | ok | True |  |
| DYNAMIC-ERASED-LONG-LBOUND | ok |  |  | ok | True:9 |  |
| DYNAMIC-ERASED-LONG-UBOUND | ok |  |  | ok | True:9 |  |
| DYNAMIC-ERASED-LONG-INTROSPECTION | ok |  |  | ok | True:8195:Long() |  |
| FIXED-LONG | ok |  |  | ok | True |  |
| FIXED-ERASED-LONG | ok |  |  | ok | True |  |
| VARIANT-EMPTY | ok |  |  | ok | False |  |
| VARIANT-ARRAY-LITERAL | ok |  |  | ok | True |  |
| VARIANT-DYNAMIC-ARRAY-COPY | ok |  |  | ok | True |  |
| VARIANT-UNALLOCATED-DYNAMIC-COPY | ok |  |  | ok | True |  |
| VARIANT-UNALLOCATED-DYNAMIC-COPY-INTROSPECTION | ok |  |  | ok | True:8195:Long() |  |
| ERASE-VARIANT-ARRAY | ok |  |  | ok | True |  |
| ERASE-VARIANT-ARRAY-LBOUND | ok |  |  | ok | True:9 |  |
| ERASE-VARIANT-ARRAY-UBOUND | ok |  |  | ok | True:9 |  |
| ERASE-VARIANT-ARRAY-INTROSPECTION | ok |  |  | ok | True:8204:Variant() |  |

Raw JSON: results.json
