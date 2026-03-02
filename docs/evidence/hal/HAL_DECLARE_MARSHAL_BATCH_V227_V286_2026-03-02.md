# HAL Declare/Marshal Batch Summary (v227..v286)

Date: 2026-03-02

## Implemented

- Compiler bytecode now carries deterministic `external_call_descriptors`.
- `IntrinsicInvokeSymbolHost` now carries descriptor ID + symbol token.
- VM dynamic-link dispatch is descriptor-aware when descriptor tables are present; legacy symbol-only mode is preserved when absent.
- HAL `DynamicLinkHal` trait now exposes phased contract hooks:
  - `bind_descriptor`
  - `prepare_invoke`
  - `invoke_bound`
  - `invoke_descriptor` (default composition)
  - legacy `invoke_symbol` shim
- Standard adapter implements descriptor binding cache and lane/convention validation in `bind_descriptor`.
- HAL conformance now probes descriptor-driven dynamic-link path (`dynlink.invoke_descriptor`) and emits checks for `HAL-DYN-011..020` with partial/deferred semantics where appropriate.

## Updated Clause Status

- `HAL-DYN-011..020` moved from `specified-pending` to `implemented-partial` in clause catalog.
- Verification scope for executable clauses now points to conformance or host tests.

## Evidence Commands (this batch)

- `cargo test -p oxvba-compiler compile_declare_`
- `cargo test -p oxvba-vm declare_invoke_`
- `cargo test -p oxvba-hal`
- `cargo test -p oxvba-host hal_runtime_host_backed_declare_`
- `cargo test --workspace --no-run`
- `./scripts/check-hal-clause-drift.ps1`

## Deferred/Backlog

- Full `M1` Automation legality matrix (`VARIANT`, `SAFEARRAY`, `BSTR`) remains partial/deferred.
- Full `M2` native ABI marshaling, pointer-string ownership contracts, and byref writeback remain staged and clause-tracked.
- Loader breadth remains bounded known-symbol subset in host-backed lanes pending unrestricted ABI loader work.
