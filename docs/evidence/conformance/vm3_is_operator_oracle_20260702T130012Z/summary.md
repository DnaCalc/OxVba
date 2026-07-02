# vm3 `Is` operator oracle

Captured: 2026-07-02T13:00:13Z

Purpose: verify real Excel/VBA 7.1 compile-time and runtime behavior for `Is`
when operands are not object references.

Modal handling: the probe used the established VBE-visible
Debug -> Compile VBAProject path through command id `578`, UI Automation
inspection scoped to the owned Excel PID, VBE selected-token/line capture, and
owned-dialog dismissal before PID-scoped cleanup.

Observed behavior:

- `Dim a As Long, b As Long: r = (a Is b)` is a compile error: `Type mismatch`.
  The VBE selected token was `b` on `r = (a Is b)`.
- `Variant` operands holding scalars compile, then evaluating `a Is b` raises
  runtime error 424, `Object required`.
- `Object Is Variant` also compiles when the Variant holds a scalar and raises
  runtime error 424, `Object required`.
- An unset object variable compares equal to `Nothing`.
- Real object identity remains identity-based: an aliased `Collection` compares
  `True`, a distinct `Collection` compares `False`, and a live object is not
  `Nothing`.

Raw results: `results.json`.
