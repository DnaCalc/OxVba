# HAL UI Interaction Conformance V1

Status: `working-draft`  
Step: `v188`  
Date: 2026-03-02

## Objective

Specify `MsgBox` and `InputBox` HAL behavior for:
- Windows GUI,
- Windows headless,
- Linux non-GUI (`stdio`),
- wasm classes,
- null-floor.

## Surface

Trait surface:
- `msg_box(prompt, style) -> token`
- `input_box(prompt, default_value) -> token`

Current boundary token model is `ValueToken = i32`.

## Modes

UI behavior is determined by both profile and policy.

Policy controls:
- `allow_interaction`
- `ui_virtualization` (`Disabled`, `ScriptedResponses`, `FailOnPrompt`)

## Normative Behavior (V1)

1. If capability unsupported: return `HAL-E-CAP-UNAVAILABLE`.
2. If `allow_interaction=false`: return `HAL-E-POLICY-DENIED`.
3. `ScriptedResponses`:
- `MsgBox` returns deterministic response token.
- `InputBox` returns deterministic supplied/default token.
4. `FailOnPrompt`: return `HAL-E-POLICY-DENIED`.
5. `Disabled`:
- `windows-gui` may use native UI in non-deterministic policy lanes.
- `windows-headless` and `linux-stdio` must never require GUI availability.
- `linux-stdio` uses text prompt flow in interactive mode.

## Profile Notes (V1 intent)

- `windows-gui`: native path allowed in interactive lane; deterministic virtualization in deterministic lanes.
- `windows-headless`: no GUI calls; virtualization or denial only.
- `linux-stdio`: console/stdin integration (no GUI toolkit dependency).
- `wasm-wasi-local`: virtualization only.
- `wasm-browser-sandbox`: capability unavailable.
- `null-floor`: capability unavailable.

## Clause Candidates

| Clause | Statement | Verification Layer |
|---|---|---|
| `HAL-UI-V1-001` | Unsupported profile returns `HAL-E-CAP-UNAVAILABLE` for both ops. | HAL conformance |
| `HAL-UI-V1-002` | Policy deny returns `HAL-E-POLICY-DENIED` with stable operation field. | HAL + host runtime |
| `HAL-UI-V1-003` | Deterministic lanes produce deterministic token responses. | HAL tests |
| `HAL-UI-V1-004` | `windows-headless` never blocks waiting for native GUI event loop. | host integration |
| `HAL-UI-V1-005` | `linux-stdio` prompt path is line-oriented and deterministic under scripted policy. | host integration |

## Conformance Harness Extensions

Add harness rows:
- `ui.msg_box.scripted`
- `ui.msg_box.denied`
- `ui.input_box.scripted`
- `ui.input_box.denied`

Host integration checks:
- compile-time gating when `allow_interaction=false`,
- runtime `On Error Resume Next` mapping for denied/unsupported modes.

## Open Items

- final mapping from VBA `MsgBox` style/button constants to HAL response tokens.
- `InputBox` cancel representation and empty-string equivalence rules.
- exact transcript behavior for `linux-stdio` automation mode.
