# HAL Operating Envelope v1

Status: `working-draft`  
Date: 2026-03-02  
Scope: current deterministic HAL contract boundary used by OxVba runtime/host integration

## 1. Purpose

Define the exact operating envelope guaranteed by HAL v1 so compiler/runtime optimization work can rely on explicit host-boundary semantics.

## 2. Guaranteed Properties (v1)

1. Deterministic failure model:
- unsupported capability -> `HAL-E-CAP-UNAVAILABLE`
- policy denial -> `HAL-E-POLICY-DENIED`
- adapter state/argument fault -> `HAL-E-ADAPTER-FAULT`

2. Descriptor floor:
- all capabilities present exactly once in `HalDescriptor`.
- profile and capability support flags are queryable and test-covered.

3. Unsupported-mode policy:
- `CompileTime`: host-sensitive bytecode preflight rejects unsupported/policy-denied paths.
- `Runtime`: host-sensitive operations may execute and then fail deterministically.

4. Null profile floor:
- deterministic unsupported behavior for non-supported capabilities.
- explicitly supported baseline capabilities remain available by descriptor contract.

5. Filesystem model:
- deterministic in-memory handle state machine (`open/close/seek/eof/lof/free_file`).
- no OS filesystem binding guarantees in v1.

## 3. Profile Envelope (Current)

| Profile | COM activation/dispatch | Notes |
|---|---|---|
| `Windows` | Supported (deterministic projection model) | Real COM bridge is future work (H2 track). |
| `Linux` | Unsupported | Deterministic unsupported behavior. |
| `MacOs` | Unsupported | Deterministic unsupported behavior. |
| `Wasm` | Unsupported | Deterministic unsupported behavior. |
| `Null` | Unsupported | Deterministic unsupported floor profile. |

## 4. Verification Envelope

Current verification includes:
- clause-mapped conformance probes (`crates/oxvba-hal/src/conformance.rs`);
- deterministic adapter tests (unit + selected property checks);
- host integration tests for compile-time/runtime unsupported modes and error routing.

Artifacts:
- `docs/evidence/hal/HAL_CONFORMANCE_<timestamp>.md|jsonl`
- `docs/evidence/hal/HAL_PHASE1_BASELINE_AUDIT_2026-03-02.md`
- `docs/evidence/hal/HAL_PHASE2_CONTRACT_CHECKS_2026-03-02.md`
- `docs/evidence/hal/HAL_PHASE3_ADAPTER_REFINEMENT_2026-03-02.md`

## 5. Non-Guarantees (Explicitly Outside v1 Envelope)

1. Native Win32 semantics parity for UI/process/filesystem/com/dynlink operations.
2. Queue fairness/ordering guarantees for `DoEvents`.
3. ABI-stable external adapter boundary (`hal_abi_v1` is not implemented yet).
4. Rich boundary value model beyond `ValueToken = i32`.

## 6. Open Constraints and Uncertainties

Authoritative registers:
- `docs/evidence/hal/HAL_UNCERTAINTY_REGISTER.md`
- `docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`

Optimization work must treat these as boundary constraints until explicitly retired by clause updates and evidence.
