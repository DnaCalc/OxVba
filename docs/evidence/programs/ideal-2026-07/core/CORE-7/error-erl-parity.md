# CORE-7 Error, Erl and Line Seating

Date: 2026-08-18
Bead: `bd-59co.2.9.3`
Status: in-progress delivery evidence. This does not close `CORE-JIT-LOWERING`.

## Outcome

JIT now seats the active source line, reads `Erl`, and writes `Err` fields
through shared `oxvba-rt-abi` helpers. Fault dispatch uses the current frame
line instead of hard-coded zero.

Changes:

- `OxInst::SetLineNumber` updates `JitFrame.current_line`
- `FaultDispatch` passes that line into `rt_route_fault`
- `OxInst::ErlGet` reads `err_engine.erl_line`
- `OxInst::ErrFieldSet` applies the same Number/Description/Source/HelpFile/
  HelpContext writes as VM3; `LastDllError` remains read-only

## Commands

- `cargo test -p oxvba-differential --test jit_portable_vm3_parity -- --nocapture`
- `cargo test -p oxvba-jit erl -- --nocapture`
- `cargo test -p oxvba-rt-abi err_field_set_and_erl_get -- --nocapture`
- `cargo clippy -p oxvba-rt-abi -p oxvba-jit --all-targets -- -D warnings`

Expected: error/erl_numeric_line and error/err_number_write are exact VM3/JIT
matches; focused helpers and Clippy are clean.

## Residual

Unused-Declare whole-image decline remains `bd-59co.2.9.4`. Remaining CORE-7
architecture remains `bd-59co.2.9.9`.
