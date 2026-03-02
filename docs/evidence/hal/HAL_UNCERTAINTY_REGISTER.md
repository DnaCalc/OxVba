# HAL Uncertainty Register

Status: `active`  
Purpose: track unresolved HAL contract questions that can affect compatibility and optimization assumptions.

## Entries

| ID | Area | Uncertainty | Current assumption | Impact | Planned resolution phase | Status |
|---|---|---|---|---|---|---|
| `HAL-U-001` | Boundary value model | Whether `ValueToken = i32` can support robust host interop without ambiguity. | Keep token model for current scaffold; migrate to richer typed boundary in H1/H2 evolution. | High for real Win32/COM/UI/file semantics. | H1 clause refinement | Open |
| `HAL-U-002` | `DoEvents` semantics | Minimum cross-profile event-pump guarantees are not yet defined formally. | Deterministic token return without queue fairness guarantees. | Medium for host-sensitive behavioral parity. | H1/H2 | Open |
| `HAL-U-003` | UI virtualization | Exact Prompt/Cancel/result mapping contract for `MsgBox`/`InputBox` across profiles and modes. | Deterministic policy branches; no full parity claim. | Medium for interaction semantics. | H1 | Open |
| `HAL-U-004` | File model semantics | Extent of required parity between deterministic in-memory file model and host filesystem behavior. | In-memory deterministic handle model is valid baseline. | High for runtime library conformance claims. | H2 | Open |
| `HAL-U-005` | COM baseline evolution | Whether non-Windows COM remains permanently unsupported or gains constrained support later. | Unsupported on non-Windows in v1. | Medium for cross-platform roadmap commitments. | H2 planning gates | Open |
| `HAL-U-006` | Dynamic linking contract | Required ABI safety and symbol resolution guarantees by profile. | Policy-gated deterministic placeholder behavior. | Medium for Declare/interop features. | H2/H3 | Open |
| `HAL-U-007` | Error mapping granularity | Required mapping from host-layer failures to stable code families and runtime error numbers. | `HAL-E-*` + deterministic VM mapping is sufficient baseline. | Medium for diagnosability stability. | H1 test expansion | Open |
| `HAL-U-008` | Maturity promotion | Exact objective criteria for `Stub -> Experimental -> Provisional -> Stable`. | Advisory maturity metadata with manual review. | Medium for profile confidence claims. | H1 governance | Open |
| `HAL-U-009` | Policy bootstrap/orchestration | Remaining governance scope for host policy/profile bootstrap (beyond implemented precedence) and long-term embedding contract for non-CLI hosts. | Deterministic bootstrap is implemented via host runner (`CLI > ENV > config > defaults`) with fingerprinting; external-host governance details remain open. | Medium for multi-host embedding consistency. | H2/H3 governance pass | Open |

## Governance

- New uncertainty items must be added before landing contract-affecting behavior where semantics are unresolved.
- Closure requires:
  - explicit decision note,
  - clause updates,
  - verification mapping updates.
