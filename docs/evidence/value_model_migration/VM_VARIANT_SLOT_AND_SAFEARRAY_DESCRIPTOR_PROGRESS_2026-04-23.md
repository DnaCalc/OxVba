# VM Variant Slot And SAFEARRAY Descriptor Progress (2026-04-23)

Status: `vmm-e6` progress evidence, not closure.

Implemented:

1. `crates/oxvba-vm/src/register_file.rs` no longer stores VM registers as
   `Vec<RuntimeValue>`.
2. VM registers now store `RuntimeSlot::Variant(Variant)` for normal VBA
   values, with `RuntimeSlot::BindingHandle` retained only for the explicitly
   non-VBA internal token lane.
3. Interpreter read/write/snapshot helpers project through `RuntimeSlot`, so
   existing semantic helper code can continue to operate while the actual slot
   storage for general values is a `Variant`.
4. Fast scalar slot helpers now update `RuntimeSlot` values directly rather
   than writing `RuntimeValue` into the register file.
5. `ReDim` now creates typed `SAFEARRAY` payloads using the declared runtime
   array element type instead of normalizing every resized array to
   `VT_VARIANT`.
6. `SafeArray::bounds()` now exposes one-dimensional zero-lower-bound
   descriptor bounds instead of hiding them as `None`, so the internal wrapper
   reflects the descriptor state required by `ReDim Preserve`.
7. A brittle object-pointer unit test was corrected to keep the object alive
   while comparing against binding-token identity, avoiding allocator-address
   reuse as a false failure.
8. `ForEachIteratorState` now stores materialized items as `RuntimeSlot`
   values rather than `RuntimeValue` values.
9. `withevents_bindings` now stores bound values as `RuntimeSlot` values
   rather than `RuntimeValue` values.
10. `crates/oxvba-jit/src/slot_abi.rs` no longer defines a custom
    tag/payload slot. `RtSlot` is now a transparent owner for the canonical
    16-byte `Variant` carrier: `VARTYPE` at offset `0`, reserved words at
    offsets `2/4/6`, and the 8-byte union payload at offset `8`.
11. JIT generated fast paths now inspect `VT_I4` and the VARIANT union payload
    for Long arithmetic rather than using a private JIT tag. Loads and
    conditional branching use runtime helpers where overwrite/drop or semantic
    truthiness can involve non-scalar carriers.
12. JIT WithEvents retained state now stores `JitRuntimeSlot` values instead
    of `RuntimeValue` values, keeping normal VBA values in `Variant` form and
    retaining `BindingHandle` only as the explicit non-VBA side token.
13. JIT `ReDim` helper paths now create typed `SAFEARRAY` payloads for the
    declared runtime element type, matching the VM-side typed SAFEARRAY
    migration.
14. Object-valued canonical runtime `Variant` cells now use `VT_UNKNOWN`
    (`0x000D`) for the IUnknown-backed object identity/lifetime lane rather
    than `VT_DISPATCH`.
15. `VarPtr(Variant)` in VM and JIT now returns the address of the actual
    VM/JIT `Variant` slot cell for Variant variables, not a copied
    runtime-pointer-helper projection. The older copied helper remains only as
    a boundary/manual pointer-helper utility.
16. COM runtime event callback queues no longer retain callback arguments as
    `ComValue`. `ComRuntimeState` now stores queued callback arguments as
    `Variant` via `ComEventCallbackValue`, and projects back to `ComValue` only
    when existing COM/HAL polling and callback-argument APIs are read.
17. Dynamic COM request/event payloads no longer use `DynamicValue = ComValue`.
    `DynamicValue` is now a `Variant`-backed carrier that accepts and projects
    `ComValue` at the existing conversion boundaries, so `DynamicCallArg` and
    `DynamicEventPayload` own `Variant` values rather than semantic `ComValue`
    values.
18. COM invoke requests now retain argument payloads through `ComInvokeValue`,
    a `Variant`-backed carrier. `ComInvokeArg` constructors still accept
    `ComValue` for boundary compatibility, but `ComInvokeRequest.args` no
    longer own semantic `ComValue` payloads.
19. Polled COM callback payloads now retain callback arguments through
    `ComCallbackValue`, a `Variant`-backed carrier. Existing callback payload
    consumers can still project to `ComValue`, but `ComCallbackPayload.args` no
    longer owns semantic `ComValue` payloads.
20. `SafeArray` now has Variant-native construction, element-read, and
    element-replacement APIs. Existing `RuntimeValue` element APIs are
    compatibility projections over the Variant path, and clone/equality now use
21. VM/JIT `StrPtr`, generic `VarPtr`, `VarPtr(String)`, and `ObjPtr` now read
    canonical slot `Variant` values before constructing boundary pointer cells.
    The generic `VarPtr` array-buffer lane stays on the byte-payload path for
    `VarPtr(buf(0))`, and typed `VT_UI1` SAFEARRAY replacement now coerces
    canonical `Empty` default slots back to zero during typed element encoding
    so byte-buffer pointer/native lanes stay green.
    canonical Variant element reads rather than semantic `RuntimeValue`
    element reads.
21. The Windows `VARIANT`/`SAFEARRAY` bridge now constructs internal
    `SafeArray` values from `Variant` element vectors and emits Windows
    `SAFEARRAY` values from `variant_elements()`. Typed scalar conversion still
    performs per-element projection where needed, but the bridge no longer
    owns `Vec<RuntimeValue>` as its array carrier.
22. VM and JIT `ReDim` / `ReDim Preserve` helper paths now allocate default
    typed SAFEARRAY elements as canonical `Variant` values and preserve overlap
    through `variant_elements()` / `replace_variant_elements()`, keeping the
    resize carrier on the native Variant path. The legacy runtime projection
    bridge now also handles `VT_UI1` Byte variants so compatibility reads of
    typed Byte arrays do not fail.
23. VM and JIT intrinsic array literal/append helpers now construct and extend
    internal arrays through `Variant` element vectors instead of storing
    `RuntimeValue` element vectors. Slot reads still project through the
    existing semantic execution APIs, but the resulting SAFEARRAY payload uses
    the native Variant path.
24. VM `For Each` array materialization now reads array payloads through
    `variant_elements()` and retains iterator items as `RuntimeSlot::Variant`
    directly, removing the old `RuntimeValue`-to-slot conversion helper from
    that path.
25. VM semantic array get/set now read and replace payload slots through
    `variant_elements()` / `replace_variant_elements()`. The semantic API still
    returns and accepts `RuntimeValue` at its boundary, but the array payload
    mutation path no longer uses `RuntimeValue` as the stored element carrier.
26. Windows `IEnumVARIANT` materialization now builds the resulting internal
    array with `SafeArray::from_variants()` rather than `SafeArray::from_values()`,
    keeping the enumerator payload array on the Variant carrier path after COM
    object binding/projection completes.
27. Pointer-helper array projection paths now read array payloads through
    `variant_elements()`. The helper still projects to byte buffers or Windows
    `VARIANT` values at the pointer-helper boundary, but it no longer starts
    from the legacy `SafeArray::elements()` `RuntimeValue` projection.
28. SafeArray debug formatting now reads `variant_elements()`, and pointer
    byte-array readback reconstructs arrays with `SafeArray::from_variants()`
    using `VT_UI1` element variants instead of rebuilding through
    `RuntimeValue` elements.
29. VM dynamic-dispatch `ParamArray` binding now constructs the array payload
    from retained argument `Variant` values with `SafeArray::from_variants()`
    instead of converting each argument to `RuntimeValue` before array
    construction.
30. JIT descriptor-backed external calls now avoid constructing a
    `Vec<RuntimeValue>` for descriptor arguments. The no-descriptor legacy
    `invoke_symbol` path still projects the first argument to `RuntimeValue`,
    but descriptor-backed calls now read slot `Variant` values and route both
    single-argument and multi-argument descriptor calls through
    `invoke_descriptor_variants()`.
31. HAL conformance SAFEARRAY shape probing now validates multidimensional
    descriptor bounds through `SafeArray::from_variants_nd()` with canonical
    `Variant` element carriers instead of constructing the probe array through
    the legacy `RuntimeValue` compatibility constructor.
32. VM descriptor-backed external calls now mirror the JIT descriptor path:
    descriptor calls read slot `Variant` values and route both single-argument
    and multi-argument descriptor invocations through
    `invoke_descriptor_variants()`. Only the no-descriptor legacy
    symbol-token fast path still projects the first argument to `RuntimeValue`
    for `invoke_symbol()`.
33. VM project-dynamic argument binding now builds bound callee arguments as
    `RuntimeSlot` values and writes callee slots directly through a slot-native
    inline invocation path. Existing project-symbol callbacks can still enter
    through the semantic `RuntimeValue` wrapper, but dynamic dispatch no longer
    builds a `Vec<RuntimeValue>` before entering the target procedure.
34. VM and JIT dynamic COM dispatch result writes now retain `DynamicValue`
    results as `Variant` slot payloads instead of projecting through
    `RuntimeValue` before writing the destination slot. Legacy COM error-tag
    normalization is preserved by converting `VT_I4` error tags to `VT_ERROR`
    before the slot write.
35. JIT WithEvents retained-value get/set/owner-search helpers now read, write,
    and compare retained values as `Variant` slots directly. `BindingHandle`
    remains an explicit control-plane escape outside the VBA/COM value model,
    and explicit `VT_I4` zero remains the clear/remove carrier.
36. VM `For Each` next-item writes now copy the retained `RuntimeSlot` directly
    into the item slot instead of projecting iterator items through
    `RuntimeValue` before the slot write.
37. VM WithEvents retained-value get/set/owner-search helpers now read, write,
    and compare retained normal VBA values as `Variant` slots. The project COM
    subscription sync still projects to `RuntimeValue` at its object-boundary
    check, but retained storage and destination writes stay on the Variant slot
    carrier.
38. VM project-dynamic dispatch now returns `RuntimeSlot` results from
    project-routed members and writes dispatch destinations through
    `write_runtime_slot()`. Object `_NewEnum` fallback wraps the retained
    `DynamicValue` `Variant` into a slot before array inspection, leaving
    projection only where the semantic array boundary is still required.
39. SAFEARRAY `RuntimeValue` API usage has been audited in
    [SAFEARRAY_RUNTIMEVALUE_PROJECTION_AUDIT_2026-04-23.md](SAFEARRAY_RUNTIMEVALUE_PROJECTION_AUDIT_2026-04-23.md).
    The scan did not identify a current production payload-retention path that
    owns `Vec<RuntimeValue>` as the internal SAFEARRAY carrier, but the public
    compatibility APIs remain open classification work.
40. `ComHal` now exposes Variant-native dispatch methods:
    `dispatch_invoke_variant()` and `dispatch_invoke_dynamic_variant()`. The
    standard host implements those as the primary path and leaves
    `dispatch_invoke_runtime_value_v2()` /
    `dispatch_invoke_dynamic_runtime_value_v2()` as compatibility projections
    from the Variant path.
41. VM object `_NewEnum` array inspection now matches on the returned
    `RuntimeSlot::Variant` and reads `Variant::as_safearray()` directly before
    materializing iterator item slots. `to_runtime_value()` remains only for the
    unsupported-value diagnostic message on non-array variants.
42. JIT execution context result extraction now has a Variant-native
    `extract_user_variants()` path that returns exact user-visible `Variant`
    slot carriers. The existing `extract_user_values()` API remains as a
    compatibility projection over those carriers for current public snapshot
    callers.
43. VM execution context result extraction now has a Variant-native
    `snapshot_variants()` path that returns exact user-visible `Variant` slot
    carriers. The existing `snapshot()` and `snapshot_values()` APIs now project
    from that Variant-native surface and remain compatibility result APIs for
    current callers.
44. VM and JIT public execution helpers now have Variant-native snapshot
    surfaces. VM exposes `execute_and_snapshot_variants*()` helpers. JIT exposes
    `JitEngine::execute_and_snapshot_variants*()` and
    `cranelift::execute_bytecode_rtslot_variants()`. Existing
    `RuntimeValue`-returning execution helpers now project from those Variant
    result surfaces where the execution path can produce Variant carriers.
45. Host/project execution snapshot APIs now have Variant-native companion
    surfaces. `Engine::execute_source_with_variant_snapshot*()`,
    `Engine::execute_project_with_variant_snapshot_phased()`,
    `ProjectRuntimeSession::snapshot_variants()`, and
    `ProjectRuntimeSession::read_variant_slot()` expose host-visible results as
    exact `Variant` carriers before the legacy `RuntimeValue` snapshot APIs
    project them for existing callers.
46. Host bundle execution now has `execute_bundle_with_variant_snapshot()`,
    with the existing `execute_bundle_with_snapshot()` compatibility API
    projecting from the Variant snapshot path.
