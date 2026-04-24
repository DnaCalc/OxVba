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
   invocation no longer retain it as their backing value store for normal VBA
   values.
   Debugger frame value projection now starts from Variant slot reads before
   compatibility projection.
3. `SafeArray` still stores local ownership metadata adjacent to the
   descriptor; the descriptor and payload are native-shaped, but exact
   cross-platform `SAFEARRAY` identity still needs a final ownership/metadata
   audit before closure can be claimed.
4. Completion still requires an audit and migration/classification of all
   remaining projection seams that can expose or retain general values:
   interpreter/JIT helper internals outside slot storage, host and immediate
   surfaces that still use semantic values by contract, HAL surfaces that still
   use semantic values by contract, legacy dynamic-link compatibility APIs, legacy
   `SafeArray` element compatibility APIs documented in
   `SAFEARRAY_RUNTIMEVALUE_PROJECTION_AUDIT_2026-04-23.md`, COM compatibility
   projection APIs that still expose `RuntimeValue`, legacy COM dispatch
   `RuntimeValue` compatibility methods, embedded/immediate compatibility APIs
   that still expose `RuntimeValue`, and any remaining non-Variant
   pointer-helper/manual registry utilities that still accept semantic values
   by contract.
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
