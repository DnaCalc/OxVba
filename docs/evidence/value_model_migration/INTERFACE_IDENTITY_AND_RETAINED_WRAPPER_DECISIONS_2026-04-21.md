# Interface Identity And Retained Wrapper Decisions

Date: 2026-04-21
Bead: `bd-t8rr.6.2` / `vmm-f1`
Status: active

## Purpose

Lock the intended internal interface-identity target for the value-model
migration before broad F-lane edits land.

This decision note is about the canonical migrated runtime representation, not
about whether Windows-facing boundary projections must materialize real COM
interfaces. Those boundary projections remain required.

## Evidence Inputs

Primary evidence:

1. [WINDOWS_VBA71_X64_INTERFACE_EVENT_LAYOUT_FACT_PACK_2026-04-20.md](/C:/Work/DnaCalc/OxVba/docs/evidence/runtime/WINDOWS_VBA71_X64_INTERFACE_EVENT_LAYOUT_FACT_PACK_2026-04-20.md)
2. [WINDOWS_VBA71_X64_VALUE_MODEL_FACT_PACK_CONSOLIDATION_2026-04-20.md](/C:/Work/DnaCalc/OxVba/docs/evidence/runtime/WINDOWS_VBA71_X64_VALUE_MODEL_FACT_PACK_CONSOLIDATION_2026-04-20.md)

Current implementation anchors:

1. [variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/variant.rs)
2. [model.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/model.rs)
3. [windows_runtime_state.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_runtime_state.rs)
4. [windows_variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_variant.rs)
5. [windows_invoke.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_invoke.rs)

Current verification anchors re-run on 2026-04-21:

1. [com_client_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_client_end_to_end.rs)
   - `object_variant_results`
   - `plain_unknown`
2. [com_early_project_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_early_project_end_to_end.rs)
   - `registered_testeventserver_withevents_callback_invokes_handler_body`
   - `registered_testeventserver_withevents_callback_preserves_value_payload`

## Decisions

### D1. Canonical migrated runtime identity becomes `ObjectRef`, backed by an `IUnknown`-implementing object base

Decision:

1. The migrated canonical runtime continues to represent COM object identity as
   semantic object identity, but that identity is no longer carried by a
   standalone integer token.
2. `ObjectHandle` does not survive as the canonical runtime-facing identity
   carrier in the migrated model.
3. The canonical runtime object carrier becomes `ObjectRef`.
4. `ObjectRef` is a typed runtime object reference backed by a base object
   structure that implements `IUnknown`.
5. The canonical `Variant` object lane becomes an `ObjectRef` lane rather than
   an integer-token lane or a permanently resident raw interface-pointer lane.

Why:

1. The fact pack requires stable COM identity under `IUnknown`, but it does not
   require every internal runtime value carrier to physically store raw COM
   pointers at all times.
2. The current integer-token model is an implementation convenience, not the
   architectural end state we now want.
3. `ObjectRef` lets identity and lifetime collapse onto the same runtime object
   reference without scattering untyped raw pointers everywhere.
4. It removes the current split between semantic token identity and
   COM-style reference-counted object identity.

### D2. Native COM pointer truth lives in retained boundary state and is anchored on `IUnknown`

Decision:

1. Windows-native interface-pointer truth remains owned by retained COM bridge
   state, not by arbitrary runtime values.
2. The current `windows_runtime_state.rs` model is the right migration target
   direction:
   - retained native `IUnknown` identity/lifetime anchor ownership
   - retained native `IDispatch` invocation pointer ownership where applicable
   - runtime object-base records that implement `IUnknown`
   - object-reference allocation/retention
   - repeated native-result dedup by canonical retained `IUnknown` identity
   - subscription and callback lifetime state
3. `vmm-f2` must route runtime object lifetime through `IUnknown::AddRef` /
   `IUnknown::Release` on those object-base records rather than through a
   parallel bespoke reference-count discipline.
4. This must not dissolve the separation between canonical semantic identity
   and retained native boundary state.

Why:

1. The current code already centralizes native pointer lifetime in one place.
2. That separation is compatible with the fact pack requirement that COM
   identity be anchored by `IUnknown` semantics.
3. It allows the runtime to stay portable while still projecting Windows/VBA
   truth at observable boundaries.

### D3. The retained-wrapper strategy is the chosen migration direction

Decision:

1. The migrated F lane adopts a retained-wrapper strategy rather than a
   raw-pointer-everywhere strategy.
