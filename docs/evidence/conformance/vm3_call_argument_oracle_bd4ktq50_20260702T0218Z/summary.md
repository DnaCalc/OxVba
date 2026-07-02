# VM3 Call Argument Excel Oracle

- Run ID: vm3_call_argument_oracle_bd4ktq50_20260702T0218Z
- Captured: 2026-07-02T02:03:03Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-call-argument-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE document, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| CALL-BARE-BYREF-MUTATES | ok |  |  | ok | 105 |
| CALL-BYVAL-PARAM-NO-MUTATE | ok |  |  | ok | 5 |
| CALL-STMT-PAREN-BYVAL | ok |  |  | ok | 5 |
| CALL-FORM-PARENS-BYREF | ok |  |  | ok | 105 |
| CALL-BYREF-TYPE-MISMATCH | compile-error | Compile error: /  / ByRef argument type mismatch | TakeLong x | not-run |  |
| CALL-EXTRA-ARG | compile-error | Compile error: /  / Wrong number of arguments or invalid property assignment | TakeOne 1, 2 | not-run |  |
| CALL-MISSING-ARG | compile-error | Compile error: /  / Argument not optional | TakeTwo 1 | not-run |  |
| CALL-OPTIONAL-MISSING-OK | ok |  |  | ok | 12 |
| CALL-PARAMARRAY-EXTRA-OK | ok |  |  | ok | 6 |
| CALL-PARAMARRAY-SCALAR-ELEMENT-ALIASES-CALLER | ok |  |  | ok | 99 |
| CALL-PARAMARRAY-VARIANT-ELEMENT-ALIASES-CALLER | ok |  |  | ok | 99 |
| CALL-PARAMARRAY-ARRAY-ELEMENT-LVALUE-ALIASES-CALLER | ok |  |  | ok | 99 |
| CALL-PARAMARRAY-OBJECT-ELEMENT-REBIND-ALIASES-CALLER | ok |  |  | ok | err:91:Object variable or With block variable not set |
| CALL-PARAMARRAY-VARIANT-ARRAY-ELEMENT-MUTATION-ALIASES-CALLER | ok |  |  | ok | ok:99 |

Raw JSON: results.json
