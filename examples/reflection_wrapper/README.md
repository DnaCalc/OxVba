# Reflection Wrapper Samples

These samples are meant for the standalone `oxvba-reflect-wrapper` console executable.

Build/check the binary:

```powershell
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- --help
```

## EngineeringMath

```powershell
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/engineering_math/EngineeringMath.basproj list
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/engineering_math/EngineeringMath.basproj describe EngineeringMath.Hypotenuse
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/engineering_math/EngineeringMath.basproj call EngineeringMath.AddLongs 20 22
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/engineering_math/EngineeringMath.basproj call EngineeringMath.ScaleLoad 12.5 2
```

## BusinessCalc

```powershell
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/business_calc/BusinessCalc.basproj list
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/business_calc/BusinessCalc.basproj describe BusinessCalc.GrossMargin
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/business_calc/BusinessCalc.basproj call BusinessCalc.ApplyDiscount 100 0.15
cargo run -p oxvba-cli --bin oxvba-reflect-wrapper -- examples/reflection_wrapper/business_calc/BusinessCalc.basproj call BusinessCalc.UnitsAfterBundle 1000 250
```

The wrapper loads the project, reflects descriptors from the compiled bundle, and invokes through the neutral typed callable path.
