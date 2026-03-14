# Workset: IP-04 oxvba-com / HAL Extraction Closure

Date: 2026-03-14  
Status: in-progress  
Scope: complete `IP-04` by finishing the architectural repurpose of `oxvba-com`, moving the remaining substantive Windows COM client ownership out of `oxvba-hal`, contracting the public HAL COM seam to the intended long-term delegation/bootstrap boundary, and removing transitional COM-in-HAL debt.

## 1. Purpose

This workset is the authoritative end-to-end closure plan for `IP-04`.

It extends the earlier `oxvba-com` repurpose workset from architectural direction and staged extraction into a full closure program with explicit execution phases, verification gates, and closure criteria.

Use this workset to answer:
1. what still has to move before `IP-04` can close,
2. what is in scope for `IP-04` and what is not,
3. what sequence should be executed to finish the COM/HAL refactor honestly,
4. what must be true before `IP-04` can be described as closed.

## 2. In-scope closure target

`IP-04` closes only when all of the following are true:
1. `oxvba-com` is the real owner of live Windows COM client behavior:
   - activation,
   - member/DISPID resolution,
   - invoke planning/execution,
   - invoke-result rebinding/lifecycle,
   - event subscription/callback transport,
   - typelib/runtime metadata and binding state.
2. `oxvba-hal` is reduced to:
   - capability/policy gating,
   - apartment/bootstrap hooks,
   - delegation,
   - boundary error mapping.
3. the public HAL COM surface is contracted to the intended long-term seam and no longer acts as a runtime-value-wrapped proxy for substantive COM execution policy.
4. VM and host call sites use that contracted seam.
5. `standard.rs` is no longer the substantive home of Windows COM execution logic.
6. transitional wrapper methods, helper layers, and stale ownership docs are removed.
7. verification and documentation prove that COM execution ownership now lives materially in `oxvba-com`.

## 3. Out of scope for IP-04 closure

This workset does not require closure of:
1. full VBA/Excel late-bound `IDispatch` parity (`IP-03`),
2. full VBA/Excel early-bound COM/type-library parity (`IP-05`),
3. COM server/export parity (`IP-06`),
4. full Office-style hosting parity (`IP-08`),
5. full marshal/oracle/formal closure for all downstream feature areas.

`IP-04` is an architecture/ownership closure.
It must leave the repo on the correct long-term boundary so the remaining parity work can proceed without HAL remaining the de facto COM implementation home.

## 4. Current entry state

Completed before this closure workset:
1. shared COM request/callback/value transport/types,
2. semantic runtime-value integration groundwork,
3. deterministic typelib catalog/build logic,
4. metadata cache and COM runtime state extraction,
5. Windows invoke helper extraction,
6. callback/subscription bookkeeping extraction,
7. event transport-choice and invoke-policy extraction,
8. major COM binding-table and object-release ownership extraction.

Current wall:
1. the remaining live Windows COM execution seam is still centered partly in `oxvba-hal::standard`,
2. the public `ComHal` contract is still broader and more transitional than the intended end-state,
3. final invoke-result lifecycle ownership and remaining contract authority still need to move behind an `oxvba-com` surface.

Primary active blocker:
1. `BLK-COM-BOUNDARY-001` in `CURRENT_BLOCKERS.md`

## 5. Execution plan

### Phase A. End-state lock

1. Lock the exact end-state for this phase.
2. Define the finish line explicitly in the owning docs.
3. Keep `oxvba-com` as the live Windows COM client bridge.
4. Keep `oxvba-hal` as policy/profile/bootstrap/delegation only.

Primary outcome:
1. subsequent edits are measured against an explicit closure target rather than an implicit “less COM in HAL” direction.

### Phase B. Additive `ComHal` contraction

5. Do the `ComHal` contraction as an additive migration rather than a destructive swap.
6. Add typed methods beside current wrappers first.
7. Avoid a one-shot breaking rewrite that leaves the repo half-migrated.

