# V0.2 IDispatch Controlled COM VM/JIT/Host Evidence

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.3.4`
Status: complete

## Scope

This bead extends executable evidence for the V0.2 late-bound `IDispatch`
supported rows published in
`docs/evidence/v0_2/V02_IDISPATCH_SUPPORTED_MATRIX_2026-04-27.md`.

The evidence is deliberately bounded to controlled COM and metadata-backed
rows. It does not expand the unsupported Office-wide or metadata-missing rows.

## Evidence Map

| Matrix row | Evidence |
| --- | --- |
| `LBD-001` activation | `createobject_dispatchinvoke_vm_jit_snapshots_match` |
| `LBD-002` token-backed invocation | `createobject_dispatchinvoke_vm_jit_snapshots_match` |
| `LBD-003` name-backed member resolution | `dispatchinvoke_runtime_string_named_member_routes_are_deterministic` |
| `LBD-004` scalar/object positional invocation | `createobject_dispatchinvoke_vm_jit_snapshots_match` |
| `LBD-005` wider value-shape transport | Existing `com_client_end_to_end` wide/string/array rows remain in the controlled COM corpus |
| `LBD-006` named arguments | `early_bound_project_named_argument_calls_vm_jit_snapshots_match` |
| `LBD-007` default-member dispatch | `dispatchinvoke_runtime_string_value_and_default_member_routes_are_deterministic` and `call_statement_runtime_string_named_default_member_dispatch_is_deterministic` |
| `LBD-008` event callback payload projection | `early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload` and `formal_com_event_callback_runtime_dispatch_invokes_two_arg_handler` |
| `LBD-009` deterministic diagnostics | `dispatchinvoke_runtime_string_member_unknown_name_surfaces_deterministically` |

## Verification

Passed:

- `cargo test -p oxvba-host --test com_client_end_to_end createobject_dispatchinvoke_vm_jit_snapshots_match`
- `cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_runtime_string_named_member_routes_are_deterministic`
- `cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_runtime_string_value_and_default_member_routes_are_deterministic`
- `cargo test -p oxvba-host --test com_client_end_to_end call_statement_runtime_string_named_default_member_dispatch_is_deterministic`
- `cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_runtime_string_member_unknown_name_surfaces_deterministically`
- `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_named_argument_calls_vm_jit_snapshots_match`
- `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload`
- `cargo test -p oxvba-host formal_com_event_callback_runtime_dispatch_invokes_two_arg_handler --lib`

Non-blocking formal verification residual:

- `cargo test -p oxvba-vm com_event_subscription_intrinsics_roundtrip_multi_arg_callback_lane --lib`
  failed in this environment because the VM-only intrinsic lane activates
  `OxVba.TestDispatch` through native COM without the registered ProgID
  available, producing `CLSIDFromProgID failed for OxVba.TestDispatch with
  HRESULT 0x800401F3`. The host-backed registered-event evidence above covers
  the supported callback payload row for this bead; the VM-only intrinsic
  residual remains a formal verification environment issue, not a supported-row
  implementation blocker.

## Closure Boundary

`bd-bqm8.3` remains in-progress after this evidence bead. Final lane closure
requires `bd-bqm8.3.5` to run the checklist, confirm supported rows remain
green, and keep unsupported rows explicit.
