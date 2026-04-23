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

Remaining blocker:

1. This does not close `vmm-e6`.
2. `RuntimeValue` remains a semantic projection type used across interpreter
   helper functions, JIT helper functions, host callbacks, legacy `SafeArray`
   compatibility element APIs, and `ComValue` bridges. VM register storage, JIT slot
   storage, `For Each` iterator storage, and VM/JIT WithEvents binding storage
   no longer retain it as their backing value store for normal VBA values.
3. `SafeArray` still stores local ownership metadata adjacent to the
   descriptor; the descriptor and payload are native-shaped, but exact
   cross-platform `SAFEARRAY` identity still needs a final ownership/metadata
   audit before closure can be claimed.
4. Completion still requires an audit and migration of all remaining
   projection seams that can expose or retain general values: interpreter/JIT
   helpers, HAL callback surfaces, legacy `SafeArray` element compatibility
   APIs, remaining COM boundary `ComValue` projection points, and non-Variant
   pointer-helper behavior such as `StrPtr`, `ObjPtr`, and generic `VarPtr`
   over non-Variant variables.
5. `BindingHandle` remains intentionally outside the VBA/COM value model; JIT
   slot writes project it to `VT_I4` rather than inventing a custom VARIANT
   tag, while retained internal side lanes keep it separate where needed.
