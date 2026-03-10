# In-Progress Feature Execution Ledger

Date: 2026-03-10  
Run context: execution pass over `docs/IN_PROGRESS_FEATURE_WORKLIST.md`

Purpose:
1. record the execution outcome for each active in-progress feature area,
2. capture work completed in this pass,
3. capture exact blockers and the next required action for every area not closed.

Status vocabulary:
- `completed`: parity-complete for the scoped feature area
- `blocked`: cannot be completed in the current run without an explicit blocker being removed
- `in-progress`: currently being worked in this run; must end as `completed` or `blocked`

## Processing order

1. `IP-03` Windows late-bound COM client parity
2. `IP-04` `oxvba-com` architectural repurpose and HAL COM extraction
3. `IP-05` Windows early-bound COM and type-library parity
4. `IP-06` Windows COM server/export parity
5. `IP-07` Event runtime parity
6. `IP-09` Declare/native marshaling parity
7. `IP-02` VBA property model and default-member semantics
8. `IP-08` Host project / Office-style hosting parity
9. `IP-10` Oracle/differential parity closure
10. `IP-11` Formal foldback for active parity claims
11. `IP-01` Full VBA 7.1 language/runtime parity

## Execution records

### `IP-03` Windows late-bound COM client parity

- Status: blocked
- Owning docs:
  - `docs/worksets/WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md`
  - `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`
- Progress in this run:
  - corrected one active subset-misrepresentation:
    - late-bound default-member calls with named arguments are now rejected at compile time instead of silently erasing argument names before host invoke
    - files changed:
      - `crates/oxvba-compiler/src/typecheck.rs`
      - `crates/oxvba-compiler/src/lib.rs`
  - verification:
    - `cargo test -p oxvba-compiler late_bound_named_argument_call_is_rejected_until_transport_supports_names --quiet` -> PASS
    - `cargo test -p oxvba-compiler compile_named_args_call_is_supported --quiet` -> PASS
    - `cargo test -p oxvba-compiler --quiet` -> PASS
- Blocker:
  - the current transport cannot preserve named-argument or omission metadata:
    - `ComInvokeRequest` only carries `object`, `member`, `args`, and optional invoke-kind hint
    - bytecode `IntrinsicDispatchInvokeHost` only carries positional arg slots
    - VM host invoke construction only marshals a positional vector
    - native invoke builder only supports the special `DISPID_PROPERTYPUT` named-arg case, not general `rgdispidNamedArgs`
  - object/interface-pointer and broad `VARIANT`/`SAFEARRAY` marshalling are also still below parity target.
- Next required action:
  - widen the invoke transport in `oxvba-com`/bytecode/VM to carry named-arg and omission metadata, then implement full `DISPPARAMS` packing and richer result/error-channel fidelity.

### `IP-04` `oxvba-com` architectural repurpose and HAL COM extraction

- Status: blocked
- Owning docs:
  - `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- Progress in this run:
  - audited current state against the extraction workset and current code ownership.
- Blocker:
  - final extraction is blocked on unresolved behavior closure in:
    - `IP-03` late-bound invoke semantics,
    - `IP-05` early-binding parity,
    - `IP-06` COM server/export parity,
    - `IP-07` COM event parity residuals.
  - the remaining COM runtime still lives materially in `crates/oxvba-hal/src/adapters/standard.rs`, but moving it now without the final invoke/property/server contracts would lock in unstable boundaries again.
- Next required action:
  - complete the late-bound invoke transport redesign first, then extract the stabilized client/event/server slices into `oxvba-com` in that order.

### `IP-05` Windows early-bound COM and type-library parity

- Status: blocked
- Owning docs:
  - `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`
  - `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- Progress in this run:
  - audited the current early-binding scope against the parity doctrine and worklist owner docs.
- Blocker:
  - current early binding still rides a constrained rewrite-based subset over late-bound transport.
  - full closure is blocked by:
    - `IP-03` late-bound invoke/marshalling completion,
    - `IP-04` final COM ownership extraction,
    - unresolved broader typelib/member/default-member coverage and Office oracle closure.
- Next required action:
  - close `IP-03`, then expand early-bound binder/runtime/member coverage from the current subset to full typelib-driven parity.

### `IP-06` Windows COM server/export parity

