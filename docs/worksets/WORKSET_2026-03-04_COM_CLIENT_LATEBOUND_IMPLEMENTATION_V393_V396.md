# WORKSET_2026-03-04_COM_CLIENT_LATEBOUND_IMPLEMENTATION_V393_V396

## Scope

Execute the first implementation block after C2 spec closure:
- bridge/lifetime hardening,
- deterministic COM error taxonomy,
- conformance/process scaffolding.

Profiles covered: `v393..v396`

## Deliverables

1. HAL COM state stores durable native dispatch bindings for object tokens (no per-invoke reactivation).
2. Native COM binding lifetime cleanup path is deterministic and test-covered.
3. Machine-readable COM client error mapping artifact and adapter label mapping are in place.
4. Conformance scaffold directory for C2 late-bound lane is created and documented.
5. Meta-check includes gate-sync validation across `AGENTS.md`/`AUTORUN_STATE.md`/readme targets.
6. HAL uncertainty and implementation-defined entries are mapped to concrete C2 implementation steps.

## Verification Commands

- `cargo test -p oxvba-hal windows_native_com_dictionary_lane_executes_when_available -- --nocapture`
- `cargo test -p oxvba-hal windows_native_com_binding_keeps_stable_dispatch_identity -- --nocapture`
- `./scripts/check-hal-clause-drift.ps1`
- `./scripts/meta-check.ps1 -Fast`
