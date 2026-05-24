# Standalone Reflection Wrapper Executable

Date: 2026-05-24
Bead: `bd-q31v`

## What changed

Added a runnable standalone console binary:

- Source: `crates/oxvba-cli/src/bin/oxvba-reflect-wrapper.rs`
- Build: `cargo build -p oxvba-cli --bin oxvba-reflect-wrapper`
- Run: `cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- <project> <command>`

Commands:

- `list`
- `describe Module.Procedure`
- `call Module.Procedure [typed positional args...]`

The executable loads `.basproj`, `.vbp`, or workspace-directory targets through `oxvba_project::load_workspace_target`, compiles to an `OxBundle`, loads through the descriptor-driven reflection wrapper, and invokes through the neutral typed callable path.

## Samples added

- `examples/reflection_wrapper/engineering_math/EngineeringMath.basproj`
- `examples/reflection_wrapper/engineering_math/EngineeringMath.bas`
- `examples/reflection_wrapper/business_calc/BusinessCalc.basproj`
- `examples/reflection_wrapper/business_calc/BusinessCalc.bas`
- `examples/reflection_wrapper/README.md`

## Manual verification

```text
cargo build -p oxvba-cli --bin oxvba-reflect-wrapper
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/engineering_math/EngineeringMath.basproj list
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/engineering_math/EngineeringMath.basproj describe EngineeringMath.Hypotenuse
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/engineering_math/EngineeringMath.basproj call EngineeringMath.AddLongs 20 22
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/engineering_math/EngineeringMath.basproj call EngineeringMath.ScaleLoad 12.5 2
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/business_calc/BusinessCalc.basproj list
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/business_calc/BusinessCalc.basproj describe BusinessCalc.GrossMargin
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/business_calc/BusinessCalc.basproj call BusinessCalc.ApplyDiscount 100 0.15
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/business_calc/BusinessCalc.basproj call BusinessCalc.UnitsAfterBundle 1000 250
cargo check --workspace --all-targets
```

Observed successful outputs included `42`, `25`, `85`, and `1250` for the sample calls.
