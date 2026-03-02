# HAL Policy Presets (v1)

Status: `working-draft`  
Date: 2026-03-02

## Purpose

Define explicit named host-policy presets so profile execution is reproducible and comparable across CI, local runs, and conformance evidence.

Code anchor:
- `crates/oxvba-hal/src/model.rs` (`HostPolicyPreset`, `HostPolicy::for_preset`)

## Preset Table

| Preset | Name | Interaction | Process Spawn | Filesystem Mutation | Dynamic Link | COM Activation | Deterministic Mode | UI Virtualization | Unsupported Feature Mode | Wasm Runtime Class | Intended Use |
|---|---|---:|---:|---:|---:|---:|---:|---|---|---|---|
| `StrictCi` | `strict-ci` | false | false | false | false | false | true | `FailOnPrompt` | `CompileTime` | `wasi` | maximum guard rails for CI and gate preflight |
| `DeterministicRuntime` | `deterministic-runtime` | false | true | false | false | true | true | `ScriptedResponses` | `Runtime` | `wasi` | deterministic execution with runtime host-failure surfacing |
| `DeterministicCompileTime` | `deterministic-compile-time` | false | true | false | false | true | true | `ScriptedResponses` | `CompileTime` | `wasi` | deterministic execution with compile-time host gating |
| `InteractiveDev` | `interactive-dev` | true | true | true | true | true | false | `Disabled` | `Runtime` | `wasi` | exploratory local debugging and integration probing |

## Notes

- Presets are explicit and versioned by code review, not inferred from environment.
- Presets do not yet enforce maturity governance at gate level; governance checks remain non-blocking.
- Adapter behavior must remain deterministic where preset settings are deterministic.
- Wasm runtime class can be overridden per run through `HostPolicy::with_wasm_runtime_class` (`wasi` vs `browser-sandbox`).
