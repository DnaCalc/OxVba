# IR_DESIGN.md

## IR layers
- `VbaHir`
- `VbaMir`
- `CfgIr`

## Current state

The three IR structs and lowering passes are implemented as scaffolding with test coverage for sequence preservation. `On Error Resume Next` guarded regions are modeled via the error handling instructions (`SetOnErrorResumeNext`, `SetOnErrorGoto0`, `SetOnErrorGotoLabel`, `Resume`, `ResumeNext`, `ResumeLabel`).

## Next work
- Optimization passes (currently no-ops): dead code elimination, constant folding, control flow simplification.
- Add semantic-preservation tests for optimization pass contracts.
