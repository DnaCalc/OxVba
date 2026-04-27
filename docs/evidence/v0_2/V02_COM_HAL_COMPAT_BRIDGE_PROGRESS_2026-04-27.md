# V0.2 COM/HAL Compat Bridge Progress

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.2.4`
Status: in-progress

## Change

This slice started externalizing COM/HAL legacy compatibility bridges behind
explicit adapter modules.

Implemented:

- Added `oxvba_hal::compat` for HAL `RuntimeValue` to retained `Variant`
  projection and retained `Variant` back to legacy `RuntimeValue` projection.
- Moved the standard process/environment HAL legacy wrappers for `Shell`,
  `Environ`, and `Dir` onto `oxvba_hal::compat` while leaving the retained
  `_variant` methods as the primary implementations.
- Added `oxvba_com::compat` for COM `ComValue` projection to/from legacy
  `RuntimeValue` and slot-token transport.
- Rewired `ComValue::{from_runtime_value,from_runtime_token,to_runtime_value,
  to_runtime_token}` to delegate through `oxvba_com::compat`.

This does not close `bd-bqm8.2.4`. File-system, console/UI/time/diagnostic,
dynamic-link, and Windows COM bridge compatibility wrappers still need the same
classification or adapter delegation before COM/HAL compatibility bridges are
fully externalized.

## Verification

Passed:

- `cargo check -p oxvba-com -p oxvba-hal -p oxvba-host`
- `cargo test -p oxvba-com com_value --lib`
- `cargo test -p oxvba-hal process --lib`
