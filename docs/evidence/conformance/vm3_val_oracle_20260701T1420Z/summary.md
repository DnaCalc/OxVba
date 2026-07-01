# VM3 Val Prefix Parsing Excel Oracle

- Run ID: vm3_val_oracle_20260701T1420Z
- Captured: 2026-07-01T14:20Z
- Harness: one-shot PowerShell Excel/VBA probe based on the repo's VBE compile-oracle pattern
- Modal handling: fresh owned Excel PID, VBE visible, Debug -> Compile VBAProject (`ID=578`) before `Application.Run`, UI Automation scan for owned VBE compile modal text, owned-dialog dismissal if needed, PID-scoped Excel cleanup.
- Compile modal observed: no
- Public reference: Microsoft Learn `Val` function
  (`https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/val-function`).

The probe executes `Val(text)` under `On Error Resume Next` and records either
the `CStr` value or `ERR:<number>:<description>`. The selected cases capture the
`bd-4ktq.29` fix: `Val` stops at the first unrecognized non-whitespace
continuation, treats incomplete `E`/`D` exponents as not part of the number,
accepts complete `E`/`D` exponents, strips ASCII spaces/tabs before parsing, and
keeps the existing `&H`/`&O` radix behavior.

Raw output is recorded in `results.json`.
