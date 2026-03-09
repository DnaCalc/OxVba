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

Block-A expansion companion docs (`v187..v196`):
- [`HAL_RUNTIME_PROFILE_MATRIX_V1.md`](HAL_RUNTIME_PROFILE_MATRIX_V1.md)
- [`HAL_UI_INTERACTION_CONFORMANCE_V1.md`](HAL_UI_INTERACTION_CONFORMANCE_V1.md)
- [`HAL_DOEVENTS_CONFORMANCE_V1.md`](HAL_DOEVENTS_CONFORMANCE_V1.md)
- [`HAL_COM_BRIDGE_SCOPE_V1.md`](HAL_COM_BRIDGE_SCOPE_V1.md)
- [`HAL_DECLARE_ABI_SPEC_V1.md`](HAL_DECLARE_ABI_SPEC_V1.md)
- [`HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`](HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md)
- [`HAL_FILESYSTEM_IO_CONFORMANCE_V1.md`](HAL_FILESYSTEM_IO_CONFORMANCE_V1.md)
- [`HAL_WASM_RUNTIME_CLASSES_V1.md`](HAL_WASM_RUNTIME_CLASSES_V1.md)
- [`HAL_TIME_SEMANTICS_V1.md`](HAL_TIME_SEMANTICS_V1.md)
- [`HOST_RUNNER_POLICY_BOOTSTRAP_V1.md`](HOST_RUNNER_POLICY_BOOTSTRAP_V1.md)
- [`HAL_CONFORMANCE_EXPANSION_PLAN_V196.md`](HAL_CONFORMANCE_EXPANSION_PLAN_V196.md)

Block-B/C implementation companion docs (`v197..v220`):
- [`HAL_RUNTIME_PROFILE_BOOTSTRAP_IMPLEMENTATION_V2.md`](HAL_RUNTIME_PROFILE_BOOTSTRAP_IMPLEMENTATION_V2.md)
- [`HAL_UI_PLATFORM_IMPLEMENTATION_V2.md`](HAL_UI_PLATFORM_IMPLEMENTATION_V2.md)
- [`HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md`](HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md)
- [`../evidence/hal/HAL_BLOCK_BCD_IMPLEMENTATION_2026-03-02.md`](../evidence/hal/HAL_BLOCK_BCD_IMPLEMENTATION_2026-03-02.md)

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
- `TypeLibraryHal`
- `TimeLocaleHal`
- `DynamicLinkHal`
- `DiagnosticsHal`

Adapter factory:
- `crates/oxvba-hal/src/adapters/mod.rs` (`for_profile`, `for_profile_with_runtime_class`)

Implemented profile adapters:
- `windows`, `linux`, `macos`, `wasm`, `null`

Current implementation shape:
- `windows`/`linux`/`macos` use a shared contract core (`StandardHostServices`) with profile-specific descriptor/capability surfaces;
- `wasm` and `null` are dedicated adapters with explicit deterministic profile floors (no wrapper-only aliasing);
- in deterministic policy presets, behavior stays deterministic by contract;
- on host-matching Windows/Linux builds with non-deterministic policy presets (for example `interactive-dev`), selected domains use host-backed behavior paths.
- current factory construction instantiates `StandardHostServices` directly for Windows/Linux/macOS profiles.

## 4. Capability Model

Each adapter publishes a `HalDescriptor`:
- `profile`
- `runtime_class`
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
- `Date`, `Time`, `Now`, `Timer` -> `TimeLocale`
- `FreeFile` -> `FileSystemIo`
- `MsgBox`, `InputBox` -> `UiInteraction`
- `DoEvents` -> `EventPump`
- `CreateObject`, `DispatchInvoke` -> `ComActivationDispatch`

Current COM callback/runtime note:
- `EventPumpHal::do_events()` is still the host/event-pump intrinsic surface.
- COM callback consumption now also has a payload-returning path via `ComHal::poll_event_callback()`.
- legacy callback-token interrogation methods remain temporarily present for compatibility with older VM/compiler scaffolding and should be treated as transitional.

Named preset table:
- [`HAL_POLICY_PRESETS.md`](HAL_POLICY_PRESETS.md) defines reproducible policy bundles:
  - `strict-ci`
  - `deterministic-runtime`
  - `deterministic-compile-time`
  - `interactive-dev`

