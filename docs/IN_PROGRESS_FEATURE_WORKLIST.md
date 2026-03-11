# In-Progress Feature Worklist

Date: 2026-03-10  
Status: active  
Purpose: canonical repo-level register of feature areas that remain `in-progress` under the workset completion doctrine in `OPERATIONS.md`.

This file is the authoritative consolidation point for part-implemented feature work.

Latest execution pass:
1. `docs/IN_PROGRESS_FEATURE_EXECUTION_2026-03-10.md`

Use it to answer:
1. what major behavior areas are still unfinished,
2. why they are still `in-progress`,
3. which workset/spec/register owns the remaining work,
4. what must be true before the area can be described as implemented/closed.

Do not use this file for:
1. immutable historical gate records,
2. line-by-line execution logging,
3. deferred formal lane row management,
4. detailed oracle capture inventories.

Those remain in:
1. `docs/IMPLEMENTATION_LOG.md`,
2. `docs/profile-status/`,
3. `docs/evidence/formal/DEFERRED_GATES.md`,
4. `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`.

## Status vocabulary

- `in-progress`: partial implementation exists but parity for the scoped area is not complete.
- `blocked`: in-progress and currently constrained by an active blocker in `CURRENT_BLOCKERS.md`.
- `planned`: explicitly accepted area with no shipped parity slice yet.

## Active feature register

| ID | Feature area | Status | Current floor | Remaining gap to close | Canonical owners |
|---|---|---|---|---|---|
| `IP-01` | Full VBA 7.1 language/runtime parity | in-progress | large executable language/runtime subset completed through historical ladders | full VBA 7.1 parity claim is still open at program level, including residual semantic, oracle, and matrix closure work | `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-02` | VBA property model and default-member semantics | planned | partial foundations exist in compiler/runtime and COM metadata lanes | `Property Get/Let/Set`, `Set` vs `Let`, default member resolution, indexed/default property behavior, and Office-style call-vs-value parity still need end-to-end closure | `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`, `docs/worksets/WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md` |
| `IP-03` | Windows late-bound COM client (`IDispatch`) parity | in-progress | invoke transport now carries named/omitted metadata; `oxvba-com` now owns a first semantic request/callback value carrier (`ComValue`) for the recoverable subset; array/null/error intent now survives into the COM boundary; named method/property-get and member-known property-put/property-putref lanes execute; controlled dispatch now roundtrips `VT_NULL`, `VT_ERROR`, `VT_I2`, and `VT_UI2`; bounded `ArgErr`/`EXCEPINFO` fidelity is now preserved in the controlled native lane; explicit metadata-backed default-member dispatch via `DispatchInvoke(obj, 0, ...)` now resolves when authoritative identity exists; controlled/registered lanes and deterministic member resolution subset remain in place | natural/default-member syntax without authoritative identity, broader `VARIANT`/object/BSTR/real-SAFEARRAY marshalling, faithful external `ArgErr`/`ExcepInfo`/`VarResult`, and practical Office automation parity remain open | `docs/worksets/WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md`, `docs/worksets/WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`, `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md` |
| `IP-04` | `oxvba-com` architectural repurpose and HAL COM extraction | in-progress | shared COM transport/types, typelib catalog pieces, the first semantic request/callback value carrier, and an executable generic dynamic-object protocol API now live in `oxvba-com` | main COM activation/invoke/event/type-library state still lives largely in `oxvba-hal`; final ownership split, runtime-wide protocol wiring, and HAL contraction remain open | `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`, `docs/worksets/WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`, `docs/worksets/WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md`, `docs/ARCHITECTURE.md` |
| `IP-05` | Windows early-bound COM and type-library parity | in-progress | implemented rewrite-based early-binding subset with typelib identity hints, binder lowering, and executable conformance lanes | full VBA/Excel early-binding parity, richer typelib/member coverage, and final architectural migration out of transitional HAL-owned execution remain open | `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`, `docs/worksets/WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md`, `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-06` | Windows COM server/export parity | planned | COM client/event groundwork exists; scope tiering is defined (`S0..S3`) | native COM server behavior is still below parity target; class exposure, typelib publication, outward `IDispatch` parity, and host policy model remain to be implemented | `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`, `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-07` | Event runtime parity (non-COM + COM adapter lanes) | in-progress | baseline event runtime pass completed; runtime owner iteration and major non-COM/event infrastructure are executable | full `WithEvents` instance graph semantics, unified host ingress parity, and complete COM event parity lanes remain open | `docs/worksets/WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md`, `CURRENT_BLOCKERS.md` |
| `IP-08` | Host project / Office-style hosting parity | planned | host bridge and object/event ingress contracts are now locked at design level | full Host Project semantics, Office-style root/global exposure, callback-path hosting parity, and integration with final COM/property model remain open | `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`, `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-09` | Declare/native marshaling parity | in-progress | deterministic declaration subset, descriptor routing, and bounded host-backed lanes are implemented | full Automation legality matrix, pointer-string lanes, byref writeback, richer native ABI shapes, and `IDispatch::Invoke` output obligations remain open | `docs/spec/HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`, `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md` |
| `IP-10` | Oracle/differential parity closure for required behavior areas | in-progress | deferred-oracle structure and topic tracking are in place; some targeted probes have been captured | required Office/host differential captures are not yet exhausted for open parity areas, so claim closure cannot rely only on local subset evidence | `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`, `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-11` | Formal foldback for active parity claims | in-progress | formal infrastructure and many obligations exist; policy for non-blocking deferred lanes is defined | open deferred gates and failed/deferred formal lanes still require foldback or bounded resolution before full parity claims can close | `docs/evidence/formal/DEFERRED_GATES.md`, `docs/FORMAL.md` |

