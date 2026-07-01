# VM3 Default Error Message Excel Oracle

- Run ID: vm3_default_error_message_oracle_20260701T1410Z
- Captured: 2026-07-01T14:10Z
- Harness: one-shot PowerShell Excel/VBA probe based on the repo's VBE compile-oracle pattern
- Modal handling: fresh owned Excel PID, VBE visible, Debug -> Compile VBAProject (`ID=578`) before `Application.Run`, UI Automation scan for owned VBE modal text, owned-dialog dismissal if needed, PID-scoped Excel cleanup.
- Compile modal observed: no
- Primary public reference: Microsoft Learn "Trappable errors" table
  (`https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/trappable-errors`),
  with live Excel/VBA 7.1 strings taking precedence where wording differs.

The broad probe executes `Err.Raise CLng(code)` under `On Error Resume Next`
after `Err.Clear`, then records `Err.Description`. A representative follow-up
probe executes `Error CLng(code)` for selected codes and confirms the same
default-message wording and generic fallback. The selected broad-probe codes
cover the default-message table added for `bd-4ktq.28`; unmapped code `12345`
remains the generic application/object-defined fallback.

Raw output is recorded in `results.json`.
