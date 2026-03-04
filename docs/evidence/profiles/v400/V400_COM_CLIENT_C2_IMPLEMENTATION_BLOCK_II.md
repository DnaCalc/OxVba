# V400 COM Client C2 Implementation Block II

## Scope
- Ladder: `v387..v406`
- Completed slice: `v397..v400`
- Workset: `WORKSET_2026-03-05_COM_CLIENT_LATEBOUND_IMPLEMENTATION_V397_V400.md`

## Outputs
- Compiler now lowers known ProgID string literals (`"Scripting.Dictionary"`) for `CreateObject`.
- Controlled COM lane now uses OxVba-owned in-process test ProgID alias (`"OxVba.TestDispatch"`) mapped into the same selector token for deterministic integration coverage.
- Compiler now lowers known member-name string literals (`"Count"`, `"Exists"`) for `DispatchInvoke`.
- `DispatchInvoke` accepts a 2-argument property-get form and a 3-argument scalar form.
- HAL Windows native COM lane now caches resolved member DISPIDs per object for known member-token lanes.
- Missing third-argument semantics are deterministic (`DispatchInvoke` 2-arg route):
  - projection fallback normalizes missing arg to `0`,
  - native lanes reject missing-arg calls for members that require an argument.
- C2 fixture pack includes success and `On Error Resume Next` failure-path examples.

## Gate Signal
- `v400` implementation block is complete and verified; C2 lane can proceed to runner/lane automation steps (`v401+`).