47. Immediate sessions now expose `snapshot_variants()`, forwarding the prepared
    project runtime session's exact `Variant` snapshot while leaving immediate
    evaluation result display and legacy `RuntimeValue` snapshot APIs unchanged.
47a. Host legacy slot snapshots now project directly from retained `Variant`
     snapshot carriers. `ProjectRuntimeSession::snapshot_slots()` and the host
     slot-snapshot test helpers use the Variant snapshot companions and
     `Variant::project_compat_slot_i32()` rather than round-tripping through a
     `Vec<RuntimeValue>` first.
47b. `Variant::project_compat_slot_i32()` now projects the exact legacy slot
     subset directly from the retained `Variant` payload rather than converting
     through `to_runtime_value()` first. Supported compat-slot carriers
     (`Empty`, `Null`, `Integer`, `Long`, narrow `LongLong`, `Byte`, `Boolean`,
     `Error`, `Object`, and encodable `SAFEARRAY`) stay exact, while
     non-representable carriers still fail with the existing compatibility-lane
     diagnostics.
47c. `Variant::try_from_compat_slot_i32()` now decodes the legacy tag subset
     directly into canonical `Variant` carriers rather than detouring through
     `RuntimeValue::from_compat_slot_i32()`. Empty/null/error/array tags and
     plain legacy integers now bridge straight into retained `Variant` payloads
     on the exact subset boundary.
47d. JIT `JitContext::read_slot()` / `write_slot()` now treat the
     `RuntimeValue` slot API as a compatibility projection over the existing
     Variant-native slot accessors. Legacy writes first project into the
     canonical `Variant` carrier, including the explicit `BindingHandle ->
     VT_I4` compatibility lane, and legacy reads now project back from
     `read_variant_slot()` instead of bypassing the Variant path.
47e. JIT `oxrt_array_literal` and `oxrt_array_append` now consume and write
     exact slot `Variant` carriers directly. Those helpers no longer read slots
     as `RuntimeValue` only to convert straight back into `Variant` before
     building the SAFEARRAY payload, and destination writes now use
     `write_variant_slot()` with `Variant::from_safearray(...)`.
47f. Pointer-helper string and byte-array readback now have Variant-native
     companions: `read_back_string_payload_variant()` and
     `read_back_byte_array_payload_variant()`. JIT external-call writeback now
     uses those Variant companions directly instead of reading back
     `RuntimeValue` payloads only to convert them immediately into `Variant`
     before the destination slot write.
47g. VM semantics now has Variant-native array get/set companions
     (`runtime_array_get_variant()` / `runtime_array_set_variant()`), and JIT
     `oxrt_array_get` / `oxrt_array_set` now use those helpers with
     `read_variant_slot()` / `write_variant_slot()` so array element access no
     longer round-trips the array carrier and source value through
     `RuntimeValue` before updating the slot.
47h. JIT `oxrt_varptr` now reads the canonical slot `Variant` directly and
     routes all payload kinds, including SAFEARRAY-backed array values, through
     `pointer_helpers::register_variant_pointer()`. The helper no longer reads
     a temporary `RuntimeValue` just to special-case `ArrayIntent` before
     falling back to the same Variant projection path.
47i. JIT `oxrt_array_resize` and `oxrt_array_resize_preserve` now keep ReDim
     results on the canonical Variant carrier. `ReDim Preserve` now reads the
     existing array through `read_variant_slot()` and preserves over
     `Variant::as_safearray()`, while both helpers write the resized SAFEARRAY
     back with `write_variant_slot!(..., Variant::from_safearray(...))`
     instead of detouring through `RuntimeValue::ArrayIntent`.
47j. VM semantics now has Variant-native `runtime_array_lbound_variant()` and
     `runtime_array_ubound_variant()` companions, and JIT `oxrt_lbound` /
     `oxrt_ubound` now read the source slot through `read_variant_slot()` and
     compute bounds against the retained SAFEARRAY-backed Variant carrier
     instead of projecting the operand into `RuntimeValue::ArrayIntent` first.
47k. VM semantics now also has Variant-native tag-classifier companions for
     `runtime_vartype_tag_bounded`, `runtime_typename_tag_bounded`, and
     `runtime_is_numeric_tag_bounded`. JIT `oxrt_vartype_tag`,
     `oxrt_typename_tag`, and `oxrt_is_numeric_tag` now classify the retained
     slot `Variant` directly instead of reading a temporary `RuntimeValue`
     before checking array-shaped or scalar tag behavior.
47l. JIT simple type predicates now also read the retained slot `Variant`
     directly for non-coercive cases. `oxrt_is_numeric`, `oxrt_is_error`,
     `oxrt_is_null`, `oxrt_is_empty`, and `oxrt_is_array_tag` no longer
     project the source slot through `RuntimeValue` just to answer those
     carrier-shape questions; they classify the canonical `Variant` payload
     directly.
47m. JIT `oxrt_vartype` now also reads the retained slot `Variant` directly.
     It uses a Variant-native compatibility helper that preserves the current
     VM/JIT heuristic behavior, including the existing `vbInteger` result for
     `Long` values inside the i16 range and the current `LongLong -> vbLong`
     compatibility mapping, while removing the slot-level `RuntimeValue`
     projection from the JIT helper path.
47n. JIT `oxrt_is_date_tag` now also reads the retained slot `Variant`
     directly. VM semantics now has a Variant-native
     `runtime_variant_is_date()` helper that preserves the current `CDate`
     compatibility heuristic for string parsing, packed-date integer
     recognition, and truncating `Byte`/`Currency`/`Decimal` numeric fallback
     instead of projecting the slot through `RuntimeValue` before testing date
     compatibility.
47o. Runtime pointer-helper `register_variant_pointer()` now reads canonical
     Variant payloads directly for integer, floating/date, and SAFEARRAY byte
     buffer lanes. It no longer detours through `Variant::to_runtime_value()`
     for those cases, and byte-buffer extraction for `VarPtr` over
     SAFEARRAY-backed Variant arrays now walks `variant_elements()` instead of
     the legacy `RuntimeValue` projection API.
47p. Windows pointer-helper `VARIANT` cell materialization now also reads
     canonical Variant payloads directly for integer and floating/date lanes.
     `set_windows_variant_from_variant()` no longer detours through
     `Variant::to_runtime_value()` to recover `VT_I4`, `VT_R4`, `VT_R8`, or
     `VT_DATE` payloads before writing the owned VARIANT cell used by
     `VarPtr(variantVar)`.
47q. `IsObject` classification is no longer a literal stub. VM
     `IntrinsicIsObjectTag` now classifies object-valued slots through the
     existing object semantics, and JIT `oxrt_is_object_tag` now reads the
     retained slot `Variant` directly and returns `1` for canonical
     `VT_UNKNOWN` object carriers instead of always returning `0`.
47r. Interpreter intrinsic classifier paths now also read retained slot
     `Variant` carriers directly for `IsArray`, `VarTypeTag`, `VarType`,
     `TypeNameTag`, `IsNumericTag`, `IsNumeric`, `IsError`, `IsDate`,
     `IsObject`, `IsNull`, and `IsEmpty`. The VM now uses the same
     Variant-native helpers and vtype checks as the JIT instead of projecting
     each slot through `RuntimeValue` before classifying it.
48r. The legacy 4-byte Cranelift fallback lane now projects execution results
     directly from compat slot tags into exact `Variant` carriers through
     `execute_bytecode_variants()`. `JitEngine::execute_and_snapshot_variants()`
     no longer detours that fallback path through
     `RuntimeValue::from_compat_slot_i32()` and
     `Variant::try_from_runtime_value()`.
49r. Public VM/JIT snapshot/result compatibility APIs are now explicitly named
     as compatibility projections over Variant-native execution paths.
     `Vm::snapshot_compat_values()`, `oxvba_vm::execute_and_snapshot_compat_values*()`,
     and `JitEngine::execute_and_snapshot_compat_values*()` now make the
     `Variant -> RuntimeValue` boundary explicit while preserving the existing
     legacy method names as delegating compatibility aliases.
50r. Host project-runtime and immediate-session snapshot compatibility APIs are
     now explicitly named as compatibility projections over Variant-native
     runtime state. `ProjectRuntimeSession::snapshot_compat_values()`,
     `ProjectRuntimeSession::read_compat_slot()`, and
     `ImmediateSession::snapshot_compat_values()` now expose the
     `Variant -> RuntimeValue` boundary directly while preserving the older
     `snapshot()` / `snapshot_values()` / `read_slot()` entrypoints as
     delegating compatibility aliases.
51r. Typed SAFEARRAY element decoding now constructs `Variant` carriers
     directly for intrinsic element kinds instead of decoding to
     `RuntimeValue` and immediately converting back to `Variant`.
     `SafeArray::variant_elements()` therefore preserves typed payload carriers
     before the existing `SafeArray::elements()` compatibility projection.
52r. The standard HAL legacy dynamic-link symbol-token Variant path now
     projects the input token directly from `Variant` and returns a `Variant`
     result directly for the deterministic m0 lane. `invoke_symbol_variant()`
     no longer detours through `RuntimeValue` before computing the
     no-descriptor legacy token result.
53r. The standard HAL descriptor-driven dynamic-link Variant path now keeps
     deterministic m0 calls Variant-native as well. `invoke_descriptor_variants()`
     validates/binds the descriptor, projects the single token argument
     directly from `Variant`, and returns a `Variant` result without entering
     the `RuntimeValue` multi-invoke compatibility path for non-native m0
     descriptors.
48. `ComValue::from_variant()` and `ComValue::to_variant()` now convert directly
    against `Variant` accessors and constructors. The `RuntimeValue` bridge
    methods remain as compatibility projection helpers, but the COM value bridge
    no longer uses `RuntimeValue` as the intermediate carrier for normal
    Variant-shaped values.
49. VM and host embedded procedure invocation now have Variant-native companion
    paths. `Vm::invoke_procedure_with_variants()`,
    `Engine::invoke_procedure_with_variants()`, and
    `EmbeddedRunSession::invoke_procedure_variant()` carry procedure arguments
    and return values as exact `Variant` carriers; existing embedded
    `RuntimeValue` request/result APIs remain compatibility projections over
    those Variant-native paths.
50. Immediate procedure invocation now parses literal arguments into `Variant`
    carriers and invokes through `Engine::invoke_procedure_with_variants()`.
    Immediate display results still project to `RuntimeValue` at the UI-facing
    output boundary.
51. Debugger frame value projection now reads `Variant` slots first through a
    `DebugFrameVariantValue` companion and projects the existing
    `DebugFrameValue` compatibility shape from that carrier. Debugger display
    text and identifier evaluation remain UI-facing projection surfaces.
52. Host class member invocation now has
    `Engine::invoke_member_on_object_with_variants()`, including the implicit
    `Me` argument as a `Variant::from_object_ref()` carrier. The existing
    `RuntimeValue` member invocation API remains a compatibility projection over
    the Variant path.
53. Host COM event callback ingress and runtime dispatch now have Variant-native
    companion surfaces. `ComEventCallbackVariantDispatch`,
    `Engine::poll_com_event_callback_variants()`, and
    `Engine::dispatch_com_event_callback_variants_into_runtime()` preserve
    callback payloads as exact `Variant` carriers and invoke project handlers
    through `Vm::invoke_procedure_with_variants()`. The existing
    `ComEventCallbackDispatch` / `poll_com_event_callback()` /
    `dispatch_com_event_callback_into_runtime()` APIs remain compatibility
    projections.
54. Dynamic-link legacy symbol-token invocation now has a Variant-native
    companion path. `DynamicLinkHal::invoke_symbol_variant()` lets VM/JIT
    no-descriptor external-call sites pass and write exact slot `Variant`
    carriers, while `invoke_symbol()` remains the compatibility method for
    existing HAL callers.
55. HAL dynamic COM bridge invocation now returns dynamic values from
    `ComHal::dispatch_invoke_dynamic_variant()` directly. `DynamicValue` has a
    `from_variant()` constructor, so dynamic bridge invocation no longer
    detours through `RuntimeValue` before returning the retained COM dynamic
    payload.

Validation:

1. `cargo test -p oxvba-runtime --lib`
2. `cargo test -p oxvba-vm --lib intrinsic_array_resize_1d_materializes_zeroed_byte_payload`
3. `cargo test -p oxvba-vm --lib runtime_redim_preserve_1d_retains_overlapping_byte_values`
4. `cargo test -p oxvba-vm --lib`
   - result: `77` passed, `1` ignored, `2` failed
   - the two failures are machine-local COM registration failures for
     `OxVba.TestDispatch`:
     `CLSIDFromProgID failed for OxVba.TestDispatch with HRESULT 0x800401F3`
5. `cargo test -p oxvba-vm --lib withevents`
   - result: `5` passed, `1` ignored
