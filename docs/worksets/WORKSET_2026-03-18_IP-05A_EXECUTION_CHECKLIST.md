# WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST

## Purpose

Turn `IP-05A` into an explicit execution checklist so the COM reference-facade phase is driven by a contract instead of ad hoc early-bind slices.

`IP-05` remains `in-progress` until the imported COM metadata path is authoritative for the supported early-bound scope and the remaining transitional compiler-side assumptions are called out explicitly.

## Governing sources

Primary contract sources:
- [OPERATIONS.md](C:\Work\DnaCalc\OxVba\OPERATIONS.md)
- [MACH1000_PLAN.md](C:\Work\DnaCalc\OxVba\MACH1000_PLAN.md)
- [WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md)
- [WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md)
- [COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md)
- [COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md)
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
- [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)

Binding doctrine pulled from those sources:
- `IP-05A` is the metadata/reference-authority phase, not the whole of early-bound COM parity.
- Partial rewrite subsets remain `in-progress` until imported COM symbols behave like compiler-visible reference metadata in the supported scope.
- Compiler-side hardcoded external-member assumptions must shrink in favor of `oxvba-com` metadata, but native/internal PMR routes may remain transitional until their own phase owns that cleanup.

## Exit gate

`IP-05A` is complete only when all of the following are true:

- [ ] `oxvba-com` exposes one authoritative synthetic metadata path for the supported imported COM types and members.
- [ ] Compiler early-bound lowering consumes that metadata path for supported external member resolution instead of compiler-local hardcoded external token tables.
- [x] Supported early-bound diagnostics for unresolved qualifiers, missing/ambiguous imported members, unsupported out-of-subset members, and unsupported shapes remain deterministic after the metadata handoff.
- [ ] Binder/lowering evidence shows imported COM members are treated more like referenced-library metadata than an ad hoc side-domain in the supported scope.
- [ ] [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) no longer describes the `IP-05` current floor as only a rewrite-based subset with typelib identity hints.
- [ ] Remaining transitional hardcoded routes are isolated and documented explicitly as post-`IP-05A` work.

## Lane matrix

Each lane must be classified as exactly one of:
- `proved-exec`
- `proved-diagnostic`
- `implemented-unproved`
- `missing-semantics`
- `missing-diagnostic`
- `oracle-needed`

Each lane is identified by these axes:

1. Imported identity source
- known typelib identity
- unresolved/import-missing typelib identity

2. Consumer
- compiler external type declaration binding
- compiler early-bound member lowering
- metadata-backed late-bound reuse

3. Metadata responsibility
- type identity
- member token
- invoke kind/member kind
- default-member identity
- parameter metadata

4. Outcome
- executable rewrite/lowering
- deterministic compile-time reject
- deterministic runtime reuse

## Current proved floor

Already evidenced in the repo today:

- rewrite-based early-bound COM subset executes for the controlled `OxVba.TestDispatch` lanes
- imported external type declarations already validate known typelib qualifiers and deterministic `CreateObject` selectors in the supported subset
- runtime-string selector recovery already uses name-based synthetic typelib lookup inside `oxvba-com`
- external early-bound member-call lowering now resolves member tokens through `oxvba-com` synthetic typelib metadata for the current known typelibs instead of using the compiler’s hardcoded external member-token switch
- external `As New` lowering now resolves deterministic `CreateObject` selectors through `oxvba-com` synthetic typelib metadata for the current known imported types instead of using the compiler’s hardcoded external selector switch
- external early-bound call lowering now also enforces argument arity from synthetic typelib metadata in the supported imported-member subset instead of letting wrong-arity calls drift to runtime
- external early-bound call lowering now also consults imported invoke-kind metadata for the current supported subset, proving required-arg `PropertyGet` lowering directly while rejecting imported `PropertyPut` / `PropertyPutRef` shapes at compile time instead of letting setter-shaped members flow through the generic read-call rewrite
- external early-bound parenthesized default-member call syntax now resolves authoritative imported default-member identity from metadata for the current supported subset instead of leaving typed external receivers on the generic unresolved call path
- the remaining compiler-local token table is now explicitly isolated to native/internal PMR dynamic-object routing and is no longer part of the imported external member-lowering path
- imported member lookup now distinguishes metadata-backed missing versus ambiguous member/default-member identity, and imported default-member parenthesized call syntax no longer escapes lowering silently when authoritative metadata does not resolve a unique default member
- supported imported early-bound bindings now carry their authoritative metadata blob through the compiler binding/lowering path, so the active rewrite subset consumes bound metadata directly instead of re-resolving imported type identity by string at each member-call rewrite

## Remaining checklist by closure domain

### A. Facade authority

- [x] Start an explicit `IP-05A` checklist and exit gate.
- [x] Move external early-bound member-token lookup onto `oxvba-com` metadata for the currently supported imported types.
- [x] Move external `As New` activation-selector lookup onto `oxvba-com` metadata for the currently supported imported types.
- [ ] Replace remaining supported external lowering assumptions that still derive member shape from compiler-local hardcoded knowledge.
- [x] Isolate and document any still-transitional compiler-local token tables that are not part of the external imported-member path anymore.

### B. Binder and lowering integration

 - [x] Prove imported member lookup works through the authoritative metadata path across the supported early-bound execution subset.
- [x] Prove metadata-backed argument-arity validation on the current supported external member subset.
- [x] Fold invoke kind/member kind/default-member metadata into the binder/lowering path where the current subset still infers them indirectly.
- [x] Add direct compiler evidence for the supported metadata-driven lowering lanes beyond token lookup alone.
- [x] Keep unsupported external members/shapes on deterministic compile-time diagnostics.

### C. Reference-facade cleanup

- [x] Expand compiler-visible imported metadata so supported COM imports behave like synthetic reference-owned symbols rather than a side-channel rewrite hint.
- [ ] Narrow docs and blocker language so the remaining `IP-05` gap is the real residual parity surface, not already-landed metadata authority work.
- [ ] Define the exact handoff boundary between `IP-05A` metadata authority and `IP-05B` broader early-bound parity.
