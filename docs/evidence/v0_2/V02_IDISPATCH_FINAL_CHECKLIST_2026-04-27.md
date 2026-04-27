# V0.2 Late-Bound IDispatch Final Checklist

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.3.5`
Parent: `bd-bqm8.3`
Status: complete

## Checklist Result

`bd-bqm8.3` is complete for the V0.2 bounded late-bound `IDispatch` lane.
Supported rows have executable evidence, and unsupported rows remain explicit
and justified.

## Evidence Chain

- Scope rollout:
  `docs/evidence/v0_2/V02_IDISPATCH_PARITY_ROLLOUT_2026-04-27.md`
- Supported/unsupported matrix:
  `docs/evidence/v0_2/V02_IDISPATCH_SUPPORTED_MATRIX_2026-04-27.md`
- Metadata-backed member/default/named-argument behavior:
  `docs/evidence/v0_2/V02_IDISPATCH_METADATA_BACKED_BEHAVIOR_2026-04-27.md`
- Controlled COM VM/JIT/host evidence:
  `docs/evidence/v0_2/V02_IDISPATCH_CONTROLLED_COM_VM_JIT_HOST_EVIDENCE_2026-04-27.md`

## Verification

Passed:

- `rg -n "LBD-|unsupported|late-bound `IDispatch`|V02_IDISPATCH" docs/CONFORMANCE.md docs/evidence/v0_2/V02_IDISPATCH_SUPPORTED_MATRIX_2026-04-27.md docs/evidence/v0_2/V02_IDISPATCH_METADATA_BACKED_BEHAVIOR_2026-04-27.md docs/evidence/v0_2/V02_IDISPATCH_CONTROLLED_COM_VM_JIT_HOST_EVIDENCE_2026-04-27.md docs/worksets/WORKSET_2026-04-06_V0_2_SCOPE_ROUNDOUT_EXECUTION.md`
- `cargo test -p oxvba-com invoke_policy --lib`
- `cargo test -p oxvba-host formal_com_event_callback_runtime_dispatch_invokes_two_arg_handler --lib`
- `cargo test -p oxvba-host --test com_client_end_to_end createobject_dispatchinvoke_vm_jit_snapshots_match`
- `cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_runtime_string_named_member_routes_are_deterministic`
- `cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_runtime_string_value_and_default_member_routes_are_deterministic`
- `cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_runtime_string_member_unknown_name_surfaces_deterministically`
- `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_named_argument_calls_vm_jit_snapshots_match`
- `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload`
- `cargo check -p oxvba-com -p oxvba-hal -p oxvba-host -p oxvba-vm -p oxvba-jit`

Rejected command shape:

- An attempted multi-filter `cargo test` invocation was rejected by Cargo
  syntax because Cargo accepts one filter per command. The same test set was
  rerun successfully as separate filter executions.

## Residual Unsupported Rows

The following rows remain intentionally unsupported for V0.2 and are not
claimed by this lane:

- `LBD-U01`: full Office-wide behavioral parity for arbitrary `IDispatch`
  servers.
- `LBD-U02`: natural untyped default-member syntax without authoritative
  metadata.
- `LBD-U03`: arbitrary optional-argument or missing-argument synthesis without
  metadata.
- `LBD-U04`: general property-put/property-set parity beyond fixture-proved
  rows.
- `LBD-U05`: non-Windows COM late-bound parity.

## Formal Residual

The registration-sensitive VM-only intrinsic callback test recorded in
`V02_IDISPATCH_CONTROLLED_COM_VM_JIT_HOST_EVIDENCE_2026-04-27.md` remains
non-blocking under the current formal verification policy. It does not block
this bounded lane because host-backed callback payload evidence is green and
the unsupported boundary is explicit.
