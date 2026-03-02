# HAL Declare/Marshal Closure Pass (2026-03-02)

Scope: targeted closure pass over outstanding Declare/marshaling topics that can be resolved without deferred-oracle or full M1/M2 implementation.

## Closures Landed

1. Descriptor metadata contract hardening (`DynamicLinkHal::bind_descriptor`)
- Added deterministic validation for:
  - `marshal_lane` (`m0-deterministic` only in current subset),
  - `calling_convention` (`platform-default` only in current subset),
  - `selection_policy` consistency (`case-insensitive-canonical` for symbolic aliases, `ordinal-literal-canonical` for ordinal aliases, legacy shim allowed only for legacy descriptors),
  - non-empty descriptor identity fields,
  - ordinal alias shape (`#` + digits).

2. Ordinal alias descriptor policy emission
- Compiler now emits `selection_policy = ordinal-literal-canonical` for `#ordinal` aliases.
- Added compiler coverage for canonical ordinal metadata emission.

3. Executable conformance for unsupported marshaling lanes
- `evaluate_dynlink_contract_paths` now asserts deterministic rejection (adapter-fault shape) for:
  - pointer-string lane descriptor (`m2-pointer-lpstr`),
  - byref-writeback lane descriptor (`m2-byref-writeback`).
- Replaced prior placeholder/deferred-only clause details with executable checks.

4. Declaration-surface evidence expansion
- Added tests explicitly proving deterministic rejection of:
  - `Variant` boundary parameter in current subset,
  - array boundary parameter in current subset.

## Clause/Spec Updates

- Updated clause catalog statuses:
  - `HAL-DYN-005..007`: `specified-pending` -> `implemented-partial` (deterministic subset restriction evidence now explicit).
- Updated clause narratives for `HAL-DYN-018..019` to reference executable rejection checks.
- Updated ABI + conformance docs to reflect current partial closure and remaining breadth.

## Validation Commands (PASS)

- `cargo test -p oxvba-compiler compile_declare_`
- `cargo test -p oxvba-hal`
- `cargo test -p oxvba-vm declare_invoke_`
- `cargo test -p oxvba-host hal_runtime_host_backed_declare_`
- `./scripts/check-hal-clause-drift.ps1`

## Remaining Outstanding (Non-Deferred)

1. `HAL-DYN-008` remains `specified-pending`.
- COM `IDispatch::Invoke` out-parameter obligations (`VarResult`/`ExcepInfo`/`ArgErr`) still need full executable lane coverage.

2. `HAL-DYN-005..007` remain partial.
- Full Automation/native legality matrices are not yet implemented (current closure is deterministic subset restriction + unsupported-lane rejection).

3. Loader breadth remains bounded host-backed symbol subset.
- Full unrestricted loader + ABI marshaling closure is still pending by design.
