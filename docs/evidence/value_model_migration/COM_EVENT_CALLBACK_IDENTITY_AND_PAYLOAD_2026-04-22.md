# COM event callback identity and payload reconciliation

Date: 2026-04-22

Scope:

1. `bd-t8rr.6.6` / `vmm-f5`
2. callback object identity under the migrated `ObjectRef` model
3. projection-triggered callback payload transport under the migrated `ComValue` model

Findings:

1. Native and projection callback queues were not equivalent after the `ObjectRef` migration.
   - `ComCallbackPayload.object` was being rebuilt from a compat id in
     `ComRuntimeState::take_polled_callback()`.
   - That preserved `raw()` compatibility identity but lost retained `IUnknown`
     / `ObjectRef` identity.
2. Projection-triggered callbacks still depended on a legacy `i32` array path.
   - `queue_projection_event_callbacks_shared(...)` previously required legacy
     callback argument transport even though callback payloads are stored as
     `Vec<ComValue>`.
3. Callback and subscription tokens still remain useful and should stay.
   - They continue to act as queue-control identifiers.
   - No new evidence in this bead required making callback or subscription
     tokens themselves object references.

Delivered changes:

1. `ComEventSubscription` and `ComEventCallback` now retain `ObjectRef` directly.
2. `ComCallbackPayload.object` now returns the retained callback object instead
   of reconstructing a fresh compat object.
3. Projection event trigger capture now derives callback arguments from
   `ComInvokeArg` / `ComValue` rather than requiring legacy `i32`
   callback-argument arrays.
4. The host callback surface now preserves callback object and event metadata in
   `ComEventCallbackDispatch`.

Observed no-change decision:

1. callback tokens and subscription tokens remain explicit control-plane tokens
   rather than being migrated into object identity carriers

Verification:

1. `cargo test -p oxvba-com --lib -- --test-threads=1`
   - includes new passing rows:
     - `runtime_state::tests::runtime_state_queues_projection_callbacks`
     - `windows_runtime_state::tests::projection_event_callback_args_accept_non_legacy_com_values`
     - `windows_runtime_state::tests::projection_callback_queue_preserves_retained_object_identity`
2. `cargo test -p oxvba-host formal_com_event_callback_ingress -- --test-threads=1`
3. `cargo test -p oxvba-host formal_com_evt_b_source_interface_callback_ingress_maps_to_registered_handler_symbol -- --exact --test-threads=1`
4. `cargo test -p oxvba-host formal_com_event_callback_runtime_dispatch -- --test-threads=1`
5. `cargo test -p oxvba-host --test com_early_project_end_to_end registered_testeventserver_withevents_callback -- --test-threads=1 --nocapture`
6. `./scripts/check-governance.ps1`

Result:

1. callback object identity is now retained through the queued callback payload
   path
2. projection-triggered event callbacks now use migrated value carriers for
   payload capture instead of re-entering the legacy `i32` callback-argument
   lane
3. event callback ingress, runtime dispatch, and early-bound `WithEvents`
   registered-server rows remain green after the change
