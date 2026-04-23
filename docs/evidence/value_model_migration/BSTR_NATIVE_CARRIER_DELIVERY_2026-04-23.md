# BSTR Native Carrier Delivery (2026-04-23)

Status: `vmm-d7` delivery evidence.

Scope:

1. `crates/oxvba-runtime/src/bstr.rs`
2. `crates/oxvba-runtime/src/variant.rs`
3. `crates/oxvba-runtime/src/safe_array.rs`
4. `crates/oxvba-runtime/src/pointer_helpers.rs`
5. `crates/oxvba-com/src/windows_variant.rs`

Implemented:

1. `BStr` is now a pointer-sized owner of a raw BSTR payload.
2. Windows builds allocate, clone, measure, and free the payload through
   `SysAllocStringLen`, `SysStringLen`, and `SysFreeString`.
3. Non-Windows builds keep the same length-prefix-plus-UTF-16-payload layout
   as a COM-compatible emulation for the cross-platform runtime.
4. `OwnedBStrCore` remains only as a snapshot/view compatibility surface. It is
   no longer the canonical internal string carrier.
5. `Variant`, `SafeArray`, pointer-helper, and COM string conversion paths now
   clone from the canonical `BStr` payload instead of maintaining separate
   local raw-BSTR allocators.

Validation:

1. `cargo test -p oxvba-runtime --lib`
2. `cargo test -p oxvba-com windows_variant::tests:: --lib`

Remaining scope:

1. The string/BSTR family still requires the `vmm-d8` intrinsic closure
   checklist before the epic can close.
2. The broader value-model migration remains open because the internal
   late-bound/general `Dim x` value is still not exactly Windows/COM `VARIANT`
   everywhere and array payloads are not yet comprehensively represented as
   real `SAFEARRAY*` union payloads.
