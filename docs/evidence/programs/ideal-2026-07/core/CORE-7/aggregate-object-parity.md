# CORE-7 Portable Arrays, Records, Strings and Project Objects

Date: 2026-08-18
Bead: `bd-59co.2.9.6`
Status: in-progress delivery evidence. This does not close `CORE-JIT-LOWERING`.

## Outcome

Portable aggregate/object basics match VM3 on the current stack.

Commands and results:

- `cargo test -p oxvba-differential --test jit_portable_vm3_parity -- --nocapture` — strings, dynamic arrays, `Array()`, For Each, simple UDT
- `cargo test -p oxvba-differential --test jit_project_objects -- --nocapture` — 45 passed, including TypeOf, default members, CallByName, Dim As New
- `cargo test -p oxvba-differential --test jit_udt_class_aggregates -- --nocapture` — 14 passed
- `cargo test -p oxvba-differential --test jit_local_type_carriers -- --nocapture` — 5 passed

No additional JIT lowering change was required for this slice.

## Residual

COM/native object dispatch remains declined. Remaining CORE-7 architecture
remains `bd-59co.2.9.9`.
