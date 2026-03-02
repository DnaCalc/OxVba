# HAL Implementation-Defined Register

Status: `active`  
Purpose: capture explicit HAL behaviors chosen by OxVba that are not yet fully constrained by canonical VBA/host specs.

## Entries

| ID | Area | Decision | Why | Compatibility risk | Evidence |
|---|---|---|---|---|---|
| `HAL-ID-001` | Boundary value representation | HAL trait boundary uses `ValueToken = i32`. | Matches current VM register model and enables early integration. | High for real host interop expressiveness; planned evolution item. | `crates/oxvba-hal/src/traits.rs` |
| `HAL-ID-002` | UI interaction | `MsgBox`/`InputBox` return deterministic token outputs based on policy/virtualization mode, not native UI behavior. WASM adapter requires virtualization flow and denies `Disabled` interactive mode in v1. | Deterministic testability before full host UI integration and explicit sandbox behavior. | Medium for behavior parity. | `crates/oxvba-hal/src/adapters/standard.rs`, `crates/oxvba-hal/src/adapters/wasm.rs` |
| `HAL-ID-003` | Event pump | `do_events` returns deterministic token `0`; in host-matching non-deterministic mode it yields scheduler, and in `windows-gui` runtime class it additionally performs a non-blocking message-queue pump before yielding. | Preserve deterministic token contract while enabling bounded host event integration. | Medium for host integration parity. | `standard.rs`, `HAL-U-002` |
| `HAL-ID-004` | File model | Default path is deterministic in-memory handle state; host-matching Windows/Linux non-deterministic mode maps path tokens to temporary host files for mutable open/seek growth. | Keep deterministic floor while probing host-backed filesystem integration without claiming VBA path parity. | Medium to high for real filesystem parity claims. | `standard.rs` + adapter tests |
| `HAL-ID-005` | Process/env | Default path uses deterministic token projections; host-matching Windows/Linux non-deterministic mode enables shell spawn probe + host env/path projections. | Supports exploratory host-backed behavior with explicit policy control and deterministic fallback. | Medium for host parity. | `standard.rs`, `conformance.rs` |
| `HAL-ID-006` | COM behavior | Windows COM capability currently maps to deterministic token projection (`create_object`, `dispatch_invoke`) rather than real COM. | Preserve boundary shape while COM bridge is pending. | High for COM parity claims. | `standard.rs`, host tests |
| `HAL-ID-007` | Time/locale | Deterministic modes return constants; host-matching Windows/Linux non-deterministic mode returns system-time derived tokens. | Preserve deterministic CI/formal behavior while enabling initial host-backed runtime probing. | Medium for real-time parity semantics. | `standard.rs` |
| `HAL-ID-008` | Dynamic linking | `invoke_symbol` is policy-gated; deterministic lanes use token projection, while host-backed Windows/Linux lanes resolve a bounded known-symbol set and return adapter fault for unresolved symbols. | Enable executable Declare subset before full native ABI loader integration. | Medium for Declare semantics and portability claims. | `standard.rs`, `HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md` |
| `HAL-ID-009` | Null profile support set | Null profile supports `TimeLocale` and `DiagnosticsTelemetry`, while other capabilities are unsupported. | Enables deterministic floor with minimum diagnostics/time hooks. | Low to medium; must remain explicitly documented. | `crates/oxvba-hal/src/adapters/null.rs`, profile descriptor matrix |
| `HAL-ID-010` | Wasm runtime class split | Wasm descriptor includes runtime class (`wasi` or `browser-sandbox`); `browser-sandbox` disables `UiInteraction` capability while `wasi` keeps virtualization-based UI. | Make wasm operating envelope explicit instead of treating wasm as a single undifferentiated profile. | Medium; runtime class selection must be explicit in evidence. | `model.rs`, `standard.rs`, `wasm.rs`, conformance artifacts |
| `HAL-ID-011` | Host-backed activation semantics | Host-backed mode is active only when policy is non-deterministic and profile matches host OS (`Windows` on Windows, `Linux` on Linux). | Keep deterministic floor stable while allowing bounded native behavior probing. | Medium; cross-host profile runs remain deterministic fallback. | `model.rs`, `conformance.rs`, `HAL_CONFORMANCE_*` artifacts |
| `HAL-ID-012` | Policy configuration surface | Engine default is `Windows + deterministic-runtime`; host-runner bootstrap supports deterministic `CLI > ENV > config > defaults` resolution with stable fingerprint output. API-driven configuration remains available. | Provide reproducible startup selection while retaining embedding flexibility. | Medium for non-CLI embedding governance consistency. | `crates/oxvba-host/src/engine.rs`, `crates/oxvba-host/src/runner.rs`, `HAL-U-009` |
| `HAL-ID-013` | UI platform lanes | In host-backed non-deterministic mode, `windows-gui` may route `MsgBox` through native `MessageBoxW`; Linux `linux-stdio` emits deterministic console prompt lines and returns stable tokens without blocking input reads. | Keep non-GUI and GUI behaviors explicit and testable while avoiding hidden host dependencies. | Medium for host parity interpretation. | `crates/oxvba-hal/src/adapters/standard.rs`, `HAL_UI_PLATFORM_IMPLEMENTATION_V2.md` |

## Governance

- Any implementation-defined decision affecting HAL-visible behavior must be registered here before merge.
- If a decision is later formalized into a normative clause:
  - update clause catalog,
  - retain historical entry with migration note.
