# VM_ARCHITECTURE.md

## Current state
The VM crate provides:
- register-file abstraction,
- interpreter entry point with MVP arithmetic opcode execution (`LoadConstI32`, `AddConstI32`, `SubConstI32`, `Halt`),
- placeholder broadword helper,
- error-state enum scaffold.

## Next work
- Register-window frame model.
- Spill/fill semantics and bounds checks.
- Opcode dispatch and error-state machine behavior.

## Feature flags
- `mach_broadword_dispatch` (crate: `oxvba-vm`): enables broadword dispatch optimization path when promoted.
