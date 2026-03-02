# HAL Phase-1 Baseline Audit (2026-03-02)

Status: `phase-1-baseline`  
Source set: `crates/oxvba-hal`, `crates/oxvba-host`, HAL spec drafts

## 1. Objective

Capture current HAL contract reality before deeper formalization and implementation expansion.

## 2. Capability-Domain Audit

| Capability | Current state | Verification state | Key gap |
|---|---|---|---|
| `UiInteraction` | Deterministic policy-driven token behavior (`msg_box`, `input_box`) | minimal conformance probe on `msg_box` | no per-mode clause tests for all branches |
| `EventPump` | Deterministic token return (`do_events`) | conformance probe | no formal queue/fairness contract yet |
| `FileSystemIo` | Stateful in-memory handle model (`open/close/seek/eof/lof/free_file`) | dedicated adapter tests + conformance probe | not yet OS-backed semantics; no read/write statement model |
| `ProcessEnv` | Deterministic token behavior (`shell`, `environ`, `dir`) with policy gate on `shell` | conformance probe + host compile-time policy test | no real process/env/path interop contract yet |
| `ComActivationDispatch` | Windows supported as deterministic token model; non-Windows unsupported | profile-support tests + host compile/runtime mode tests | no real COM activation/dispatch contract yet |
| `TimeLocale` | Deterministic token constants | timer probe in conformance | no per-method clause tests |
| `DynamicLinking` | Deterministic token behavior with policy gate | conformance probe | no ABI/symbol-loading contract yet |
| `DiagnosticsTelemetry` | Deterministic token behavior | conformance probe | no structured payload schema contract yet |

## 3. Cross-Cutting Contract Audit

| Area | Current state | Verification state | Key gap |
|---|---|---|---|
| Descriptor completeness | enforced in harness | direct verification present | maturity metadata constraints are informal |
| Stable error taxonomy | stable codes in place (`HAL-E-*`) | partially verified through runtime tests | no exhaustive error-to-clause mapping tests |
| Unsupported mode policy | compile-time and runtime modes both implemented | host tests present | coverage only for selected host-sensitive intrinsics |
| Null profile floor | deterministic unsupported baseline present | covered by multi-profile conformance runs | clause-level assertions incomplete |
| Spec traceability | capability anchors drafted | crosswalk drafted | many anchors are candidate-level, behavior details still weak |

## 4. Phase-1 Deliverables Produced

1. HAL formalization program:
- `docs/spec/HAL_FORMALIZATION_PROGRAM.md`

2. Clause catalog v1:
- `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md`

3. HAL uncertainty register:
- `docs/evidence/hal/HAL_UNCERTAINTY_REGISTER.md`

4. HAL implementation-defined register:
- `docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`

## 5. Baseline Conclusion

Phase-1 confirms a deterministic and testable HAL skeleton exists, with strongest contract confidence in:
- descriptor/policy/error floors,
- unsupported behavior determinism,
- file-handle state model.

Main formalization pressure points for next phase:
- richer boundary value model (beyond `ValueToken = i32`),
- precise per-domain behavioral contracts for real host interop,
- clause-level test expansion across all domain methods and policy branches.