Primary outcome:
1. the public COM contract can narrow in controlled slices while the repo remains buildable.

### Phase C. Adapter and caller migration

8. Migrate the standard adapter to the typed event-side API.
9. Migrate null/wasm adapters to the same contracted shape.
10. Move VM COM intrinsics onto the typed API.
11. Move host COM event helpers onto the typed API.
12. Remove the temporary wrapper methods once all callers are moved.

Primary outcome:
1. the public `ComHal` surface is genuinely contracted rather than cosmetically wrapped.

### Phase D. Final invoke-result lifecycle extraction

13. Move the final invoke-result lifecycle glue into `oxvba-com`.
14. Consolidate dispatch-result rebinding, final object-handle lifecycle, and remaining execute/classify ownership there.
15. Re-center `standard.rs` on policy/apartment/delegation only.

Primary outcome:
1. HAL no longer owns the last substantive Windows invoke/result execution seam.

### Phase E. Stale HAL debt cleanup

16. Clean up stale COM-specific HAL debt.
17. Remove dead helpers, unused imports, transitional compatibility glue, and stale COM-in-HAL comments/docs.
18. Re-run the focused verification matrix after each slice.
19. Close this subphase explicitly once the contracted event-side surface, caller migration, and final invoke-result ownership are all complete.

Primary outcome:
1. the first COM/HAL contraction subphase finishes cleanly instead of leaving a transitional API tail.

### Phase F. Runtime-wide dynamic protocol completion for COM-backed objects

20. Finish the remaining runtime-wide protocol wiring for COM-backed objects.
21. Ensure COM-backed objects fully adapt into the shared internal dynamic-object protocol rather than forcing COM-special branching at VM/host boundaries.
22. Keep callback ingress, object release, and dynamic invoke on the shared semantic object/value model.

Primary outcome:
1. COM-backed objects behave as adapters onto the runtime protocol instead of pulling HAL back into a COM-special execution path.

### Phase G. Supported object/value carrier closure inside `oxvba-com`

23. Complete the supported interface/object carrier migration into `oxvba-com`.
24. Complete the supported SAFEARRAY/value-carrier ownership needed for the current architectural scope.
25. Keep this at the level required for `IP-04` ownership closure, not full Office parity.

Primary outcome:
1. supported current COM object/value/event transport ownership is centered in `oxvba-com`, not HAL.

### Phase H. Typelib/reference-facade ownership cleanup

26. Finish type-library and reference-facade ownership cleanup where it still leaks through HAL.
27. Keep compiler-visible COM reference projection clearly owned by `oxvba-com`.
28. Make HAL a delegator/bootstrap seam for metadata access rather than a hidden metadata-policy owner.

Primary outcome:
1. metadata/reference ownership lines match the architectural intent.

### Phase I. Final Windows COM client facade/service

29. Introduce the final Windows COM client facade/service in `oxvba-com`.
30. Replace the current pattern of many helper-level exports plus HAL-local orchestration with a clearer bridge/service layer for:
   - activation/binding,
   - invoke,
   - event subscription/callback,
   - metadata/reference access.
31. Rebind HAL to that service layer.

Primary outcome:
1. `oxvba-com` has a coherent live client-bridge surface rather than a pile of extracted helpers.

### Phase J. Collapse the long-term HAL COM seam

32. Collapse the public HAL COM surface to the minimal long-term seam.
33. Remove methods that existed only because HAL used to own deeper COM execution details.
34. Preserve only the capability/bootstrap/delegation interface that VM and host actually need.

Primary outcome:
1. the public COM-facing HAL contract matches the long-term architecture rather than the transitional migration shape.

### Phase K. Prove HAL is no longer the COM home

35. Remove remaining COM implementation ownership from `standard.rs`.
36. Audit for any residual raw `IDispatch` execution logic, DISPID/member lookup authority, result rebinding logic, event transport state ownership, or COM value translation still centered in HAL.
37. Move or remove any remaining substantive ownership discovered by that audit.

