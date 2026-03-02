# HAL Declare Execution Implementation V2

Status: `implemented-partial`
Scope: `v212..v286`
Date: 2026-03-02

## Implemented Pipeline

1. Resolver capture:
- `Declare` statements are captured in `BoundModule.external_declarations` with deterministic metadata:
  - declared name,
  - normalized `Lib`,
  - normalized `Alias`,
  - ordinal/symbolic alias mode,
  - `PtrSafe` marker.

2. Compiler lowering:
- external calls lower to `Instruction::IntrinsicInvokeSymbolHost`.
- instruction payload includes both:
  - deterministic symbol token hash, and
  - stable descriptor ID.
- bytecode now carries `external_call_descriptors` with deterministic ordering and explicit metadata:
  - `marshal_lane` (currently `m0-deterministic`),
  - `calling_convention` (currently `platform-default`),
  - `selection_policy` (currently `case-insensitive-canonical`).

3. VM execution:
- runtime supports two modes:
  - legacy symbol-only invocation when descriptor table is absent,
  - descriptor-driven invocation when descriptor table is present.
- descriptor mismatch/unknown-ID conditions surface deterministic adapter faults.

4. HAL dynamic-link contract evolution:
- trait now supports split phases:
  - `bind_descriptor`,
  - `prepare_invoke`,
  - `invoke_bound`,
  - plus compatibility shims (`invoke_descriptor`, legacy `invoke_symbol`).
- standard adapter implements descriptor binding cache and deterministic validation of lane/convention.

5. Policy and mode gates:
- compile-time preflight still enforces policy gates for declare invocation in compile-time unsupported mode.
- runtime mode preserves deterministic host error mapping (`Err.Number`/stable HAL code path).

6. Host-backed lane subset:
- Windows/Linux host-backed lane supports bounded known symbols:
  - `host!ping!HostPing` -> `arg + 1`
  - `host!double!HostDouble` -> `arg * 2`
- unresolved symbol in host-backed mode returns deterministic adapter fault.
- non-host-backed deterministic mode preserves token-projection behavior.

## Current Scope Boundary

- `M0` deterministic lane is executable and descriptor-driven.
- `M1` (`VARIANT`/`SAFEARRAY`/`BSTR`) and `M2` native ABI marshaling lanes are still partial/deferred and tracked as implementation-defined backlog with explicit clause coverage status.
- Loader contracts are currently validated through host-backed lane probes, not yet through full unrestricted OS symbol loading.

## Clause Relationship Snapshot

- implemented-verified:
  - `HAL-DYN-001`, `HAL-DYN-002`, `HAL-DYN-004`.
- implemented-partial:
  - `HAL-DYN-003`, `HAL-DYN-009`, `HAL-DYN-010`,
  - `HAL-DYN-011..020`.
- specified/deferred:
  - rich Automation/native marshaling legality breadth (`HAL-DYN-005..008`) remains staged and deferred-oracle linked.

## Test/Conformance Evidence Anchors

- Compiler:
  - `compile_declare_function_stub_binding_subset_is_accepted`
  - `compile_declare_descriptor_table_is_stable_for_identical_source`
- VM:
  - `declare_invoke_routes_through_dynlink_host_service`
  - `declare_invoke_uses_descriptor_table_when_present`
  - `declare_invoke_descriptor_id_mismatch_is_reported`
- HAL conformance:
  - probe `dynlink.invoke_symbol`
  - probe `dynlink.invoke_descriptor`
  - dynamic-link contract checks in `evaluate_dynlink_contract_paths`.
