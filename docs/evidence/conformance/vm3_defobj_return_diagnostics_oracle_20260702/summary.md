# VM3 DefObj and Function Return Diagnostic Excel Oracle

- Run ID: vm3_defobj_return_diagnostics_oracle_20260702
- Captured: 2026-07-02T23:53:41Z
- Harness: temporary PowerShell harness following the repo's VBE compile-oracle pattern.
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog Text | Selected Text | Selected Line |
|---|---|---|---|---|
| DEFAULT-TYPE-PARAM-DEFOBJ | compile-error | Compile error: Type mismatch | 1 | Call Use(1) |
| FUNCTION-RETURN-SUFFIX-AS-CONFLICT | compile-error | Compile error: Expected: end of statement | As | Function alpha%() As Object |

Raw JSON: results.json
