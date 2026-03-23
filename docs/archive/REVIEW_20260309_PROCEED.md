# Review Proceed Triage — 2026-03-09

Triaged from `docs/REVIEW_20260309.md` using `docs/REVIEW_20260309_TRIAGE_PLAN.md`.

Entries are canonicalized, deduplicated, and ordered for immediate execution value.

Execution status:
- `P-01` through `P-18` completed as of 2026-03-10 through the staged COM/HAL continuation batches recorded in `docs/IMPLEMENTATION_LOG.md`.
- This file remains the canonical record of what was accepted from the review, but no `PROCEED` items remain open after the 2026-03-10 execution pass.

## [P-01] COM Invoke V2 and Multi-Argument Event Closure

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `5`, `H2`, `PR-10`, `XR`
- Additional sources: `PR-1` suggestion 3, `PR-12`
- Summary: The review correctly identifies that OxVba still exposes a single-argument late-bound COM invoke shape at the HAL boundary, while recent evidence shows the registered COM event lane is now green but still relies on legacy projection behavior in one path. The next work period should finish the COM HAL v2 invoke contract so method/property get/put/putref, true multi-argument dispatch, and event-trigger projection all use one authoritative argument model.
- Why it matters: correctness | compatibility | delivery
- Decision: proceed now as the primary COM continuation batch.
- Rationale: This is on the active compliance ladder critical path and already has a partially prepared follow-on workset. It directly affects the truth of the Windows Office COM parity claim.
- Duplicates merged: `5`, `H2`, `PR-10`, `PR-12`
- Next step: promote the existing COM continuation workset into the next active implementation batch and treat multi-arg late-bound invoke as the leading closure item.
- Proposed workset: `WORKSET_2026-03-09_COM_INTEROP_CONTINUATION_MULTIARG_LATEBOUND_AND_EVENT_PROJECTION.md`
- Ladder:
  - `v506-v510` COM invoke contract, dispatch flags, property intent, diagnostics
  - `v511-v513` argument bundle and marshaling closure
  - `v524-v526`, `v538` late-bound parity and event edge-matrix closure
- Dependencies: `DG-01`, `DG-02`, `DG-03`; recent external COM event evidence already committed
- Verification: registered COM event lanes, controlled COM multi-arg fixtures, property get/put/putref conformance lanes, explicit pair-event projection assertions

## [P-02] Typed COM Tokens and Callback Payload Normalization

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `2`, `8`, `H1`, `H3`, `PR-1`, `PR-10`
- Additional sources: `XR`
- Summary: The review surfaces two related problems: callback payloads still carry historical single-argument artifacts (`arg0`) and the COM layer still overloads raw `i32` tokens for object handles, subscriptions, callbacks, and value transport. The next work period should normalize callback payloads around a single authoritative `args` vector and introduce typed wrappers/value objects where the boundary currently relies on interchangeable integer tokens.
- Why it matters: correctness | compatibility | maintainability
- Decision: proceed now as part of the COM value-transport cleanup.
- Rationale: This aligns with the active decision gate for runtime value/object reference modeling and reduces the chance of silently mixing token classes while the COM invoke surface is being widened.
- Duplicates merged: `2`, `8`, `H1`, `H3`, `PR-1`, `PR-10`
- Next step: define the typed token set and callback payload struct, remove `arg0`, and update consumers before adding more invoke/event surface area.
- Proposed workset: `WORKSET_2026-03-09_TYPED_COM_TOKENS_AND_CALLBACK_PAYLOADS.md`
- Ladder:
  - `v473` runtime value/object transport model lock
  - `v506-v513` COM HAL v2 scaffold plus marshaling layers
  - `v538` callback signature and event-argument edge coverage
- Dependencies: `DG-02`; should be coordinated with `P-01`
- Verification: compile-time type separation at the HAL/host boundary, callback payload tests, pair-argument assertions, host-engine callback routing tests