Host-backed mode availability:
- `interactive-dev` can activate host-backed paths only when profile matches current OS build target:
  - Windows profile on Windows host build,
  - Linux profile on Linux host build.
- other profile/host combinations stay on deterministic fallback paths.

Policy bootstrap/orchestration note:
- deterministic bootstrap resolution is implemented in host runner (`CLI > ENV > config > defaults`) with deterministic startup fingerprinting.
- CLI integration is available through `oxvba-cli run` bootstrap flags.
- remaining governance questions for non-CLI embedding and long-term orchestration are tracked as `HAL-U-009` in [`../evidence/hal/HAL_UNCERTAINTY_REGISTER.md`](../evidence/hal/HAL_UNCERTAINTY_REGISTER.md).

Current host-backed domains (Windows/Linux host-matching mode):
- `FileSystemHal` (token-mapped temp-dir file backing for mutable open/seek growth),
- `ProcessEnvHal` (`shell` spawn probe, host env projection, directory enumeration probe),
- `TimeLocaleHal` (system-time derived tokens),
- `EventPumpHal` (`thread::yield_now`, with non-blocking Windows queue pump in `windows-gui` runtime class),
- `UiInteractionHal` (`windows-gui` native `MessageBoxW` lane; `linux-stdio` non-blocking prompt/response lane),
- `DynamicLinkHal` (known-symbol host-backed subset plus deterministic projection fallback),
- `DiagnosticsHal` (stderr emission side-effect while preserving token contract).

Current type-library note:
- `TypeLibraryHal` is part of the current public HAL surface and is implemented in `StandardHostServices`.
- This is current truth, not a long-term architecture endorsement; the active COM extraction plan intends to move deeper typelib ownership toward `oxvba-com`.

## 7. Deterministic Error Taxonomy

HAL stable codes (implemented in `crates/oxvba-hal/src/error.rs`):
- `HAL-E-CAP-UNAVAILABLE`
- `HAL-E-POLICY-DENIED`
- `HAL-E-ADAPTER-FAULT`
- `HAL-E-UNSUPPORTED-PROFILE`

Related implemented families outside the centralized HAL enum:
- `COM-E-*` string-prefixed adapter/host diagnostics for COM activation/dispatch/event lifecycle failures

VM propagation:
- host errors map to deterministic runtime error numbers (`53xxx`) with capability+kind encoding.
- if `On Error` handlers are active, control follows VBA error-routing behavior; otherwise execution fails with stable diagnostic detail.

## 8. Null HAL Contract

`null` adapter is a deterministic floor/oracle profile:
- unsupported capabilities must fail with `HAL-E-CAP-UNAVAILABLE`.
- selected pure deterministic capabilities remain available (`TimeLocale`, `DiagnosticsTelemetry` in current model).
- no silent no-op behavior for unsupported operations.

## 8.5 Wasm HAL Contract (v1)

`wasm` adapter in v1 is deterministic and sandbox-oriented, with explicit runtime classes:
- `wasi`:
  - `UiInteraction` remains supported under virtualization policy,
  - host integration capabilities (`FileSystemIo`, `ProcessEnv`, `ComActivationDispatch`, `DynamicLinking`) are unsupported.
- `browser-sandbox`:
  - `UiInteraction` is capability-unavailable by descriptor contract,
  - host integration capabilities remain unsupported.

Common v1 wasm guarantees:
- unsupported capabilities (`FileSystemIo`, `ProcessEnv`, `ComActivationDispatch`, `DynamicLinking`) fail with `HAL-E-CAP-UNAVAILABLE`;
- `UiInteraction` (when supported by runtime class) requires policy-enabled interaction plus virtualization (`ScriptedResponses`); `Disabled`/`FailOnPrompt` return deterministic policy denial;
- `EventPump`, `TimeLocale`, and `DiagnosticsTelemetry` remain available with deterministic token semantics.

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
4. The current HAL surface still contains COM-heavy areas (`ComHal`, `TypeLibraryHal`) that are planned extraction targets, so this draft describes current contract shape while that refactor is underway.

These are tracked as design-stage uncertainty, not blockers for deterministic HAL scaffolding.
