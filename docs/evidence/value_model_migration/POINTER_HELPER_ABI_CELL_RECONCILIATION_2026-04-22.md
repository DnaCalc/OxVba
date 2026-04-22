## Pointer Helper ABI Cell Reconciliation

Date: `2026-04-22`
Bead: `bd-t8rr.7.2` / `vmm-g1`

## Outcome

The pointer-helper ABI-sensitive cell lane is now aligned with the migrated
substrate where the new runtime truth made that possible.

Delivered changes:

1. `ObjPtr(obj)` now returns the raw runtime `IUnknown` identity pointer for a
   live `ObjectRef` instead of a synthesized compat-identity token.
2. generic runtime object pointer cells now store the `IUnknown` pointer value
   when the helper needs variable-storage style indirection.
3. `VarPtr(v As Variant)` now supports object-valued variants by materializing a
   real Windows `VARIANT` container with `VT_UNKNOWN` and a retained
   `IUnknown*`.
4. `VarPtr(v As Variant)` now supports array-valued variants by materializing a
   real Windows `VARIANT` container with `VT_ARRAY | VT_VARIANT` and a real
   `SAFEARRAY`.

## Files

- [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
- [pointer_helpers_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/pointer_helpers_end_to_end.rs)

## Verification

Executed and passing:

1. `cargo test -p oxvba-runtime pointer_helpers -- --test-threads=1`
2. `cargo test -p oxvba-host --test pointer_helpers_end_to_end -- --test-threads=1 --nocapture`
3. `cargo test -p oxvba-host --test native_declare_string_marshalling_end_to_end -- --test-threads=1 --nocapture`

Observed truth after landing:

1. object-valued `Variant` pointer-helper cells no longer reject explicitly in
   the host E2E lane; they expose `VT_UNKNOWN` with a non-null interface
   pointer
2. array-valued `Variant` pointer-helper cells no longer reject explicitly in
   the host E2E lane; they expose `VT_ARRAY | VT_VARIANT` with an accessible
   `SAFEARRAY`
3. existing `StrPtr`, `VarPtr(String)`, scalar `VarPtr`, byte-buffer
   dereference, decimal `VarPtr(Variant)`, wide `I64` `VarPtr(Variant)`, and
   native declare writeback lanes stayed green

## Remaining Boundaries

This bead does not claim complete closure for every pointer-adjacent ABI case.
The remaining ABI/layout lane still owns:

1. native declare/writeback reconciliation beyond the already-green pointer
   helper cases
2. any UDT/layout-sensitive cases that require one more bounded rollout after
   the native ABI lane
3. the final old/new ABI-layout matrix in `vmm-g5`
