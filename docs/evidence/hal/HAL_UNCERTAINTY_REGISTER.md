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
| `HAL-U-010` | External name matching semantics | MS-VBAL allows implementation-defined case sensitivity and alias-based selection behavior for external procedures. | Keep deterministic normalized selector semantics per profile, but avoid broad parity claims until empirical oracle pass. | High for cross-host compatibility and migration from token-projection subset to native loaders. | Declare/marshal conformance lanes + deferred oracle | Open |
| `HAL-U-011` | Calling-convention defaults | Default convention behavior when declaration omits explicit convention is not fully pinned for all profiles/runtime classes. | Require explicit policy defaults per profile; unsupported combinations fail deterministically. | Medium to high for ABI correctness. | Declare/marshal phase H2 | Open |
| `HAL-U-012` | Pointer-string marshaling ownership/length | LPSTR/LPWSTR handling needs explicit ownership, termination, and length semantics when moving beyond deterministic token lane. | Treat pointer strings as explicit metadata-bearing types only; reject ambiguous declarations in strict modes. | High for memory safety and compatibility. | Declare/marshal phase H2/H3 | Open |
| `HAL-U-013` | Project catalog boundary | Host-project discovery and project-kind classification rules vary by host and are implementation-defined in MS-VBAL. | Language-level ProjectGraph remains normative, and explicit HAL capability contracts plus callback-backed runtime plumbing now exist; remaining open question is the live Excel/VBIDE callback-provider contract and oracle-backed host behavior. | High for project/module/reference compatibility claims and host portability. | PMR HAL integration phase | Open |
| `HAL-U-014` | Project storage boundary | MS-OVBA section-level obligations are not yet extracted in Foundation run; storage contract shape cannot yet be clause-complete. | Defer storage-parity claims and keep `ProjectStorage` capability planned until OVBA extraction depth is improved and mapped to executable clauses. | High for import/export parity and roundtrip guarantees. | PMR storage + Foundation extraction follow-up | Open |
| `HAL-U-015` | `CreateObject` selector semantics | C2 late-bound design needs explicit selector model for ProgID text + optional server-name forms without destabilizing deterministic failure behavior. | Preserve tokenized activation floor and formalize text-selector subset before implementation. | High for compatibility with real VBA/Automation behavior. | COM client C2 (`v387..v406`) | Open |
| `HAL-U-016` | Late-bound invoke error-channel parity | Exact mapping for `IDispatch::Invoke` outputs (`VarResult`, `ExcepInfo`, `ArgErr`) to OxVba diagnostics/`Err` model needs tighter empirical validation. | Define deterministic translation contract first; fold parity deltas through deferred-oracle lanes. | High for debugging and behavioral parity under failures. | COM client C2 (`v387..v406`) + oracle foldback | Open |
| `HAL-U-017` | Early-bound vtable expansion scope | Beyond controlled `OxVba.TestDispatch`, the safe/portable envelope for vtable-preferred invocation against arbitrary dual interfaces is not yet pinned. | Keep default strategy as dispatch-only and require explicit policy opt-in for bounded vtable lanes. | High for memory safety and parity claims. | COM early-binding (`v427..v466`) | Open |
| `HAL-U-018` | Typelib version drift and repair behavior | Exact host parity for typelib version selection, broken-reference diagnostics, and repair flows across project loads is not fully specified. | Track as deferred oracle topics and avoid strong compatibility claims until empirical capture. | Medium to high for project-reference compatibility narratives. | COM early-binding oracle prep (`v446..v447`) | Open |

## Governance

- New uncertainty items must be added before landing contract-affecting behavior where semantics are unresolved.
- Closure requires:
  - explicit decision note,
  - clause updates,
  - verification mapping updates.
