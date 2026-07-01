# VM3 Nothing Excel Oracle

- Run ID: vm3_nothing_oracle_20260701T151239Z
- Captured: 2026-07-01T15:17:13Z
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE document, owned-dialog dismissal, then PID-scoped process cleanup.
- Note: the first exploratory Variant assignment case timed out when `v = Nothing`
  surfaced a modal runtime error. The assignment follow-up used `On Error Resume Next`
  so the owned Excel/VBE process stayed responsive while pinning the `Let` versus `Set`
  behavior.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| NOTHING-LITERAL-INFO | ok |  |  | ok | OK:9<pipe>OK:Nothing<pipe>OK:True<pipe>OK:False |
| OBJECT-VAR-SET-NOTHING | ok |  |  | ok | VT=9;E0:<pipe>TN=Nothing;E0:<pipe>IO=True;E0:<pipe>IE=False;E0:<pipe>IS=True;E0: |
| OBJECT-VAR-UNSET | ok |  |  | ok | VT=9;E0:<pipe>TN=Nothing;E0:<pipe>IO=True;E0:<pipe>IE=False;E0:<pipe>IS=True;E0: |
| EMPTY-BASELINE | ok |  |  | ok | 0:Empty:False:True |
| LET-VARIANT-NOTHING-OERN | ok |  |  | ok | 91:Object variable or With block variable not set:0:Empty:False:True |
| SET-VARIANT-NOTHING-OERN | ok |  |  | ok | 0::9:Nothing:True:False |

Raw JSON: results.json, assignment_oern.json