## Area notes

### `IP-01` Full VBA 7.1 language/runtime parity

Why still open:
1. the repo has many completed historical ladders, but the current governing claim is the full compliance program,
2. that program explicitly requires zero unresolved in-scope divergences, no open in-scope deferred gates, and a green Office differential matrix,
3. those terminal conditions are not met yet.

### `IP-02` VBA property model and default-member semantics

Why still open:
1. the full-compliance workset calls this out as its own closure domain,
2. current COM/client and host work does not yet amount to full `Set`/`Let`/default-member parity,
3. this is one of the highest-risk semantic areas for “looks implemented but is not parity-complete” confusion.

### `IP-03` Windows late-bound COM client parity

Why still open:
1. the current lane is stronger than before but remains a subset,
2. the scope doc still limits current maturity to C2 runway/subset behavior,
3. the dedicated `IDispatch` completion workset exists because the parity gap is real and specific.

### `IP-04` `oxvba-com` repurpose and HAL COM extraction

Why still open:
1. the direction is locked,
2. some transport/types and deterministic typelib logic are already extracted,
3. but the main COM runtime state/behavior still has not fully moved into `oxvba-com`.

### `IP-05` Windows early-bound COM and type-library parity

Why still open:
1. the current early-binding implementation is explicitly a constrained subset,
2. broader Office parity was intentionally left out of the earlier tranche,
3. the doctrine now requires this to remain `in-progress` until the real parity target is closed.

### `IP-06` Windows COM server/export parity

Why still open:
1. the scope doc still identifies server behavior as below parity target,
2. current COM progress has primarily been on client/event and shared transport foundations,
3. the server side remains a major unfinished domain.

### `IP-07` Event runtime parity

Why still open:
1. the event workset itself says the baseline pass is complete but residual parity work remains,
2. host-event ingress, sink-instance graph parity, and COM event tiers are still open,
3. these behaviors must converge before an events parity claim is valid.

### `IP-08` Host project / Office-style hosting parity

Why still open:
1. the bridge contracts are better defined now,
2. but the actual Office-style hosting model is not yet parity-complete,
3. this area depends on property semantics, object/value identity, and COM boundary completion.

### `IP-09` Declare/native marshaling parity

Why still open:
1. many HAL dynamic-link clauses remain `implemented-partial`,
2. the current supported subset is deliberately narrow,
3. the docs explicitly call out remaining Automation/native ABI work.

### `IP-10` Oracle/differential parity closure

Why still open:
1. this is not a feature by itself, but it is required for parity closure of multiple features,
2. several implementation-defined or deferred-oracle topics remain open,
3. without oracle foldback the repo cannot honestly claim full VBA/Excel parity in those areas.

### `IP-11` Formal foldback for active parity claims

Why still open:
1. many formal lanes are historical and folded,
2. but open deferred/failing lanes still exist in the live register,
3. the full-compliance claim model requires these to be folded or explicitly bounded for in-scope parity claims.

## Operating rules

When any feature area above changes:
1. update this file,
2. update the owning workset/spec/register,
3. keep the status as `in-progress` until the scoped parity target is actually complete,
4. only remove an entry when its scope is truly parity-complete or when the scope is explicitly retired/replaced.

## Update checklist

1. Is the area still part of the active parity target?
2. Is there still any open blocker, deferred gate, oracle gap, or unimplemented parity behavior in scope?
3. If yes, keep the entry `in-progress`.
4. If no, update the owning docs first, then remove or mark the entry complete through an explicit documented decision.