## [P-03] COM Callback Polling and `release_object` Lifecycle Closure

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `H1`, `H3`, `H5`, `PR-1`
- Additional sources: `XR`
- Summary: The current callback-consumption path is a chatty token interrogation protocol spread across `do_events()` and multiple `ComHal` calls, and there is still no `release_object` method for deterministic COM object teardown. The next work period should replace the callback interrogation sequence with one payload-returning poll API and close the missing object-release lifecycle path.
- Why it matters: correctness | safety | delivery
- Decision: proceed now in the same COM HAL v2 phase as invoke widening.
- Rationale: Callback delivery and deterministic object teardown are both real contract gaps, not aesthetic refactors. They affect event parity, lifecycle correctness, and long-lived host sessions.
- Duplicates merged: `H1`, `H3`, `H5`, `PR-1`
- Next step: introduce a payload-returning callback poll shape, add `release_object`, and update engine orchestration/tests in one batch.
- Proposed workset: `WORKSET_2026-03-09_COM_CALLBACK_POLLING_AND_RELEASE_OBJECT.md`
- Ladder:
  - `v503-v504` object lifecycle and teardown parity
  - `v506` COM HAL v2 scaffold
  - `v536-v539` event parity integration and gate
- Dependencies: `DG-01`, `DG-02`; should be sequenced with `P-01` and `P-02`
- Verification: lifecycle teardown stress tests, callback poll-path unit/integration tests, subscription cleanup assertions, registered COM event reruns

## [P-04] Cross-Platform Default Profile Selection and Smoke Coverage

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `4`, `10`, `TC`, `CI`, `CB-16`
- Additional sources: `XR`
- Summary: The review identifies a concrete correctness issue in hardcoded `HalProfileId::Windows` defaults and a broader gap in non-Windows end-to-end smoke coverage. Even though the active claim target is Windows, wrong defaults and untested non-Windows stubs create avoidable regressions and reduce confidence in host/service profile selection.
- Why it matters: correctness | maintainability | delivery
- Decision: proceed now as an immediate enabling correction and smoke-test expansion.
- Rationale: This is bounded, easy to verify, and prevents silent profile skew while keeping the active ladder's integrated runs honest.
- Duplicates merged: `4`, `10`, `TC` non-Windows code paths, `CI` consequence, `CB-16`
- Next step: centralize default profile selection and add one basic source-execution smoke test per non-Windows profile with no COM dependencies.
- Proposed workset: `WORKSET_2026-03-09_PROFILE_DEFAULTS_AND_NONWINDOWS_SMOKE_COVERAGE.md`
- Ladder:
  - immediate enabling work for `v562` robustness and `v565-v570` integrated reruns
  - supports later host/profile work at `v540-v544`
- Dependencies: none
- Verification: unit tests for default-profile helpers, host-engine smoke tests across `Linux`, `MacOs`, `Wasm`, and `Null` profiles, existing Windows lanes remain green

## [P-05] BYTECODE Format Authority Closure

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `SD-1`, `Documentation Consistency`, `XR`
- Additional sources: `Audit Summary`
- Summary: `docs/BYTECODE_FORMAT.md` is badly stale relative to `bytecode.rs`. The review is correct that the current file cannot remain as a quasi-spec without either being expanded or explicitly superseded by code plus a narrower normative note.
- Why it matters: correctness | maintainability | delivery
- Decision: proceed now as a docs-first closure item.
- Rationale: This is a critical drift issue and the user explicitly wants the triage output to serve as the next-period basis. Leaving the bytecode document ambiguous would make later review and governance work harder.
- Duplicates merged: `SD-1`, documentation-consistency assessment, `XR` priority 2
- Next step: choose the conservative path first: either refresh the document enough to be truthful, or mark it superseded and point to the authoritative code/spec location explicitly.
- Proposed workset: `WORKSET_2026-03-09_BYTECODE_FORMAT_AUTHORITY_SYNC.md`
- Ladder:
  - `v477-v482` spec/clause closure support
  - `v563-v566` documentation and governance closure support
