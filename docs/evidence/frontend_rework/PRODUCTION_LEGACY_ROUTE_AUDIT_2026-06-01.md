# Production Legacy Route Audit Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.6`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_legacy_route_audit.rs`, an executable audit report for
the FE-9 terminal route gate.

Current bounded recorded-fixture route audit result: **passed for the fixture set in this file**.
This is not terminal workset closure. The 2026-06-02 workset rework adds `bd-aprs.10.7` for the
broader accepted grammar matrix, compiler fixture corpus, host project corpus, language-service
corpus, and selected Excel oracle route audit before terminal closure.

The audit proves the good path and exposes the remaining production residuals:

- scoped procedure/local/assignment/arithmetic fixtures classify as `HirProduction`;
- simple same-module procedure call statement fixtures now reach `HirProduction`; the remaining
  call/coercion seed-row delta is a documented source-map metadata improvement, not a syntax-route
  residual or bytecode/call-descriptor bug;
- same-module statement-form procedure calls with bare arguments now reach `HirProduction`;
- statement-form member calls with bare arguments now reach `HirProduction`;
- no-keyword statement-form `DispatchInvoke` host intrinsic calls with named arguments now reach
  `HirProduction`;
- simple multiline `If ... Then ... End If` fixtures now reach `HirProduction`;
- multiline `If ... Else ... End If` and `If ... ElseIf ... Else ... End If` fixtures now reach
  `HirProduction`;
- single-line `If ... Then ... Else ...` fixtures now reach `HirProduction`;
- simple front-checked `Do While ... Loop`, `Do Until`, and post-check loop fixtures now reach
  `HirProduction`;
- `While`/`Wend` fixtures now reach `HirProduction`;
- simple `For` range fixtures now reach `HirProduction`;
- simple single-value `Select Case` fixtures now reach `HirProduction`;
- `Select Case` range fixtures now reach `HirProduction`;
- multi-value `Select Case` fixtures now reach `HirProduction`;
- `Select Case Is` fixtures now reach `HirProduction`;
- `For Each` fixtures now reach `HirProduction`;
- `Exit Do`, `Exit For`, and `Exit Sub` fixtures now reach `HirProduction`;
- basic non-label `On Error` and `Resume` fixtures now reach `HirProduction`;
- label-targeted `On Error GoTo` and `Resume` fixtures now reach `HirProduction`;
- identifier and numeric-label `GoTo` fixtures now reach `HirProduction`;
- `GoSub` / `Return` fixtures now reach `HirProduction`;
- `Erase` fixtures now reach `HirProduction`;
- fixed and dynamic UDT array-field index fixtures now reach `HirProduction`;
- `Event` declaration plus `RaiseEvent` fixtures now reach `HirProduction`;
- single-source `Implements` directive fixtures now reach `HirProduction`;
- explicit-receiver value-side dot-member read/call fixtures now reach `HirProduction`;
- simple explicit-receiver member assignment target fixtures now reach `HirProduction`;
- bang member assignment target fixtures now reach `HirProduction`;
- statement-form member calls with bare arguments now reach `HirProduction`;
- read-side `With` member fixtures now reach `HirProduction`;
- the active-project `Set obj = New Widget` construction route now has an executable project
  compile audit: it consumes the HIR `New` binding, preserves `Set obj = New Widget` in the
  compiled source artifact, emits `LoadProjectObjectRef`, retains dynamic object metadata, and
  does not leave `__oxvba_project_instance(...)` helper source in the compiled artifact;
- the active-project `WithEvents` field assignment route now has a separate executable project
  compile audit: `Set src = New Emitter` materializes a generated temporary, restores that temporary
  to explicit `New Emitter` HIR source, routes the event setter through the temporary, retains
  dynamic object metadata, and does not leave `__oxvba_project_instance(...)` helper source in the
  compiled artifact;
- `oxvba-languageservice` now uses compiler query/HIR facts for symbols, callable signatures,
  diagnostics, signature help, and the PtrSafe quick-fix diagnostic; `semantic.rs` no longer builds
  a legacy `BoundModule` fallback when HIR binding is unavailable.

Continuation update: the FE-9.7 seed-corpus route audit now includes a second imported typelib
projection row beyond the controlled `OxVba.TestDispatch` fixture. The inline
`Scripting.Dictionary` early-bound project (`Dim obj As New Scripting.Dictionary`;
`countValue = obj.Count()`) compiles through the HIR production project boundary with a
`ReferenceKind::TypeLibrary` reference to `Scripting`, so imported COM route evidence now covers
both the OxVba-controlled typelib and a known external/registered ProgID projection shape. This is
still route evidence, not live COM execution or full Office/versioned/broken-reference parity.

