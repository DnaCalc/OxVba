# VM3 Integer Literal Excel Oracle

- Run ID: vm3_integer_literal_oracle_20260701T1200Z
- Captured: 2026-07-01T11:51:58Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-integer-literal-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE document, owned-dialog dismissal, then PID-scoped process cleanup.
- Probe shape: CStr(VarType(expr)) & ":" & TypeName(expr) & ":" & CStr(expr).

| Case | Expr | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|---|
| INTLIT-DEC-SMALL | `7` | ok |  |  | ok | 2:Integer:7 |
| INTLIT-DEC-INT-MAX | `32767` | ok |  |  | ok | 2:Integer:32767 |
| INTLIT-DEC-LONG-MIN | `32768` | ok |  |  | ok | 3:Long:32768 |
| INTLIT-DEC-LONG-MAX | `2147483647` | ok |  |  | ok | 3:Long:2147483647 |
| INTLIT-DEC-LONGLONG-MIN | `2147483648` | ok |  |  | ok | 5:Double:2147483648 |
| INTLIT-DEC-PERCENT | `7%` | ok |  |  | ok | 2:Integer:7 |
| INTLIT-DEC-AMPERSAND | `7&` | ok |  |  | ok | 3:Long:7 |
| INTLIT-DEC-CARET | `7^` | ok |  |  | ok | 20:LongLong:7 |
| INTLIT-HEX-INT-WIDTH | `&HFFFF` | ok |  |  | ok | 2:Integer:-1 |
| INTLIT-HEX-LONG-WIDTH | `&H10000` | ok |  |  | ok | 3:Long:65536 |
| INTLIT-HEX-AMPERSAND | `&HFFFF&` | ok |  |  | ok | 3:Long:65535 |
| INTLIT-HEX-LONGLONG-WIDTH | `&H100000000` | compile-error | Compile error: /  / Syntax error | RunProbe = CStr(VarType(&H100000000)) & ":" & TypeName(&H100000000) & ":" & CStr(&H100000000) | not-run |  |
| INTLIT-HEX-CARET | `&HFFFFFFFFFFFFFFFF^` | ok |  |  | ok | 20:LongLong:-1 |
| INTLIT-OCT-INT-WIDTH | `&O177777` | ok |  |  | ok | 2:Integer:-1 |
| INTLIT-OCT-LONG-WIDTH | `&O200000` | ok |  |  | ok | 3:Long:65536 |
| INTLIT-OCT-AMPERSAND | `&O177777&` | ok |  |  | ok | 3:Long:65535 |
| INTLIT-OCT-LONGLONG-WIDTH | `&O40000000000` | compile-error | Compile error: /  / Syntax error | RunProbe = CStr(VarType(&O40000000000)) & ":" & TypeName(&O40000000000) & ":" & CStr(&O40000000000) | not-run |  |

Raw JSON: results.json