6. `cargo test -p oxvba-vm --lib foreach`
   - result: compile/pass with `0` tests selected
7. `cargo test -p oxvba-jit --lib slot_abi -- --nocapture`
   - result: `6` passed
8. `cargo test -p oxvba-jit --lib runtime_preserve_resize_helper_retains_existing_byte_values -- --nocapture`
   - result: `1` passed
9. `cargo test -p oxvba-jit --lib -- --nocapture`
   - result: `29` passed, `2` failed
   - the two failures are the same machine-local COM registration failures for
     `OxVba.TestDispatch`:
     `CLSIDFromProgID failed for OxVba.TestDispatch with HRESULT 0x800401F3`
10. `cargo test -p oxvba-runtime --lib variant_runtime_value_bridge_roundtrips_supported_exact_subset -- --nocapture`
    - result: `1` passed
11. `cargo test -p oxvba-vm --lib variant_varptr_returns_actual_register_variant_cell -- --nocapture`
    - result: `1` passed
12. `cargo test -p oxvba-jit --lib variant_cell_pointer_exposes_actual_slot_storage -- --nocapture`
    - result: `1` passed
13. `cargo test -p oxvba-com --lib runtime_state -- --nocapture`
    - result: `5` passed
14. `cargo fmt --check`
    - result: passed
15. `cargo test -p oxvba-com --lib dynamic_object -- --nocapture`
    - result: `4` passed
16. `cargo test -p oxvba-hal --lib dispatch_invoke_dynamic_projection_resolves_name_selector_for_testdispatch -- --nocapture`
    - result: `1` passed
17. `cargo test -p oxvba-vm --lib project_dynamic -- --nocapture`
    - result: `4` passed
18. `cargo test -p oxvba-jit --lib -- --nocapture`
    - result: `30` passed, `2` failed
    - the two failures are the same machine-local COM registration failures for
      `OxVba.TestDispatch`:
      `CLSIDFromProgID failed for OxVba.TestDispatch with HRESULT 0x800401F3`
19. `cargo test -p oxvba-com --lib model -- --nocapture`
    - result: `8` passed
20. `cargo test -p oxvba-com --lib invoke_policy -- --nocapture`
    - result: `8` passed
21. `cargo test -p oxvba-jit --lib slot_abi -- --nocapture`
    - result: `7` passed
22. `cargo test -p oxvba-hal --lib event_callback -- --nocapture`
    - result: `2` passed
23. `cargo test -p oxvba-hal --lib poll_event_callback -- --nocapture`
    - result: compile/pass with `0` tests selected
24. `cargo test -p oxvba-runtime --lib safe_array -- --nocapture`
    - result: `12` passed
25. `cargo fmt --check`
    - result: passed
26. `./scripts/check-governance.ps1`
    - result: passed
27. `cargo test -p oxvba-com --lib windows_variant -- --nocapture`
    - result: `28` passed
28. `cargo fmt --check`
    - result: passed
29. `./scripts/check-governance.ps1`
    - result: passed
30. `cargo test -p oxvba-runtime --lib variant_runtime_value_bridge_roundtrips_supported_exact_subset -- --nocapture`
    - result: `1` passed
31. `cargo test -p oxvba-vm --lib runtime_redim_preserve_1d_retains_overlapping_byte_values -- --nocapture`
    - result: `1` passed
32. `cargo test -p oxvba-jit --lib runtime_preserve_resize_helper_retains_existing_byte_values -- --nocapture`
    - result: `1` passed
33. `cargo fmt --check`
    - result: passed
34. `./scripts/check-governance.ps1`
    - result: passed
35. `cargo test -p oxvba-vm --lib array -- --nocapture`
    - result: `8` passed
36. `cargo test -p oxvba-jit --lib array -- --nocapture`
    - result: compile/pass with `0` tests selected
37. `cargo fmt --check`
    - result: passed
38. `./scripts/check-governance.ps1`
    - result: passed
39. `cargo test -p oxvba-vm --lib foreach -- --nocapture`
    - result: compile/pass with `0` tests selected
40. `cargo fmt --check`
    - result: passed
41. `./scripts/check-governance.ps1`
    - result: passed
42. `cargo test -p oxvba-vm --lib runtime_array -- --nocapture`
    - result: compile/pass with `0` tests selected
43. `cargo test -p oxvba-vm --lib array -- --nocapture`
    - result: `8` passed
44. `cargo fmt --check`
    - result: passed
45. `./scripts/check-governance.ps1`
    - result: passed
46. `cargo test -p oxvba-com --lib unknown_enumvariant_result_materializes_to_runtime_array -- --nocapture`
    - result: `1` passed
47. `cargo test -p oxvba-com --lib windows_variant -- --nocapture`
    - result: `28` passed
48. `cargo fmt --check`
    - result: passed
49. `./scripts/check-governance.ps1`
    - result: passed
50. `cargo test -p oxvba-runtime --lib pointer -- --nocapture`
    - result: `15` passed
51. `cargo fmt --check`
    - result: passed
52. `./scripts/check-governance.ps1`
    - result: passed
53. `cargo test -p oxvba-runtime --lib "safe_array|pointer" -- --nocapture`
    - result: compile/pass with `0` tests selected
54. `cargo fmt --check`
    - result: passed
55. `./scripts/check-governance.ps1`
    - result: passed
56. `cargo test -p oxvba-hal dynlink -- --nocapture`
    - result: `7` passed
    - note: the existing COM/dynlink property now asserts stable
      `ObjectRef::compat_identity()` rather than full `ObjectRef` pointer
      equality, because separate `CreateObject` calls intentionally produce
      distinct IUnknown/ObjectRef instances even when their compatibility
      identity is stable
57. `cargo test -p oxvba-vm --lib external -- --nocapture`
    - result: compile/pass with `0` tests selected
58. `cargo test -p oxvba-jit --lib external -- --nocapture`
    - result: compile/pass with `0` tests selected
59. `cargo check -p oxvba-hal -p oxvba-vm -p oxvba-jit`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
60. `cargo fmt --check`
    - result: passed
61. `./scripts/check-governance.ps1`
    - result: passed
62. `cargo test -p oxvba-com --lib event_callback -- --nocapture`
    - result: `1` passed
63. `cargo test -p oxvba-hal --lib event_callback -- --nocapture`
    - result: `2` passed
64. `cargo test -p oxvba-vm --lib event_callback -- --nocapture`
    - result: compile/pass with `0` tests selected
65. `cargo test -p oxvba-jit --lib event_callback -- --nocapture`
    - result: compile/pass with `0` tests selected
66. `cargo check -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-jit`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
67. `cargo fmt --check`
    - result: passed
68. `./scripts/check-governance.ps1`
    - result: passed
69. `cargo fmt --check`
    - result: passed
70. `cargo test -p oxvba-vm --lib project_dynamic -- --nocapture`
    - result: `4` passed
71. `cargo test -p oxvba-vm --lib array -- --nocapture`
    - result: `8` passed
72. `./scripts/check-governance.ps1`
    - result: passed
73. `cargo fmt --check`
    - result: passed
74. `cargo test -p oxvba-jit --lib external -- --nocapture`
    - result: compile/pass with `0` tests selected
75. `cargo test -p oxvba-jit --lib dynlink -- --nocapture`
    - result: compile/pass with `0` tests selected
76. `cargo check -p oxvba-jit -p oxvba-hal -p oxvba-vm`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
77. `./scripts/check-governance.ps1`
    - result: passed
78. `cargo fmt --check`
    - result: passed
79. `cargo check -p oxvba-hal`
    - result: passed
80. `cargo test -p oxvba-hal conformance -- --nocapture`
    - result: `11` passed; bin test target compile/pass with `0` tests
      selected
81. `./scripts/check-governance.ps1`
    - result: passed
82. `cargo fmt --check`
    - result: passed
83. `cargo test -p oxvba-vm --lib external -- --nocapture`
    - result: compile/pass with `0` tests selected
84. `cargo check -p oxvba-vm -p oxvba-hal`
    - result: passed with existing dead-code warnings in VM digit helper
      functions
85. `./scripts/check-governance.ps1`
    - result: passed
86. `cargo fmt --check`
    - result: passed
87. `cargo test -p oxvba-vm --lib project_dynamic -- --nocapture`
    - result: `4` passed
88. `cargo check -p oxvba-vm`
    - result: passed with existing dead-code warnings in VM digit helper
      functions
89. `./scripts/check-governance.ps1`
    - result: passed
90. `cargo fmt --check`
    - result: passed
91. `cargo test -p oxvba-vm --lib dynamic -- --nocapture`
    - result: `4` passed
92. `cargo test -p oxvba-jit --lib dynamic -- --nocapture`
    - result: compile/pass with `0` tests selected
93. `cargo check -p oxvba-vm -p oxvba-jit`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
94. `./scripts/check-governance.ps1`
    - result: passed
95. `cargo fmt --check`
    - result: passed
96. `cargo test -p oxvba-jit --lib withevents -- --nocapture`
    - result: compile/pass with `0` tests selected
97. `cargo check -p oxvba-jit`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
98. `./scripts/check-governance.ps1`
    - result: passed
99. `cargo fmt --check`
    - result: passed
100. `cargo test -p oxvba-vm --lib withevents -- --nocapture`
    - result: `5` passed, `1` ignored
101. `cargo test -p oxvba-vm --lib foreach -- --nocapture`
    - result: compile/pass with `0` tests selected
102. `cargo check -p oxvba-vm`
    - result: passed with existing dead-code warnings in VM digit helper
      functions
103. `./scripts/check-governance.ps1`
    - result: passed
104. `cargo fmt --check`
    - result: passed
105. `cargo test -p oxvba-vm --lib project_dynamic -- --nocapture`
    - result: `4` passed
106. `cargo test -p oxvba-vm --lib foreach -- --nocapture`
    - result: compile/pass with `0` tests selected
107. `cargo check -p oxvba-vm`
    - result: passed with existing dead-code warnings in VM digit helper
      functions
108. `./scripts/check-governance.ps1`
    - result: passed
109. `rg -n "SafeArray::from_values|from_values_nd|from_typed_values|from_shape_and_values|\.elements\(\)|replace_elements\(" crates/oxvba-runtime crates/oxvba-vm crates/oxvba-jit crates/oxvba-com crates/oxvba-hal --glob "*.rs"`
    - result: remaining hits are compatibility API definitions and
      tests/property fixtures; no production retained-payload `Vec<RuntimeValue>`
      path was identified by this scan
110. `cargo fmt --check`
    - result: passed
111. `cargo check -p oxvba-hal`
    - result: passed
112. `cargo test -p oxvba-hal dynlink -- --nocapture`
    - result: `7` passed; bin test target compile/pass with `0` tests selected
113. `./scripts/check-governance.ps1`
    - result: passed
114. `cargo fmt --check`
    - result: passed
115. `cargo test -p oxvba-vm --lib foreach -- --nocapture`
    - result: compile/pass with `0` tests selected
116. `cargo test -p oxvba-vm --lib project_dynamic -- --nocapture`
    - result: `4` passed
117. `cargo check -p oxvba-vm`
    - result: passed with existing dead-code warnings in VM digit helper
      functions
118. `./scripts/check-governance.ps1`
    - result: passed
119. `cargo fmt --check`
    - result: passed
120. `cargo test -p oxvba-jit --lib jit_context_extracts_user_variants_before_projection -- --nocapture`
    - result: `1` passed
121. `cargo test -p oxvba-jit --lib slot_abi -- --nocapture`
    - result: `7` passed
122. `cargo check -p oxvba-jit`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
123. `./scripts/check-governance.ps1`
    - result: passed
124. `cargo fmt --check`
    - result: passed
125. `cargo test -p oxvba-vm --lib snapshot_variants_exposes_variant_cells_before_projection -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM digit helper
      functions
126. `cargo test -p oxvba-vm --lib snapshot_values_preserve_non_legacy_runtime_values -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM digit helper
      functions
127. `cargo check -p oxvba-vm`
    - result: passed with existing dead-code warnings in VM digit helper
      functions
128. `./scripts/check-governance.ps1`
    - result: passed
129. `cargo fmt --check`
    - result: passed
130. `cargo test -p oxvba-vm --lib snapshot_variants_exposes_variant_cells_before_projection -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM digit helper
      functions
131. `cargo test -p oxvba-jit --lib execute_and_snapshot_variants_exposes_jit_results_before_projection -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM/JIT digit
      helper functions
132. `cargo check -p oxvba-vm`
    - result: passed with existing dead-code warnings in VM digit helper
      functions
133. `cargo check -p oxvba-jit`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
134. `./scripts/check-governance.ps1`
    - result: passed
135. `cargo fmt --check`
    - result: passed
