# XLL Excel Application Acquisition Research - 2026-04-28

## Scope

This note records the acquisition constraints for exposing Excel's root `Application` object to an OxVba XLL-hosted project.

## Findings

- Excel may load an XLL early enough that host object availability should not be treated as a hard `xlAutoOpen` prerequisite. The shim must allow registration to continue if `Application` cannot yet be acquired.
- The XLL should not manufacture a new Excel instance to satisfy the host root. The correct target is the running Excel instance that loaded the add-in.
- `GetActiveObject("Excel.Application")` is the right first acquisition seam for the current Windows XLL lane because it asks the Running Object Table for a live Excel automation object instead of calling `CoCreateInstance`.
- Office object model access must remain on the appropriate foreground STA lane. The implementation must not acquire Excel on a background thread and then invoke the object model asynchronously from that worker.
- Acquisition is best done lazily during session creation or first invocation, not as a brittle `xlAutoOpen` fatal path. Failure should be diagnostic trace, not add-in load failure.

## Implementation Direction

The current implementation follows this shape:

1. Generated XLL source calls a lazy `try_bind_excel_application_root(&Engine)` when its thread-local runtime session is first created.
2. On Windows, the shim initializes or joins the COM apartment, resolves `Excel.Application` to a CLSID, calls `GetActiveObject`, queries the result to `IDispatch`, and releases the intermediate `IUnknown`.
3. The engine exposes a narrow native object binding method that consumes the retained `IDispatch` pointer and binds it into the host COM state under `Excel.Application`.
4. The COM host adapter returns that bound object for subsequent `CreateObject("Excel.Application")` calls on that engine, which gives host-injected modules a concrete way to surface the Excel root without placeholders.

## Boundaries

- This does not claim full direct `Application.Member` syntax for arbitrary Excel members without a project host surface. Existing host-injected project binding still controls what source names are visible.
- The supported project-side bridge for this bead is a host-injected root module that returns `CreateObject("Excel.Application")`, for example `Application.Value`.
- Non-Windows XLL builds trace unavailability and continue.
- If Excel is not in the Running Object Table yet, the add-in still loads and traces the failure.

## Sources

- Microsoft XLL lifecycle and `xlAutoOpen`/`xlAutoFree12`: https://learn.microsoft.com/en-us/office/client-developer/excel/creating-xlls
- Microsoft Excel callback guidance for XLLs: https://learn.microsoft.com/en-us/office/client-developer/excel/calling-into-excel-from-the-dll-or-xll
- Microsoft `GetActiveObject`: https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-getactiveobject
- Microsoft Office object model asynchronous access cautions: https://learn.microsoft.com/en-us/previous-versions/office/troubleshoot/office-developer/asynchronous-programming-to-object-model
