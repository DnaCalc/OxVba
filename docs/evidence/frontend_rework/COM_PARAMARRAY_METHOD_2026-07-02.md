# COM ParamArray Method Evidence

Date: 2026-07-02
Bead: `bd-aprs.8.8.8` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Target Behavior

Microsoft's VBA documentation defines a `ParamArray` as a final `Variant()` parameter and
shows callers supplying a comma-separated tail of arguments. `Option Base` does not affect
ParamArray arrays; they are zero-based. The Win32 `FUNCDESC` documentation defines the COM
Automation representation used here: `cParamsOpt = -1` means the last parameter is a pointer
to a SAFEARRAY of Variants, and callers package any extra arguments into that final array.

Sources:
- `https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/understanding-parameter-arrays`
- `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/option-base-statement`
- `https://learn.microsoft.com/en-us/windows/win32/api/oaidl/ns-oaidl-funcdesc`

## Outcome

The M13 live COM matrix row is now executable instead of a documented fixture gap.

`OxVba.TestEventServer` exposes `SumParamArray(params object[] nums)` as a COM Automation
vararg member. The generated fixture type library carries `FUNCDESC::cParamsOpt = -1`,
which OxVBA now records as an explicit `OptionalParamDefault::ParamArray` marker on the
final SAFEARRAY(VARIANT) parameter.

Early-bound COM call binding boxes the positional tail into one zero-based array argument.
Late-bound `Object` calls keep their source arguments until runtime, then the resolved COM
member metadata packages the tail into the same SAFEARRAY(VARIANT) argument before
`IDispatch::Invoke`.

## Regression Shape

- The fixture interface declares `SumParamArray(params object[] nums)` with `DISPID 127`.
- Local TestEventServer type-library metadata reports a single Variant parameter with
  `TypeLibWireType::SafeArrayVariant` and `OptionalParamDefault::ParamArray`.
- Typed `Dim s As OxVba.TestEventServer : r = s.SumParamArray(1, 2, 3)` lowers to a
  descriptor-backed `EarlyCom` call with one zero-based `ArrayLiteral` tail argument after
  the receiver.
- Runtime canonicalization for metadata-known late-bound COM calls boxes multiple
  positional source arguments into one `ComValue::ArrayIntent(SafeArray::from_variants(...))`.
- The M13 matrix row verifies late-bound, early-bound PreferVtable, and early-bound
  DispatchOnly executions all leave `verdict = 42`, with the preferred early-bound route
  using exactly one vtable call.

## Checks

- `dotnet build tools\OxVba.TestEventServer\OxVba.TestEventServer.csproj -c Debug`
- `powershell -NoProfile -ExecutionPolicy Bypass -File tools\OxVba.TestEventServer\register.ps1 -ExportTypeLibOnly -SkipBuild -Configuration Debug`
- `powershell -NoProfile -ExecutionPolicy Bypass -File tools\OxVba.TestEventServer\register.ps1 -Configuration Debug -Scope CurrentUser -SkipBuild -SkipTypeLibExport`
- `cargo test -p oxvba-com testeventserver_typed_record_safearray_descriptors_carry_record_info -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip typed_com_paramarray_method_boxes_tail_to_zero_based_array -- --nocapture`
- `cargo test -p oxvba-com canonicalize_member_known_args_boxes_paramarray_tail -- --nocapture`
- `cargo test -p oxvba-host --test com_matrix_methods m12_test_event_server_byref_out_method_writes_back -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test com_matrix_methods m13_test_event_server_paramarray_method_sums_tail -- --ignored --exact --test-threads=1 --nocapture`
- `cargo clippy -p oxvba-com --tests -- -D warnings`
- `cargo clippy -p oxvba-bind --tests -- -D warnings`
- `cargo clippy -p oxvba-host --tests -- -D warnings`
- `cargo fmt --all --check`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`

## Boundary

This closes the bounded M13 TestEventServer ParamArray COM method row. It does not claim
complete ParamArray parity for every fixed-prefix, named-argument, omitted-tail, mutation, or
non-Automation vararg shape. Those remain residual compatibility work if accepted by a later
bead.

No legacy OxVBA behavior is the target here; the target is the documented VBA and COM
Automation behavior above.
