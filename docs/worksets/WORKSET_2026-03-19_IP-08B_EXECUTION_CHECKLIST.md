# WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST

Purpose: turn the completed `IP-08A` host foundation into an explicit `IP-08B` parity-breadth checklist for the scoped Office-style hosting surface.

## Scope discipline

- `IP-08A` is already the host-foundation floor. This checklist must not relitigate whether the repo has a working host substrate.
- `IP-08B` owns parity breadth on top of that substrate:
  - richer root/global/project behavior,
  - broader imported member/property/default-member breadth on host-returned COM objects,
  - final host integration with the completed property/event/COM model.
- Keep the workset completion doctrine from `OPERATIONS.md` section `3.1` active: if a scoped parity area is not complete, status remains `in-progress`.

## Exit gate

`IP-08B` is complete only when all of the following are true:

- [ ] The supported host root/global/project behavior matrix is explicit across assignment intent, invoke shape, and precedence rules for the scoped hosting target.
- [ ] The supported host-returned COM-object matrix is explicit across the intended imported member/property/default-member breadth for the scoped hosting target.
- [x] Host callback / event behavior no longer carries `IP-08`-owned parity gaps that belong above the completed `IP-08A` substrate.
- [x] `CURRENT_BLOCKERS.md` and `IN_PROGRESS_FEATURE_WORKLIST.md` describe only the truly remaining host parity breadth, not missing host foundation semantics.

## Lane matrix

Classify each lane as exactly one of:

- `proved-exec`
- `proved-diagnostic`
- `implemented-unproved`
- `missing-semantics`
- `missing-diagnostic`
- `oracle-needed`

Axes:

1. Receiver family
- host-injected root
- active-project root / same-name local neighbor
- plain referenced-project neighbor
- host-returned native object
- host-returned COM-backed object

2. Exposure / identity mode
- `VB_PredeclaredId`
- `VB_GlobalNamespace`
- no implicit exposure
- same-name local project neighbor
- same-name plain referenced-project neighbor

3. Syntax / intent
- explicit `Set`
- explicit `Let`
- implicit assignment
- explicit `Call`
- bare statement-context
- parenthesized zero-arg
- indexed
- named-argument

4. Result / traffic kind
- scalar getter
- object-valued getter
- scalar setter
- object setter
- imported method/default-member invoke
- event/callback behavior

## Immediate frontier

The next bounded executable neighbors after `IP-08A` are:

- [x] widen host-root object-valued getter assignment-intent evidence beyond typed `Object` targets into the `Variant` matrix
- [x] widen host-root object-valued getter syntax breadth into parenthesized zero-arg named-property getter `Variant` lanes
- [x] widen host-root authoritative default-member object-get assignment-intent evidence into the `Variant` matrix
- [x] widen host-root object-valued getter syntax breadth through the parenthesized authoritative default-member `Variant` neighbor
- [ ] widen host-returned COM imported breadth beyond the currently proved bounded member/property/default-member subset where parity requires it
- [x] capture the remaining host callback / event breadth that still belongs to `IP-08` rather than `IP-07`

Current event/callback boundary:

- the host-backed callback floor now has direct compiler and host evidence for zero/one-argument ingress on referenced `HostInjected` event sources across both `VB_PredeclaredId` and `VB_GlobalNamespace`,
- the same floor now also proves source-instance routing, same-name plain-project precedence on one-argument routes, and deterministic rejection of higher-arity forwarded host ingress on live host-backed source handles,
- remaining event-runtime residuals stay under `IP-07` (`DIV-0004`, `ODG-038`, `ODG-039`, and the remaining COM event parity lanes), not under `IP-08`.

Current host-returned COM breadth boundary:

