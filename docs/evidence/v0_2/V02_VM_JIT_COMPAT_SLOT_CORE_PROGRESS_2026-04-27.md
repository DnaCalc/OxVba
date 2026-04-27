# V0.2 VM/JIT Compat-Slot Core Progress

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.2.2`
Status: complete

## Change

This slice narrows VM/JIT core compatibility helpers that were only test
scaffolding:

- `Vm::snapshot_slots`
- `Vm::snapshot`
- `Vm::snapshot_compat_values`
- `Vm::snapshot_values`
- `JitContextOwned::extract_user_values`
- `JitContext::read_slot`
- `JitContext::write_slot`

These APIs are now compiled only for tests. Normal builds retain the direct
`Variant` surfaces:

- `Vm::snapshot_variants`
- `JitContextOwned::extract_user_variants`
- `JitContext::read_variant_slot`
- `JitContext::write_variant_slot`

This does not close `bd-bqm8.2.2`. Public VM/JIT root execution aliases that
return `RuntimeValue` still exist for downstream compatibility callers and must
be removed or moved to an explicit external adapter in a later slice.

Follow-up slice in the same bead moved the VM public `RuntimeValue` snapshot
functions out of the root VM API into `oxvba_vm::compat`. In-repo callers now
name that adapter explicitly when they still need legacy semantic snapshots.
The normal VM root API exposes retained `Variant` snapshot functions directly.

`bd-bqm8.2.2` still remains in-progress because `JitEngine` still exposes
root `RuntimeValue` snapshot methods. Those JIT methods are the next core
surface to externalize before this delivery bead can close.

Second follow-up slice moved JIT normal-build `RuntimeValue` snapshot access
behind `oxvba_jit::compat`. The launcher now calls the explicit compatibility
adapter when it runs JIT and still needs legacy semantic snapshots. `JitEngine`
normal-build methods now expose retained `Variant` snapshots; the old
`RuntimeValue` methods remain test-only while the test/conformance migration
beads continue.

`bd-bqm8.2.2` remains in-progress because VM/JIT still contain internal
compat-slot constructors and projectors such as `RuntimeSlot::{from_compat_slot_i32,
project_compat_slot_i32}` and legacy Cranelift subset bridges. These must be
classified as either explicit internal bridges or migrated before the bead can
close.

Third follow-up slice hid the remaining JIT `RuntimeValue`-returning Cranelift
and RtSlot compatibility helpers from normal builds where they are only
test-facing:

- `cranelift::execute_bytecode`
- `cranelift::execute_bytecode_rtslot`
- `RtSlot::{from_runtime_value,to_runtime_value}`
- `rtslot_from_runtime_value`

Normal JIT/host builds continue through retained `Variant` APIs. Remaining
`RuntimeValue`/compat-slot hits in JIT normal code are now concentrated in the
explicit `oxvba_jit::compat` adapter and in retained Variant construction from
the legacy 4-byte subset.

Final slice narrowed the VM register-file compatibility bridge methods to
crate visibility and test-only visibility where applicable. The remaining
normal-build VM compat-slot methods are internal interpreter bridges rather than
public VM API:

- `RuntimeSlot::from_compat_slot_i32`
- `RuntimeSlot::project_compat_slot_i32`
- `RuntimeSlot::as_i32_lossy`

These are used by the bounded legacy scalar fast paths and are explicitly
internal. `RuntimeSlot` `RuntimeValue` ingress/egress helpers are test-only.

`bd-bqm8.2.2` is complete for VM/JIT core public snapshot/slot-token surfaces.
The parent `bd-bqm8.2` remains in-progress because host/CLI/debugger,
COM/HAL, and conformance/doc surfaces are still open under downstream child
beads.

## Verification

Passed:

- `cargo check -p oxvba-vm -p oxvba-jit`
- `cargo check -p oxvba-vm -p oxvba-jit -p oxvba-host -p oxvba-launcher`
- `cargo fmt --check`
- `cargo test -p oxvba-vm compat_snapshot_api_projects_variant_snapshot_results --lib`
- `cargo test -p oxvba-vm snapshot_variants_exposes_variant_cells_before_projection --lib`
- `cargo test -p oxvba-jit jit_context_extracts_user_variants_before_projection --lib`
- `cargo test -p oxvba-jit execute_and_snapshot_variants_exposes_jit_results_before_projection --lib`

Not clean in this host:

- `cargo test -p oxvba-vm -p oxvba-jit --lib`

The broader package test run reached executable tests but failed on host-local
COM registration assumptions before the relevant assertions:

- `OxVba.TestDispatch` was not registered, causing `CLSIDFromProgID` failures
  in COM event/object tests.
- Two JIT fallback comparison tests also remained red in the broad run and need
  follow-up in the continuing `bd-bqm8.2.2` slice before this bead can close.
