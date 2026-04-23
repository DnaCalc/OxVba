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

Validation:

1. `cargo test -p oxvba-runtime --lib`
2. `cargo test -p oxvba-vm --lib intrinsic_array_resize_1d_materializes_zeroed_byte_payload`
3. `cargo test -p oxvba-vm --lib runtime_redim_preserve_1d_retains_overlapping_byte_values`
4. `cargo test -p oxvba-vm --lib`
   - result: `77` passed, `1` ignored, `2` failed
   - the two failures are machine-local COM registration failures for
     `OxVba.TestDispatch`:
     `CLSIDFromProgID failed for OxVba.TestDispatch with HRESULT 0x800401F3`

Remaining blocker:

1. This does not close `vmm-e6`.
2. `RuntimeValue` remains a semantic projection type used across interpreter
   helper functions, host callbacks, `SafeArray` element construction APIs, and
   `ComValue` bridges.
3. `SafeArray` still stores local ownership metadata adjacent to the
   descriptor; the descriptor and payload are native-shaped, but exact
   cross-platform `SAFEARRAY` identity still needs a final ownership/metadata
   audit before closure can be claimed.
4. Completion still requires the internal late-bound/general value used for
   `Dim x` to be exactly Windows/COM `VARIANT` as the actual carrier, not only
   VM register storage plus semantic projections.
