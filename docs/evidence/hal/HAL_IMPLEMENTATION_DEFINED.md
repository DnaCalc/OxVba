# HAL Implementation-Defined Register

Status: `active`  
Purpose: capture explicit HAL behaviors chosen by OxVba that are not yet fully constrained by canonical VBA/host specs.

## Entries

| ID | Area | Decision | Why | Compatibility risk | Evidence |
|---|---|---|---|---|---|
| `HAL-ID-001` | Boundary value representation | HAL trait boundary uses `ValueToken = i32`. | Matches current VM register model and enables early integration. | High for real host interop expressiveness; planned evolution item. | `crates/oxvba-hal/src/traits.rs` |
| `HAL-ID-002` | UI interaction | `MsgBox`/`InputBox` return deterministic token outputs based on policy/virtualization mode, not native UI behavior. WASM adapter requires virtualization flow and denies `Disabled` interactive mode in v1. | Deterministic testability before full host UI integration and explicit sandbox behavior. | Medium for behavior parity. | `crates/oxvba-hal/src/adapters/standard.rs`, `crates/oxvba-hal/src/adapters/wasm.rs` |
| `HAL-ID-003` | Event pump | `do_events` returns deterministic token `0`; in host-matching Windows/Linux non-deterministic mode it additionally yields scheduler (`thread::yield_now`) before returning. | Preserve deterministic token contract while enabling initial host-backed scheduling hook in exploratory mode. | Medium for host integration parity. | `standard.rs`, `HAL-U-002` |
| `HAL-ID-004` | File model | Default path is deterministic in-memory handle state; host-matching Windows/Linux non-deterministic mode maps path tokens to temporary host files for mutable open/seek growth. | Keep deterministic floor while probing host-backed filesystem integration without claiming VBA path parity. | Medium to high for real filesystem parity claims. | `standard.rs` + adapter tests |
| `HAL-ID-005` | Process/env | Default path uses deterministic token projections; host-matching Windows/Linux non-deterministic mode enables shell spawn probe + host env/path projections. | Supports exploratory host-backed behavior with explicit policy control and deterministic fallback. | Medium for host parity. | `standard.rs`, `conformance.rs` |
| `HAL-ID-006` | COM behavior | Windows COM capability currently maps to deterministic token projection (`create_object`, `dispatch_invoke`) rather than real COM. | Preserve boundary shape while COM bridge is pending. | High for COM parity claims. | `standard.rs`, host tests |
| `HAL-ID-007` | Time/locale | Deterministic modes return constants; host-matching Windows/Linux non-deterministic mode returns system-time derived tokens. | Preserve deterministic CI/formal behavior while enabling initial host-backed runtime probing. | Medium for real-time parity semantics. | `standard.rs` |
| `HAL-ID-008` | Dynamic linking | `invoke_symbol` is deterministic token projection with policy gate. | Reserve interface while ABI loader contract matures. | Medium for Declare semantics. | `standard.rs` |
| `HAL-ID-009` | Null profile support set | Null profile supports `TimeLocale` and `DiagnosticsTelemetry`, while other capabilities are unsupported. | Enables deterministic floor with minimum diagnostics/time hooks. | Low to medium; must remain explicitly documented. | `crates/oxvba-hal/src/adapters/null.rs`, profile descriptor matrix |
| `HAL-ID-010` | Wasm runtime class split | Wasm descriptor includes runtime class (`wasi` or `browser-sandbox`); `browser-sandbox` disables `UiInteraction` capability while `wasi` keeps virtualization-based UI. | Make wasm operating envelope explicit instead of treating wasm as a single undifferentiated profile. | Medium; runtime class selection must be explicit in evidence. | `model.rs`, `standard.rs`, `wasm.rs`, conformance artifacts |
| `HAL-ID-011` | Host-backed activation semantics | Host-backed mode is active only when policy is non-deterministic and profile matches host OS (`Windows` on Windows, `Linux` on Linux). | Keep deterministic floor stable while allowing bounded native behavior probing. | Medium; cross-host profile runs remain deterministic fallback. | `model.rs`, `conformance.rs`, `HAL_CONFORMANCE_*` artifacts |
| `HAL-ID-012` | Policy configuration surface | Engine default is `Windows + deterministic-runtime`, and policy/profile changes are currently API-driven (`set_hal_profile`, `set_host_policy`, `set_host_policy_preset`). | Keeps embedding simple while top-level runtime bootstrap contract is still being designed. | Medium to high for operational consistency until external config contract is formalized. | `crates/oxvba-host/src/engine.rs`, `HAL-U-009` |

## Governance

- Any implementation-defined decision affecting HAL-visible behavior must be registered here before merge.
- If a decision is later formalized into a normative clause:
  - update clause catalog,
  - retain historical entry with migration note.