Continuation update: the predeclared document route audit now covers a public document method in
addition to the original `ThisWorkbook.Path` property-get seed. The inline `ThisWorkbook.FullName()`
project compiles through the HIR production project boundary, rewrites the backend call to the
normal `pmr_hostworkbook_thisworkbook_fullname()` function symbol, and retains the matching runtime
metadata entry. The compiler compatibility rewrite was generalized from predeclared property reads
to predeclared member reads so public predeclared functions can use the same PMR-backed route.
This is still synthetic document-module route evidence; live Excel object model behavior remains
separate oracle work.

Continuation update: the selected Excel oracle source route now has a matching ignored host
execution lane. `excel_application_activation_smoke.bas` was narrowed to the behavior actually
proved live here: `CreateObject("Excel.Application")`, root `Visible` property get, and explicit
`Quit` cleanup. The command
`cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_application_activation_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`
passed on this machine. The previous property-set shape is not claimed by this evidence; richer
Excel property-put, range/default-member, and member-mutation lanes were still open at this point
in the chronology. Later COM/Excel oracle follow-ups closed scoped live lanes for
`DispatchInvoke(sheet, "Range", "A1")`, `Worksheets.Add After:=sheet`, null `Cells.Find` results,
`Range("A1").Value` property-put, and indexed `Range("A1")(1)` default-member mutation; broader
Excel object-model parity remains open.

Continuation update: the Excel oracle lane now includes a second source-backed fixture and ignored
host execution test for workbook/range-object automation. The seed corpus route audit includes
`excel_oracle_workbook_range_smoke`, and the narrowed
`conformance/com/office/excel/excel_workbook_range_smoke.bas` fixture classifies as
`HirProduction` and passed live execution with:
`cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_workbook_range_object_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`.
The first broader attempt exposed two real gaps and the fixture was narrowed accordingly:
statement-form `DispatchInvoke(app, "DisplayAlerts", False)` was not accepted by HIR as an
expression-statement shape at that point, and live Excel `Range("A1")` dispatch failed through the
current `DispatchInvoke` adapter with `HAL-E-ADAPTER-FAULT [dispatch_invoke]
com-dispatch-arg-error`. Follow-up probing showed the native member syntax `sheet.Range("A1")`
does execute live, so the fixture now proves application activation, `Workbooks`,
`Workbooks.Add`, `Worksheets(1)`, native `Range("A1")` object access, `Close`, and `Quit`; it does
not claim range value/default-member mutation, property-put, or Excel mutation parity.

Continuation update: the compiler-side no-keyword statement-form `DispatchInvoke` residual from the
Excel oracle broadening attempt is now closed. HIR production lowering accepts
no-keyword `StructuralIntrinsicCallWithArgs` as an expression statement, the regression
`hir_production_lowering_accepts_statement_form_dispatchinvoke_arguments` proves named dispatch
arguments survive to `IntrinsicDispatchInvokeHost`, and the production route audit includes a
no-keyword statement-form named `DispatchInvoke` fixture. `Call DispatchInvoke(...)` remains on the
compatibility route where project/imported-COM rewrites need to attach early-bound COM metadata.
That compiler-side fix did not by itself reclassify the explicit
`DispatchInvoke(sheet, "Range", "A1")` adapter fault.

Continuation update: the explicit live Excel Range `DispatchInvoke` adapter fault is now closed for
range object access. The COM dynamic-name bridge still tries the OLE Automation combined
method/property-get get-or-call dispatch first, then retries `DISPATCH_PROPERTYGET` for strict
parameterized properties such as Excel `Range("A1")`; if the retry also fails, diagnostics preserve
the original combined-dispatch failure. The new
`conformance/com/office/excel/excel_dispatchinvoke_range_smoke.bas` fixture proves `Workbooks`,
`Workbooks.Add`, `Worksheets(1)`, and `DispatchInvoke(sheet, "Range", "A1")` through live Excel
execution with:
`cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_dispatchinvoke_range_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`.
This does not claim range value/default-member mutation, property-put, or broader Excel object-model
parity.

Continuation update: the Excel named-argument oracle fixture is now executable and live-proven for a
non-null object result. The previous `sheet(What:=...)` shape did not name the intended Excel member;
the corrected fixture uses explicit `Worksheets.Add After:=sheet` through `DispatchInvoke`, proving
named argument DISPID resolution and object-argument marshaling against live Excel with:
`cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_named_argument_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`.
The initial `Cells.Find(What:=...)` probe returned Excel `Nothing` and exposed a separate null COM
result handling gap (`unknown COM object handle 0`); follow-up COM runtime-state work now
short-circuits null native runtime-object results to `ObjectRef(0)` instead of resolving them as
registered COM bindings. The dedicated
`conformance/com/office/excel/excel_find_null_result_smoke.bas` fixture proves the no-match
`Cells.Find` case through live Excel execution with:
`cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_find_null_result_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`.