136. `cargo test -p oxvba-host --lib variant_snapshot_api_exposes_host_results_before_projection -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM/JIT digit
      helper functions
137. `cargo check -p oxvba-host`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
138. `./scripts/check-governance.ps1`
    - result: passed
139. `cargo fmt --check`
    - result: passed
140. `cargo test -p oxvba-host --lib variant_snapshot_api_exposes_host_results_before_projection -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM/JIT digit
      helper functions
141. `cargo check -p oxvba-host`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
142. `./scripts/check-governance.ps1`
    - result: passed
143. `cargo fmt --check`
    - result: passed
144. `cargo test -p oxvba-host --lib immediate_session_snapshot_variants_exposes_runtime_state_before_projection -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM/JIT digit
      helper functions
145. `cargo check -p oxvba-host`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
146. `./scripts/check-governance.ps1`
    - result: passed
147. `cargo fmt --check`
    - result: passed
148. `cargo test -p oxvba-com --lib com_value -- --nocapture`
    - result: `11` passed
149. `cargo check -p oxvba-com`
    - result: passed
150. `./scripts/check-governance.ps1`
    - result: passed
151. `cargo test -p oxvba-vm --lib invoke_procedure_with_variants_preserves_exact_carrier -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM digit helper
      functions
152. `cargo test -p oxvba-host --lib embedded_run_session_invokes_procedure_with_variant_args -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM/JIT digit
      helper functions
153. `cargo fmt --check`
    - result: passed
154. `cargo check -p oxvba-vm -p oxvba-host`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
155. `./scripts/check-governance.ps1`
    - result: passed
156. `cargo test -p oxvba-host --lib invoke_procedure_variant_request_preserves_exact_args -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM/JIT digit
      helper functions
157. `cargo test -p oxvba-host --lib immediate -- --nocapture`
    - result: `11` passed with existing dead-code warnings in VM/JIT digit
      helper functions
158. `cargo fmt --check`
    - result: passed
159. `cargo check -p oxvba-host`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
160. `./scripts/check-governance.ps1`
    - result: passed
161. `cargo test -p oxvba-host --lib debugger -- --nocapture`
    - result: `4` passed with existing dead-code warnings in VM/JIT digit
      helper functions
162. `cargo fmt --check`
    - result: passed
163. `cargo check -p oxvba-host`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
164. `./scripts/check-governance.ps1`
    - result: passed
165. `cargo test -p oxvba-host --test invoke_procedure_tests create_class_and_invoke_member_returns_value -- --nocapture`
    - result: `1` passed with existing dead-code warnings in VM/JIT digit
      helper functions
166. `cargo fmt --check`
    - result: passed
167. `cargo check -p oxvba-host`
    - result: passed with existing dead-code warnings in VM/JIT digit helper
      functions
168. `./scripts/check-governance.ps1`
    - result: passed
169. `cargo test -p oxvba-host --lib formal_com_event_callback -- --nocapture`
     - result: `7` passed with existing dead-code warnings in VM/JIT digit
       helper functions
170. `cargo fmt --check`
     - result: passed
171. `cargo check -p oxvba-host`
     - result: passed with existing dead-code warnings in VM/JIT digit helper
       functions
172. `./scripts/check-governance.ps1`
     - result: passed
173. `cargo test -p oxvba-vm --lib external -- --nocapture`
     - result: compile/pass with `0` tests selected; existing dead-code
       warnings in VM digit helper functions
174. `cargo test -p oxvba-jit --lib external -- --nocapture`
     - result: compile/pass with `0` tests selected; existing dead-code
       warnings in VM/JIT digit helper functions
175. `cargo fmt --check`
     - result: passed
176. `cargo check -p oxvba-hal -p oxvba-vm -p oxvba-jit`
     - result: passed with existing dead-code warnings in VM/JIT digit helper
       functions
177. `./scripts/check-governance.ps1`
     - result: passed
178. `cargo test -p oxvba-com --lib dynamic_value_retains_payload_as_variant -- --nocapture`
     - result: `1` passed
179. `cargo fmt --check`
     - result: passed
180. `cargo check -p oxvba-com -p oxvba-hal`
     - result: passed
181. `./scripts/check-governance.ps1`
     - result: passed

Implementation progress:

1. HAL dynamic-link descriptor multi-call now has a `Variant`-native transport:
   `DynamicLinkHal::invoke_descriptor_variants()` and
   `invoke_bound_variants()` return `(Variant, Vec<Variant>)`.
2. The Standard host overrides the descriptor variant path so VM/JIT
   descriptor calls preserve canonical slot payloads through the host boundary;
   the old `RuntimeValue` descriptor multi-call remains as a compatibility
   projection for older adapters.
3. VM external descriptor calls now read source slots as `Variant`, write
   return values with `write_variant_slot()`, and apply ByRef writebacks as
   `Variant` payloads. Pointer string/byte-array writebacks still call
   pointer-helper APIs, but immediately convert those boundary projections back
   into `Variant` before slot writeback.
4. JIT external descriptor calls now mirror the VM path with exact slot-level
   `read_variant_slot()` / `write_variant_slot()` helpers over the `RtSlot`
   Windows `VARIANT` layout.
5. COM event callback argument retrieval now has a Variant-native path:
   native Windows callback state returns the queued `ComEventCallbackValue`
   carrier, `WindowsComBridge::event_callback_variant()` exposes the retained
   `Variant`, `ComHal::event_callback_variant()` carries it through HAL, and
   VM/JIT event callback helpers write callback arguments directly into Variant
   slots.
6. VM project-dynamic `ParamArray` binding now uses `Variant` element carriers
   when building the internal SAFEARRAY payload. The public dynamic-dispatch
   binder still returns through the existing semantic API boundary, but this
   production array-construction path no longer creates the payload from
   `RuntimeValue` elements.
7. JIT descriptor-backed external calls now use the Variant-native descriptor
   transport for both single-argument and multi-argument descriptor calls. Only
   the no-descriptor legacy symbol-token path still projects through
   `RuntimeValue` before calling `invoke_symbol()`.
8. HAL conformance multidimensional SAFEARRAY shape probing now constructs the
   probe array from `Variant` elements, leaving `RuntimeValue` compatibility
   construction to tests/projection seams rather than the conformance runtime
   path.
9. VM descriptor-backed external calls now use the Variant-native descriptor
   transport for both single-argument and multi-argument descriptor calls. Only
   the no-descriptor legacy symbol-token path still projects through
   `RuntimeValue` before calling `invoke_symbol()`.
10. VM project-dynamic argument binding now carries bound target arguments as
    `RuntimeSlot` values and writes callee slots through a slot-native inline
    invocation path. The project-symbol callback wrapper remains a semantic
    compatibility boundary.
11. VM and JIT dynamic COM dispatch result writes now preserve the retained
    `DynamicValue` as a slot `Variant`, with legacy error-tag normalization
    performed directly on the Variant before writing the destination slot.
12. JIT WithEvents retained value helpers now keep get/set/owner enumeration on
    the `Variant` slot carrier for normal VBA values instead of projecting the
    retained value through `RuntimeValue`; the retained `BindingHandle` lane
    remains a deliberate non-VBA control-plane exception.
13. VM `For Each` next-item writes now keep iterator item delivery on the
    retained `RuntimeSlot` carrier, and VM WithEvents retained value helpers now
    keep get/set/owner enumeration on `Variant` slots for normal VBA values.
14. VM project-dynamic dispatch now returns project-routed member results as
    `RuntimeSlot` values and writes dynamic dispatch destinations directly from
    those slots. Object `_NewEnum` fallback also starts from the retained
    `DynamicValue` `Variant` rather than projecting before slot formation.
15. The SAFEARRAY legacy API scan is now recorded as support evidence. It
    classifies remaining `RuntimeValue` SAFEARRAY hits as compatibility/test
    surfaces by current scan, but does not close the public compatibility API
    question.
16. HAL COM dispatch now has Variant-native direct and dynamic dispatch seams.
    Existing `RuntimeValue` COM dispatch methods remain as compatibility
    projections over the Variant path for older callers.
17. VM object `_NewEnum` array inspection now uses `Variant::as_safearray()`
    from the returned slot rather than projecting the returned slot to
    `RuntimeValue` before checking for an array.
18. JIT result extraction now has a Variant-native result surface through
    `extract_user_variants()`, with `extract_user_values()` kept as a public
    compatibility projection.
19. VM result extraction now has a Variant-native result surface through
    `snapshot_variants()`, with `snapshot()` and `snapshot_values()` kept as
    public compatibility projections over that surface.
20. VM and JIT public execution snapshot helpers now have Variant-native result
    surfaces. Compatibility helpers still return `RuntimeValue`, but they
    project from Variant result paths instead of making `RuntimeValue` the only
    public execution snapshot carrier.
21. Host/project snapshot APIs now have Variant-native companion surfaces for
    source execution, project execution, prepared project sessions, and direct
    session slot reads. Existing host `RuntimeValue` snapshot APIs remain
    compatibility projections for existing callers.
22. Host bundle execution now has a Variant-native snapshot companion, leaving
    the existing bundle `RuntimeValue` snapshot API as a compatibility
    projection.
23. Immediate sessions now have a Variant-native snapshot companion over the
    prepared project runtime session. Immediate evaluation display semantics
    remain on the existing `RuntimeValue` projection surface.
24. `ComValue` Variant bridges now convert directly against `Variant` accessors
    and constructors. Its `RuntimeValue` methods remain compatibility
    projections, but normal COM value bridge conversion no longer uses
    `RuntimeValue` as the intermediate carrier.
25. Embedded host procedure invocation now has Variant-native request/result
    companions over a VM Variant invocation path. Existing embedded
    `RuntimeValue` APIs project into and out of that Variant path for callers
    that still use the compatibility surface.
26. Immediate procedure invocation now parses arguments as `Variant` and invokes
    through the host Variant procedure path. Its display result remains a
    compatibility projection because the Immediate Window exposes formatted
    values, not the internal runtime carrier.
27. Debugger frame value projection now reads `Variant` slots first and exposes
    a `DebugFrameVariantValue` companion. Existing debugger frame/evaluation
    values remain compatibility projections for UI/display callers.
28. Host class member invocation now has a Variant-native companion and uses a
    `Variant` carrier for the implicit `Me` argument. Existing `RuntimeValue`
    member invocation remains a compatibility projection.
29. Host COM callback ingress and project-handler dispatch now have
    Variant-native companion APIs. Callback payload normalization keeps
    `DynamicValue` payloads as `Variant` values, and project handler invocation
    routes those values through the VM Variant procedure path. The old
    `RuntimeValue` callback dispatch APIs now project into or out of the
    Variant callback dispatch carrier.
30. Dynamic-link legacy symbol-token invocation now has a Variant-native
    companion path. VM/JIT no-descriptor external-call sites read slot
    `Variant` values and write returned `Variant` values directly; the legacy
    `invoke_symbol()` method remains as a HAL compatibility API.
31. HAL dynamic COM bridge invocation now uses
    `ComHal::dispatch_invoke_dynamic_variant()` and wraps the returned
    `Variant` directly in `DynamicValue`. Object release remains a
    control/status projection through the object-release API rather than a
    retained value payload seam.
32. JIT `JitContext` legacy slot read/write helpers now project through
    `read_variant_slot()` / `write_variant_slot()` instead of maintaining a
    separate `RuntimeValue` slot-storage path. The compatibility API remains
    open classification work, but the JIT context no longer bypasses the
    canonical Variant slot carrier when that API is used.
33. JIT array literal/append helpers now read slot inputs through
    `read_variant_slot()` and materialize result arrays with
    `write_variant_slot()` rather than detouring through `RuntimeValue`
    conversions for both element collection and destination writes.
34. Pointer-helper payload readback now has Variant-native companions for
    string and byte-array lanes, and JIT external-call pointer writeback now
    consumes those Variant results directly instead of re-projecting from
    `RuntimeValue`.
35. JIT array get/set now use Variant-native semantics companions over the
    retained SAFEARRAY slot carrier. RuntimeValue remains only on the index
    compatibility boundary, not as the array/source retained-value carrier.
36. JIT `VarPtr` now uses the exact slot `Variant` as its sole input carrier,
    including the array lane, instead of reading a temporary `RuntimeValue`
    only to special-case `ArrayIntent` before returning to the Variant pointer
    registration path.
37. JIT `ReDim` / `ReDim Preserve` no longer write resized arrays through the
    compatibility `RuntimeValue::ArrayIntent` path. The retained array slot now
    stays on the canonical Variant/SAFEARRAY carrier across resize and
    preserve-resize writes, and the preserve helper now inspects the existing
    array via `Variant::as_safearray()` instead of pattern-matching a temporary
    semantic value.
