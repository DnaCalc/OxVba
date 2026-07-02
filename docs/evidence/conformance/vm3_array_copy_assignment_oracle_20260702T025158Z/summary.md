# VM3 Array Copy/Assignment Excel Oracle

- Run ID: vm3_array_copy_assignment_oracle_20260702T025158Z
- Captured: 2026-07-02T02:55:29Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-array-copy-assignment-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value | Error |
|---|---|---|---|---|---|---|
| DYNAMIC-BYREF-MUTATES | ok |  |  | ok | 99:0:0:1 |  |
| ARRAY-BYVAL-PARAM-COMPILE | compile-error | Compile error: /  / Array argument must be ByRef / Gauge | Private Sub Touch(ByVal a() As Long) | not-run |  |  |
| VARIANT-BYVAL-ARRAY-COPY | ok |  |  | ok | 7:8:1 |  |
| VARIANT-BYREF-ARRAY-MUTATES | ok |  |  | ok | 99:8:1 |  |
| DYNAMIC-ASSIGN-COPY | ok |  |  | ok | 7:8:2:3 |  |
| DYNAMIC-ASSIGN-PRESERVE-INDEPENDENT | ok |  |  | ok | 7:8:0:1:2 |  |
| FIXED-LHS-FROM-DYNAMIC | compile-error | Compile error: /  / Can't assign to array / Gauge | dst = src | not-run |  |  |
| FIXED-LHS-FROM-FIXED | compile-error | Compile error: /  / Can't assign to array / Gauge | dst = src | not-run |  |  |
| DYNAMIC-LHS-FROM-FIXED | ok |  |  | ok | 7:0:1 |  |

Raw JSON: results.json
