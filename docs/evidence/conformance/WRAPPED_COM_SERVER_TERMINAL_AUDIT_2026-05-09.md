# WrappedComServer terminal audit

Date: 2026-05-10
Bead: `bd-wcs1.10.2`
Workset: `docs/worksets/WORKSET_2026-05-09_WRAPPED_COM_SERVER_INTERFACE_EVENT_UDF_EXECUTION.md`
Matrix rows: `COM-0007`, `COM-0008`, `COM-0009`, `COM-0010`, `PH-0011`

## Audit result

The wrapped COM/server/UDF workset has an implemented subset with evidence for:

- WrappedComServer late-bound DLL publication and registered activation.
- Generated TypeLib publication and controlled TypeLib-aware client calls.
- One Automation-safe dual-interface vtable path with dispatch/vtable parity.
- Source dispinterface metadata and controlled connection-point event delivery.
- Host-call descriptor metadata, typed host UDF catalog enumeration, and scalar
  stable-ID host UDF invocation with caller/dependency/volatile context shape.
- OxIde/direct-host `WrappedComServer` build-plan/build-result/registration DTOs.
- OxIde/direct-host `WrappedComServer` executable build-result semantics:
  success now requires produced `.oxb`/`.dll`/`.tlb`/registration artifacts,
  with typed failed diagnostics for unsupported or failed build paths.
- Validation matrices, traceability, derived summaries, and governance aligned
  to the implemented subset.

## Evidence rollup

- `COM-0007`:
  `docs/evidence/conformance/WRAPPED_COM_SERVER_LATEBOUND_COM0007_2026-05-09.md`
- `COM-0008`:
  `docs/evidence/conformance/WRAPPED_COM_SERVER_TYPELIB_COM0008_2026-05-09.md`
- `COM-0009`:
  `docs/evidence/conformance/WRAPPED_COM_SERVER_DUAL_INTERFACE_COM0009_2026-05-09.md`
- `COM-0010`:
  `docs/evidence/conformance/WRAPPED_COM_SERVER_EVENTS_COM0010_2026-05-09.md`
  and
  `docs/evidence/conformance/oracle_captures/wrapped_com_events_20260509T000000Z/summary.md`
- `PH-0011` descriptor slice:
  `docs/evidence/conformance/WRAPPED_COM_SERVER_HOST_UDF_PH0011_2026-05-09.md`
- `PH-0011` catalog/invoke slice:
  `docs/evidence/conformance/WRAPPED_COM_SERVER_HOST_UDF_CATALOG_INVOKE_PH0011_2026-05-09.md`
- OxIde/direct-host DTO slice:
  `docs/evidence/conformance/WRAPPED_COM_SERVER_OXIDE_BUILD_DTOS_2026-05-09.md`
- Validation refresh:
  `docs/evidence/conformance/WRAPPED_COM_SERVER_VALIDATION_TRACEABILITY_REFRESH_2026-05-09.md`

## Terminal commands

```powershell
cargo test -p oxvba-build generate_typelib --quiet
cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet
cargo test -p oxvba-host --test invoke_procedure_tests host_udf --quiet
cargo test -p oxvba-host wrapped_com_server_build_plan_reports_artifacts_and_registration_dtos --quiet
cargo test -p oxvba-host wrapped_com_server_build_workspace_requires_disk_only_source_policy --quiet
cargo test -p oxvba-host embedded::tests --quiet
./scripts/generate-validation-derived-summaries.ps1
./scripts/check-governance.ps1
```

## Terminal command results

- `cargo test -p oxvba-build generate_typelib --quiet`: passed, 5 tests.
- `cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet`:
  passed, 1 test, 164.39s.
- `cargo test -p oxvba-host --test invoke_procedure_tests host_udf --quiet`:
  passed, 3 tests.
- `cargo test -p oxvba-host wrapped_com_server_build_plan_reports_artifacts_and_registration_dtos --quiet`:
  passed, 1 focused test (includes live WrappedComServer artifact build).
- `cargo test -p oxvba-host wrapped_com_server_build_workspace_requires_disk_only_source_policy --quiet`:
  passed, 1 focused test.
- `cargo test -p oxvba-host embedded::tests --quiet`:
  passed, 16 focused embedded-host tests.
- `./scripts/generate-validation-derived-summaries.ps1`: regenerated latest
  validation summary.
- `./scripts/check-governance.ps1`: passed, including validation ownership,
  bead traceability, closure taxonomy, and validation-derived summary checks.

## Residuals

- Superseded 2026-06-17: clean `COM-0008` evidence now covers Office/VBA
  project-reference dispatch-interface calls for method, property, object
  return, array return, and external Automation error behavior.
- Superseded 2026-06-17: clean `COM-0010` evidence now covers Office/VBA
  `WithEvents` client proof and bounded connection enumerators. Multi-event
  selection and richer event payload ordering remain outside the current
  `COM-0010` implemented subset.
- Broader dual-interface argument/property/byref/object/array/error vtable
  parity remains outside `COM-0009`.
- Host UDF richer scalar coercion, array returns, error returns, explicit
  worksheet volatile/dependency semantics, and DnaOneCalc/OxIde host-context
  harness evidence remain open under `PH-0011`, which therefore remains
  `in-progress`.
- 2026-05-10 host/UDF design correction: the UDF context, descriptor source of
  truth, and function-vs-Sub descriptor questions are moved to
  `docs/worksets/WORKSET_2026-05-10_HOST_PROGRAM_DESIGN_AND_UDF_REWORK.md`
  (`bd-sg5h`) rather than treated as complete WrappedComServer closure work.

## Governance stance

The workset terminal audit is an implemented-subset audit; it is not a claim of
full Office/VBA or arbitrary-host parity. The validation rows deliberately
retain `implemented-subset` or `in-progress` statuses where broader behavior
remains unproved.