38. JIT `LBound` / `UBound` no longer need a `RuntimeValue::ArrayIntent`
    projection for normal array-slot operands. VM semantics now exposes
    Variant-native bound helpers, and the JIT array-bound lane reads the
    retained SAFEARRAY-backed `Variant` directly before writing the scalar
    result.
39. JIT tag-classifier helpers for `VarType`, `TypeName`, and `IsNumeric`
    array-tag classification now use Variant-native semantics companions over
    the retained slot carrier for their tag-only paths, removing another
    array-shape compatibility projection from the JIT helper surface.
40. JIT simple retained-carrier predicates for numeric/error/null/empty/array
    shape now classify the slot `Variant` directly rather than reading a
    temporary `RuntimeValue` for those non-coercive checks.
41. JIT `VarType` now uses a Variant-native compatibility classifier over the
    retained slot carrier. The helper preserves the current oracle-tracked
    compatibility heuristic rather than switching semantics to a raw VARTYPE
    mirror during this migration bead.
42. JIT `IsDate` now uses a Variant-native date-classification helper over the
    retained slot carrier, preserving the current string parse, packed-date
    integer, and truncating numeric `CDate` compatibility heuristics while
    removing another semantic slot-projection seam from the JIT helper path.
43. Runtime pointer-helper registry entry materialization now keeps canonical
    Variant payload inspection on the Variant side for integer, floating/date,
    and SAFEARRAY byte-buffer lanes, removing another manual
    `Variant -> RuntimeValue` compatibility detour from the migration surface.
44. Windows pointer-helper owned `VARIANT` cell materialization now keeps
    integer and floating/date payload decoding on the Variant side as well,
    removing another manual `Variant -> RuntimeValue` detour from the
    `VarPtr(variantVar)` compatibility surface.
45. `IsObject` classification now has real object semantics again across the
    interpreter and JIT. The interpreter no longer hardcodes `0` for
    `IntrinsicIsObjectTag`, and the JIT now classifies retained object
    `Variant` carriers directly instead of returning a stubbed false result.
46. Interpreter intrinsic classifier/tag evaluation now reads retained slot
    `Variant` carriers directly across the full predicate family, removing a
    broader VM-side `Variant -> RuntimeValue` projection seam and aligning the
    VM classifier path with the already-migrated JIT path.
47. The legacy 4-byte Cranelift fallback path now extracts execution results
    directly as `Variant` carriers from compat slot tags, removing an internal
    `compat slot -> RuntimeValue -> Variant` detour from JIT result
    materialization while preserving the public compatibility result APIs.
48. VM/JIT public snapshot/result APIs now expose explicit
    `*_compat_values*` aliases for the `Variant -> RuntimeValue` projection
    boundary, making the remaining compatibility surface named and auditable
    instead of implied behind the legacy snapshot method names alone.
49. Host project-runtime and immediate-session snapshot surfaces now expose
    explicit compatibility aliases for the `Variant -> RuntimeValue`
    projection boundary, reducing another unnamed public compatibility seam in
    the value-model migration surface.
50. Typed SAFEARRAY `variant_elements()` decoding now remains Variant-native
    across intrinsic element kinds, removing an internal
    `SAFEARRAY typed payload -> RuntimeValue -> Variant` detour while leaving
    the legacy `elements()` compatibility projection in place.
51. Standard HAL legacy dynamic-link symbol-token invocation now keeps the
    no-descriptor `invoke_symbol_variant()` path Variant-native for the
    deterministic token lane, reducing another `Variant -> RuntimeValue ->
    Variant` bridge inside a remaining compatibility surface.
52. Standard HAL descriptor-driven dynamic-link Variant invocation now also
    keeps deterministic m0 descriptor calls Variant-native, leaving the
    `RuntimeValue` projection path for legacy semantic multi-invoke and native
    FFI marshalling surfaces.
53. VM intrinsic array literal and append instructions now read source operands
    from retained `Variant` slots and write their resulting SAFEARRAY payload
    back as `Variant::from_safearray(SafeArray::from_variants(...))`. The
    remaining legacy zero/Empty append compatibility case is still accepted,
    but normal literal/append construction no longer detours through
    `RuntimeValue::ArrayIntent` inside the interpreter instruction path.
54. VM `ReDim` / `ReDim Preserve` instruction writes now also keep the resized
    SAFEARRAY carrier on retained `Variant` slots through
    `Variant::from_safearray(...)`. The preserve helper now accepts the
    existing retained `Variant` directly and inspects it via `as_safearray()`;
    bound coercion still uses the existing compatibility numeric path.
55. VM intrinsic array get/set instruction paths now read array operands and
    assigned values from retained `Variant` slots and dispatch through the
    Variant-native semantic helpers. Retrieved elements are written back as
    retained Variants; index coercion remains on the existing compatibility
    numeric path.
56. VM `For Each` array initialization now reads the iterable source as a
    retained `Variant`, materializing SAFEARRAY items directly from
    `as_safearray()` and retaining each item as a `RuntimeSlot::Variant`.
    Object-source discovery now uses `Variant::as_object_ref()` rather than
    projecting the source through `RuntimeValue` first.
57. VM `LBound` / `UBound` array instructions now read retained source
    `Variant` carriers directly and call the Variant-native bound helpers,
    keeping SAFEARRAY bound inspection out of the `RuntimeValue::ArrayIntent`
    compatibility path for those interpreter instructions.
58. VM generic `VarPtr` now detects array carriers from the retained
    `Variant::as_safearray()` path and registers the SAFEARRAY payload pointer
    directly, while non-array sources still register the exact retained Variant
    cell.
59. VM project-dynamic `ParamArray` binding now constructs populated and empty
    ParamArray slots directly as retained SAFEARRAY-backed `Variant` carriers
    with `Variant::from_safearray(SafeArray::from_variants(...))`, instead of
    routing the packed argument array through `RuntimeValue::ArrayIntent`.
60. Runtime typed SAFEARRAY construction from `Variant` carriers now encodes
    non-Variant intrinsic element payloads directly from Variant accessors,
    removing the `Variant -> RuntimeValue` detour from
    `SafeArray::from_typed_variants*()` while keeping the legacy
    `from_typed_values*()` API as the explicit compatibility entrypoint.
61. HAL dynamic COM release now has a `ComHal::release_object_variant`
    companion and `HalComDynamicBridge::release_dynamic_object()` maps that
    retained `Variant` result directly into `DynamicValue`. The standard
    adapter emits deterministic release status as `Variant::from_i32(...)`,
    while the legacy `release_object()` surface remains as the explicit
    `Variant -> RuntimeValue` compatibility projection.
62. VM and JIT `CreateObject` host-return paths now use
    `ComHal::create_object_variant` and write the returned object-valued
    `Variant` directly into destination slots. The standard HAL adapter emits
    `Variant::from_object_ref(...)` for deterministic projection handles, while
    the legacy `create_object()` surface remains as an explicit compatibility
    projection. ProgID string coercion now uses the Variant-native BSTR string
    path instead of detouring through `RuntimeValue`.
63. VM and JIT COM event unsubscribe/release status paths now use
    `ComHal::unsubscribe_event_variant` and
    `ComHal::release_event_callback_variant`, writing retained `Variant`
    status carriers into destination slots. The standard HAL adapter emits
    `Variant::from_i32(1)` for successful native status returns, while the
    legacy `RuntimeValue` methods remain compatibility projections. Internal
    subscription/callback token inputs remain on the existing handle-token
    coercion path.
64. VM and JIT WithEvents owner-iteration and clear-owner status outputs now
    write retained `Variant` carriers (`Variant::from_object_ref(...)` or
    `Variant::from_i32(0)`) instead of constructing destination
    `RuntimeValue` objects/status values first. VM project COM WithEvents
    subscription sync now inspects bound object-valued Variants via
    `Variant::as_object_ref()` rather than projecting the binding through
    `RuntimeValue` before `describe_object()` / `subscribe_event()`.
65. VM `TypeOf...Is` now reads the object operand from the retained
    object-valued `Variant` slot and uses `Variant::as_object_ref()` for
    project dynamic object lookup, instead of projecting the operand to
    `RuntimeValue::Object` before class/interface comparison. The boolean
    result remains on the existing scalar compatibility output path.
66. VM project dynamic dispatch now inserts the implicit `Me` argument as an
    object-valued `RuntimeSlot::Variant(Variant::from_object_ref(...))` before
    inline procedure invocation, instead of constructing a
    `RuntimeValue::Object` and converting it back into a runtime slot.
67. VM project COM WithEvents callback pumping now builds inline handler
    arguments as retained `Variant` carriers, using
    `Variant::from_object_ref(...)` for the owner object and
    `ComHal::event_callback_variant(...)` for callback payloads. The old
    project-symbol inline helper that accepted `RuntimeValue` arguments was
    removed because this callback path now invokes with `RuntimeSlot::Variant`
    arguments directly.
68. Host event ingress now has a Variant-native
    `dispatch_host_event_variants_into_runtime` path. Guarded event dispatch
    prepends the source instance as `Variant::from_object_ref(...)` and invokes
    project handlers with `Vm::invoke_procedure_with_variants(...)`; the
    legacy `dispatch_host_event_into_runtime` method remains as a
    `RuntimeValue -> Variant` compatibility wrapper for existing callers.
69. VM project dynamic optional default binding now constructs the callee
    argument slot as `RuntimeSlot::Variant(Variant::from_i32(...))` directly,
    removing the `default ProjectDynamicParamRoute -> RuntimeValue -> Variant`
    hop while preserving the existing compatibility projection assertions.
70. Host class-instance lifecycle dispatch now invokes `Class_Initialize`
    through `Vm::invoke_procedure_with_variants(...)` rather than the legacy
    `RuntimeValue` procedure wrapper. The lifecycle path has no explicit user
    arguments, so this removes a host-side compatibility API detour without
    changing initializer semantics.
71. VM and JIT console `Print` host helpers now call the
    `ConsoleHal::print_line_variant(...)` companion, reading the printed value
    as a retained `Variant` and writing the returned status as a retained
    `Variant`. The legacy `ConsoleHal::print_line(RuntimeValue)` method remains
    as the compatibility implementation for adapters that have not yet grown a
    native Variant override.
72. VM and JIT UI `MsgBox` / `InputBox` helpers now call
    `UiInteractionHal::msg_box_variant(...)` and
    `UiInteractionHal::input_box_variant(...)`, including Variant-native
    default prompt style/default text values for omitted optional arguments.
    Legacy `RuntimeValue` UI HAL methods remain as adapter compatibility
    implementations.
73. VM and JIT diagnostics/event-pump helpers now use
    `DiagnosticsHal::debug_print_variant(...)` and
    `EventPumpHal::do_events_variant(...)` for slot-facing host dispatch.
    Pending callback tokens returned through the VM `DoEvents` instruction are
    also written as retained `Variant::from_i32(...)` status/token carriers.
74. VM and JIT process/environment helpers now use
    `ProcessEnvHal::shell_variant(...)`, `environ_variant(...)`, and
    `dir_variant(...)`, reading `Shell`, `Environ`, and `Dir` inputs as
    retained Variants and writing returned values directly as retained
    Variants. The default `Shell` window style and `Dir` attribute arguments
    are now constructed as `Variant::from_i32(0)`.
75. VM and JIT time/locale host helpers now use
    `TimeLocaleHal::*_variant(...)` companions for `Date`, `Time`, and
    `Timer`, writing returned values as retained Variants. `Now` reads the
    retained date/time Variants before the existing semantic combiner and then
    writes the combined date/time value back as a retained Variant.
76. VM and JIT file-system host helpers now use
    `FileSystemHal::*_variant(...)` companions for `Open`, `Close`, `Kill`,
    `FreeFile`, `Input#`, `Line Input#`, `Print#`, `Write#`, `EOF`, `LOF`,
    `Seek`, and `Loc`, reading and writing slot-facing values as retained
    Variants. `Open` mode/file-number, `EOF` truthiness, and `Seek` position
    arithmetic still use the existing compatibility coercion/classification
    semantics where VBA behavior requires numeric or Boolean interpretation.
77. VM and JIT console input/line-input helpers now use
    `ConsoleHal::input_fields_variant(...)` and
    `ConsoleHal::line_input_variant(...)`, and `Beep` uses
    `DiagnosticsHal::emit_variant(...)`. Slot-facing console input, line-input,
    and diagnostic status values are therefore read and written as retained
    Variants, with the legacy HAL methods retained as adapter compatibility
    implementations.
78. VM and JIT dynamic COM dispatch argument construction now reads retained
    argument slots as `Variant` values and constructs `DynamicValue` with
    `DynamicValue::from_variant(...)` instead of projecting through
    `RuntimeValue` first. The object/member selector inputs still use the
    existing compatibility coercions because they are dispatch control-plane
    selectors/tokens rather than normal argument payload storage.
