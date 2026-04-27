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
- Follow-up slice routed the remaining standard HAL local projection helpers
  for UI, console, diagnostics, filesystem, dynamic-link, and COM wrapper
  paths through `oxvba_hal::compat`.

This does not close `bd-bqm8.2.4`. File-system, console/UI/time/diagnostic,
dynamic-link, and standard COM local projection helpers now delegate through
the explicit HAL adapter boundary. Windows COM bridge compatibility entry
points and broader cross-adapter classification still need final review before
COM/HAL compatibility bridges are fully externalized.

## Verification

Passed:

- `cargo check -p oxvba-com -p oxvba-hal -p oxvba-host`
- `cargo test -p oxvba-com com_value --lib`
- `cargo test -p oxvba-hal process --lib`
- `cargo test -p oxvba-hal console --lib`
- `cargo test -p oxvba-hal diagnostics --lib`
- `cargo test -p oxvba-hal filesystem --lib`
- `cargo test -p oxvba-hal dynlink --lib`
- `cargo test -p oxvba-hal com --lib`
