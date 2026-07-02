# COM ByRef Event Writeback Blocker

Date: 2026-07-02
Bead: `bd-aprs.8.8.9` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Target Behavior

The target remains real VBA compile-time and runtime behavior, not legacy OxVBA
event-queue behavior. Microsoft documents Excel `Workbook.BeforeClose(Cancel)`
and `Application.WorkbookBeforeClose(Wb, Cancel)` as cancellable events: setting
`Cancel = True` inside the event procedure stops the close operation when the
procedure finishes.

The COM Automation dispatch contract also makes the writeback requirement
concrete: `DISPPARAMS.rgvarg` arguments are passed as `VARIANT`s, and the callee
may mutate entries only when the `VT_BYREF` flag is set. Therefore a ByRef event
sink must run the handler and copy the changed value back before
`IDispatch::Invoke` returns to the event source.

Sources:
- `https://learn.microsoft.com/en-us/office/vba/api/excel.workbook.beforeclose`
- `https://learn.microsoft.com/en-us/office/vba/api/excel.application.workbookbeforeclose`
- `https://learn.microsoft.com/en-us/previous-versions/windows/desktop/automat/passing-parameters`

## Current OxVBA State

The current Windows connection-point sink receives event `DISPPARAMS`, maps raw
argument order, converts each argument into a retained `ComValue` or object
handle, queues a `ComEventCallback`, returns `S_OK` from
`IDispatch::Invoke`, and only later runs the VBA handler from `Vm3::pump_com_events`.

That design is correct enough for the existing by-value event matrix rows, but
it cannot preserve real ByRef event writeback:

- the native `VARIANT | VT_BYREF` storage belongs to the active COM `Invoke`
  frame;
- the current queue stores owned value snapshots, not mutable pointers back into
  `DISPPARAMS`;
- by the time `DoEvents` drains the callback, the event source has already
  resumed and can no longer observe handler-side changes to the ByRef argument.

Adding only a fixture event such as `OnBeforeCancel(ByRef cancel As Boolean)` would
therefore create a fake convenience path unless the callback transport is changed.

## Required Unblock

V11 needs a synchronous, scoped COM event callback transport for ByRef-capable
dispatch events. The implementation needs at least:

- event metadata that preserves per-parameter ByRef/value and wire-type shape;
- dispatch-sink argument capture that can identify supported `VT_BYREF` scalar
  slots without extending pointer lifetime beyond `Invoke`;
- a same-call handler execution path that runs the VBA handler before returning
  from the native event sink and writes supported changed values back to the
  raw `DISPPARAMS` slots;
- explicit handling or rejection for cross-apartment/out-of-process event
  threads where synchronous VM re-entry would not be sound.

Until that exists, V11 remains an in-progress compatibility blocker. No legacy
OxVBA event queue behavior is accepted as the target.
