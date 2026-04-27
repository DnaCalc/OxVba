# V0.2 Compat-Slot Excision Rollout

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.2.1`
Parent lane: `bd-bqm8.2`
Status: rollout complete; delivery lane remains in-progress

## Purpose

Roll out `v02.2` into concrete delivery beads before changing code. The scan
confirms the remaining compat-slot projection seam is spread across multiple
product surfaces, so `bd-bqm8.2` cannot honestly close on a support-only audit.

## Current Surface Map

The 2026-04-27 scan found active compatibility projection surfaces in these
families:

1. VM and JIT execution compatibility APIs:
   - `crates/oxvba-vm/src/lib.rs`
   - `crates/oxvba-vm/src/interpreter.rs`
   - `crates/oxvba-vm/src/register_file.rs`
   - `crates/oxvba-jit/src/lib.rs`
   - `crates/oxvba-jit/src/jit_context.rs`
   - `crates/oxvba-jit/src/slot_abi.rs`
2. Host, CLI, debugger, and integration-test observation surfaces:
   - `crates/oxvba-host/src/engine.rs`
   - `crates/oxvba-host/src/immediate.rs`
   - `crates/oxvba-host/src/debugger.rs`
   - `crates/oxvba-cli/src/main.rs`
   - `crates/oxvba-host/tests/project_integration_suite.rs`
   - `crates/oxvba-host/tests/project_hosting_examples_end_to_end.rs`
3. COM and HAL compatibility adapters:
   - `crates/oxvba-com/src/model.rs`
   - `crates/oxvba-com/src/dynamic_object.rs`
   - `crates/oxvba-com/src/runtime_state.rs`
   - `crates/oxvba-com/src/windows_runtime_state.rs`
   - `crates/oxvba-hal/src/traits.rs`
   - `crates/oxvba-hal/src/adapters/**`
4. Architecture and historical evidence docs that still mention compat-slot
   projection as a surviving seam.

## Rollout Beads

The parent delivery lane is now split as follows:

1. `bd-bqm8.2.1`: audit and roll out the child delivery graph.
2. `bd-bqm8.2.2`: remove or externalize VM/JIT core compat-slot snapshot and
   slot-token APIs so retained `Variant`/rich values are the primary execution
   observation path.
3. `bd-bqm8.2.3`: move host, CLI, debugger, immediate, and project-test
   observation to retained rich values, leaving any slot-shaped display as an
   explicit compatibility adapter.
4. `bd-bqm8.2.4`: reconcile COM and HAL compatibility bridges so legacy
   `RuntimeValue` or slot-token entry points are adapter boundaries rather than
   core execution truth.
5. `bd-bqm8.2.5`: migrate tests, conformance notes, and product docs away from
   normalizing compat-slot assertions as execution truth.
6. `bd-bqm8.2.6`: run the final excision checklist and close `v02.2` only if
   the executable evidence proves the seam is removed or explicitly external.

## Current Blocker State

No global blocker exists. The next ready delivery bead is `bd-bqm8.2.2`.

`bd-bqm8.2` remains in-progress until all delivery beads above are complete and
the final validation bead proves the workset target.