79. VM project COM WithEvents callback pumping now polls pending host callback
    tokens through `EventPumpHal::do_events_variant(...)` and decodes the
    returned callback token directly from the retained Variant carrier. The
    zero/no-callback token check remains a control-plane token classification,
    not normal VBA value storage.
80. VM and JIT COM event subscription/callback token helpers now read object,
    member, subscription, and callback token carriers from retained Variant
    slots and write subscription/callback token results back as `Variant`
    values. The token interpretation remains an explicit COM control-plane
    classification boundary, not normal VBA value storage.
81. VM and JIT dynamic COM dispatch object/member selector helpers now read
    object and member selector carriers from retained Variant slots and convert
    them directly to COM object handles and dynamic member selectors. Dynamic
    dispatch argument payloads continue to enter `DynamicValue` from retained
    Variants, so the remaining selector interpretation is a COM control-plane
    classification boundary.
82. VM and JIT WithEvents owner/source object helper paths now read owner and
    source carriers from retained Variant slots and convert them directly to
    object handles. Binding handles remain explicit internal control-plane
    carriers, while bound WithEvents values continue to stay as retained
    Variants.
83. VM and JIT COM event callback-argument index helpers now read the index
    carrier from retained Variant slots and decode it directly as an integer
    control-plane index. Callback tokens and returned callback argument payloads
    already stay on retained Variant carriers.
84. VM and JIT `Now` host helpers now combine retained Date/Time Variant
    carriers directly with a Variant-native serial combiner and write the
    resulting Date Variant back to destination slots. The older
    `RuntimeValue` date/time combiner remains only as a semantic compatibility
    helper.
85. VM and JIT file `Seek` helper paths now decode the host `Loc` return
    carrier through a Variant-native numeric compatibility decoder before
    writing the retained `Seek` result Variant. File-position arithmetic remains
    an explicit numeric compatibility classification, but no longer requires a
    `Variant -> RuntimeValue -> Variant` detour.
86. VM and JIT file `Open` mode/file-number control carriers and `EOF`
    truthiness now use Variant-native numeric compatibility decoders before
    packing the file handle request or normalizing the host `EOF` result to a
    Boolean Variant. These remain explicit file-control compatibility
    classifications, but no longer require `Variant -> RuntimeValue` detours at
    the VM/JIT boundary.
87. Standard HAL `CreateObject` ProgID coercion now uses a Variant-native
    VBA-string conversion helper and returns BSTR carriers directly from
    retained `Variant` inputs. VM/JIT `CreateObject` dispatch already writes
    returned object Variants directly; this removes the remaining
    `Variant -> RuntimeValue -> Variant` ProgID coercion detour from the
    standard activation path.
88. Null, WASM, replay, and recording COM adapters now override
    `create_object_variant` directly instead of inheriting the compatibility
    fallback that projected through `RuntimeValue`. Unsupported adapters
    preserve the same `create_object` capability fault, and recording delegates
    to the wrapped host's retained-Variant activation path.
89. Standard console HAL `print_line_variant`, `input_fields_variant`, and
    `line_input_variant` now implement their Variant companion paths directly.
    Console display formatting, input field parsing, BSTR returns, and status
    returns no longer require the trait-level `Variant -> RuntimeValue ->
    Variant` fallback for stdio hosts.
90. Standard UI HAL `msg_box_variant` and `input_box_variant` now implement
    retained Variant handling directly. Prompt/default display formatting,
    callback dispatch, scripted responses, and BSTR/string returns no longer
    require the trait-level `Variant -> RuntimeValue -> Variant` fallback for
    UI interaction hosts.
91. Standard event-pump, diagnostics, and time/locale HAL Variant companions
    now return retained Variant carriers directly. `do_events_variant`,
    `emit_variant`, `debug_print_variant`, `date_serial_now_variant`,
    `time_serial_now_variant`, and `timer_ticks_variant` no longer require the
    trait-level `Variant -> RuntimeValue -> Variant` fallback for their status
    and date/time payloads.
92. Standard process/environment HAL `shell_variant`, `environ_variant`, and
    `dir_variant` now implement retained Variant handling directly. Command,
    environment-key, and directory path/string carriers stay as Variants through
    deterministic and native process/environment dispatch instead of inheriting
    the trait-level `Variant -> RuntimeValue -> Variant` fallback.
93. Standard file-system status/handle HAL companions `close_variant`,
    `eof_variant`, `lof_variant`, `free_file_variant`, and `loc_variant` now
    return retained Variant status and handle-position carriers directly. This
    removes the trait-level fallback projection from the first standard
    file-system subset; text/byte payload companions remain a separate
    migration slice.
94. Standard file-system `open_variant` and `kill_variant` now implement
    retained Variant path/mode handling directly. Deterministic token-backed
    opens, string path opens, requested-handle encoding, and string-only kill
    paths no longer inherit the trait-level `Variant -> RuntimeValue ->
    Variant` fallback; text/byte payload companions remain pending.
95. Standard file-system text/byte payload companions `read_bytes_variant`,
    `write_bytes_variant`, `print_line_variant`, `input_fields_variant`, and
    `line_input_variant` now implement retained Variant handling directly.
    File read/write text payloads, quoted `Write#` formatting, `Print#`
    payloads, `Input#` field parsing, and `Line Input#` BSTR returns no longer
    inherit the trait-level `Variant -> RuntimeValue -> Variant` fallback.
96. VM and JIT error-state reads now write retained `Variant` carriers for
    `Err.Number`, `Err.Description`, and `Err.Source` instead of constructing
    temporary `RuntimeValue` scalar/string results. VM `LoadNull`,
    `TypeOf...Is`, `IsNull`, and `IsEmpty` result writes now also use direct
    `Variant` slot carriers, leaving broader comparison/Boolean operators on
    their existing compatibility helper path for a separate migration slice.
97. JIT constant-load, null-load, array-bound, and collection scalar-result
    helpers now write retained `Variant` destination carriers directly.
    `oxrt_load_i32`, `oxrt_load_bool`, `oxrt_load_null`, `oxrt_lbound`,
    `oxrt_ubound`, and the JIT collection add/item/remove/count helpers no
    longer create temporary `RuntimeValue` destination payloads before the
    slot write.
98. VM/JIT locally computed string/scalar intrinsic result writes now use
    retained `Variant` carriers for the string helper subset that computes
    results inline: constant string/double loads, `Len`, `Left`, `Right`,
    `Mid`, `InStr`, `InStrRev`, `LCase`, `UCase`, `Replace`, `Trim`,
    `LTrim`, `RTrim`, and `StrComp`. Semantic helper-returning string paths
    such as `Split`/`Join`/`Like` remain separate migration work because those
    helpers still return `RuntimeValue` by contract.
99. VM/JIT locally computed date/format scalar result writes now use retained
    `Variant` carriers for `DateDiff`, `Year`, `Month`, `Day`, `Weekday`,
    `Format`, and `StrReverse`. Date/time helpers that still receive a
    `RuntimeValue` from a semantic helper, such as `DateSerial`, `DateAdd`,
    `DateValue`, `CDate`, `TimeValue`, and `MonthName`, remain explicit
    semantic-helper companion work.
100. VM/JIT PRNG scalar result writes now use retained `Variant` carriers for
     `Rnd` and `Randomize`, and the VM `Int` F64 fast branch now writes its
     integer result as a retained `Variant` instead of creating a temporary
     `RuntimeValue`. Math/random paths that still return through semantic
     helpers remain explicit companion work.
101. VM/JIT comparison and logical Boolean result writes now use retained
     Boolean `Variant` carriers. VM slow and typed-fast comparison paths,
     VM `BoolNot`/`BoolAnd`/`BoolOr`, and JIT comparison/logical runtime
     helpers no longer materialize temporary `RuntimeValue` or integer
     stand-ins for normal Boolean destination slots.
102. VM constant-load destination writes now use retained `Variant` carriers
     for integer/tag, Boolean, string, and F64 constants while preserving the
     existing `NULL_TAG` integer-constant compatibility behavior. VM `For Each`
     next-item delivery still keeps the iterator ID as an internal control
     token, but the loop-continuation Boolean flag now writes a retained
     Boolean `Variant`.
103. VM/JIT slot-copy helpers now copy retained `Variant` carriers directly.
     VM `CopySlot` and JIT `oxrt_copy_slot` no longer project the source slot
     through `RuntimeValue` before writing the destination slot.
104. VM/JIT core arithmetic destination writes now re-enter retained `Variant`
     carriers after the existing semantic arithmetic helpers return. This
     covers VM/JIT add/subtract/multiply/divide/integer-divide/modulo/power,
     negation, concatenation, and JIT add/subtract/increment helper writes.
     The arithmetic semantic helpers themselves still return `RuntimeValue`,
     so true Variant-native arithmetic companion helpers remain open work.
105. VM/JIT bounded math and VM string-conversion helper result writes now also
     re-enter retained `Variant` carriers after the existing semantic helper
     result. This covers VM `Abs`, `Sgn`, `Round`, `Sqr`, `Sin`, `Cos`, `Log`,
     `Exp`, `Atn`, `Tan`, `Chr`, `Asc`, `Space`, `String$`, `CStr`, `Str`,
     `Val`, `CDate`, `Hex`, `Oct`, and `MonthName` destination writes, plus
     JIT `Abs`, `Sgn`, and `Int`/`Fix` destination writes. The helper bodies
     still return `RuntimeValue`, so exact Variant-native helper companions
     remain open.
106. VM/JIT helper-result destinations now also re-enter retained `Variant`
     carriers for the remaining migrated string/date/time/math helper subset.
     This covers VM `Mid` statement, `Split`, `Join`, `Like`, `DateSerial`,
     `TimeSerial`, `DateValue`, `TimeValue`, `DateAdd`, `StrConv`, and
     increment writes, plus JIT `Split`, `Join`, `Like`, `StrConv`, `Chr`,
     `Asc`, `Space`, `String$`, `Hex`, `Oct`, `DateSerial`, `TimeSerial`,
     `DateValue`, `CDate`, `TimeValue`, `DateAdd`, `MonthName`, `Round`,
     `Sqr`, `Sin`, `Cos`, `Log`, `Exp`, `Atn`, and `Tan`. The semantic helper
     bodies still return `RuntimeValue`, so exact Variant-native helper
     companions remain open.
107. VM/JIT financial helper destination writes now use retained `Variant`
     carriers directly from compatibility-slot results. This covers VM/JIT
     `FV`, `PV`, `PMT`, `NPV`, `IRR`, `MIRR`, `Rate`, and `NPer`, preserving
     existing compatibility-tag meanings through `Variant::from_compat_slot_i32`
     instead of routing successful destination writes through
     `RuntimeValue::from_compat_slot_i32`.
108. Shape-only `SafeArray` constructors now use the Variant-native allocation
     path directly. `SafeArray::vector`, `SafeArray::from_shape`, and
     `SafeArray::from_shape_typed` no longer enter the legacy
     `RuntimeValue`-named construction helper for empty payload allocation.
     The public `RuntimeValue` compatibility constructors and accessors remain
     open classification work.
109. VM/JIT public snapshot APIs now classify `RuntimeValue`-returning entry
     points as compatibility projection boundaries in code comments, with
     retained `Variant` snapshot APIs documented as the preferred value-model
     surface. This does not remove the legacy aliases; it makes their boundary
     role explicit before any later API narrowing.
110. HAL trait surfaces now classify `RuntimeValue` methods as compatibility
     projection contracts and `_variant` companion methods as retained
     value-model entry points for VM/JIT callers. Default companion methods
     that still project through the compatibility contract remain open
     migration/classification work and do not close `vmm-e6`.
111. Public `SafeArray` `RuntimeValue` constructors/accessors now classify
     themselves as compatibility projection APIs and point new value-model call
     sites at the retained `Variant` constructors/accessors. This covers
     `from_values`, `from_values_nd`, `from_typed_values`,
     `from_typed_values_nd`, `from_shape_and_values`, `elements`, and
     `replace_elements`; the APIs remain available for legacy callers and do
     not close `vmm-e6`.
112. Host-facing project-runtime, immediate-session, and embedded invocation
     `RuntimeValue` surfaces now classify themselves as compatibility
     projections and point retained-value callers at `Variant` snapshot,
     request, and result APIs. This covers `ProjectRuntimeSession` snapshots
     and slot reads, source/project/bundle execution snapshot aliases,
     immediate session snapshots and evaluation projection fields, and embedded
     procedure request/result projection helpers. The compatibility APIs remain
     available for legacy callers and do not close `vmm-e6`.
