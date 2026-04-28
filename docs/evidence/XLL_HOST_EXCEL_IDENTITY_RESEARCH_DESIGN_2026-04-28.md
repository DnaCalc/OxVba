# XLL Host Excel Identity Research And Design - 2026-04-28

## Finding

The existing XLL host-object binding path used `GetActiveObject("Excel.Application")`.
That is insufficient when multiple Excel processes are running because the ROT
selection is not evidence that the returned automation object belongs to the
Excel process hosting the loaded `.xll`.

## Identity Contract

The XLL shim must bind `Excel.Application` only when it can prove the object is
the host Excel instance:

1. Ask Excel for the host top-level window handle with `Excel12(xlGetHwnd, ...)`.
2. Resolve the host process ID from that window with `GetWindowThreadProcessId`.
3. Confirm that the host window belongs to the current process hosting the XLL.
4. Enumerate running COM objects from the ROT.
5. For each candidate dispatch object, read either `Hwnd` directly or
   `Application.Hwnd`.
6. Resolve the candidate HWND to a process ID.
7. Prefer a candidate whose `Application.Hwnd` exactly matches the host HWND.
   Multiple ROT monikers may resolve to the same host `Application`; these are
   duplicate evidence and are collapsed after retaining one dispatch pointer.
8. If no exact HWND candidate exists, accept a PID-only candidate only when it
   is unique. If no candidate exists, or PID-only candidates cannot be
   disambiguated, leave `Excel.Application` unbound and retry later.

## Fail-Closed Rules

- Plain `GetActiveObject("Excel.Application")` must not be used as sufficient
  authority for host injection.
- Ambiguous candidates are rejected. Duplicate ROT entries for the same exact
  host HWND are not ambiguous; they are collapsed.
- Failure to obtain host HWND/PID is diagnostic, not an XLL registration failure.
- Trace output must include enough identity data to diagnose mismatches.

## Implementation And Validation Evidence

- Implementation:
  - `crates/oxvba-build/src/xll.rs` generated XLL shim no longer uses
    `GetActiveObject`.
  - It obtains host identity with `xlGetHwnd` plus `GetWindowThreadProcessId`,
    enumerates the ROT with `GetRunningObjectTable` /
    `IRunningObjectTable::EnumRunning`, and binds only a matching
    `Excel.Application` dispatch.
  - Acquisition is lazy and retryable so early Excel lifecycle misses do not
    fail XLL registration.
- Fixture:
  - `examples/xll/application_addin/ApplicationExports.bas` now exports
    `ExcelVersion()` and `ExcelHwnd()`.
  - `ExcelHwnd()` is the identity-specific witness; `ExcelVersion()` alone is
    insufficient because host and decoy Excel processes can share the same
    version.
- Harness:
  - `scripts/run-xll-excel-application-identity-smoke.ps1` starts distinct host
    and decoy Excel COM instances, opens the XLL only in the host, evaluates the
    exported functions in the host workbook, and verifies both formula output
    and trace identity.
- Regression found during validation:
  - Run `20260428TIDENTITY01` failed because multiple ROT monikers resolved to
    the same host Excel `Application`, and the first implementation treated this
    as ambiguous.
  - The selector now collapses duplicate exact-HWND host candidates while
    preserving fail-closed behavior for competing PID-only matches.
- Passing evidence:
  - `target/xll-host-validation/excel-application-identity/20260428TIDENTITY02/identity_result.json`
    reports `status=passed`, `passed=7`, `failed=0`.
  - The run used host HWND `7347230` / PID `45772` and decoy HWND `68229344` /
    PID `44872`; the worksheet observed HWND was `7347230`.
  - The XLL trace includes the matching host candidate, the non-matching decoy
    candidate, and duplicate host candidate collapse.

## Checks

- `cargo test -p oxvba-build --lib xll --quiet` - passed.
- `cargo test -p oxvba-host --test xll_application_binding --quiet` - passed.
- `cargo test -p oxvba-project --quiet` - passed.
- `cargo check -p oxvba-host --quiet` - passed.
- `scripts/run-xll-excel-application-identity-smoke.ps1 ... -RunId 20260428TIDENTITY02`
  - passed.
- `scripts/run-xll-excel-worksheet-smoke.ps1 ... -RunId 20260428TIDENTITY02`
  - passed for the Application add-in worksheet smoke.

## Sources

- `xlGetHwnd`: <https://learn.microsoft.com/en-us/office/client-developer/excel/xlgethwnd>
- `Application.hWnd`: <https://learn.microsoft.com/en-us/office/vba/api/excel.application.hwnd>
- `GetWindowThreadProcessId`: <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid>
- `GetRunningObjectTable`: <https://learn.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-getrunningobjecttable>
- `IRunningObjectTable::EnumRunning`: <https://learn.microsoft.com/en-us/windows/desktop/api/objidl/nf-objidl-irunningobjecttable-enumrunning>
