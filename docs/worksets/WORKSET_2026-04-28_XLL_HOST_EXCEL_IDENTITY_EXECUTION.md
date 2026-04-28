# Workset: XLL Host Excel Application Identity

## Problem

The prior XLL `Application` binding used `GetActiveObject("Excel.Application")`.
That was not identity-safe when more than one Excel process was running. The
ROT may return a running Excel automation object that is not the process
currently hosting the loaded `.xll`.

This workset corrects the binding contract: XLL-hosted VBA must receive the
Excel `Application` object for the hosting Excel process, or no object at all.
Binding a plausible but unverified Excel instance is a correctness bug.

## Public API Facts

- `xlGetHwnd` is callable from an XLL and returns the top-level Excel window
  handle for the Excel instance calling the XLL:
  <https://learn.microsoft.com/en-us/office/client-developer/excel/xlgethwnd>
- Excel's VBA `Application.hWnd` property returns the top-level Excel window
  handle for that `Application` object:
  <https://learn.microsoft.com/en-us/office/vba/api/excel.application.hwnd>
- `GetWindowThreadProcessId` maps a window handle to the owning process ID:
  <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid>
- `GetRunningObjectTable` returns the local ROT:
  <https://learn.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-getrunningobjecttable>
- `IRunningObjectTable::EnumRunning` enumerates currently registered ROT
  monikers:
  <https://learn.microsoft.com/en-us/windows/desktop/api/objidl/nf-objidl-irunningobjecttable-enumrunning>

## Proposed Contract

1. During XLL session creation or first `Application` demand, ask Excel for the
   host window handle via `Excel12(xlGetHwnd, ...)`.
2. Resolve the host process ID with `GetWindowThreadProcessId(host_hwnd, ...)`.
3. Enumerate ROT entries instead of using `GetActiveObject` as the binding
   authority.
4. For each Excel automation candidate:
   - bind/get its object from the ROT;
   - query `IDispatch`;
   - invoke/read `Application.hWnd`;
   - resolve that HWND to a process ID;
   - accept only candidates whose HWND and/or process ID match the hosting XLL
     process identity.
5. Bind the candidate into the engine host-object registry only after a
   host-identity match. Multiple ROT monikers that resolve to the same host
   HWND are duplicate evidence and are collapsed; competing PID-only candidates
   remain ambiguous and fail closed.
6. Keep acquisition lazy and retryable. Early Excel lifecycle failures must not
   fail `xlAutoOpen` or XLL registration.

## Important Details

- `GetActiveObject("Excel.Application")` can remain useful only as a fallback
  probe for single-instance diagnostics; it must not be the final authority for
  host object injection.
- The generated shim should record trace fields that make identity issues
  visible: host HWND, host PID, candidate count, accepted candidate HWND/PID,
  and reason for rejection.
- Real Excel validation needs at least two Excel processes, with the XLL loaded
  into a known one. The expected witness should include `Application.hWnd` or a
  process-derived value, not only `Application.Version`, because version is not
  identity-specific.

## Implementation Status

- `crates/oxvba-build/src/xll.rs` now uses `xlGetHwnd`,
  `GetWindowThreadProcessId`, and ROT enumeration instead of
  `GetActiveObject`.
- The generated shim records host HWND/PID, ROT candidate count, each candidate
  HWND/PID/match result, duplicate host candidate collapse, and bind outcome in
  `OXVBA_XLL_TRACE`.
- `examples/xll/application_addin` exports both `ExcelVersion()` and
  `ExcelHwnd()`. `ExcelHwnd()` is the identity witness used by the multi-process
  Excel smoke because two Excel instances commonly share the same version.
- `scripts/run-xll-excel-application-identity-smoke.ps1` creates a host Excel
  process and a decoy Excel process, loads the XLL only into the host, evaluates
  `ExcelVersion()` and `ExcelHwnd()` in the host workbook, and checks the trace
  for both the matching host candidate and non-matching decoy candidate.

## Validation Evidence

- Initial multi-instance validation exposed a real bug: the host appeared in the
  ROT through multiple monikers, and the first implementation rejected the host
  as ambiguous. Evidence:
  `target/xll-host-validation/excel-application-identity/20260428TIDENTITY01/identity_result.json`
  (`status=failed`, `failed=1`).
- After collapsing duplicate host-HWND candidates, multi-instance validation
  passed. Evidence:
  `target/xll-host-validation/excel-application-identity/20260428TIDENTITY02/identity_result.json`
  (`status=passed`, `passed=7`, `failed=0`; host HWND `7347230`, decoy HWND
  `68229344`, observed HWND `7347230`).
- Existing worksheet validation for the Application add-in still passes:
  `target/xll-host-validation/excel-application-worksheet/20260428TIDENTITY02/worksheet_result.json`
  (`status=passed`, `passed=1`, `failed=0`).

Targeted checks run:

- `cargo test -p oxvba-build --lib xll --quiet` - passed, 5 tests.
- `cargo test -p oxvba-host --test xll_application_binding --quiet` - passed,
  1 test.
- `cargo test -p oxvba-project --quiet` - passed, 112 tests across the crate's
  test targets.
- `cargo check -p oxvba-host --quiet` - passed.

## Bead Tree

Parent:

- `bd-p5bb` - correct XLL Excel Application identity binding

Sequence:

- `bd-p5bb.1` - research host-specific Excel Application binding options for XLL
- `bd-p5bb.2` - design hosting-process-matched Excel Application acquisition
- `bd-p5bb.3` - implement host-matched ROT enumeration for XLL Application binding
- `bd-p5bb.4` - test XLL Application binding rejects wrong Excel ROT instance
- `bd-p5bb.5` - validate XLL Application binding with multiple Excel instances
- `bd-p5bb.6` - document corrected XLL Excel Application identity behavior

## Exit Criteria

- The generated XLL shim no longer treats plain `GetActiveObject` as sufficient
  evidence for host `Application` identity.
- Tests prove wrong-instance and ambiguous ROT candidates are rejected.
- Excel-host validation proves an XLL loaded in one Excel process receives that
  process's `Application`, even when another Excel process is also present.
- Documentation states the identity contract and the remaining lifecycle caveats.