113. COM model `RuntimeValue` and legacy-token helpers now classify themselves
     as compatibility projections around retained `Variant` invoke/callback
     payloads. This covers `ComValue::from_runtime_value`,
     `ComValue::from_runtime_token`, `ComValue::to_runtime_value`,
     `ComValue::to_runtime_token`, `ComValue::to_legacy_dispatch_token`,
     `ComInvokeValue` token projections, legacy `ComInvokeArg` constructors,
     `ComInvokeRequest::legacy`, and retained `ComInvokeValue` /
     `ComCallbackValue` payload accessors. The APIs remain available for
     legacy callers and do not close `vmm-e6`.
114. Windows COM bridge/invoke `RuntimeValue` result and callback-argument
     APIs now classify themselves as compatibility projections beside retained
     `Variant`/`ComValue` transport. This covers
     `event_callback_arg`, `event_callback_variant`,
     `dispatch_invoke_runtime_value`, `dispatch_invoke_dynamic_runtime_value`,
     `invoke_dispatch_runtime_value`, `invoke_member_spec_runtime_value`, and
     `invoke_direct_dispid_runtime_value`. The compatibility APIs remain
     available for legacy callers and do not close `vmm-e6`.
115. Dynamic COM value and portable dispatch surfaces now classify
     `RuntimeValue` entry points as compatibility projections around retained
     `Variant`/`ComValue` carriers. This covers
     `DynamicValue::from_runtime_value`, `DynamicValue::to_runtime_value`,
     retained `DynamicValue` `Variant` access, and the portable
     `PortableDispatch` trait boundary. The compatibility APIs remain
     available for legacy callers and do not close `vmm-e6`.
116. VM legacy scalar helper writes now materialize compatibility-tagged
     `Variant` slots directly. `write_legacy_scalar_slot` still represents a
     compatibility-token lane, but it no longer routes the destination write
     through a temporary `RuntimeValue::from_compat_slot_i32` carrier.
117. Runtime `Variant`/`RuntimeValue` bridge helpers now classify the retained
     `Variant` carrier as primary and the `RuntimeValue`/i32 slot-token routes
     as compatibility projections. This covers `Variant::{try_from_runtime_value,
     from_runtime_value,to_runtime_value,try_from_compat_slot_i32,
     from_compat_slot_i32}` and `RuntimeValue::{to_variant,from_variant,
     from_compat_slot_i32,project_compat_slot_i32}`.
118. JIT/Cranelift `RuntimeValue` execution and slot helper APIs now classify
     themselves as compatibility projections over retained `Variant` execution
     APIs. This covers `execute_bytecode`, `execute_bytecode_rtslot`,
     `JitContextOwned::extract_user_values`, `JitContext::read_slot`, and
     `JitContext::write_slot`.
119. VM `RuntimeSlot` and JIT `RtSlot` conversion helpers now classify
     `RuntimeValue` and i32 slot-token entry/exit points as compatibility
     projections around retained `Variant` slot carriers. `BindingHandle`
     remains an internal side-lane because it is not a VBA/COM value.
120. Runtime pointer-helper `RuntimeValue` registration/readback APIs now
     classify themselves as compatibility projections beside retained
     `Variant` pointer-helper APIs. This covers runtime-value pointer
     registration, string/Variant/object variable pointer wrappers, legacy
     direct-projection pointer wrappers, and string/byte-array readback
     wrappers.
121. HAL standard process legacy `RuntimeValue` methods now classify
     themselves as compatibility wrappers beside retained `Variant` process
     APIs. This covers `shell`, `environ`, and `dir`; behavior is preserved so
     existing compatibility slot-token projection remains unchanged.
122. VM shared semantic helpers and JIT runtime helper bridges now classify
     `RuntimeValue` helper contracts as compatibility layers over retained
     `Variant` slot storage. This documents the remaining helper families as
     migration work without confusing them with retained VM/JIT storage.
123. HAL dynamic-link legacy `RuntimeValue` trait methods/default adapters and
     standard adapter hooks now classify themselves as compatibility layers
     beside retained `Variant` invoke paths. This covers binding-token
     projection, legacy prepare/invoke paths, and Variant-to-runtime default
     adapters used only when an implementation has not overridden the retained
     Variant methods.
124. HAL diagnostics, UI, event-pump, and time legacy `RuntimeValue` methods
     now classify themselves as compatibility wrappers beside retained
     `Variant` companion methods. This covers telemetry/debug print, message
     box/input box, DoEvents, and Date/Time/Timer APIs.
125. HAL filesystem legacy `RuntimeValue` methods now classify themselves as
     compatibility wrappers beside retained `Variant` filesystem companion
     methods. This covers open/close/kill/seek, EOF/LOF/Loc/FreeFile, byte
     I/O, Print/Input, and Line Input APIs.
126. HAL console legacy `RuntimeValue` methods now classify themselves as
     compatibility wrappers beside retained `Variant` console companion
     methods. This covers Print, Input, Line Input, and the legacy input-field
     parser.
127. HAL COM activation/dispatch/event legacy `RuntimeValue` methods now
     classify themselves as compatibility result projections beside retained
     `Variant` COM companion methods. This covers object activation/release,
     static/dynamic dispatch result projection, event unsubscribe, event
     callback argument projection, and callback release status.
128. Host debugger `RuntimeValue` frame/evaluation APIs now classify
     themselves as compatibility projections from retained `Variant` frame
     reads. `DebugFrameVariantValue` remains the retained value read shape, and
     `DebugFrameValue`/evaluation results preserve existing debugger clients.
129. Non-standard HAL null/WASM/replay adapter `RuntimeValue` methods now
     classify themselves as compatibility wrappers beside retained `Variant`
     companion methods. Replay journal `RuntimeValue` decoding remains a legacy
     journal parser, while replay `_variant` entry points retain direct
     value-model companions for adapter callers.
130. VM/JIT string slice intrinsics `Len`, `Left`, `Right`, and `Mid` now read
     retained `Variant` slots directly through Variant-native text/count
     coercion helpers. This removes the first string-intrinsic helper group from
     the `RuntimeValue` projection path while preserving legacy snapshot
     projections for public compatibility APIs.
131. VM/JIT text transform/search intrinsics `InStr`, `InStrRev`, `LCase`,
     `UCase`, `Replace`, `Trim`, `LTrim`, `RTrim`, `StrComp`, and
     `StrReverse` now read retained `Variant` slots directly through
     Variant-native text coercion helpers. Broader string semantic helpers that
     still return `RuntimeValue` remain explicitly outside this slice.
132. VM/JIT char/format-adjacent intrinsics `Chr`, `Asc`, `Space`, `String$`,
     `Hex`, `Oct`, and `MonthName` now read retained `Variant` slots directly
     through Variant-native coercion helpers and write retained `Variant`
     results directly. `Val`, `CStr`, `Str$`, `Format`, `StrConv`, `Like`,
     date/math, and array/string aggregate helpers remain outside this slice
     where they still use broader legacy semantic contracts.
133. VM/JIT `Like` and `StrConv` now read retained `Variant` slots directly
     through Variant-native text/conversion coercion helpers and write retained
     `Variant` results directly. `Val`, `CStr`, `Str$`, `Format`, date/math,
     and array/string aggregate helpers remain outside this slice where they
     still use broader legacy semantic contracts.
134. VM/JIT `Format` now reads retained `Variant` value/format slots directly
     through Variant-native numeric/text coercion helpers and writes retained
     `Variant` string results directly. `Val`, `CStr`, `Str$`, date/math, and
     array/string aggregate helpers remain outside this slice where they still
     use broader legacy semantic contracts.
135. VM/JIT date/time intrinsics `DateSerial`, `TimeSerial`, `DateValue`,
     `TimeValue`, `DateAdd`, `DateDiff`, `Year`, `Month`, `Day`, `Weekday`,
     and the JIT `CDate` helper now read retained `Variant` slots directly
     through Variant-native date/time coercion helpers and write retained
     `Variant` results directly. `Val`, `CStr`, `Str$`, math, and
     array/string aggregate helpers remain outside this slice where they still
     use broader legacy semantic contracts.
136. VM/JIT math intrinsics `Abs`, `Sgn`, `Round`, `Sqr`, `Sin`, `Cos`,
     `Log`, `Exp`, `Atn`, and `Tan` now read retained `Variant` slots directly
     through Variant-native numeric coercion helpers and write retained
     `Variant` results directly. `Val`, `CStr`, `Str$`, and array/string
     aggregate helpers remain outside this slice where they still use broader
     legacy semantic contracts.
137. VM-only conversion intrinsics `CStr`, `Str$`, `Val`, and `CDateValue`
     now read retained `Variant` slots directly through Variant-native
     conversion helpers and write retained `Variant` results directly.
     Array/string aggregate helpers remain outside this slice where they still
     use broader legacy semantic contracts.
138. VM/JIT aggregate string intrinsics `Mid` statement, `Split`, and `Join`
     now read retained `Variant` slots directly through Variant-native
     string/array helpers and write retained `Variant` results directly.
     Remaining broad projection seams are arithmetic/operator reads,
     binding/COM, random seed, dynamic array bounds/constructor, and
     compatibility APIs.
139. VM/JIT core arithmetic operators `Add`, `Sub`, `Mul`, `Div`, `IntDiv`,
     `Mod`, `Pow`, `Concat`, `Neg`, `AddConst`, `SubConst`, and `Inc` now read
     retained `Variant` slots directly through Variant-native arithmetic
     helpers and write retained `Variant` results directly. Remaining broad
     projection seams include comparison/logical operator reads, binding/COM,
     random seed, dynamic array bounds/constructor, and compatibility APIs.
140. VM/JIT comparison and Boolean operators now read retained `Variant` slots
     directly through Variant-native comparison/truthiness helpers and write
     retained `Variant` Boolean results directly. Remaining broad projection
     seams include binding/COM, random seed, dynamic array bounds/constructor,
     and compatibility APIs.
141. VM/JIT `Rnd` and `Randomize` seed operands now read retained `Variant`
     slots directly through Variant-native numeric seed coercion while
     preserving retained `Variant` result writes. Remaining broad projection
     seams include binding/COM, dynamic array bounds/constructor, and
     compatibility APIs.

Remaining blocker:

