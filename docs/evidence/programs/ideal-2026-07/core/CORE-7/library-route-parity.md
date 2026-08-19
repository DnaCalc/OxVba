# CORE-7 Portable VBA Library Routes

Date: 2026-08-18
Bead: `bd-59co.2.9.7`
Status: in-progress delivery evidence. This does not close `CORE-JIT-LOWERING`.

## Outcome

Portable library routes exercised by the basics harness and the Linux-safe
generated suite match VM3:

- Abs, Mid, Left$, Len
- New Collection / Count / Add
- generated scalar, string, Mid statement, loop/array, call, and error cases
- benchmark UDT/Collection and string-concat fixtures

Commands:

- `cargo test -p oxvba-differential --test jit_portable_vm3_parity -- --nocapture`
- `cargo test -p oxvba-differential --test jit_linux_safe_generated -- --nocapture` — 11 passed

Used Declare remains an explicit decline owned by later Windows/CORE-7
architecture work.

## Residual

Host-denied and Windows-only library/COM/native routes stay outside this
tranche. `bd-59co.2.9.9` owns remaining architecture.
