# Wrapper Plan Abstractions

Date: 2026-05-24
Bead: `bd-hjys.9`

## Implementation summary

Added `crates/oxvba-build/src/wrapper_plan.rs` and exported it from `oxvba-build` with generic wrapper-generation planning types:

- `ProjectReflectionInput`
- `WrapperGenerationPlan`
- `WrapperOutputKind`
- `CallableSelectionPlan`
- `WrapperConversionLane`
- `WrapperDiagnosticsPolicy`
- `ArgumentParserPlan`
- `GeneratedWrapperArtifact`
- `WrapperPlanDiagnostic`

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| Plan abstractions independent of XLL/UDF assumptions. | Types are generic and consume neutral `ProjectReflection`; tests use CLI/native profiles without XLL/UDF substrate. |
| Plans can select callables by descriptor identity and host/build policy. | `wrapper_plan_selects_by_explicit_identity` and `wrapper_plan_public_function_selection_is_host_build_policy`. |
| Conversion lanes and diagnostics are explicit. | `WrapperConversionLane::{typed_scalar_first_tier, variant_positional}` and `WRAPPER-NO-CONVERSION-LANE` diagnostic test. |
| Future XLL, COM, CLI, and native-library profiles can be represented. | `wrapper_plan_represents_cli_native_com_and_future_xll_profiles` covers `CliExe`, `IntrospectionExe`, `NativeLibrary`, `ComServer`, and `FutureXll`, plus parser profiles. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-build wrapper_plan -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- The plan layer is selection/conversion/diagnostics metadata only; it does not generate XLL or native-library code yet.
- XLL is represented only as `FutureXll`, a profile over wrapper plans rather than the foundation.
