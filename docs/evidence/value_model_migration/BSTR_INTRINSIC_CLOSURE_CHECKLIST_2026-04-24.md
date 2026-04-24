# BSTR Intrinsic Closure Checklist (2026-04-24)

Status: `vmm-d8` support checklist.

Scope:

1. canonical runtime string carrier
2. string payloads inside `Variant` and `SafeArray`
3. pointer-helper and COM BSTR boundary behavior
4. remaining bounded string-lane decisions

Checklist:

1. The canonical OxVba string carrier is an owned BSTR payload, not a Rust
   `String`, BSTR-shaped side buffer, or boundary-only projection.
   - Result: `implemented`
   - Evidence:
     - [bstr.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/bstr.rs)
     - [BSTR_NATIVE_CARRIER_DELIVERY_2026-04-23.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/BSTR_NATIVE_CARRIER_DELIVERY_2026-04-23.md)
     - `BStr` is a pointer-sized raw BSTR owner.
     - Windows builds allocate/clone/measure/free through Automation BSTR APIs.
     - Non-Windows builds use the same length-prefix-plus-UTF-16 payload layout
       as a compatibility emulation.

2. Canonical string payloads inside the value model reuse the intrinsic BSTR
   carrier rather than introducing a second string truth.
   - Result: `implemented`
   - Evidence:
     - [variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/variant.rs)
     - [safe_array.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/safe_array.rs)
     - `Variant` `VT_BSTR` payloads and SAFEARRAY string lanes clone from
       canonical `BStr` values.

3. Pointer-helper and COM BSTR boundary behavior no longer creates the only
   real BSTR representation.
   - Result: `implemented`
   - Evidence:
     - [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
     - [windows_variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_variant.rs)
     - [OXVBA_POINTER_HELPERS_CONTRACT_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/OXVBA_POINTER_HELPERS_CONTRACT_V1.md)
     - `StrPtr`, `VarPtr(String)`, and Windows COM string conversion clone or
       expose from the canonical BSTR payload rather than rebuilding from a
       Rust string carrier.

4. The string-lane outcome is explicit for final report inputs.
   - Result: `implemented`
   - Classification:
     - `implemented`: intrinsic BSTR carrier, Variant string payloads,
       SAFEARRAY string payloads, pointer-helper BSTR payload/cell materialization,
       and COM BSTR projection from the canonical carrier.
     - `projected`: external COM and pointer-helper windows still materialize
       native BSTR cells or temporary boundary views where ABI ownership requires
       a boundary object.
     - `bounded`: Automation allocator caching behavior and byte-oriented
       `SysAllocStringByteLen` edge lanes are not modeled as separate internal
       runtime semantics; they remain boundary/optimization follow-up scope, not
       blockers for the intrinsic carrier.

Validation:

1. `cargo test -p oxvba-runtime --lib`
2. `cargo test -p oxvba-com windows_variant::tests:: --lib`

Decision:

1. `vmm-d8` support checklist is satisfied for the string/BSTR lane.
2. The string/BSTR family can be treated as intrinsically migrated.
3. This does not close the full value-model migration by itself; `vmm-e6` and
   other family closure/checklist beads still control final migration status.
