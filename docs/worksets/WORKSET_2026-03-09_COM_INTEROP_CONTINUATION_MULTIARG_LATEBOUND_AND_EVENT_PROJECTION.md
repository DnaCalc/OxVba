# Workset: COM Interop Continuation - Multi-Arg Late-Bound Invoke and Event Projection Follow-Up

Date: 2026-03-09  
Status: planned  
Primary ladder mapping: `v506..v526`  
Secondary ladder mapping: `v538`  
Program anchor: `docs/worksets/PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE.md`

## Scope

Continue COM interop design and implementation beyond the now-green registered event callback lane, with focus on:

1. upgrading the late-bound COM invoke boundary from the current single-argument contract to a true multi-argument invoke model,
2. aligning runtime, HAL, compiler, and typelib metadata around that invoke model,
3. closing the residual correctness risk in synthetic event projection for multi-argument callbacks,
4. making the supported COM invoke/event scope explicit in docs, diagnostics, and evidence.

This work item explicitly includes the currently observed example:
- general late-bound `DispatchInvoke` is still limited by `dispatch_invoke(object, member, arg)`,
- pair-event native callback delivery is green,
- synthetic projection trace still shows duplicated pair payload shape (`[arg, arg]`) where native callback delivery produces `[arg, arg + 1]`.

## Current baseline

Completed and usable:
1. registered external COM event lane now passes with `OxVba.TestEventServer`,
2. zero-, one-, and two-argument callback payloads can be delivered back into OxVBA through the event callback path,
3. Windows message pumping and connection-point IID handling were hardened,
4. script/evidence support for registered callback runs is in place.

Still incomplete:
1. `ComHal::dispatch_invoke` remains a single-argument API,
2. general late-bound COM method/property invocation with two or more call arguments is not implemented as a real `IDispatch::Invoke` argument-vector path,
3. current array-token marshalling is not equivalent to full multi-argument COM invoke semantics,
4. synthetic event projection for pair callbacks still needs direct correctness coverage and likely implementation cleanup.

## Objectives

1. Define and implement a `COM-HAL-V2` invoke contract that can carry:
   - method/property kind,
   - argument vector,
   - named-arg metadata where required,
   - property-put/property-putref semantics,
   - LCID/diagnostic context as needed.
2. Preserve deterministic diagnostics and explicit strategy behavior while expanding capability.
3. Prove the new invoke path with controlled fixtures before widening the supported surface.
4. Close the pair-event projection gap or explicitly remove/bound that path where native transport is authoritative.

## Design questions to lock

### CID-01 Invoke payload shape

Choose the boundary shape for COM invoke requests:
1. `dispatch_invoke(object, member, args: &[i32])`
2. `dispatch_invoke_v2(request_struct)`
3. retain legacy API and add `dispatch_invoke_multi(...)` for staged migration

Recommendation:
- use an explicit request struct, because invoke kind, named args, LCID, and property-put metadata do not fit a bare slice cleanly.

### CID-02 Runtime value transport

Decide how multi-argument payloads cross the VM/HAL boundary:
1. packed token referencing runtime-owned argument bundle,
2. direct borrowed slice at host boundary,
3. staged array-token extension with richer metadata

Recommendation:
- use a request/bundle model that separates "single VBA value" from "invoke argument pack"; do not overload current array tags further.

### CID-03 Event projection authority

Decide what should happen when both native callback transport and synthetic projection mapping exist:
1. native callback path is authoritative; synthetic projection is disabled for those members,
2. synthetic projection remains as fallback but must match native payload shape exactly,
3. split trigger-only behavior from callback-delivery behavior explicitly

Recommendation:
- make native callback delivery authoritative and keep synthetic projection only for lanes that do not have native callback ingress.

## Execution phases

### CIP1 - Contract design lock

Deliverables:
1. COM invoke v2 design note with request/response/error shape,
2. migration strategy from `dispatch_invoke(object, member, arg)`,
3. diagnostics map for unsupported invoke shapes during transition.

Mapped ladder steps:
- `v506`
- `v507`

### CIP2 - HAL/runtime boundary upgrade

Deliverables:
1. new HAL trait/API for multi-arg invoke,
2. VM/runtime plumbing for argument bundles,
3. compatibility bridge for existing single-arg call sites during migration.

Mapped ladder steps:
- `v506`
- `v507`
- `v508`
- `v509`

### CIP3 - Controlled fixture expansion

Deliverables:
1. extend controlled COM fixtures to expose real two-arg and three-arg late-bound method/property cases,
2. add registrationless and registered tests that exercise true multi-arg invoke,
3. add explicit property-get/put/putref/named-arg coverage where feasible.

Mapped ladder steps:
- `v507`
- `v508`
- `v509`
- `v517`
- `v519`

### CIP4 - Pair-event projection cleanup

Deliverables:
1. direct tests for synthetic pair-event projection payload shape,
2. implementation fix or authoritative disablement when native callback transport is present,
3. trace semantics updated so evidence does not imply stale/incorrect fallback behavior.

Mapped ladder steps:
- `v520`
- `v538`

### CIP5 - Metadata and compiler integration

Deliverables:
1. typelib metadata carries what the invoke path actually needs,
2. compiler/project lowering can express multi-arg/named/property intent without hidden packing assumptions,
3. docs/specs stop overstating current late-bound capability.

Mapped ladder steps:
- `v517`
- `v518`
- `v519`
- `v521`
- `v522`
- `v523`
- `v524`
- `v525`
- `v526`

## Concrete examples that must be covered

1. True multi-arg method call:
   - `x = DispatchInvoke(obj, "Foo", ???)` is not enough today for `Foo(a, b)`.
   - add a fixture method that requires two distinct arguments and verify both arrive correctly.
2. Property put semantics:
   - verify `PROPERTYPUT` carries the value argument and named-arg metadata correctly.
3. Property putref semantics:
   - verify object-reference assignment path is distinct from scalar put.
4. Pair-event projection parity:
   - native callback lane currently shows `[77, 78]`,
   - synthetic projection trace currently shows `[77, 77]`,
   - close that discrepancy explicitly.

## Verification commands

Core host/HAL lanes:

```powershell
cargo test -p oxvba-hal -- --nocapture
cargo test -p oxvba-host --test com_client_end_to_end -- --test-threads=1 --nocapture
cargo test -p oxvba-host --test com_client_registered_lane -- --ignored --test-threads=1 --nocapture
pwsh -ExecutionPolicy Bypass -File scripts/run-com-registered-events.ps1 -ProgId OxVba.TestEventServer -EnableTrace
```

Governance and doc sync:

```powershell
./scripts/check-governance.ps1
./scripts/meta-check.ps1 -Fast -NoArtifacts
```

## Exit criteria

1. General late-bound COM invoke supports true multi-argument method/property calls through an explicit and documented contract.
2. Existing single-arg lanes remain green with no silent regression.
3. Controlled and registered fixture lanes prove at least one real two-arg late-bound call and one real pair-event callback case.
4. Synthetic event projection is either corrected or explicitly bounded/disabled for native callback-authoritative lanes.
5. Specs, implementation-defined notes, and evidence accurately describe the supported surface.

## Related documents

- `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- `docs/worksets/PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE.md`
- `docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`
- `docs/evidence/SPEC_CHECKLIST.md`
- `docs/REVIEW_20260309.md`