- same-name plain-project precedence is now explicit for the currently proved imported scalar read-assignment, named-argument read-assignment, positional read-assignment, and compile-time diagnostic lanes on host-returned COM-backed objects,
- active-project same-name `Application` precedence is now also explicit for the matching imported scalar, named-argument, and positional read-assignment lanes, the imported property-put/get, property-putref, indexed-setter, exception-invoke, and non-parenthesized object-property-get lanes, the current positional/default-member explicit-`Call` and bare statement-context subsets across both parenthesized and no-paren forms, and the current named-argument explicit-`Call` and bare statement-context subsets across both parenthesized and no-paren forms, on host-returned COM-backed objects,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_ACCESSOR_DESCRIPTOR_SELECTION_2026-07-02.md`) makes imported and host-injected COM accessor descriptor selection order-independent: read/default-member lookups no longer carry write-only descriptors when a put row precedes a get row, write/default-member assignment asks for the required `PropertyLet`/`PropertySet` descriptor, and statically known COM/reference object types no longer fall back to late binding when the requested member/accessor is absent,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_NAMED_ARGUMENT_BINDING_2026-07-02.md`) makes early-bound imported and host-injected COM named arguments descriptor-bound: known COM receivers validate parameter names at bind time, reorder supplied names into typelib parameter order, preserve omitted optional gaps, and keep dynamic `CoreArg::Named` only for true `Object`/`Variant` late binding,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_RETURN_CHAINING_2026-07-02.md`) makes named interface-pointer COM returns feed the next descriptor-backed lookup: imported and host-injected `Application`-style object-return chains stay early-bound for the returned object, while generic COM `Object` returns remain dynamic late-bound,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_INTERFACE_RETURN_PROVIDER_EXPANSION_2026-07-02.md`) follows those named interface-pointer returns through the same typelib/provider request, so `Application.Workbooks.Count` no longer needs a separate fake `Workbooks` reference to stay early-bound,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_LIBRARY_MEMBER_SCOPING_2026-07-02.md`) stops library-wide typelib blobs from leaking one coclass/interface's members onto another: full-library providers keep coclass names known for activation/type-error behavior, while source-used and returned interfaces get scoped providers for member/default-member lookup,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_EVENT_SOURCE_SCOPING_2026-07-02.md`) stops COM event-source lookup from widening an empty coclass-filtered event set back to the full library event list: `WithEvents` route construction and direct COM event-member lookup no longer synthesize events from a different coclass in the same library,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_DEFAULT_MEMBER_EXPRESSION_CONTEXT_2026-07-02.md`) makes typed imported and host-injected COM objects consume descriptor-backed default members in ordinary binary value expressions such as arithmetic and concatenation, while `Is` remains object-identity based,
- 2026-07-06 continuation evidence (`docs/evidence/frontend_rework/COM_PORTABLE_HOST_RETURNED_OBJECT_CHAIN_2026-07-06.md`) proves the runtime side of the typed host-injected object-return chain on the portable projection: `Application.Workbooks.Count` and `Application.Workbooks` now execute through a retained host-returned `Excel.Workbooks` object, including its descriptor-backed default member, without requiring native COM on Linux,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_BYREF_METHOD_WRITEBACK_2026-07-02.md`) turns M12 from a documented TestEventServer gap into an executable live-COM matrix row: `Increment(ByRef value As Long)` is exposed through the fixture typelib, typed and late COM call sites preserve unparenthesized l-values as runtime `ByRef` arguments, and both the vtable and member-metadata-backed `IDispatch` paths write back the changed Long value,
- 2026-07-02 continuation evidence (`docs/evidence/frontend_rework/COM_PARAMARRAY_METHOD_2026-07-02.md`) turns M13 from a documented TestEventServer gap into an executable live-COM matrix row: `SumParamArray(params object[] nums)` exposes the Automation `FUNCDESC::cParamsOpt = -1` shape, early-bound COM binding boxes the positional tail into a zero-based SAFEARRAY(VARIANT), and metadata-known late-bound dispatch packages the same tail before `IDispatch::Invoke`,
- 2026-07-02 blocker evidence (`docs/evidence/frontend_rework/COM_BYREF_EVENT_WRITEBACK_BLOCKER_2026-07-02.md`) keeps V11 in progress rather than closing a fake fixture-only path: COM ByRef event writeback needs the VBA handler to mutate `VT_BYREF` event arguments before native `IDispatch::Invoke` returns, while the current event bridge queues value snapshots and drains them later via `DoEvents`,
- 2026-07-06 truth reconciliation (`bd-aprs.8.8.10`) updates `CURRENT_BLOCKERS.md` and `IN_PROGRESS_FEATURE_WORKLIST.md` so the current status no longer repeats the superseded March closure claim: `IP-08A` remains closed, the bounded `IP-08B` precedence/projection evidence remains valid, and the remaining host parity breadth stays `in-progress`,
- the remaining `IP-08B` COM breadth is therefore narrower than the original frontier: richer imported member/property/default-member rows may still remain, and live COM event writeback still has the V11 blocker, but these newer read/diagnostic/runtime projection neighbors are no longer only implied by earlier precedence evidence.