- Dependencies: none
- Verification: document cross-links, bytecode opcode count reconciliation, governance/doc-drift checks

## [P-06] ARCHITECTURE and HAL Spec Structural Sync

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `SD-2`, `SD-3`, `Documentation Consistency`, `CB-12`
- Additional sources: `Audit Summary`
- Summary: The structural docs are materially out of sync with the actual workspace and HAL surface. `ARCHITECTURE.md` omits `oxvba-hal`, and `HAL_SPEC_WORKING_DRAFT.md` lags the current public trait surface by excluding `TypeLibraryHal` and related support types.
- Why it matters: maintainability | delivery
- Decision: proceed now as a documentation sync batch.
- Rationale: These are low-risk docs corrections that will materially improve future design discussion quality. The HAL spec can record `TypeLibraryHal` as current-but-provisional without prejudging the later `H7` redesign question.
- Duplicates merged: `SD-2`, `SD-3`, `CB-12`
- Next step: update the architecture overview to reflect the real crate graph and revise the HAL draft so it truthfully describes the current public surface and marks unstable areas.
- Proposed workset: `WORKSET_2026-03-09_ARCHITECTURE_AND_HAL_SPEC_SYNC.md`
- Ladder:
  - `v477-v482` spec closure support
  - `v563-v566` documentation and governance closure support
- Dependencies: none
- Verification: workspace member reconciliation, trait/method count reconciliation, doc-link validation

## [P-07] Error Code Catalog and Family Register

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `CB-6`, `CB-11`, `EH`, `XR`
- Additional sources: `Audit Summary`
- Summary: The review is directionally right that the error-code families need one authoritative catalog, but it overstates the gap by claiming `HAL-E-*` is undefined. The next work period should create a single error-code register that catalogs existing `HAL-E-*`, `PMR-E-*`, `COM-E-*` surfaces and records the status of proposal-stage families such as `VBP-E-*`.
- Why it matters: maintainability | delivery
- Decision: proceed now as a docs-first normalization task.
- Rationale: This is a compact, high-value documentation improvement that also resolves contradictions inside the review itself and will help later diagnostics/governance work.
- Duplicates merged: `CB-6`, `CB-11`, `EH` assessment, `XR` priority 4
- Next step: inventory stable code families from source/specs, add `docs/ERROR_CODES.md`, and mark each family as implemented, reserved, or proposal-only.
- Proposed workset: `WORKSET_2026-03-09_ERROR_CODE_CATALOG_AND_FAMILY_REGISTER.md`
- Ladder:
  - `v483` diagnostics taxonomy sync
  - `v565-v566` governance and catalog drift closure
- Dependencies: none
- Verification: `rg`-based source inventory, cross-links from specs/worksets, zero false claims about existing families

## [P-08] COM FFI `// SAFETY:` Annotation Pass

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `SEC`, `TD`, `XR`
- Additional sources: `Audit Summary`
- Summary: All `unsafe` blocks in the codebase sit in the Windows COM FFI adapter and none currently carry `// SAFETY:` comments. The review is correct that this is the highest-priority safety auditability gap.
- Why it matters: safety | maintainability
- Decision: proceed now, starting with the highest-risk hotspots and then finishing the full surface.
- Rationale: This is actionable, bounded, and improves both human review quality and future lintability without forcing a risky architectural refactor first.
- Duplicates merged: `SEC` critical finding, `TD` unsafe-block concentration, `XR` priority 1
- Next step: annotate the raw ownership/release pairs, owner-dereference helpers, and vtable dispatch calls first, then complete the remainder of the unsafe surface.
- Proposed workset: `WORKSET_2026-03-09_COM_FFI_SAFETY_ANNOTATION_PASS.md`
- Ladder:
  - supports `v553-v556` formal/safety obligation work
  - supports `v559-v566` robustness and governance closure
- Dependencies: none
- Verification: unsafe-block review checklist, optional `clippy::undocumented_unsafe_blocks` dry-run, hotspot spot-check by file section

