## 2026-03-14 - Moved activation-time COM binding creation into oxvba-com

- Continued the `IP-04` extraction slice in:
  - [windows_client.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_client.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- `oxvba-com` now owns activation-time COM binding creation through `activate_runtime_binding(...)`, combining:
  - runtime dispatch activation,
  - deterministic typelib-backed binding assembly,
  - the existing registered-test-dispatch selection policy.
- `standard.rs::create_object(...)` now delegates binding creation to `oxvba-com` and only retains:
  - apartment readiness,
  - optional registered-event override application,
  - insertion of the resulting binding into shared host state.
- Net effect:
  - HAL no longer owns activation-time COM binding assembly,
  - the remaining `IP-04` wall is narrowed to the shared-state object/result rebinding wrappers and the final invoke-result lifecycle authority.
- Verification:
  - `cargo clippy -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --all-targets -- -D warnings`
  - `cargo test -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --quiet`
  - `./scripts/check-governance.ps1`
  - `./scripts/meta-check.ps1 -Fast -NoArtifacts`
## 2026-03-14 - Moved bound runtime invoke orchestration into oxvba-com

- Continued the IP-04 extraction slice in:
  - [windows_invoke.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_invoke.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- oxvba-com now owns the high-level bound runtime invoke orchestration helper that chooses between:
  - legacy vtable fast-path invocation,
  - named default-member resolution,
  - metadata-backed member-spec dispatch,
  - direct-DISPID dispatch,
  - bound-dispatch fallback.
- standard.rs now delegates that routing choice to oxvba-com and only supplies the remaining Windows-native execution closures.
- Net effect:
  - HAL no longer owns the high-level bound runtime invoke policy,
  - the remaining IP-04 wall is narrowed further to raw Windows activation plus final invoke-result/object-lifecycle ownership.
- Verification:
  - cargo test -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --quiet
  - cargo clippy -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --all-targets -- -D warnings
  - ./scripts/check-governance.ps1
  - ./scripts/meta-check.ps1 -Fast -NoArtifacts
## 2026-03-14 - Contracted the event-side ComHal boundary to typed COM handles

- Completed the first coordinated public `ComHal` migration slice across:
  - [traits.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\traits.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [null.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\null.rs)
  - [wasm.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\wasm.rs)
  - [interpreter.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-vm\src\interpreter.rs)
  - [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs)
- `ComHal::{subscribe_event,unsubscribe_event,event_callback_subscription,event_callback_arity,event_callback_arg,release_event_callback}` now use typed COM/object handles on the public seam.
- VM COM intrinsics and host COM event helpers now decode/encode typed tokens at the edge instead of routing event-side identity through `RuntimeValue` wrappers.
- Null/wasm adapters now compile against the contracted trait while preserving explicit legacy-only test helpers outside the public trait.
- Net effect:
  - the event-side public COM contract is no longer transitional,
  - the remaining `IP-04` wall is narrowed to the live Windows invoke-result lifecycle seam and the last HAL-to-`oxvba-com` execution delegation work.
- Verification:
  - `cargo test -p oxvba-hal -p oxvba-vm -p oxvba-host --quiet`
  - `cargo clippy -p oxvba-hal -p oxvba-vm -p oxvba-host --all-targets -- -D warnings`
  - `./scripts/check-governance.ps1`
  - `./scripts/meta-check.ps1 -Fast -NoArtifacts`
## 2026-03-14 - Reached the public ComHal contraction wall

- After the resolved-member DISPID cache extraction, the next remaining COM/HAL work was tested as a typed-token `ComHal` contraction slice.
- That attempt showed the remaining boundary is no longer a local helper move:
  - it touches the public `ComHal` trait,
  - VM COM host intrinsics,
  - host COM event helper surfaces,
  - null/wasm adapter stubs,
  - and the final result-lifecycle glue still routed through HAL.
- I reverted the partial uncommitted contract-edit attempt rather than leaving the repo in a half-migrated state.
- Current conclusion:
  - the next COM/HAL step must be executed as one coordinated public contract migration program,
  - not as another incremental helper extraction.
## 2026-03-14 - Moved resolved-member DISPID cache lookup/update into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns the reusable helper that:
  - resolves member metadata fallback for a bound COM object,
  - performs raw `GetIDsOfNames` member lookup when needed,
  - updates the bound-object DISPID cache in shared COM runtime state.
- `oxvba-hal::standard` now delegates that cache/lookup behavior and keeps only binding fallback selection and error mapping around it.
- Net effect:
  - resolved-member cache authority is no longer HAL-owned,
  - the remaining COM extraction wall is now centered on final invoke-result lifecycle glue and public contract contraction.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet
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

## 2026-03-14 - Added explicit IP-04 closure workset

- Added:
  - [WORKSET_2026-03-14_IP04_OXVBA_COM_HAL_EXTRACTION_CLOSURE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_IP04_OXVBA_COM_HAL_EXTRACTION_CLOSURE.md)
- Purpose:
  - turn the approved 1-24 COM/HAL extraction plan into the authoritative end-to-end `IP-04` closure workset,
  - make explicit what is and is not required to close `IP-04`,
  - define the final verification and ownership-audit gates needed before `IP-04` can be described as complete.
- Cross-linked the new closure workset from:
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)


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










