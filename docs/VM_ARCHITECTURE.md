# VM_ARCHITECTURE.md

## Current state
The VM crate provides:
- register-file abstraction,
- interpreter entry point,
- placeholder broadword helper,
- error-state enum scaffold.

## Next work
- Register-window frame model.
- Spill/fill semantics and bounds checks.
- Opcode dispatch and error-state machine behavior.
