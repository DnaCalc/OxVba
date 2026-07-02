# VM3 Width # Excel Oracle

- Run ID: vm3_width_oracle_20260702T0004Z
- Captured: 2026-07-02T00:02:40Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-width-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| WIDTH-LONG-FIELD | ok |  |  | ok | abcdef<CR><LF> |
| WIDTH-ADJACENT-FIELDS | ok |  |  | ok | abcd<CR><LF>ef<CR><LF> |
| WIDTH-CROSS-STATEMENT | ok |  |  | ok | abcde<CR><LF>f<CR><LF> |
| WIDTH-NUMERIC-WRAP | ok |  |  | ok | _12_<CR><LF>_34_<CR><LF> |
| WIDTH-COMMA-ZONE | ok |  |  | ok | a<CR><LF>b<CR><LF> |
| WIDTH-SPC-TAB | ok |  |  | ok | A___B<CR><LF>__C<CR><LF>D<CR><LF> |
| WIDTH-SPC-LONG | ok |  |  | ok | _A<CR><LF> |
| WIDTH-TAB-FAR | ok |  |  | ok | ____A<CR><LF> |
| WIDTH-WRITE-UNAFFECTED | ok |  |  | ok | "abcdef",1<CR><LF> |
| WIDTH-ZERO-DISABLES | ok |  |  | ok | abcdef<CR><LF> |
| WIDTH-REOPEN-RESETS | ok |  |  | ok | abcdef<CR><LF> |
| WIDTH-NEGATIVE-ERROR | ok |  |  | ok | 5\|Invalid procedure call or argument |
| WIDTH-256-ERROR | ok |  |  | ok | 5\|Invalid procedure call or argument |

Raw JSON: results.json
