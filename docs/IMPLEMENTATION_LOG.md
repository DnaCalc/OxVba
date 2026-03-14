## 2026-03-14 - Moved member-spec/direct-DISPID runtime invoke helpers into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_invoke.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_invoke.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns the reusable runtime-value invoke helpers for:
  - member-metadata-backed COM dispatch,
  - direct-DISPID COM dispatch,
  - property-get / method / property-put / property-putref routing inside those helpers.
- `oxvba-hal::standard` now keeps lookup/cache/state/error-mapping responsibilities around those calls instead of owning the invoke execution strategy itself.
- Net effect:
  - the remaining live HAL-owned COM seam is narrower again,
  - the extraction wall is now centered on resolved-member DISPID/cache authority, final invoke-result lifecycle glue, and public contract contraction.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet
## 2026-03-14 - Moved callback payload polling and metadata access into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns:
  - polling the queued callback payload,
  - resolving callback -> subscription identity,
  - reporting callback arity and argument payloads,
  - callback release bookkeeping.
- `oxvba-hal::standard` now keeps only policy/error mapping around those callback queries.
- Net effect:
  - callback interrogation is no longer a HAL-owned COM state concern,
  - the remaining extraction wall is narrower again and centered on the final invoke/result-lifecycle/contract seam.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet
## 2026-03-14 - Moved COM runtime-value invoke execution helper into oxvba-com

- Extended `crates/oxvba-com/src/windows_invoke.rs` with a higher-level runtime-value `IDispatch::Invoke` helper that:
  - executes the Windows invoke call,
  - classifies the semantic result,
  - delegates dispatch-backed result rebinding through caller-provided closures.
- Rebound `oxvba-hal::standard` so its `native_dispatch_invoke_runtime_value_args(...)` helper is now a thin delegation wrapper over that shared `oxvba-com` surface.
- This removes the generic raw execute-and-classify path from HAL-owned COM authority.
- The remaining extraction wall is now narrower again:
  - resolved-member DISPID lookup/cache update and final object rebinding/lifecycle glue still live in HAL,
  - public HAL COM contract contraction/rebinding still pending.
## 2026-03-14 - Moved COM invoke-policy planning into oxvba-com

- Added `crates/oxvba-com/src/invoke_policy.rs` as the shared policy surface for:
  - named-argument ordering validation,
  - metadata-backed argument canonicalization,
  - bound default-member/direct-DISPID/member-spec routing,
  - unbound fallback property-get planning.
- Rebound `oxvba-hal::standard` so the Windows native dispatch path now asks `oxvba-com` to plan the route before executing raw `IDispatch` calls.
- This removes the high-level default-member/direct-DISPID/member-spec routing rules from HAL-owned COM authority.
- The remaining extraction wall is now narrower:
  - live `IDispatch` execution / DISPID resolution / result rebinding still live in HAL,
  - public HAL COM contract contraction/rebinding still pending.
# Implementation Log

## 2026-03-13 - Moved COM event transport-choice resolution into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns the binding/spec-to-transport decision for COM event subscriptions, including the projection-vs-native connection-point choice.
- `oxvba-hal::standard` now keeps only apartment/policy/error-mapping responsibilities around subscription transport setup.
- Net effect:
  - the remaining COM extraction wall is now centered on invoke-policy/default-member/direct-DISPID sequencing and final HAL contract contraction,
  - event transport-choice authority is no longer primarily HAL-owned.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet

## 2026-03-13 - Moved COM binding-table mutation into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns:
  - activation-time binding insertion for native COM objects,
  - per-object DISPID cache mutation,
  - the previously extracted bound-dispatch/object-release/subscription teardown bookkeeping.
- `oxvba-hal::standard` now delegates binding-table mutation and retains only activation policy, invoke-policy sequencing, event transport choice, and contract-level error mapping.
- Net effect:
  - the remaining COM extraction wall is no longer basic object/binding bookkeeping,
  - it is now the higher-level invoke-policy/contract authority still centered in HAL.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet

## 2026-03-13 - Moved bound-dispatch and subscription teardown ownership into oxvba-com
## 2026-03-13 - Moved bound-dispatch and subscription teardown ownership into oxvba-com




