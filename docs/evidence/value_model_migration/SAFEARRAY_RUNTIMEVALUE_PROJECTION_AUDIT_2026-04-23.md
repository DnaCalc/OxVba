# SAFEARRAY RuntimeValue Projection Audit (2026-04-23)

Status: `vmm-e6` support evidence, not closure.

Purpose:

1. classify remaining `RuntimeValue`-named SAFEARRAY APIs and call sites after
   the Variant-native SAFEARRAY migration,
2. keep `vmm-e6` open unless every remaining use is either migrated or
   explicitly classified as a boundary/test compatibility projection,
3. prevent support-only audit evidence from being mistaken for completion.

Audit command:

```powershell
rg -n "SafeArray::from_values|from_values_nd|from_typed_values|from_shape_and_values|\.elements\(\)|replace_elements\(" crates/oxvba-runtime crates/oxvba-vm crates/oxvba-jit crates/oxvba-com crates/oxvba-hal --glob "*.rs"
```

Observed categories:

1. `crates/oxvba-runtime/src/safe_array.rs` still exposes legacy
   `RuntimeValue` constructors and accessors:
   `from_values`, `from_values_nd`, `from_typed_values`,
   `from_typed_values_nd`, `from_shape_and_values`, `elements`, and
   `replace_elements`.
2. Those legacy APIs now bridge through canonical `Variant` payload formation
   and `variant_elements()` projection rather than storing `RuntimeValue`
   elements as the internal payload carrier.
3. Remaining VM/JIT/COM/HAL call sites found by this scan are tests,
   property fixtures, compatibility assertions, or helper-boundary fixtures.
   They do not currently identify a production payload-retention path that
   owns `Vec<RuntimeValue>` as the internal SAFEARRAY payload carrier.
4. Production array paths already migrated in this lane include VM/JIT
   `ReDim`, `ReDim Preserve`, intrinsic array literal/append, VM array get/set,
   VM `For Each` materialization, COM `IEnumVARIANT` materialization,
   pointer-helper array projection, Windows `VARIANT`/`SAFEARRAY` bridge
   element transport, dynamic `ParamArray` construction, and HAL conformance
   SAFEARRAY shape probing.

Open result:

1. This audit does not close `vmm-e6`.
2. The compatibility APIs remain public and therefore still need final
   classification before the Variant/SAFEARRAY family checklist can pass.
3. If later production code starts using these legacy APIs for retained
   payload storage, that is a regression against the migration target and must
   be converted to the Variant-native API family.
4. The closure checklist must still decide whether to keep these APIs as
   explicitly documented compatibility projections, rename/narrow them, or
   remove them from production-facing surfaces.
