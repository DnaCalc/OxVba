# VM3 Err Property Writes Oracle

- Host: Excel/VBA 7.1 via `Excel.Application`.
- Modal handling: VBE made visible, Debug -> Compile VBAProject invoked with command ID `578`, and UI Automation checks were scoped to the owned Excel/VBE process before any `Application.Run`.
- Compile-dialog capture: `Err.LastDllError = 123` produced `Compile error: Can't assign to read-only property`, with selected text `.LastDllError =` on line `Err.LastDllError = 123`; the owned dialog was dismissed via its OK button.
- Results: see `results.csv`.

Findings:

- `Err.Number`, `Err.Description`, and `Err.Source` are writable.
- `Err.Number = 6` sets only the numeric property; it does not derive `Err.Description` or `Err.Source`, and by itself does not make omitted `Err.Raise` Source/Description fields inherit.
- `Err.Description = ...` and `Err.Source = ...` make later omitted `Err.Raise` fields inherit those current values, even when `Err.Number` is still `0`.
- `Err.Number = 0` does not clear previously inheritable Source/Description fields; `Err.Clear` does.
- `Err.LastDllError` is read-only at compile time.
