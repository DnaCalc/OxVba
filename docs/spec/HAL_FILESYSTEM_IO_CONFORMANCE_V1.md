# HAL FileSystem I/O Conformance V1

Status: `working-draft`  
Step: `v192`  
Date: 2026-03-02

## Objective

Specify full file I/O contract expectations for supported environments and deterministic behavior in constrained profiles.

## Surface Areas

HAL filesystem operations:
- `open(path, mode)`
- `close(handle)`
- `seek(handle, pos)`
- `eof(handle)`
- `lof(handle)`
- `free_file(range_selector)`

Language/runtime operations to align:
- `Open ...`
- `Close`
- `Seek`
- `EOF`
- `LOF`
- `FreeFile`

## V1 Contract Baseline

1. Handle range:
- valid handle space is bounded and deterministic.
- `FreeFile(0)` and `FreeFile(1)` range semantics must be stable.

2. Mode and mutation:
- mutation operations require policy `allow_filesystem_mutation=true`.
- deterministic denial in restricted policies.

3. State transitions:
- `open` allocates handle and initializes cursor/length state.
- `seek` updates cursor; mutable modes may extend logical length.
- `close` releases handle deterministically.

4. Error behavior:
- invalid handle operations are deterministic adapter faults.
- unsupported profiles return capability-unavailable errors.

## Profile Intent

- `windows-gui` / `windows-headless` / `linux-stdio`: supported with full semantics target.
- `wasm-*`: unsupported in v1 baseline unless runtime class explicitly evolves.
- `null-floor`: unsupported.

## Clause Candidates

| Clause | Statement | Verification Layer |
|---|---|---|
| `HAL-FS-V1-001` | `FreeFile` low/high ranges are deterministic and gap-aware. | HAL unit/property tests |
| `HAL-FS-V1-002` | `Open/Seek/LOF/EOF/Close` state machine preserves invariants. | HAL unit/property tests |
| `HAL-FS-V1-003` | Policy-denied mutation returns stable `HAL-E-POLICY-DENIED`. | HAL + host integration |
| `HAL-FS-V1-004` | Unsupported profiles deterministically reject FS ops. | HAL conformance |
| `HAL-FS-V1-005` | Host-backed mode behavior is bounded and deterministic in error shape. | conformance host-backed checks |

## Conformance Expansion Targets

- add host integration tests for:
  - `FreeFile` high/low lane behavior from compiled VBA paths,
  - runtime error mapping for invalid handle and policy denial cases.
- add profile-lane probes for wasm/null unsupported floor.

## Open Items

- exact parity obligations with VBA text/binary/random file modes.
- path canonicalization and locale-sensitive filesystem behaviors.
- lock/sharing semantics and concurrency model.