Primary outcome:
1. the codebase can honestly say HAL is no longer the COM implementation home.

### Phase L. Transitional debt removal and foldback

38. Remove transitional architecture debt:
   - dead compatibility wrappers,
   - duplicate glue,
   - obsolete tests that only assert the transitional boundary,
   - stale docs/comments.
39. Fold back documentation and ownership evidence into the canonical docs:
   - `ARCHITECTURE.md`,
   - `CURRENT_BLOCKERS.md`,
   - `docs/IN_PROGRESS_FEATURE_WORKLIST.md`,
   - `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`,
   - `docs/IMPLEMENTATION_LOG.md`.

Primary outcome:
1. the repo no longer has a split-brain story about where COM lives.

### Phase M. Final IP-04 verification and closeout

40. Run the final `IP-04` verification matrix.
41. Confirm the closure criteria in section 2 are all satisfied.
42. Add explicit closure evidence in the implementation log and worklist.
43. Close `IP-04` only after the evidence and code ownership both match the closure claim.

Primary outcome:
1. `IP-04` closes honestly and does not need to be reopened because the repo preserved a transitional COM-in-HAL architecture.

## 6. Verification matrix

Run these checks after each non-trivial slice where relevant:
1. `cargo fmt --all`
2. `cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings`
3. `cargo test -p oxvba-com -p oxvba-hal --quiet`
4. focused VM/host COM tests that cover:
   - dispatch invoke,
   - event subscription/callback polling,
   - object/result rebinding,
   - metadata-backed member/default-member routes.
5. `./scripts/check-governance.ps1`
6. `./scripts/meta-check.ps1 -Fast -NoArtifacts`

Final `IP-04` closure verification must additionally include:
1. an audit that `standard.rs` no longer contains substantive COM execution ownership,
2. an audit that the live Windows COM client bridge is materially centered in `oxvba-com`,
3. documentation updates proving the architectural ownership change,
4. green targeted crate/adapter/VM/host coverage through the contracted seam.

## 7. Risks and controls

Risks:
1. performing the contract migration as isolated helper pulls will leave the public COM seam permanently transitional,
2. moving parity work ahead of ownership closure could re-entrench COM behavior in HAL,
3. extracting helpers without a coherent facade/service layer could replace one oversized file with many unstructured helper modules,
4. attempting to close `IP-04` based on “most logic moved” rather than explicit ownership proof would violate the workset completion doctrine.

Controls:
1. execute contract migration as coordinated cross-crate slices when public traits change,
2. keep the runtime semantic-value and dynamic-object rules in force while contracting the boundary,
3. require ownership audits and doc foldback before closure,
4. keep `IP-04` separate from downstream parity claims so the architecture can close without pretending `IP-03`, `IP-05`, or `IP-06` are complete.

## 8. Completion criteria

This workset is complete only when:
1. `CURRENT_BLOCKERS.md` no longer lists `BLK-COM-BOUNDARY-001` as an active blocker,
2. `docs/IN_PROGRESS_FEATURE_WORKLIST.md` can honestly mark `IP-04` closed or remove it from the active in-progress register,
3. `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md` reflects a completed extraction program rather than an open architecture direction,
4. the final verification matrix in section 6 is green,
5. the code and docs agree that `oxvba-com` is now the live Windows COM client bridge and HAL is no longer the substantive COM implementation home.

## 9. Relation to adjacent worksets

This workset depends on and complements:
1. `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
2. `docs/worksets/WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md`
3. `docs/worksets/WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`
4. `docs/worksets/WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md`
5. `docs/worksets/WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md`

Interaction rule:
1. do not reduce `IP-04` scope to match a partial implementation state,
2. instead continue through the remaining steps until the ownership boundary is truly complete,
3. and let downstream parity work proceed on top of the corrected architecture.