Continuation update: scoped late-bound Excel property-put is now live-proven for `Range.Value`.
Dynamic name dispatch now honors `PropertyLet`/`PropertySet` call-kind hints by invoking
`DISPATCH_PROPERTYPUT`/`DISPATCH_PROPERTYPUTREF` with `DISPID_PROPERTYPUT` instead of the read-side
get-or-call path. The new `conformance/com/office/excel/excel_range_value_put_smoke.bas` fixture
writes `cell.Value = "needle"` and observes the mutation through named-argument `Cells.Find` with:
`cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_range_value_put_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`.
This does not claim default-member mutation or broader Excel object-model mutation parity.

Continuation update: scoped indexed Excel range default-member mutation is now live-proven. The
direct-DISPID fallback in `execute_bound_variant` now honors `PropertyPut`/`PropertyPutRef`
assignment hints instead of falling back to read-side get-or-call for trusted DISPID traffic. The new
`conformance/com/office/excel/excel_range_default_member_put_smoke.bas` fixture writes
`cell(1) = "needle"` and observes the mutation through named-argument `Cells.Find` with:
`cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_range_default_member_put_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`.
This does not claim every Excel default-member shape or broader object-model parity.

Continuation update: FE-9.8 bundle context facts now have source-backed seed-corpus proof. The
test `bundle_fact_bound_module_route_uses_hir_for_source_backed_frontend_seed_rows` walks the
frontend rework seed corpus and asserts every row with inline source produces package/module facts
through the HIR `BoundModule` route, not the `resolve_symbols` fallback. The fallback remains
quarantined for unsupported residual modules; it is not counted as HIR evidence.

Continuation update: FE-9.8 default compile fallback quarantine now has source-backed seed-corpus
proof. The test
`compile_options_default_uses_frontend_v2_for_source_backed_frontend_seed_rows` walks the frontend
rework seed corpus and asserts every row with inline source compiles under strict `frontend_v2` and
the default compiler with matching bytecode. Unsupported residual fallback remains available for
out-of-scope modules, but accepted source-backed seed rows no longer depend on it.

This audit no longer finds the previously tracked scoped production route residuals. The broader
workset still remains open for unaudited language surfaces and full terminal evidence, but this
specific FE-9.6 route audit now passes for its recorded fixtures and static checks.

Continuation update: the audit's successful fixture routes are now aligned with the ordinary
lightweight compile API as well as the explicit frontend-v2 bridge. `compile()` and
`compile_with_runtime_metadata()` try HIR production first for eligible completed constructs; the
legacy resolver path is now an explicit comparison helper plus unsupported-residual fallback rather
than the first path for completed single-source fixtures.

2026-06-02 correction: the audit no longer records a broad static pass for the whole `project.rs`
source-text rewrite bridge. That was too coarse for the reopened workset. The project entries are
now executable, scoped active-project construction checks for the direct `Set obj = New Class` and
WithEvents temporary-construction routes. Broader project rewrite retirement remains open under
FE-7/FE-8/FE-9 until each accepted project/class/default-member/COM route is either owned by
frontend/HIR facts or explicitly compatibility-quarantined outside the accepted production surface.

## Reopened Owners

The audit result records completed reopened delivery work and remaining broader workset scope:

- `bd-aprs.5.4` (`FE-4.4 CST-to-legacy bridge`) was reopened and then narrowed: the hidden bridge
  fallback was removed, so remaining unsupported constructs are owned by HIR/project delivery beads
  and outer route policy rather than the bridge itself;
- `bd-aprs.8.1` through `bd-aprs.8.6` (`FE-7.*`) and `bd-aprs.9.6`/`bd-aprs.9.7` have moved the
  accepted active-project construction and WithEvents construction subsets onto HIR route evidence,
  but still own broader replacement or quarantine of source-text lowering internals where those
  internals remain compatibility scaffolding;
- `bd-aprs.9.5` (`FE-8.5 Production HIR lowering`) for expanding production HIR lowering beyond
  the initial subset; the route audit fixtures now cover procedure calls and representative
  control-flow families through HIR production;
- `bd-aprs.10.4` (`FE-9.4 Language-service reconciliation`) retired the remaining internal
  language-service `BoundModule` fallback/diagnostic compatibility surface.

## Checks

- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
- `cargo test -p oxvba-compiler bundle_fact_bound_module_route_uses_hir_for_source_backed_frontend_seed_rows --quiet`
- `cargo test -p oxvba-compiler compile_options_default_uses_frontend_v2_for_source_backed_frontend_seed_rows --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_uses_hir_for_completed_constructs --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_statement_form_dispatchinvoke_arguments --quiet`
- `cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_application_activation_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_workbook_range_object_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_dispatchinvoke_range_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_named_argument_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_find_null_result_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_range_value_put_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test excel_office_oracle_lane windows_excel_office_oracle_lane::excel_range_default_member_put_smoke_fixture_executes_when_available -- --ignored --exact --test-threads=1 --nocapture`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The FE-9.6 audit fixture set now passes, but the workset goal is broader than that subset.
- The audit previously proved HIR reachability but not the plain `compile()` entry point. The
  lightweight compile path now has an executable route check for completed constructs. Direct and
  WithEvents active-project construction now also have executable project-compile route evidence,
  while broader project rewrites and unsupported surfaces remain separate workset scope.
