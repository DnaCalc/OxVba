# vm3 LeftB/RightB Oracle Evidence

Bead: `bd-4ktq.71`

Inventory row: `leftb-rightb-byte-fns-absent`

Captured: 2026-07-02T13:23:06Z

Oracle: Excel/VBA 7.1

## Modal Handling

The first probe compile intentionally exercised the modal path after a helper name
collided with its argument name. UI Automation captured and dismissed the owned
VBE modal:

- Dialog: `Microsoft Visual Basic for Applications`
- Text: `Compile error: Duplicate declaration in current scope`
- Selected line/token: `Private Function v(ByVal v As Variant) As String` / `v`

After renaming the helper argument, `Debug -> Compile VBAProject` completed and
the run succeeded.

## Observed Behavior

`LeftB` and `RightB` slice raw BSTR payload bytes. Odd byte counts preserve odd
BSTR byte lengths; `Len` reports only complete UTF-16 code units, while `LenB`
reports the exact stored byte count.

Representative observations for `s = "ABC"`:

- `LeftB(s, 1)`: `Len = 0`, `LenB = 1`, no complete UTF-16 code units.
- `LeftB(s, 3)`: `Len = 1`, `LenB = 3`, first code unit `65`.
- `RightB(s, 1)`: `Len = 0`, `LenB = 1`, no complete UTF-16 code units.
- `RightB(s, 3)`: `Len = 1`, `LenB = 3`, first code unit `17152`.
- Over-length counts clamp to the whole string.
- Unsuffixed `LeftB(Null, n)` and `RightB(Null, n)` return `Null`.
- Negative counts raise run-time error `5`, `Invalid procedure call or argument`.

Full raw probe output is in `results.json`.
