# VM3 Mid Start Position Excel Oracle

- Run ID: vm3_mid_start_oracle_20260701T1425Z
- Captured: 2026-07-01T14:25Z
- Harness: one-shot PowerShell Excel/VBA probe based on the repo's VBE compile-oracle pattern
- Modal handling: fresh owned Excel PID, VBE visible, Debug -> Compile VBAProject (`ID=578`) before `Application.Run`, UI Automation scan for owned VBE compile modal text, owned-dialog dismissal if needed, PID-scoped Excel cleanup.
- Compile modal observed: no
- Public reference: Microsoft Learn `Mid` function
  (`https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/mid-function`).

The probe captures both `Mid(string, start, length)` and `Mid(string, start,
length) = replacement`. Function and statement forms raise runtime error 5 for
`start < 1`; `start = 1`, overlarge function start, and a valid statement
replacement remain unchanged.

Raw output is recorded in `results.json`.