- Procedure-call syntax, including same-module statement-form procedure calls with bare arguments,
  multiline and single-line If/ElseIf syntax, front-checked Do While syntax, basic Exit and
  error-control statements, identifier/numeric-label `GoTo`, `GoSub` / `Return`, `Erase`,
  one-dimensional, two-dimensional, and explicit static lower-bound dynamic-array runtime `ReDim`,
  read/write dynamic-array element access, initial fixed-array element aliasing and fixed-array
  `ReDim` alias rematerialization, local multidimensional dynamic/fixed element access, fixed and
  dynamic UDT array-field indexed read/write access,
  simple function declarations with declared return
  slots, and simple single-value Select Case syntax, plus basic `RaiseEvent` and single- or
  multi-declarator literal `Const`, `Event` declarations paired with `RaiseEvent`, single-source
  `Implements` directives, explicit-receiver value-side dot-member read/call syntax, statement-form
  member calls with bare arguments, no-keyword statement-form `DispatchInvoke` host intrinsic calls
  with named arguments, simple dot/bang member assignment targets, and read-side `With` member
  syntax are no longer themselves route blockers. The call/coercion fixture now has matching
  bytecode/call descriptors. FE-8.5 still owns broader HIR lowering coverage for language surfaces
  outside this route-audited subset, but the audited fixtures in this file now classify as
  `HirProduction`.
- The next step is broader terminal evidence and expansion of the route-audit fixture set, plus
  deletion or compatibility quarantine of remaining project rewrite carriers, not claiming complete
  compiler front-end replacement from this audit alone.
- Imported COM route evidence now covers both `OxVba.TestDispatch` and `Scripting.Dictionary`
  projected typelib references through HIR production. Broader imported COM work remains open for
  live COM execution, Office typelib/version/broken-reference behavior, richer member/property
  families, and any residual helper-source rewrite paths that are not yet owned by front-end facts.
- Predeclared document route evidence now covers both `ThisWorkbook.Path` property reads and
  `ThisWorkbook.FullName()` public method calls through HIR production. Broader document/host work
  remains open for real host object-model semantics and live Excel oracle execution.
- The selected live Excel execution lane now proves activation, root property get, and cleanup. It
  also exposed that property-set behavior should not be claimed by the activation smoke; broader
  Excel object-model mutation remains a separate FE-7/FE-8/FE-9 delivery and oracle surface.
- The follow-up live Excel workbook/range-object lane now proves `Workbooks`, `Workbooks.Add`,
  `Worksheets(1)`, native `Range("A1")` object access, close, and cleanup. It also exposed that
  no-keyword statement-form `DispatchInvoke` and explicit `DispatchInvoke(sheet, "Range", "A1")`
  were not ready to be claimed at first. The compiler-side no-keyword statement-form
  `DispatchInvoke` gap is now route-audited through HIR production, while `Call DispatchInvoke(...)`
  remains on the compatibility route for early-bound COM metadata. The explicit Range
  `DispatchInvoke` object-access gap is now live-proven through the new Excel fixture after the COM
  bridge property-get retry. Named-argument Excel dispatch is now live-proven through
  `Worksheets.Add After:=sheet`; null COM object result handling is now live-proven through the
  no-match `Cells.Find` fixture. Scoped `Range.Value` property-put is now live-proven through
  dynamic-name `PropertyLet`; scoped indexed `Range("A1")(1)` default-member mutation is now
  live-proven through direct DISPID `PropertyLet`. Broader Excel object-model parity remains open.
- Bundle context fact extraction is now proved HIR-backed for every source-backed FE-9.7 seed row.
  The legacy resolver fallback remains a quarantined residual for unsupported modules, not an
  accepted-row production route.
- Default compile fallback is now proved unused for every source-backed FE-9.7 seed row: strict
  frontend-v2 and default compile bytecode match. The fallback remains a quarantined residual only
  for unsupported/out-of-scope modules.
- Unused host-injected reference declarations no longer force `FullLegacy`: an otherwise active
  procedural project with a declared `HostInjected` `Application` reference and no source mention
  now routes through `ActiveHir`/`HirProduction`. A matching guard keeps projects that actually name
  `Application` on the existing host rewrite/compatibility path until host object-model binding is
  structurally owned by FE-7.6.
