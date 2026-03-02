# HAL Implementation-Defined Register

Status: `active`  
Purpose: capture explicit HAL behaviors chosen by OxVba that are not yet fully constrained by canonical VBA/host specs.

## Entries

| ID | Area | Decision | Why | Compatibility risk | Evidence |
|---|---|---|---|---|---|
| `HAL-ID-001` | Boundary value representation | HAL trait boundary uses `ValueToken = i32`. | Matches current VM register model and enables early integration. | High for real host interop expressiveness; planned evolution item. | `crates/oxvba-hal/src/traits.rs` |
| `HAL-ID-002` | UI interaction | `MsgBox`/`InputBox` return deterministic token outputs based on policy/virtualization mode, not native UI behavior. | Deterministic testability before full host UI integration. | Medium for behavior parity. | `crates/oxvba-hal/src/adapters/standard.rs` |
| `HAL-ID-003` | Event pump | `do_events` currently returns deterministic token `0` when supported. | Keeps deterministic execution while queue contract is unresolved. | Medium for host integration parity. | `standard.rs`, `HAL-U-002` |
| `HAL-ID-004` | File model | File I/O uses deterministic in-memory handle state with bounded handle ranges and pseudo length initialization. | Provides robust deterministic floor for contract testing. | Medium to high for real filesystem parity claims. | `standard.rs` + adapter tests |
| `HAL-ID-005` | Process/env | `shell`, `environ`, `dir` use deterministic token projections with policy gating, not OS process/path/env behavior. | Early contract wiring with explicit policy semantics. | Medium for host parity. | `standard.rs`, `conformance.rs` |
| `HAL-ID-006` | COM behavior | Windows COM capability currently maps to deterministic token projection (`create_object`, `dispatch_invoke`) rather than real COM. | Preserve boundary shape while COM bridge is pending. | High for COM parity claims. | `standard.rs`, host tests |
| `HAL-ID-007` | Time/locale | Time APIs return deterministic constants in v1. | CI reproducibility and deterministic formal lanes. | Medium for real-time parity semantics. | `standard.rs` |
| `HAL-ID-008` | Dynamic linking | `invoke_symbol` is deterministic token projection with policy gate. | Reserve interface while ABI loader contract matures. | Medium for Declare semantics. | `standard.rs` |
| `HAL-ID-009` | Null profile support set | Null profile supports `TimeLocale` and `DiagnosticsTelemetry`, while other capabilities are unsupported. | Enables deterministic floor with minimum diagnostics/time hooks. | Low to medium; must remain explicitly documented. | `capability_matrix` in `standard.rs` |

## Governance

- Any implementation-defined decision affecting HAL-visible behavior must be registered here before merge.
- If a decision is later formalized into a normative clause:
  - update clause catalog,
  - retain historical entry with migration note.
