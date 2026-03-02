# HAL Declare/Marshaling Ambiguities and Inconsistencies (2026-03-02)

Status: `active`  
Scope: external declarations (`Declare`) and boundary marshaling contracts

## 1. Purpose

Capture source-level ambiguity and extraction-quality issues that materially affect HAL contract design for `Declare` and marshaling.

This document complements:
- `HAL_DECLARE_ABI_SPEC_V1.md`
- `HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`
- `HAL_UNCERTAINTY_REGISTER.md`
- `HAL_IMPLEMENTATION_DEFINED.md`

## 2. Topic Register

| ID | Topic | Source anchor(s) | Current stance | Clause impact | Next action |
|---|---|---|---|---|---|
| `HAL-DMA-001` | External name case sensitivity | `SPEC-discovered-ms-vbal-250520-f945507e-01617` | Treat as implementation-defined; require explicit per-profile declaration in docs/descriptor. | `HAL-DYN-003` | Add descriptor-facing policy field when dynamic-link descriptor evolves. |
| `HAL-DMA-002` | Alias-based procedure selection semantics | `CONF-discovered-ms-vbal-250520-f945507e-0093`, `SPEC-...-01624..01627` | Deterministic local selector required; no broad parity claim yet. | `HAL-DYN-003` | Add oracle probe topic for Windows VBA parity. |
| `HAL-DMA-003` | Non-`PtrSafe` restrictions | `CONF-discovered-ms-vbal-250520-f945507e-0092` | Restriction policy is allowed and must be explicit; no implicit behavior. | `HAL-DYN-004` | Implement compile-time restriction matrix tests. |
| `HAL-DMA-004` | `VARIANT` byref legality coverage not yet executable in OxVba dynlink lane | `CONF-discovered-ms-oaut-240423-b76f9b41-0010`, `...-0011`, `...-0015`, `...-0016` | Keep as formal target, not implemented claim. | `HAL-DYN-005` | Add marshaling unit/property tests before enabling typed lane. |
| `HAL-DMA-005` | `SAFEARRAY`/Automation type matrix breadth | `CONF-discovered-ms-oaut-240423-b76f9b41-0023..0029`, `...-0042`, `...-0050..0060` | Track as typed-lane expansion with explicit unsupported diagnostics until complete. | `HAL-DYN-006` | Stage matrix by subtype families and test each gate. |
| `HAL-DMA-006` | Pointer-string ownership/length semantics for native ABI | `CONF-discovered-ms-dtyp-241119-518a70cb-0002..0005`, `...-0007..0009` | Require explicit metadata semantics; reject ambiguous declaration forms in strict profiles. | `HAL-DYN-007`, `HAL-DYN-010` | Define declaration metadata extension for length/termination/encoding mode. |
| `HAL-DMA-007` | `IDispatch::Invoke` output contracts in mixed COM/dynlink scenarios | `CONF-discovered-ms-oaut-210625-4fcc3347-0080..0084` | Reserve for COM-enabled lane only; no non-Windows implication. | `HAL-DYN-008` | Add Windows COM bridge conformance plan for invoke outputs. |
| `HAL-DMA-008` | Extraction quality (OCR artifacts, `candidate` confidence) | cross-family extracted JSONL rows | Use as working anchors but block stable parity claims pending canonical-source reconciliation. | all `HAL-DYN-*` pending clauses | Schedule extraction quality pass + canonical text verification sweep. |

## 3. Governance Rules

1. No `implemented-verified` promotion for dynamic-link marshaling clauses until executable checks exist.
2. Ambiguity entries must be linked to either:
- an uncertainty register item, or
- an implementation-defined decision entry.
3. Any compatibility claim touching these topics must cite both:
- anchor IDs,
- current clause status.
