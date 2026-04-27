# V0.2 COM/HAL Compat Bridge Progress

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.2.4`
Status: complete

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
- Final COM bridge slice routed dynamic object value projection and Windows
  event callback argument projection through `oxvba_com::compat`, leaving
  retained `Variant` payloads as the COM transport truth and isolating
  `RuntimeValue` projection at an explicit adapter boundary.

Remaining direct `RuntimeValue::I32` hits in the Windows COM layer are the
legacy vtable invoke result shims inside the documented compatibility dynamic
dispatch path. The broader COM/HAL conversion surfaces now delegate through
explicit `oxvba_com::compat` and `oxvba_hal::compat` adapter modules rather
than treating slot projection as core execution truth.

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
- `cargo test -p oxvba-com dynamic --lib`
- `cargo fmt --check`
