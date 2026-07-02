# COM ByRef Method Writeback Evidence

Date: 2026-07-02
Bead: `bd-aprs.8.8.7` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Outcome

The M12 live COM matrix row is now executable instead of a documented gap for the
repo-owned `OxVba.TestEventServer` fixture.

`OxVba.TestEventServer` exposes `Increment(ByRef value As Long)` in the generated
fixture type library. Typed COM calls and late-bound `Object` calls now preserve an
unparenthesized l-value argument as a runtime `ByRef` slot, and the Windows COM
bridge returns the changed Long value through the runtime writeback carrier.

This is a VBA compatibility target, not a preservation of historical OxVBA behavior.
The accepted behavior is that `s.Increment n` writes the incremented value back into
`n` for both early-bound and late-bound COM calls.

## Regression Shape

- The fixture interface declares `Increment(ByRef value As Long)` with `DISPID 126`.
- Local TestEventServer type-library metadata reports the method as `ByRefLong` with
  dual-interface vtable evidence.
- Typed `Dim s As OxVba.TestEventServer : s.Increment n` binds to descriptor-backed
  `EarlyCom` dispatch and lowers `n` as `CoreArg::ByRef`.
- Late `Dim s As Object : s.Increment n` lowers the unparenthesized l-value argument
  as `CoreArg::ByRef` instead of a ByVal temporary.
- `DynamicCallArg` preserves `RuntimeByRefSlot` through the COM request carrier.
- The vtable path and the member-metadata-backed `IDispatch::Invoke` path both return
  `RuntimeCallResult` writebacks that the standard host maps back to the caller slot.
- The M12 matrix row verifies late-bound, early-bound PreferVtable, and early-bound
  DispatchOnly executions all leave `verdict = 42`, with the preferred early-bound
  route using exactly one vtable call.

## Checks

- `cargo test -p oxvba-bind --test bind_roundtrip typed_com_receiver_member_call_lowers_to_early_com -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip typed_com_byref_method_lowers_lvalue_arg_to_byref -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip late_com_method_lowers_lvalue_arg_to_byref -- --nocapture`
- `cargo test -p oxvba-com dynamic_call_arg_roundtrip_preserves_byref_slot -- --nocapture`
- `cargo test -p oxvba-com windows_vtable::tests::byref_long_writeback_is_returned_from_writeback_capable_path -- --nocapture`
- `cargo test -p oxvba-com testeventserver_typed_record_safearray_descriptors_carry_record_info -- --nocapture`
- `cargo test -p oxvba-host --test com_matrix_methods m12_test_event_server_byref_out_method_writes_back -- --ignored --exact --test-threads=1 --nocapture`
- `cargo clippy -p oxvba-com --tests -- -D warnings`
- `cargo clippy -p oxvba-bind --tests -- -D warnings`
- `cargo clippy -p oxvba-hal --tests -- -D warnings`
- `cargo clippy -p oxvba-host --tests -- -D warnings`
- `cargo fmt --all --check`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`

## Boundary

This closes the bounded M12 ByRef Long COM method writeback row for the registered
TestEventServer fixture. It does not close ParamArray COM method parity, named
late-bound ByRef argument parity, every non-Long ByRef wire shape, direct-DISPID
ByRef fallback coverage, or imported `New OxVba.TestEventServer` activation.

Those remaining areas are residual compatibility work, not legacy behavior to keep.