2. In practice that means:
   - canonical runtime values carry semantic identity
   - `ObjectRef` is the runtime-facing object identity/reference
   - the base object reached through `ObjectRef` implements `IUnknown`
   - runtime ownership transitions use `IUnknown::AddRef` / `Release`
   - retained COM bridge state carries the native pointer anchor and lifetime
   - the canonical retained anchor is `IUnknown`
   - `IDispatch` is retained as the Automation invocation/projected-interface
     surface, not as the canonical identity key by itself
   - repeated object rebinding continues to deduplicate onto stable semantic
     identity when the same native object is observed again
3. `vmm-f2` owns making the `IUnknown` anchor explicit in retained bridge
   state and in the runtime object base rather than leaving identity keyed only
   by retained `IDispatch*` or token-map entries.
4. This happens without promoting arbitrary raw interface pointers to the
   public runtime carrier shape.

Why:

1. This satisfies the fact pack requirement that COM identity be stable under
   `IUnknown` semantics.
2. It preserves a typed runtime object abstraction instead of collapsing the
   whole runtime onto raw pointer values.
3. It keeps the migration scoped to observable correctness, memory, and timing
   rather than forcing a full runtime ownership rewrite.

### D4. `VT_UNKNOWN` policy remains "rebind if dispatch-capable, otherwise bounded failure"

Decision:

1. The migrated target keeps the current observable `VT_UNKNOWN` rule:
   - if `IUnknown::QueryInterface(IDispatch)` succeeds, rebind onto the object
     lane
   - if it fails, surface deterministic `E_NOINTERFACE`-style bounded failure
2. `vmm-f3` owns making sure that this rule still holds after any retained
   wrapper or identity-carrier changes.

Why:

1. This is already the current tested behavior.
2. It matches the fact-pack requirement that `IUnknown` identity and
   Automation-facing dispatch behavior be explicit and stable.
3. It avoids silently widening the supported scope to nondispatch interface
   execution that OxVba does not currently claim.

### D5. Event callback identity stays tokenized; payload values stay semantic

Decision:

1. Callback/subscription identity remains tokenized semantic identity:
   `ComSubscriptionToken` and `ComCallbackToken`.
2. Event payload values remain semantic `ComValue` payloads at the callback
   queue boundary.
3. Native connection-point state remains retained bridge state.
4. The existing projection-event legacy-`i32` argument path is not accepted as
   the final migrated target; it is an explicit `vmm-f5` reconciliation item.

Why:

1. Current native callback queues already use `ComCallbackPayload { args:
   Vec<ComValue> }`.
2. That shape is compatible with the broader value-model migration.
3. The remaining projection callback argument path is an implementation seam,
   not a reason to change the canonical callback-identity model.

## Explicit Non-Decisions

1. This bead does not close the exact field/layout shape of the `IUnknown`-
   implementing object base.
   - whether `ObjectRef` is a thin typed retained pointer, a small wrapper over
     one, or another equivalent typed representation remains a `vmm-f2`
     implementation detail
2. This bead does not close the exact retained-wrapper field shape for the
   canonical external/native `IUnknown` anchor.
   - whether the retained state stores both a normalized `IUnknown*` identity
     pointer and a distinct `IDispatch*` invocation pointer, or derives one
     from the other at controlled points, is also a `vmm-f2`
     implementation detail
3. This bead does not widen support for general nondispatch interface execution.
4. This bead does not close the event payload storage migration.
   - projection callback legacy-argument transport remains active follow-on work
     for `vmm-f5`
5. This bead does not promote broader COM-EVT-B/source-interface parity beyond
   the currently bounded path.

## Consequences For Follow-On Beads

1. `vmm-f2`
   - replace token-only runtime object identity with canonical `ObjectRef`
   - make the runtime object base `IUnknown`-implementing
   - route runtime object lifetime through `AddRef` / `Release`
   - reconcile retained wrapper/native identity internals around an explicit
     `IUnknown` anchor while making `ObjectRef` the canonical runtime object
     carrier
2. `vmm-f3`
   - preserve the current `VT_DISPATCH` / dispatch-capable-`VT_UNKNOWN`
     rebinding contract on top of the `IUnknown`-anchored retained wrapper
3. `vmm-f5`
   - remove or explicitly reconcile the projection callback legacy-`i32`
     payload seam while keeping callback/subscription identity tokenized
4. `vmm-f6`
   - compare baseline and candidate on object rebinding, nondispatch failure,
     event callback identity, and event payload preservation using this decision
     set as the migration contract
