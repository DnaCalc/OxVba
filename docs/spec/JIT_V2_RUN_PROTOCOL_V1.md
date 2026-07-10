# JIT V2 Run Protocol V1

> [!CAUTION]
> **Historical run protocol.** Last-program entry and leaked descriptor rules are current gaps, not destination requirements. Use the OxIR/Image and JIT architecture contracts.

Status: superseded historical M4 planning contract.

This document fixes the shared driver sequence that vm3 already implements and the JIT driver must match. It is intentionally about image/session sequencing, not instruction dispatch.

## Protocol

1. Link receives one or more `OxProgram`s. The entry program is the last program in the linked slice.
2. Build one loaded-program table per `OxProgram` before executing any user code:
   - module global slots initialized from declared type and array metadata,
   - class descriptors leaked/stabilized for runtime object identity,
   - predeclared singleton cache empty,
   - event route table materialized from `(binding, event) -> handler`.
3. Validate every non-`VBA` import by `unit_name` before running any initializer. An unresolved import is a structural link error, not a partial execution.
4. Run module-global initializers in linked-program order, `0..program_count`.
   - Set the executing program index to the initializer's program before pushing its frame.
   - The initializer frame is discarded after it returns.
   - Module-global writes land in the initializer program's own global table.
   - A fault aborts activation; later initializers and entry do not run.
5. After all initializers finish, restore the executing program index to the entry program and reset the shared pending-termination queue. This isolates the new activation from prior sessions on the same thread.
6. Entry invocation is separate from activation. If the entry program has an entry proc, push one entry frame and run it.
   - The entry frame is not popped on normal completion; it remains available for result/snapshot reads.
   - If no entry proc exists, entry invocation is a no-op.
7. End-of-run drain:
   - After entry `run_loop` returns, run the termination drain before surfacing a clean result or an uncaught VBA fault.
   - The drain runs `Class_Terminate` callbacks to a fixpoint under `ExecState.draining`.
   - Terminate callback faults are suppressed; structural defects remain defects in the owning engine.
8. `End`/halt semantics:
   - A halt status bypasses active error handlers.
   - Cleanup still releases owned temporaries/locals according to the compiled function cleanup contract.
   - Driver-visible halt is not converted into a VBA fault.

## Ownership

`ExecState` owns the observable session cells needed by this protocol: loaded-program tables, `ErrEngine`, `LibContext`, host reference, event fabric, allocation counter, and the draining guard. Frames, instruction pointers, temporary maps, and GoSub stacks remain engine-private.

## Evidence

The behavior-preservation gate for this contract is the vm3 golden snapshot plus focused vm3 unit tests after the M4-2 extraction.
