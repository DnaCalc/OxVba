# IR_DESIGN.md

## IR layers
- `VbaHir`
- `VbaMir`
- `CfgIr`

## Current state
The three IR structs and lowering passes are implemented as scaffolding with test coverage for sequence preservation.

## Next work
- Add explicit operations per tier.
- Model `On Error Resume Next` guarded regions.
- Add optimization pass contracts and semantic-preservation tests.