1. This does not close `vmm-e6`.
2. `RuntimeValue` remains a semantic projection type used across interpreter
   helper functions, JIT helper functions, legacy `SafeArray`
   compatibility element APIs, and legacy dynamic-link compatibility APIs. VM register
   storage, JIT slot storage, `For Each` iterator storage,
   VM/JIT WithEvents binding storage, VM/JIT descriptor external-call transport,
   VM/JIT COM event callback argument transport, and VM dynamic-dispatch
   `ParamArray` SAFEARRAY payload construction, and VM project-dynamic callee
   argument binding, VM/JIT dynamic COM result slot writes, JIT WithEvents
   retained-value get/set/owner-search, VM `For Each` next-item delivery, VM
   WithEvents retained-value get/set/owner-search, VM project-dynamic dispatch
   return/destination writes, VM/JIT result extraction companion APIs, VM/JIT
   public execution snapshot companion APIs, host/project snapshot companion
   APIs, host bundle snapshot companion APIs, immediate-session snapshot
   companion APIs, `ComValue` Variant bridge conversions, embedded host
   procedure invocation companion APIs, immediate procedure invocation, host
   class member invocation, host COM callback ingress/dispatch, and VM/JIT
   no-descriptor dynamic-link symbol invocation, and HAL dynamic COM bridge
   invocation/release, VM/JIT `CreateObject` host-return storage, and VM/JIT
   COM event unsubscribe/release status writes, and VM/JIT WithEvents
   owner-iteration/status outputs, and VM `TypeOf...Is` object operand lookup
   and VM project dynamic dispatch implicit `Me` binding/default optional
   argument binding, and VM project COM WithEvents callback inline arguments,
   and host event ingress guarded dispatch arguments, and host class
   initializer lifecycle dispatch, VM/JIT console `Print` host helper
   dispatch, VM/JIT UI `MsgBox` / `InputBox` host helper dispatch, VM/JIT
   diagnostics/event-pump host helper dispatch, VM/JIT process/environment
   host helper dispatch, VM/JIT time/locale host helper dispatch, VM/JIT
   file-system host helper dispatch, VM/JIT console input/line-input host
   helper dispatch, and VM/JIT `Beep` diagnostics host helper dispatch no
   longer retain it as their backing value store for normal VBA values.
   VM/JIT dynamic COM dispatch argument payloads now also enter
   `DynamicValue` as retained Variants rather than through `RuntimeValue`.
   VM project COM WithEvents callback polling now also consumes retained
   Variant callback-token carriers from the event pump.
   VM/JIT COM event subscription/callback helper token carriers now also stay
   in retained Variant slots at the VM/JIT boundary.
   VM/JIT dynamic COM dispatch object/member selector carriers now also stay
   in retained Variant slots at the VM/JIT boundary.
   VM/JIT WithEvents owner/source object helper carriers now also stay in
   retained Variant slots at the VM/JIT boundary.
   VM/JIT COM event callback-argument index carriers now also stay in retained
   Variant slots at the VM/JIT boundary.
   VM/JIT `Now` host-helper date/time carrier combination now also stays in
   retained Variant form at the VM/JIT boundary.
   VM/JIT file-position `Loc + 1` carrier handling now also stays in retained
   Variant form at the VM/JIT boundary.
   VM/JIT file `Open` mode/file-number control carriers and `EOF` truthiness
   now also stay in retained Variant form at the VM/JIT boundary before their
   explicit numeric/Boolean compatibility classifications.
   Standard HAL `CreateObject` ProgID coercion now also stays in retained
   Variant/BSTR form before activation dispatch.
   Non-standard COM activation adapters now also keep `create_object_variant`
   on explicit retained-Variant paths or explicit unsupported Variant faults.
   Standard console `Print`, `Input`, and `Line Input` companion dispatch now
   also stays in retained Variant form through the HAL adapter boundary.
   Standard UI `MsgBox` and `InputBox` companion dispatch now also stays in
   retained Variant form through the HAL adapter boundary.
   Standard event-pump, diagnostics, and time/locale companion dispatch now
   also stays in retained Variant form through the HAL adapter boundary.
   Standard process/environment companion dispatch now also stays in retained
   Variant form through the HAL adapter boundary.
   Standard file-system close/status/free-handle/location companion dispatch
   now also stays in retained Variant form through the HAL adapter boundary;
   standard file-system open/kill companion dispatch now also stays in
   retained Variant form through the HAL adapter boundary; standard file
   text/byte payload companion dispatch now also stays in retained Variant form
   through the HAL adapter boundary.
   Null/WASM/replay time/locale, diagnostics, and dynamic-link Variant
   companions now also return retained Variant carriers or explicit unsupported
   Variant faults instead of inheriting trait-level `RuntimeValue` fallback
   projections.
   Null/WASM/replay console, UI, event-pump, file-system, and
   process/environment Variant companions now also return retained Variant
   carriers, replay journal carriers, or explicit unsupported Variant faults
   without inheriting trait-level `RuntimeValue` fallback projections.
   VM/JIT pointer-helper result writes and retained-Variant classifier/tag
   result writes now also use direct `Variant` slot carriers for
   `StrPtr`/`VarPtr`/`ObjPtr`, `LBound`/`UBound`, `VarType`, `TypeName`,
   `IsNumeric`, `IsError`, `IsDate`, `IsObject`, `IsNull`, `IsEmpty`, and
   `IsArray` instead of creating temporary `RuntimeValue` scalar results.
   VM/JIT `Err.*` reads and VM `LoadNull`, `TypeOf...Is`, `IsNull`, and
   `IsEmpty` result writes now also stay on retained `Variant` carriers.
   JIT constant/null loads, `LBound`/`UBound`, and collection scalar-result
   writes now also stay on retained `Variant` carriers.
   VM/JIT locally computed string/scalar intrinsic destination writes now also
   stay on retained `Variant` carriers for the inline string helper subset.
   VM/JIT locally computed date/format destination writes now also stay on
   retained `Variant` carriers for `DateDiff`, `Year`, `Month`, `Day`,
   `Weekday`, `Format`, and `StrReverse`.
   VM/JIT PRNG destination writes now also stay on retained `Variant`
   carriers for `Rnd` and `Randomize`, and the VM `Int` F64 fast branch now
   writes its result as a retained `Variant`; broader math/random
   semantic-helper-returning paths remain companion work.
   VM/JIT comparison and logical Boolean result writes now also stay on
   retained Boolean `Variant` carriers for comparison operators and
   `BoolNot`/`BoolAnd`/`BoolOr`.
   VM constant-load destination writes now also stay on retained `Variant`
   carriers for integer/tag, Boolean, string, and F64 constants, and VM
   `For Each` next-item delivery now writes the continuation flag as a
   retained Boolean `Variant` while leaving the iterator ID classified as an
   internal control token.
   VM/JIT slot-copy helpers now also preserve retained `Variant` carriers
   directly instead of copying through `RuntimeValue` projection.
   VM/JIT core arithmetic destination writes now also re-enter retained
   `Variant` carriers after existing semantic helper results, while the
   arithmetic helper-returning contracts remain separate Variant-native
   companion work.
   VM/JIT bounded math and VM string-conversion helper destination writes now
   also re-enter retained `Variant` carriers after existing semantic helper
   results, while their helper-returning contracts remain Variant-native
   companion work.
   VM/JIT remaining migrated string/date/time/math helper destination writes
   now also re-enter retained `Variant` carriers after existing semantic
   helper results, while their helper-returning contracts remain
   Variant-native companion work.
   VM/JIT financial helper destination writes now also use retained `Variant`
   carriers directly from compatibility-slot results while preserving the
   existing compatibility-tag result meanings.
   Shape-only `SafeArray` constructors now also allocate through the
   Variant-native path directly instead of entering the legacy
   `RuntimeValue`-named helper for empty payloads.
   VM/JIT public snapshot APIs now also document retained `Variant` snapshot
   APIs as the value-model surface and `RuntimeValue` snapshots as
   compatibility projections.
   HAL trait surfaces now also document `RuntimeValue` methods as
   compatibility projection contracts and `_variant` companions as retained
   value-model entry points; default projection companions remain open
   migration/classification work.
   Public `SafeArray` `RuntimeValue` constructors/accessors now also document
   their compatibility projection role and point retained-value callers at the
   matching `Variant` APIs.
   Host project-runtime, immediate-session, and embedded invocation
   `RuntimeValue` snapshots/requests/results now also document their
   compatibility projection role and point retained-value callers at `Variant`
   APIs.
   COM model `RuntimeValue` and legacy-token helpers now also document their
   compatibility projection role around retained `Variant` invoke/callback
   payloads.
   Windows COM bridge/invoke `RuntimeValue` result and callback-argument APIs
   now also document their compatibility projection role beside retained
   `Variant`/`ComValue` transport.
   Dynamic COM value and portable dispatch surfaces now also document their
   compatibility projection role around retained `Variant`/`ComValue`
   carriers.
   VM legacy scalar helper writes now also materialize compatibility-tagged
   `Variant` slots directly instead of routing through a temporary
   `RuntimeValue` carrier.
   Runtime `Variant`/`RuntimeValue` bridge helpers now also document retained
   `Variant` as the primary carrier and `RuntimeValue`/i32 slot-token routes
   as compatibility projections.
   JIT/Cranelift `RuntimeValue` execution and slot helper APIs now also
   document their compatibility projection role over retained `Variant`
   execution APIs.
   VM `RuntimeSlot` and JIT `RtSlot` `RuntimeValue`/i32 conversion helpers now
   also document their compatibility ingress/egress role around retained
   `Variant` slot carriers.
   Runtime pointer-helper `RuntimeValue` registration/readback APIs now also
   document their compatibility projection role beside retained `Variant`
   pointer-helper APIs.
   HAL standard process legacy `RuntimeValue` methods now also document their
   compatibility wrapper role beside retained `Variant` process APIs.
   VM shared semantic helpers and JIT runtime helper bridges now also document
   their compatibility layer role over retained `Variant` slot storage.
   HAL dynamic-link legacy `RuntimeValue` methods/default adapters now also
   document their compatibility layer role beside retained `Variant` invoke
   paths.
   HAL diagnostics, UI, event-pump, and time legacy `RuntimeValue` methods now
   also document their compatibility wrapper role beside retained `Variant`
   companion methods.
   HAL filesystem legacy `RuntimeValue` methods now also document their
   compatibility wrapper role beside retained `Variant` filesystem companion
   methods.
   HAL console legacy `RuntimeValue` methods now also document their
   compatibility wrapper role beside retained `Variant` console companion
   methods.
   HAL COM activation/dispatch/event legacy `RuntimeValue` methods now also
   document their compatibility projection role beside retained `Variant` COM
   companion methods.
   Host debugger `RuntimeValue` frame/evaluation APIs now also document their
   compatibility projection role from retained `Variant` frame reads.
   Debugger frame value projection now starts from Variant slot reads before
   compatibility projection.
   Non-standard HAL null/WASM/replay adapter `RuntimeValue` methods now also
   document their compatibility wrapper role beside retained `Variant`
   companion methods and legacy replay journal parsing.
   VM/JIT string slice intrinsics `Len`, `Left`, `Right`, and `Mid` now read
   retained `Variant` slots directly through Variant-native text/count
   coercion helpers instead of projecting through `RuntimeValue` first.
   VM/JIT text transform/search intrinsics `InStr`, `InStrRev`, `LCase`,
   `UCase`, `Replace`, `Trim`, `LTrim`, `RTrim`, `StrComp`, and `StrReverse`
   now read retained `Variant` slots directly through Variant-native text
   coercion helpers.
   VM/JIT char/format-adjacent intrinsics `Chr`, `Asc`, `Space`, `String$`,
   `Hex`, `Oct`, and `MonthName` now read retained `Variant` slots directly
   through Variant-native coercion helpers and write retained `Variant`
   results directly.
   VM/JIT `Like` and `StrConv` now read retained `Variant` slots directly
   through Variant-native text/conversion coercion helpers and write retained
   `Variant` results directly.
   VM/JIT `Format` now reads retained `Variant` value/format slots directly
   through Variant-native numeric/text coercion helpers and writes retained
   `Variant` string results directly.
   VM/JIT date/time intrinsics `DateSerial`, `TimeSerial`, `DateValue`,
   `TimeValue`, `DateAdd`, `DateDiff`, `Year`, `Month`, `Day`, `Weekday`, and
   the JIT `CDate` helper now read retained `Variant` slots directly through
   Variant-native date/time coercion helpers and write retained `Variant`
   results directly.
   VM/JIT math intrinsics `Abs`, `Sgn`, `Round`, `Sqr`, `Sin`, `Cos`, `Log`,
   `Exp`, `Atn`, and `Tan` now read retained `Variant` slots directly through
   Variant-native numeric coercion helpers and write retained `Variant`
   results directly.
   VM-only conversion intrinsics `CStr`, `Str$`, `Val`, and `CDateValue` now
   read retained `Variant` slots directly through Variant-native conversion
   helpers and write retained `Variant` results directly.
   VM/JIT aggregate string intrinsics `Mid` statement, `Split`, and `Join`
   now read retained `Variant` slots directly through Variant-native
   string/array helpers and write retained `Variant` results directly.
   VM/JIT core arithmetic operators `Add`, `Sub`, `Mul`, `Div`, `IntDiv`,
   `Mod`, `Pow`, `Concat`, `Neg`, `AddConst`, `SubConst`, and `Inc` now read
   retained `Variant` slots directly through Variant-native arithmetic helpers
   and write retained `Variant` results directly.
   VM/JIT comparison and Boolean operators now read retained `Variant` slots
   directly through Variant-native comparison/truthiness helpers and write
   retained `Variant` Boolean results directly.
   VM/JIT `Rnd` and `Randomize` seed operands now read retained `Variant`
   slots directly through Variant-native numeric seed coercion while preserving
   retained `Variant` result writes.
3. `SafeArray` still stores local ownership metadata adjacent to the
   descriptor; the descriptor and payload are native-shaped, but exact
   cross-platform `SAFEARRAY` identity still needs a final ownership/metadata
   audit before closure can be claimed.
4. Completion still requires an audit and migration/classification of all
   remaining projection seams that can expose or retain general values:
   interpreter/JIT helper internals outside slot storage, host and immediate
   surfaces that still use semantic values by contract, HAL surfaces that still
   use semantic values by contract, remaining host service helper families,
   legacy dynamic-link compatibility APIs, legacy
   `SafeArray` element compatibility APIs documented in
   `SAFEARRAY_RUNTIMEVALUE_PROJECTION_AUDIT_2026-04-23.md`, COM compatibility
   projection APIs that still expose `RuntimeValue`, legacy COM dispatch
   `RuntimeValue` compatibility methods, embedded/immediate compatibility APIs
   that still expose `RuntimeValue`, and any remaining non-Variant
   pointer-helper/manual registry compatibility utilities that still accept
   semantic values by contract.
5. Public VM/JIT compatibility snapshot APIs still expose `RuntimeValue`
   compatibility results. They now project from Variant-backed companion
   surfaces, but they remain open classification work before the final `vmm-e6`
   closure checklist.
6. Post-run host evidence for `VarPtr(Variant)` must treat the returned pointer
   as an actual VM/JIT slot address rather than a long-lived registry-owned
   helper cell. Runtime unit coverage still verifies the Windows container-cell
   materialization path separately via pointer-helper boundary helpers.
6. `BindingHandle` remains intentionally outside the VBA/COM value model; JIT
   slot writes project it to `VT_I4` rather than inventing a custom VARIANT
   tag, while retained internal side lanes keep it separate where needed.