- Status: blocked
- Owning docs:
  - `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`
  - `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- Progress in this run:
  - audited current scope tiering and implementation floor.
- Blocker:
  - server scope remains below parity target (`S0` current floor in scope docs).
  - completion is blocked by missing:
    - outward COM object exposure model,
    - class/type info publication model,
    - server-side invoke/property/error parity,
    - stable long-term `oxvba-com` ownership target (`IP-04`).
- Next required action:
  - finish `IP-04` boundary extraction and `IP-02` property/default-member semantics, then implement server/export in `oxvba-com`.

### `IP-07` Event runtime parity

- Status: blocked
- Owning docs:
  - `docs/worksets/WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md`
  - `CURRENT_BLOCKERS.md`
- Progress in this run:
  - audited current event workset and blocker state.
- Blocker:
  - non-COM event baseline is stronger, but parity remains blocked by:
    - open sink-instance graph/runtime reassignment parity (`DIV-0004` / `ODG-039`),
    - advanced multi-interface oracle edge matrix (`ODG-038`),
    - remaining COM event adapter parity lanes,
    - final host-event ingress closure (`IP-08`).
- Next required action:
  - close host ingress and COM event transport residuals, then rerun event parity closure against the open divergence/oracle set.

### `IP-09` Declare/native marshaling parity

- Status: blocked
- Owning docs:
  - `docs/spec/HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`
  - `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md`
- Progress in this run:
  - audited clause owners and `implemented-partial` rows.
- Blocker:
  - parity is blocked by the still-open Automation/native ABI matrix:
    - `HAL-DYN-005..007`
    - `HAL-DYN-008`
    - `HAL-DYN-009..020`
  - `HAL-DYN-008` is directly tied to `IP-03` late-bound COM invoke output semantics.
  - broader pointer-string/byref/native ABI lanes still require explicit legality and oracle-backed behavior closure.
- Next required action:
  - finish `IP-03` invoke output semantics, then execute the remaining `HAL-DYN-*` matrix and lane closure work from the current conformance plan.

### `IP-02` VBA property model and default-member semantics

- Status: blocked
- Owning docs:
  - `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- Progress in this run:
  - audited current owner docs and dependency graph.
- Blocker:
  - completion is blocked by missing end-to-end intent transport for:
    - `Property Get/Let/Set`
    - `Set` vs `Let`
    - default-member resolution source of truth
    - call-vs-value context parity
  - these depend directly on:
    - `IP-03` late-bound invoke completion,
    - `IP-04` final COM boundary placement,
    - `IP-08` host/object bridge semantics.
- Next required action:
  - define and implement the runtime/binder/property-intent model only after the COM invoke and object-value boundary are stable.

### `IP-08` Host project / Office-style hosting parity

- Status: blocked
- Owning docs:
  - `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`
  - `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- Progress in this run:
  - audited current bridge/hosting proposal and locked contract docs.
- Blocker:
  - design-level bridge decisions are in place, but closure is blocked by unfinished core dependencies:
    - `IP-02` property/default-member semantics,
    - `IP-04` final COM boundary extraction,
    - `IP-07` unified event ingress/runtime parity.
  - full Office-style hosting also needs host-object model and callback-path behavior that is not yet implemented.
- Next required action:
  - finish property/object/event semantics in-core, then implement and evidence the Office-style hosting layer against the locked bridge contract.

### `IP-10` Oracle/differential parity closure

- Status: blocked
- Owning docs:
  - `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
  - `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- Progress in this run:
  - audited current owner registers and acceptance role.
- Blocker:
  - completion is blocked on unresolved feature behavior in:
    - `IP-03`
    - `IP-05`
    - `IP-07`
    - `IP-09`
  - external Office/host oracle captures are still required for open implementation-defined and ambiguity topics; those captures cannot close meaningfully while the underlying behavior remains unfinished.
- Next required action:
  - finish the underlying parity-critical features first, then execute the remaining deferred-oracle matrix and fold outcomes back into the claim docs.

### `IP-11` Formal foldback for active parity claims

- Status: blocked
- Owning docs:
  - `docs/evidence/formal/DEFERRED_GATES.md`
  - `docs/FORMAL.md`
- Progress in this run:
  - audited the live deferred formal register for still-open rows.
- Blocker:
  - open/failing/deferred formal lanes remain in `docs/evidence/formal/DEFERRED_GATES.md`.
  - closure is blocked by:
    - remote Linux/Kani execution dependency,
    - unresolved underlying feature behavior in multiple active areas,
    - pending foldback for rows that cannot be closed honestly until the feature work is complete.
- Next required action:
  - complete the associated feature work, then run/fold the remaining remote formal lanes and close the still-open DG rows relevant to active parity claims.

### `IP-01` Full VBA 7.1 language/runtime parity

- Status: blocked
- Owning docs:
  - `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- Progress in this run:
  - created this execution ledger and audited all current in-progress feature areas against the new worklist and doctrine.
- Blocker:
  - this umbrella program is blocked by the unresolved subordinate feature areas:
    - `IP-02`
    - `IP-03`
    - `IP-04`
    - `IP-05`
    - `IP-06`
    - `IP-07`
    - `IP-08`
    - `IP-09`
    - `IP-10`
    - `IP-11`
- Next required action:
  - work those subordinate areas to closure in dependency order; do not lift the umbrella parity claim until their blockers are resolved and the required matrix/oracle/formal gates are green.
