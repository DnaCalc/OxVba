# HAL Specification Working Draft

Status: `working-draft`  
Date: 2026-03-01  
Scope owner: OxVba runtime/host

## 1. Objective

Define a deterministic Host Abstraction Layer (HAL) contract for OxVba so host-sensitive VBA/runtime features are:
- explicit by capability,
- policy-controlled,
- testable through a repeatable conformance suite,
- portable across `windows`, `linux`, `macos`, `wasm`, and `null` adapters.

This draft is implementation-linked (code exists) but still open for compatibility refinements.

Primary formal contract companion docs:
- [`HAL_CONTRACT_CLAUSE_CATALOG_V1.md`](HAL_CONTRACT_CLAUSE_CATALOG_V1.md)
- [`../evidence/hal/HAL_UNCERTAINTY_REGISTER.md`](../evidence/hal/HAL_UNCERTAINTY_REGISTER.md)
- [`../evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`](../evidence/hal/HAL_IMPLEMENTATION_DEFINED.md)

## 2. Normative Source Families

Primary external references are maintained in `../Foundation/reference`:
- MS-VBAL (language/runtime surface),
- MS-OAUT (automation data and dispatch contracts),
- MS-DTYP (supporting ABI types),
- MS-OVBA (project/module packaging context).

Crosswalk to extracted conformance candidate IDs is in [`HAL_SPEC_CROSSWALK.md`](HAL_SPEC_CROSSWALK.md).

## 3. Contract Surface

Implemented root trait: `HostServices`  
Code: `crates/oxvba-hal/src/traits.rs`

Domain subtraits:
- `UiInteractionHal`
- `EventPumpHal`
- `FileSystemHal`
- `ProcessEnvHal`
- `ComHal`
- `TimeLocaleHal`
- `DynamicLinkHal`
- `DiagnosticsHal`

Adapter factory:
- `crates/oxvba-hal/src/adapters/mod.rs` (`for_profile`)

Implemented profile adapters:
- `windows`, `linux`, `macos`, `wasm`, `null`

## 4. Capability Model

Each adapter publishes a `HalDescriptor`:
- `profile`
- `contract_version`
- `adapter_version`
- per-capability entries: `supported`, `maturity`, `spec_anchor`

Capability identifiers:
- `UiInteraction`
- `EventPump`
- `FileSystemIo`
- `ProcessEnv`
- `ComActivationDispatch`
- `TimeLocale`
- `DynamicLinking`
- `DiagnosticsTelemetry`

## 5. COM Scope Decision

Current decision:
- Windows profile declares and exercises `ComActivationDispatch`.
- Linux/macOS/WASM/Null explicitly declare COM capability unsupported.

This is intentional and test-covered. Non-Windows COM remains future scope, not implied by current adapter availability.

## 6. Unsupported Feature Policy

`HostPolicy.unsupported_feature_mode` supports two behaviors:

1. `CompileTime`:
- host-sensitive intrinsic requirements are preflighted in `oxvba-host` before execution.
- missing capability or explicit policy-deny rules fail with compile-phase diagnostics.

2. `Runtime`:
- execution is allowed to proceed.
- unsupported/policy-denied host operations fail deterministically at runtime.

Current compile-time preflighted intrinsic families:
- `Shell`, `Environ`, `Dir` -> `ProcessEnv`
- `CreateObject`, `DispatchInvoke` -> `ComActivationDispatch`

Named preset table:
- [`HAL_POLICY_PRESETS.md`](HAL_POLICY_PRESETS.md) defines reproducible policy bundles:
  - `strict-ci`
  - `deterministic-runtime`
  - `deterministic-compile-time`
  - `interactive-dev`

## 7. Deterministic Error Taxonomy

HAL stable codes (implemented in `crates/oxvba-hal/src/error.rs`):
- `HAL-E-CAP-UNAVAILABLE`
- `HAL-E-POLICY-DENIED`
- `HAL-E-ADAPTER-FAULT`
- `HAL-E-UNSUPPORTED-PROFILE`

VM propagation:
- host errors map to deterministic runtime error numbers (`53xxx`) with capability+kind encoding.
- if `On Error` handlers are active, control follows VBA error-routing behavior; otherwise execution fails with stable diagnostic detail.

## 8. Null HAL Contract

`null` adapter is a deterministic floor/oracle profile:
- unsupported capabilities must fail with `HAL-E-CAP-UNAVAILABLE`.
- selected pure deterministic capabilities remain available (`TimeLocale`, `DiagnosticsTelemetry` in current model).
- no silent no-op behavior for unsupported operations.

## 9. Conformance Execution

Pre-engine conformance:
- `cargo test -p oxvba-hal`
- `scripts/run-hal-conformance.ps1`

In-engine integration checks:
- `oxvba-host` tests validate compile-time/runtime unsupported-mode behavior and host error surfacing.

Details: [`HAL_CONFORMANCE_SUITE.md`](HAL_CONFORMANCE_SUITE.md).

## 10. Current Spec Surprises / Gaps

1. Extracted candidate packs currently expose many host APIs as signature fragments (`may`) with limited normative behavioral detail.
2. Some key host-sensitive behaviors (e.g., `DoEvents` scheduling semantics) are not cleanly captured by current extraction runs and need dedicated review/extraction refinement.
3. Behavioral requirements for UI/process interactions are split across sources and host context; strict parity claims require empirical Office-based follow-up packs.

These are tracked as design-stage uncertainty, not blockers for deterministic HAL scaffolding.
