# Variant/SAFEARRAY Intrinsic Closure Checklist (2026-04-23)

Status: `in-progress` epic, `vmm-e6` delivery satisfied, `vmm-e7` checklist artifact published

Scope:

1. canonical runtime `Variant`
2. canonical runtime `SafeArray`
3. scoped object/string/array payload truth for the value-model migration

Checklist:

1. Canonical runtime `Variant` is no longer only a Windows-shaped core plus semantic side-owned payloads for the remaining hard cases.
   - Result: `implemented`
   - Evidence:
     - [variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/variant.rs)
     - exact 16-byte `Variant` carrier is retained for scalar, string, and `ObjectRef`-backed object lanes
     - clone/drop behavior is driven from payload bytes rather than parallel owned side state

2. String, object, and array payload lanes are represented as part of the intended internal `VARIANT`/`SAFEARRAY` truth rather than only through helper/boundary materialization.
   - Result: `implemented`
   - Evidence:
     - [safe_array.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/safe_array.rs)
     - [windows_variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_variant.rs)
     - intrinsic SAFEARRAY element-vartype preservation now covers:
       - `VT_ARRAY | VT_VARIANT`
       - typed scalar lanes
       - `VT_BSTR`
       - `VT_DECIMAL`
       - `VT_DISPATCH`
       - `VT_UNKNOWN`
     - typed interface arrays preserve their element vartype internally while carrying object identity through the canonical `ObjectRef` / `IUnknown` substrate

3. `SAFEARRAY` interaction is no longer only a boundary truth for the scoped internal array lane.
   - Result: `implemented`
   - Evidence:
     - canonical `SafeArray` now owns a raw SAFEARRAY-style descriptor and contiguous typed payload storage
     - host and bridge lanes assert typed SAFEARRAY results directly instead of normalizing them into semantic-only variant arrays
     - passing checks in this cycle:
       - `cargo test -p oxvba-runtime safe_array -- --test-threads=1`
       - `cargo test -p oxvba-com windows_variant -- --test-threads=1`
       - `cargo test -p oxvba-host --test com_client_end_to_end windows_com_e2e::dispatchinvoke_accepts_typed_dispatch_array_results -- --exact --test-threads=1 --nocapture`
       - `cargo test -p oxvba-host --test com_client_end_to_end windows_com_e2e::dispatchinvoke_accepts_typed_unknown_array_results -- --exact --test-threads=1 --nocapture`
       - `cargo test -p oxvba-host --test com_client_end_to_end windows_com_e2e::dispatchinvoke_accepts_typed_safe_array_variant_results -- --exact --test-threads=1 --nocapture`

4. Non-VBA internal tokens are not silently counted as missing `VARIANT` lanes.
   - Result: `implemented`
   - Evidence:
     - [variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/variant.rs)
     - `BindingHandle` is now explicitly rejected as an internal non-VBA token outside canonical `Variant` scope

Implemented / projected / bounded summary:

1. `implemented`
   - exact 16-byte canonical `Variant` carrier for scoped scalar/string/object lanes
   - intrinsic SAFEARRAY descriptor and typed payload storage
   - intrinsic typed SAFEARRAY element-vartype preservation for scalar/string/decimal/interface lanes in scope
2. `projected`
   - real Windows `VARIANT`, `SAFEARRAY`, `BSTR`, and interface-pointer materialization still occurs at COM/pointer-helper boundaries where native host ABI objects are required
3. `bounded`
   - no remaining bounded caveat is recorded against the scoped canonical Variant/SAFEARRAY carrier itself
   - `BindingHandle` is excluded by scope, not bounded as a missing value-model lane

Decision:

1. `vmm-e6` delivery can be treated as complete.
2. `vmm-e7` remains the explicit workset/checklist bookkeeping step before epic `vmm-e` can close.
