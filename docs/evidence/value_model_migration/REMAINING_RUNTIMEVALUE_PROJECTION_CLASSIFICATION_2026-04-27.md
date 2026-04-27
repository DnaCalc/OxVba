# Remaining RuntimeValue Projection Classification - 2026-04-27

Scope: `vmm-e6` exact internal `VARIANT` / `SAFEARRAY` migration after the
retained VM/JIT slot migration, Windows COM result bridge narrowing, HAL
companion migration, host/immediate/debugger retained display migration, and
recent Windows result-boundary cleanup.

Status: final `vmm-e6` projection classification. This note closes the
remaining classification blocker for `vmm-e6`; `vmm-e7` remains the separate
closure-checklist bookkeeping step.

## Retained Carrier Floor

The current retained value floor is:

1. VM registers and JIT runtime slots store `Variant` carriers.
2. Runtime `SafeArray` payload descriptors store native-shaped element payloads
   and expose retained `Variant` element APIs.
3. Windows COM scalar, typed SAFEARRAY, `IEnumVARIANT`, and public
   `variant_to_runtime_value()` compatibility result paths materialize retained
   `Variant` carriers before any final legacy projection.
4. HAL `_variant` companions are the retained host-service entry points used by
   VM/JIT paths; legacy `RuntimeValue` HAL methods are compatibility wrappers.
5. Host project, immediate, debugger, CLI, embedded, and JIT/VM public snapshot
   APIs have retained `Variant` companions where the current code needs them.

## Compatibility Projection Surfaces

These remaining `RuntimeValue` uses are compatibility surfaces rather than
evidence that retained slot storage is still semantic-value backed:

1. `oxvba-runtime::RuntimeValue::{to_variant,from_variant}` and
   `Variant::try_from_runtime_value` / `Variant::to_runtime_value` are the
   explicit bridge APIs between the old semantic projection type and the
   retained carrier.
2. `SafeArray::{from_values,elements,replace_elements}` are public legacy
   compatibility APIs. Retained call sites should use
   `from_variants`, `variant_elements`, and `replace_variant_elements`.
3. `RuntimeSlot::{from_runtime_value,to_runtime_value}` and `RtSlot` public
   compatibility methods are legacy VM/JIT ingress/egress APIs around retained
   `Variant` storage.
4. VM/JIT/host `snapshot_values`, `execute_*_snapshot`, and related
   `RuntimeValue` result APIs are compatibility projections from retained
   `*_variants` APIs.
5. HAL standard adapter legacy methods project through retained `_variant`
   companions at their boundary.
6. COM `ComValue::from_runtime_value`, `ComValue::to_runtime_value`, and
   dynamic value `to_runtime_value` are compatibility projections around
   retained `Variant` / `ComValue` payloads.
7. Immediate/debugger legacy result structs expose `RuntimeValue` for existing
   callers, while retained display/result paths use `Variant` directly.
8. Pointer-helper `RuntimeValue` registration/readback APIs remain legacy
   wrappers over retained pointer-helper `Variant` APIs.

## Final Delivery Disposition

The remaining delivery paths from the initial classification were resolved as
follows:

1. Reduce or classify the interpreter/JIT helper test scaffolds that still
   construct retained arrays through `SafeArray::from_values` instead of
   `SafeArray::from_variants`.
   - Disposition: retained-path VM/JIT/runtime/COM/HAL tests were migrated to
     `SafeArray::from_variants`, `from_typed_variants`, or
     `from_typed_variants_nd`; remaining hits are explicit compatibility API
     tests or compatibility result assertions.
2. Add retained public alternatives only where a compatibility API is still the
   only available path for a production caller.
   - Disposition: final scan found retained companions for the production
     surfaces in scope: VM/JIT snapshots, host/project/immediate/debugger
     surfaces, HAL service families, COM model/Windows bridge, dynamic COM,
     and pointer-helper retained entry points.
3. Decide whether to keep public compatibility APIs indefinitely or gate some
   of them to tests/features after downstream callers are migrated.
   - Disposition: keep compatibility APIs as public legacy projection
     contracts. They are not internal value storage and are documented as
     projections beside retained Variant/SAFEARRAY companions.
4. Run a final scan that separates test-only assertions, public compatibility
   APIs, and production retained paths before `vmm-e7` can run the closure
   checklist.
   - Disposition: completed. The remaining `RuntimeValue`/`SafeArray` scan
     hits are public compatibility bridge APIs, explicit compatibility result
     assertions, or tests of those compatibility APIs.

## Final Scan Result

`vmm-e6` no longer has an internal carrier blocker. The retained value floor is
the internal production path: general VM/JIT slots, SAFEARRAY payloads, Windows
COM result materialization, HAL Variant companions, host/project/immediate/
debugger retained result surfaces, and pointer-helper retained entry points all
have `Variant`/SAFEARRAY carriers before any legacy projection. Remaining
`RuntimeValue` APIs are compatibility contracts and do not own the internal
late-bound/general value model.