## [P-09] `HalError` Factory Coverage

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `TC`, `EH`, `XR`
- Additional sources: none
- Summary: The four `HalError` constructor helpers are public, stable, and currently untested. This is a small but real gap because these constructors define the project's core HAL error taxonomy.
- Why it matters: correctness | maintainability
- Decision: proceed now as a quick-win test batch.
- Rationale: The work is small, deterministic, and useful for stabilizing the error-code catalog and later governance checks.
- Duplicates merged: `TC` untested files, `EH` untested error paths, `XR` priority 6
- Next step: add focused unit tests for each constructor covering kind, message, stable code, and operation field behavior.
- Proposed workset: `WORKSET_2026-03-09_HAL_ERROR_FACTORY_TESTS.md`
- Ladder:
  - supports `v562` robustness gate
  - supports `v565-v566` diagnostics and governance closure
- Dependencies: none
- Verification: new unit tests green in `oxvba-hal`

## [P-10] Environment Variable Caching at Initialization Boundaries

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `6`, `EH`, `XR`
- Additional sources: none
- Summary: Several toggles are re-read from environment variables on every call path rather than being captured at construction time. The review is correct that this is both noisy and a source of inconsistent behavior inside a single execution session.
- Why it matters: correctness | maintainability
- Decision: proceed now as a bounded configuration cleanup.
- Rationale: This is easy to validate and reduces hidden runtime mutability in exactly the areas the review identified.
- Duplicates merged: `6`, `EH` uncached environment variable reads, `XR` env-var caching overlap
- Next step: cache these switches inside adapter/compiler configuration objects and keep the values stable for the lifetime of the constructed service.
- Proposed workset: `WORKSET_2026-03-09_ENV_VAR_CACHING_AT_INIT.md`
- Ladder:
  - supports `v559-v562` robustness and deterministic replay work
- Dependencies: none
- Verification: targeted tests for stable behavior within one service lifetime, existing env-driven paths remain functional

## [P-11] VM Execution-State Reset Consolidation

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `9`, `VM Execution`, `XR`
- Additional sources: none
- Summary: The reset/init sequence in `Vm::execute` and adjacent entry points is duplicated with slightly different behavior. The review correctly flags this as a future divergence trap.
- Why it matters: correctness | maintainability
- Decision: proceed now as a contained runtime cleanup.
- Rationale: This is not a speculative refactor; it removes inconsistent initialization behavior from a critical execution path before more host/event work piles onto it.
- Duplicates merged: `9`, VM execution concerns, `XR` priority 15
- Next step: extract one reset helper with an explicit preserve-bindings policy and update all entry points to use it.
- Proposed workset: `WORKSET_2026-03-09_VM_EXECUTION_STATE_RESET_CONSOLIDATION.md`
- Ladder:
  - supports `v497-v505` runtime semantics integration
  - supports `v559-v562` robustness work
- Dependencies: none
- Verification: existing VM tests plus targeted entry-point reset behavior tests

## [P-12] `DISPATCH_INVOKE_MISSING_ARG_TOKEN` Deduplication

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `3`, `TD`, `XR`
- Additional sources: none
- Summary: The same dispatch-missing sentinel constant is defined in multiple crates. The review is correct that this should be a single shared definition rather than duplicated magic.
- Why it matters: maintainability | correctness
- Decision: proceed now as a small shared-constant cleanup.
- Rationale: The change is low risk and sits directly inside the COM/value transport surface that is already being revisited.
- Duplicates merged: `3`, `TD` magic numbers, `XR` priority 10
- Next step: move the shared constant to a common runtime-facing location and update all users.
- Proposed workset: `WORKSET_2026-03-09_DISPATCH_MISSING_ARG_TOKEN_DEDUP.md`
- Ladder:
  - supports `v506-v513` COM invoke/marshaling work
- Dependencies: none
- Verification: compile/test sweep for compiler, runtime, HAL, and host consumers

