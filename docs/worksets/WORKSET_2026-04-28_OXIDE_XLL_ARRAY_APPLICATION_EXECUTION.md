# Workset: OxIde Direct Seams And XLL Array/Application Expansion

Date: 2026-04-28
Parent bead: `bd-iyx4`

## Intent

Execute the requested structured lane for three related but separable outcomes:

1. prove the OxVba side of direct OxIde Immediate Window and debug consumption,
2. expand generated XLL support from scalar-only worksheet functions to arrays,
3. add a safe path for XLL-hosted VBA projects to receive Excel `Application`
   as an injected host root object.

This workset is intentionally staged. The OxIde-facing proof is first because it
is already mostly present in OxVba and should become a clean evidence surface
before the XLL work broadens. XLL arrays are second because they extend the
existing scalar XLL lane. Excel `Application` injection is third because it has
host lifecycle, COM, and reentrancy risks.

## Current Findings

### OxIde Immediate/Debug

Existing OxVba surfaces:

- `oxvba_host::ImmediateSession` evaluates against a live
  `ProjectRuntimeSession`.
- `ImmediateSession::evaluate_variant` and `snapshot_variants` expose retained
  `Variant` carriers before legacy projection.
- `oxvba_host::DebugSession` exposes direct VM-backed debug controls and
  retained `Variant` pause/evaluation surfaces.
- `EmbeddedBuildRunHost` provides a direct embedded host facade, but it does not
  yet carry a combined Immediate/debug host-consumption proof.

Gap being closed first:

- prior evidence explicitly said full direct OxIde consumption of Immediate
  Window/debug seams was not yet proven.

### XLL Arrays

Existing XLL support:

- generated exports register the XLOPER12 pointer ABI through `Q` lanes,
- scalar numeric, string, boolean, integer, date/currency/error conversions
  exist,
- Excel-loaded scalar worksheet invocation is proven.

Required next shape:

- decode `xltypeMulti` arguments into retained `Variant::ArrayVariant`
  / `SafeArray`,
- return retained arrays as owned `xltypeMulti`,
- prove nested owned string storage and array buffers are freed by
  `xlAutoFree12`,
- validate with both generated-source/unit tests and an Excel worksheet fixture.

### XLL Excel Application

Public-source constraints for design:

- Microsoft documents XLL registration and direct worksheet/VBA access through
  `xlfRegister` / registered XLL functions in "Accessing XLL code in Excel":
  <https://learn.microsoft.com/en-us/office/client-developer/excel/accessing-xll-code-in-excel>
- Microsoft documents XLL callbacks through `Excel4`, `Excel4v`, `Excel12`,
  and `Excel12v`, and warns that callback failures such as abort/uncalculated
  should return control to Excel before further callbacks in "Calling into
  Excel from the DLL or XLL":
  <https://learn.microsoft.com/en-us/office/client-developer/excel/calling-into-excel-from-the-dll-or-xll>
- Microsoft documents XLL creation and `xlAutoOpen` as the place for
  registration/initialization, and `xlAutoFree12` as Excel's callback for
  XLL-owned return memory, in "Creating XLLs":
  <https://learn.microsoft.com/en-us/office/client-developer/excel/creating-xlls>
- Microsoft documents `GetActiveObject` as retrieving a running object
  registered with OLE:
  <https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-getactiveobject>
- Microsoft VBA documentation shows `GetObject(, "Excel.Application")` as the
  VBA-facing way to connect to a running Excel application object:
  <https://learn.microsoft.com/en-us/office/vba/Language/Reference/user-interface-help/getobject-function>
- Microsoft documents unsupported direct asynchronous Object Model programming
  and recommends serialization/idle-aware handling:
  <https://learn.microsoft.com/en-us/previous-versions/office/troubleshoot/office-developer/asynchronous-programming-to-object-model>

Design implication:

- the XLL shim must not assume `Application` is always safely available during
  `xlAutoOpen`;
- acquisition should be lazy/retryable and produce explicit diagnostics when
  unavailable;
- object-model calls must stay on safe Excel-owned execution paths and avoid
  background-thread/asynchronous access.

Current implementation status:

- research evidence: `docs/evidence/XLL_EXCEL_APPLICATION_ACQUISITION_RESEARCH_2026-04-28.md`;
- design evidence: `docs/evidence/XLL_EXCEL_APPLICATION_BINDING_DESIGN_2026-04-28.md`;
- generated XLL shims now lazily call `GetActiveObject("Excel.Application")`
  during first session creation, bind the retained `IDispatch` into the
  engine COM state under `Excel.Application`, and trace unavailable/failure
  cases without failing `xlAutoOpen`;
- host COM state now has a dedicated host-object-by-ProgID lane so
  `CreateObject("Excel.Application")` can return the injected running Excel
  object for the same engine;
- Excel worksheet validation with the updated generated XLL passed and the XLL
  trace recorded `Excel.Application host root bound object=20001`;
- `.basproj` project references now support `<Kind>HostInjected</Kind>`, which
  lets CLI Addin projects express a host-injected reference surface;
- the Application fixture validates `Application.Value` in Excel:
  `=ExcelVersion()` returned `16.0` from the injected running Excel root;
- the current project-side supported pattern is a host-injected root module
  exposing `Application.Value` via `CreateObject("Excel.Application")`;
  arbitrary direct `Application.Member` requires a corresponding compiled host
  surface and is not claimed by this implementation pass.

## Bead Tree

Parent:

- `bd-iyx4` - structured OxIde/XLL expansion execution

OxIde direct seam proof:

- `bd-iyx4.1` - prove direct OxIde Immediate and debug seam consumption
- `bd-iyx4.1.1` - audit OxIde direct Immediate/debug seam evidence
- `bd-iyx4.1.2` - add direct OxIde-style Immediate/debug host tests
- `bd-iyx4.1.3` - publish OxIde direct seam evidence and residuals

XLL arrays:

- `bd-iyx4.2` - add comprehensive XLL array support
- `bd-iyx4.2.1` - design and matrix XLL XLOPER12 array support
- `bd-iyx4.2.2` - implement XLL array argument unmarshalling
- `bd-iyx4.2.3` - implement XLL array return marshaling and cleanup
- `bd-iyx4.2.4` - validate XLL array fixture in Excel

XLL Excel `Application` injection:

- `bd-iyx4.3` - inject Excel Application object into XLL-hosted VBA
- `bd-iyx4.3.1` - research Excel Application acquisition constraints for XLL
- `bd-iyx4.3.2` - design XLL host Application injection contract
- `bd-iyx4.3.3` - implement XLL Application acquisition and root injection
- `bd-iyx4.3.4` - validate XLL Application object in Excel host
- `bd-iyx4.3.5` - add first-class Excel Application.Value XLL consumption
  fixture

## Execution Order

1. Close the OxIde audit/proof/evidence chain.
2. Design and implement XLL array support.
3. Validate XLL arrays in Excel.
4. Research, design, implement, and validate Excel `Application` injection.

## Boundary

This workset does not claim full OxIde UI delivery; it prepares and proves the
OxVba-side host seams. XLL arrays do not imply async/RTD or macro command
support. Excel `Application` injection is Windows/Excel-host scoped unless a
later bead explicitly widens the platform claim.