## [P-13] Repurpose `oxvba-com` as the Bidirectional COM Bridge and Extract COM From HAL

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `H6`, `H1-H5`, `H7-H11`, `Architecture and Crate Structure`, `CB-1`, `CB-2`, `CB-13`
- Additional sources: resolved user decision on 2026-03-09
- Summary: The review correctly surfaced that COM feels misplaced inside the generic HAL surface and that `oxvba-com` no longer matches its intended role. The project decision is now to repurpose `oxvba-com` into the Windows-first bidirectional COM bridge that maps external COM into OxVba/VBA object semantics and exposes OxVba outward as COM, while `oxvba-hal` contracts back toward profile/policy/bootstrap responsibilities.
- Why it matters: maintainability | compatibility | delivery
- Decision: proceed now as a design-lock and cleanup-planning workset, with staged implementation to follow.
- Rationale: This gives the ongoing COM refactor a coherent destination instead of letting more permanent COM complexity accumulate in HAL.
- Duplicates merged: `H6`, `CB-1`, `CB-2`, `CB-13`, the broader HAL COM boundary concerns from `H1-H11`
- Next step: use the new workset as the architectural source of truth for COM extraction and HAL cleanup sequencing.
- Proposed workset: `WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- Ladder:
  - `v506-v526` COM invoke, marshaling, and metadata integration
  - `v527-v533` COM server model closure
  - `v536-v539` event parity integration
- Dependencies: should be coordinated with `P-01`, `P-02`, and `P-03`
- Verification: preserved COM conformance lanes through staged extraction, reduced `standard.rs` responsibility surface, updated architecture/spec docs

## [P-14] Host-Bridge Object Value and Event-Ingress Contract Lock

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `PR-1`, `PR-10`, `PR-11`, `PR-12`
- Additional sources: resolved user decision on 2026-03-09
- Summary: The host/tooling proposal needs one authoritative contract for object-valued bridge operations and host-to-engine event delivery. The project decision is to keep a single `Variant` value boundary, represent host object references as object-capable `Variant` values, require explicit engine-side `dispatch_host_event(subscription, args)`, and keep collection/default-member semantics in the semantic/runtime layer instead of inventing bridge special cases.
- Why it matters: compatibility | delivery
- Decision: proceed now as a docs/design lock for future P5a work.
- Rationale: This removes a major source of ambiguity from the host-bridge program without forcing immediate implementation.
- Duplicates merged: `PR-1` suggestions 2-5, `PR-10`, `PR-11`, `PR-12`
- Next step: use the new workset and proposal edits as the contract source of truth for future host-bridge implementation.
- Proposed workset: `WORKSET_2026-03-09_HOST_BRIDGE_OBJECT_VALUE_AND_EVENT_INGRESS_CONTRACT.md`
- Ladder:
  - future P5a host bridge trait + typed token work
  - future P6 event bridge integration
- Dependencies: should remain aligned with the Host Project semantic plane and the `oxvba-com` repurpose decision
- Verification: proposal/spec consistency, future host harness tests, event-ingress integration tests once implemented

## [P-15] HAL COM Surface Contraction and Bootstrap-Seam Redesign

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` section `H4`
- Additional sources: resolved `oxvba-com` repurpose decision on 2026-03-09
- Summary: With `oxvba-com` now designated as the long-term home for COM transport/state/metadata behavior, the old recommendation to split `ComHal` into narrower concerns is no longer a speculative cleanup. It is part of the required HAL contraction plan so HAL stops pretending to be the permanent COM implementation boundary.
- Why it matters: maintainability | compatibility | delivery
- Decision: proceed now as an architectural cleanup item under the COM extraction program.
- Rationale: The question is no longer whether HAL should remain COM-heavy. The current task is to define the surviving HAL bootstrap/delegation seam and stop deepening the old monolithic `ComHal` shape.
- Duplicates merged: `H4`
- Next step: define the post-extraction HAL-facing seam and identify which current `ComHal` responsibilities move fully into `oxvba-com` versus remain as narrow bootstrap hooks.
- Proposed workset: `WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- Ladder:
  - `v506-v526` COM invoke, marshaling, metadata, and integration closure
  - HAL contraction follow-through after delegated COM paths are stable
- Dependencies: `P-13`; should be sequenced after the first `oxvba-com` transport/state extraction slices
- Verification: reduced HAL COM method surface, clearer ownership split in architecture/spec docs, preserved COM client/event lanes

## [P-16] `TypeLibraryHal` Internalization Into the COM Bridge Boundary

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` sections `H7`, `SD-3`
- Additional sources: resolved `oxvba-com` repurpose decision on 2026-03-09
- Summary: The review correctly identified that type library loading/metadata is really a COM bridge concern, not a peer host capability. With `oxvba-com` now defined as the COM boundary crate, this item should move from deferred cleanup to active planning and extraction.
- Why it matters: maintainability | compatibility | delivery
- Decision: proceed now as part of the COM metadata extraction path.
- Rationale: Typelib ingestion is already on the active ladder. Keeping `TypeLibraryHal` as a generic long-term HAL surface would work against the newly locked architecture.
- Duplicates merged: `H7`, `SD-3`
- Next step: move current typelib ownership and metadata-loading planning under `oxvba-com`, then reduce the HAL draft to whatever bootstrap hooks remain necessary during transition.
- Proposed workset: `WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- Ladder:
  - `v517-v520` typelib ingestion closure
  - `v521-v526` metadata integration gate
- Dependencies: `P-13`; should be coordinated with `P-06`
- Verification: metadata/type-info ownership moved or explicitly delegated to `oxvba-com`, HAL spec updated, existing early-bind and COM metadata lanes remain green

## [P-17] Collapse Redundant Platform `HostServices` Wrappers During HAL Cleanup

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` section `H8`
- Additional sources: `P-04`, `P-13`
- Summary: The per-platform `HostServices` wrapper types are increasingly hard to justify now that HAL is being simplified and COM is being extracted. They should be reconsidered as active cleanup work, not indefinite deferred polish.
- Why it matters: maintainability | delivery
- Decision: proceed now, but sequence after profile default fixes and the first HAL contraction pass.
- Rationale: This is still lower priority than the core COM extraction, but it is now clearly part of the same cleanup story rather than a disconnected later refactor.
- Duplicates merged: `H8`
- Next step: reassess the wrappers once the surviving HAL seam is clearer, then either collapse them into `StandardHostServices` or document the specific value they still add.
- Proposed workset: `WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- Ladder:
  - follows `P-04` profile-default/smoke coverage work
  - supports later HAL simplification after `v506-v526`
- Dependencies: `P-04`, `P-13`
- Verification: equivalent profile behavior after wrapper removal or simplification, cross-platform smoke coverage still green

## [P-18] Engine-Facing COM Object Capability and Identity Visibility

- Status: completed (2026-03-10)
- Source: `docs/REVIEW_20260309.md` section `H11`
- Additional sources: resolved host/object bridge decisions on 2026-03-09
- Summary: Once COM is being treated as an explicit bridge domain rather than hidden HAL internals, the engine/runtime will benefit from an explicit way to observe object identity/capabilities during the migration. The review suggestion to expose event-support or object-identity signals is now materially useful, not just a later convenience.
- Why it matters: maintainability | compatibility | delivery
- Decision: proceed now as a scoped bridge-surface enhancement tied to the extraction program.
- Rationale: This helps the migration away from opaque adapter internals and aligns with the broader move toward explicit object/value transport and bridge semantics.
- Duplicates merged: `H11`
- Next step: define the minimal capability/identity surface the engine needs during and after COM extraction, and avoid overexposing adapter internals beyond that.
- Proposed workset: `WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- Ladder:
  - supports `v524-v526` late-bind integration and metadata closure
  - supports later host-model integration where object capability checks matter
- Dependencies: `P-13`, `P-14`
- Verification: targeted engine/bridge tests for capability detection and identity reporting, no duplication of semantic ownership across host and COM bridges
