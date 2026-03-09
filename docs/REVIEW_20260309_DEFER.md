# Review Defer Triage — 2026-03-09

Triaged from `docs/REVIEW_20260309.md` using `docs/REVIEW_20260309_TRIAGE_PLAN.md`.

Entries here are accepted in principle but should not be pulled into the immediate next batch.

## [D-01] `standard.rs` Modularization and Test Fixture Extraction

- Status: defer
- Source: `docs/REVIEW_20260309.md` sections `1`, `H9`, `TD`, `SEC`
- Additional sources: `XR`
- Summary: The review is right that `standard.rs` is too large and that the embedded controlled COM test server and raw FFI helpers should eventually be isolated into dedicated modules, with `oxvba-com` now the intended extraction target for COM-specific code rather than generic HAL-owned infrastructure.
- Why it matters: maintainability | safety
- Decision: accept, but defer until the COM HAL v2 surface settles.
- Rationale: The file is actively changing for invoke/event/lifecycle work. Large structural extraction before the contract stabilizes would increase churn and merge risk without closing a current correctness gap.
- Duplicates merged: `1`, `H9`, `TD` unsafe-block concentration, `SEC`, `XR` split-standard overlap
- Next step: keep this queued behind the active COM contract work and revisit only after the metadata/invoke/event gates are green, using the `oxvba-com` repurpose workset as the extraction plan.
- Safe to defer because: the current adapter is ugly but functional, evidence-backed, and already under active change.
- Revisit when: `v526` metadata integration gate is green, or when a second COM batch starts after invoke/event closure.

## [D-02] `ComHal` Trait Decomposition Into Activation, Dispatch, and Event Subtraits

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `H4`
- Additional sources: none
- Summary: Splitting `ComHal` into narrower traits is a plausible long-term cleanup and would make capability boundaries clearer, especially now that the intended long-term direction is to move detailed COM behavior out of HAL rather than perfecting HAL as the permanent COM contract.
- Why it matters: maintainability
- Decision: accept in principle, but defer.
- Rationale: Doing this before the COM HAL v2 invoke contract and callback/lifecycle shape are finished would lock in abstractions too early.
- Duplicates merged: none
- Next step: revisit only after the v2 contract lands and the surviving HAL bootstrap/delegation surface is known.
- Safe to defer because: the current monolithic trait works and can carry the next compliance batch.
- Revisit when: after `v506-v539` closure, during the next HAL cleanup cycle.

## [D-03] `TypeLibraryHal` Internalization or Boundary Redesign

- Status: defer
- Source: `docs/REVIEW_20260309.md` sections `H7`, `SD-3`
- Additional sources: none
- Summary: The review correctly notes that `TypeLibraryHal` is consumed as an internal COM-adapter dependency more than as an external host capability.
- Why it matters: maintainability
- Decision: accept in principle, but defer until typelib ingestion work is complete.
- Rationale: This boundary should be revisited only after the expanded metadata model is actually in place. Syncing the draft spec now is still worthwhile, but redesigning the public trait boundary now would be premature.
- Duplicates merged: `H7`, `SD-3`
- Next step: document the current state now, redesign later.
- Safe to defer because: the trait is functional and can be accurately documented as provisional.
- Revisit when: `v517-v520` and `v526` are complete.

## [D-04] Remove Per-Platform `HostServices` Wrapper Boilerplate

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `H8`
- Additional sources: none
- Summary: The wrapper types around `StandardHostServices` appear mechanically redundant and could likely be collapsed.
- Why it matters: maintainability
- Decision: accept, but defer.
- Rationale: This is a cleanup refactor with little value until cross-platform smoke coverage is in place and profile-default fixes are complete.
- Duplicates merged: none
- Next step: revisit after `P-04` lands so simplification can be validated safely.
- Safe to defer because: the wrappers are low-cost indirection, not a correctness defect.
- Revisit when: after the profile/smoke-coverage batch completes.

## [D-05] Add COM Object Identity and Capability Introspection

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `H11`
- Additional sources: none
- Summary: The review suggests adding object identity or `supports_events` style introspection so the engine can make more informed decisions.
- Why it matters: maintainability | compatibility
- Decision: accept, but defer.
- Rationale: This becomes more valuable once host-model and diagnostics work reaches the point where the engine needs explicit capability reporting. It is not needed to close the immediate COM invoke/event defects.
- Duplicates merged: none
- Next step: revisit together with host-model and diagnostics work.
- Safe to defer because: the current engine can continue using explicit operation failures as the capability signal.
- Revisit when: `v540-v544` host-model work begins.

## [D-06] Replace `pending_callbacks: Vec<i32>` With `VecDeque`

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `H10`
- Additional sources: none
- Summary: The review is correct that the current callback queue uses FIFO removal from `Vec`, which is algorithmically suboptimal.
- Why it matters: maintainability | performance
- Decision: accept, but defer until the callback API redesign lands.
- Rationale: The queue representation may change entirely when the callback polling API is collapsed into a payload-returning path.
- Duplicates merged: none
- Next step: implement only in the context of `P-03`, not as a standalone micro-change.
- Safe to defer because: current callback volumes are low and there is no evidence yet that this is a practical bottleneck.
- Revisit when: the callback poll-path refactor is active or event stress evidence shows pressure.

## [D-07] `oxvba.toml` Explicit Module Mapping

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `PR-4` suggestion 1
- Additional sources: `PR-10`
- Summary: An explicit `[[modules]]` mapping table is a sensible future extension for project/tooling work where convention is insufficient.
- Why it matters: compatibility | delivery
- Decision: accept in principle, but defer.
- Rationale: This is tied to the project/tooling implementation program, not to the current COM compliance ladder.
- Duplicates merged: `PR-4`, `PR-10`
- Next step: hold this for the P2/P3 project-format period.
- Safe to defer because: no current project/tooling implementation depends on this extension today.
- Revisit when: the `oxvba.toml` parser and module discovery work becomes active.

## [D-08] Project Reference Version Constraints in `oxvba.toml`

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `PR-4` suggestion 2
- Additional sources: none
- Summary: Reserving version constraints for project references is a reasonable future-facing schema improvement.
- Why it matters: compatibility | delivery
- Decision: accept in principle, but defer.
- Rationale: It matters once distribution/versioning scenarios are active, not during the current engine/compliance closure period.
- Duplicates merged: none
- Next step: reserve this for artifact/distribution work.
- Safe to defer because: there is no active dependency-management surface yet.
- Revisit when: add-in/artifact distribution becomes in-scope.

## [D-09] Platform-Conditional Typelib Reference Warnings in Schema Docs

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `PR-4` suggestion 3
- Additional sources: none
- Summary: The proposal should eventually document that typelib reference fields are Windows-specific and produce deterministic warnings elsewhere.
- Why it matters: compatibility | maintainability
- Decision: accept, but defer.
- Rationale: This belongs in the project/tooling specification pass, not in the immediate compliance batch.
- Duplicates merged: none
- Next step: fold this into the future `oxvba.toml` documentation work.
- Safe to defer because: the project/tooling schema is not yet implemented.
- Revisit when: the project-format workset becomes active.

## [D-10] Proposal Phase Reshaping for P5, Language Services, and C API

- Status: defer
- Source: `docs/REVIEW_20260309.md` sections `PR-5`, `PR-12`
- Additional sources: `PR-10`
- Summary: The review recommends reshaping the host/tooling execution plan by splitting P5, assigning language services, and explicitly phasing the C API.
- Why it matters: delivery
- Decision: accept, but defer as proposal maintenance rather than immediate implementation work.
- Rationale: These are good planning refinements, but the active ladder is still the compliance program to `v620`. This should be revisited when the host/tooling program becomes the active execution surface.
- Duplicates merged: `PR-5` suggestions 1, 3, 4; `PR-12`
- Next step: carry these changes into the host/tooling proposal when that program is activated.
- Safe to defer because: current execution is not blocked by these proposal edits.
- Revisit when: P2/P5/P7 planning becomes active.

## [D-11] Promote the XLL Caveat to User-Facing Documentation

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `PR-6`
- Additional sources: none
- Summary: The review is right that users will over-assume equivalence if the XLL compatibility caveat is hidden.
- Why it matters: compatibility | delivery
- Decision: accept, but defer until the feature is actually closer to shipping.
- Rationale: This is a release/docs readiness item for a later workstream, not a current code or spec blocker.
- Duplicates merged: none
- Next step: add it to the UC-B shipping checklist, not the current immediate batch.
- Safe to defer because: the feature is not implemented or user-facing yet.
- Revisit when: UC-B becomes active.

## [D-12] Keep WASM Hosting Behind the Host-Bridge Workstream

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `PR-8`
- Additional sources: none
- Summary: The review recommendation is effectively a prioritization note: do not pull WASM work ahead of host-bridge and event-closure work.
- Why it matters: delivery
- Decision: accept, but defer as sequencing guidance.
- Rationale: This is not an implementation item for the next batch. It is a reminder to preserve the planned phase order.
- Duplicates merged: none
- Next step: keep this in the proposal/workset backlog, not in the immediate execution queue.
- Safe to defer because: no WASM bridge work is active now.
- Revisit when: host-bridge work is stable and P9 is approaching.

## [D-13] Embedded Host Reload and Threading Baseline

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `PR-2` suggestions 2 and 3
- Additional sources: none
- Summary: The proposal should eventually lock a v1 reload model and thread model for the embedded host pathfinder.
- Why it matters: compatibility | delivery
- Decision: accept, but defer until the pathfinder becomes active work.
- Rationale: The likely baseline is straightforward enough to note now — full reset on reload, single-threaded/cooperative yield model first — but there is no reason to spend current execution budget on it yet.
- Duplicates merged: `PR-2` suggestions 2 and 3
- Next step: carry the baseline recommendation into the future pathfinder planning pass.
- Safe to defer because: no pathfinder implementation is underway.
- Revisit when: P5b planning starts.

## [D-14] Script Runner Guide and Runner Taxonomy Cleanup

- Status: defer
- Source: `docs/REVIEW_20260309.md` sections `CB-9`, `CI`, `XR`
- Additional sources: none
- Summary: A runner guide would help future contributors navigate the large PowerShell script surface, but it does not unblock current correctness work.
- Why it matters: maintainability | delivery
- Decision: accept, but defer.
- Rationale: Useful, but lower leverage than direct code/spec/compliance closure.
- Duplicates merged: `CB-9`, `CI`, `XR` runner-guide overlap
- Next step: schedule as a documentation cleanup when the current execution program has fewer moving targets.
- Safe to defer because: existing scripts are already usable by current operators.
- Revisit when: the current compliance batch stabilizes or onboarding pain becomes material.

## [D-15] Automated CI and Cross-Platform Gate

- Status: defer
- Source: `docs/REVIEW_20260309.md` sections `CI`, `CB-16`, `TC`
- Additional sources: none
- Summary: The absence of CI is real, and a future automated gate would improve repeatability, but standing it up now would compete with active parity closure work.
- Why it matters: delivery | maintainability
- Decision: accept, but defer.
- Rationale: The repo already has substantial manual governance and runner infrastructure. CI should come after the active lane matrix and rerun pack are more stable.
- Duplicates merged: `CI`, `CB-16`, `TC` non-Windows gap references
- Next step: treat CI as a program-level follow-on after the compliance program or near its release-prep stages.
- Safe to defer because: current runs are manual but functional.
- Revisit when: `v576-v600` release/gate automation becomes the focus.

## [D-16] IR/Optimizer Cleanup and Future Optimization Work

- Status: defer
- Source: `docs/REVIEW_20260309.md` sections `CB-3`, `CB-8`, `CB-14`, `Compiler Pipeline`, `JIT Compilation`
- Additional sources: none
- Summary: The review points out some public optimizer APIs that are unused and notes the optimization passes are intentionally minimal.
- Why it matters: maintainability | performance
- Decision: accept, but defer.
- Rationale: This is not on the critical path for current parity/compliance work. The present optimization story is adequate for the current product state.
- Duplicates merged: `CB-3`, `CB-8`, `CB-14`
- Next step: revisit only when compiler/JIT work becomes an active focus again.
- Safe to defer because: there is no correctness evidence that the current optimization surface is harming the active program.
- Revisit when: JIT/IR work is reactivated or performance becomes a gating issue.

## [D-17] COM Test Timing Rationale Comments

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `CB-7`
- Additional sources: none
- Summary: Commenting the timing sleeps in the registered COM event tests would improve readability.
- Why it matters: maintainability
- Decision: accept, but defer until the next direct touch of those tests.
- Rationale: Helpful, but not worth pulling ahead of the substantive COM interop items.
- Duplicates merged: none
- Next step: add the comments opportunistically during the next COM test-lane code edit.
- Safe to defer because: the current evidence and script names already explain the lane intent well enough.
- Revisit when: the registered-event tests are next edited.

## [D-18] Promote the Three-Plane Model to the Opening of the Hosting Proposal

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `PR-3`
- Additional sources: none
- Summary: The review recommends moving the proposal's three-plane architecture model to the front because it is the foundational framing for the host/tooling design.
- Why it matters: maintainability | delivery
- Decision: accept, but defer as proposal-edit cleanup.
- Rationale: The idea is good, but it is a document-structure improvement for the future host/tooling pass rather than an immediate implementation enabler for the active compliance ladder.
- Duplicates merged: none
- Next step: apply this when the hosting proposal next receives a substantive design-lock edit.
- Safe to defer because: the model already exists in the proposal and is not missing, only buried.
- Revisit when: the host/tooling proposal becomes the active planning surface again.

## [D-19] Project Module Reference Spec Naming and Invariant Drift

- Status: defer
- Source: `docs/REVIEW_20260309.md` section `SD-4`
- Additional sources: none
- Summary: The review identifies minor naming and invariant drift between `PROJECT_MODULE_REFERENCE_SPEC_V1.md` and the current `ProjectManifest`-centric implementation.
- Why it matters: maintainability
- Decision: accept, but defer until the project/tooling model is active again.
- Rationale: This drift is real but minor, and the surrounding project-format surface is still expected to evolve. Updating it now would create documentation churn without supporting the current immediate batch.
- Duplicates merged: none
- Next step: revisit when the project model and VBP/tooling work moves back into active execution.
- Safe to defer because: no current compliance lane depends on this terminology drift.
- Revisit when: project-format or VBP integration work resumes.

## Rejected / Not Applicable Appendix

## [R-01] `#[must_use]` on `HalResult`-Returning Trait Methods

- Status: rejected
- Source: `docs/REVIEW_20260309.md` section `7`
- Additional sources: none
- Summary: The review suggests adding `#[must_use]` to HAL trait methods returning `HalResult<T>`.
- Why it matters: maintainability
- Decision: reject.
- Rationale: `Result` is already `#[must_use]`, so the proposed change would add little or no new protection while creating noise.
- Duplicates merged: none
- Next step: none.

## [R-02] Temp-File Cleanup Backlog Entries

- Status: rejected
- Source: `docs/REVIEW_20260309.md` sections `CB-4`, `CB-5`
- Additional sources: none
- Summary: The audit claimed two temp files should be cleaned from the repo.
- Why it matters: maintainability
- Decision: reject as not applicable in the current repo state.
- Rationale: The cited files are not present now. The triage should not preserve stale cleanup ghosts as active backlog.
- Duplicates merged: none
- Next step: none; if such files reappear, remove them immediately rather than backlog them.

## [R-03] Implement `PortableComBridge` and `WindowsComBridge`

- Status: rejected
- Source: `docs/REVIEW_20260309.md` section `CB-13`
- Additional sources: `H6`
- Summary: The review suggests implementing the empty bridge stubs in `oxvba-com`.
- Why it matters: maintainability | delivery
- Decision: reject in its current form.
- Rationale: The crate has now been repurposed, but the existing placeholder stub types are still not the right implementation target. The valid follow-on is the staged extraction plan in the new `oxvba-com` workset, not fleshing out the old stub shapes.
- Duplicates merged: `CB-13`, `H6`
- Next step: follow the repurpose/extraction workset instead of implementing the current stub types.

## [R-04] Consolidate COM Conformance Runners Prematurely

- Status: rejected
- Source: `docs/REVIEW_20260309.md` section `CB-15`
- Additional sources: none
- Summary: The review suggests consolidating the lane-specific COM runner scripts.
- Why it matters: maintainability | delivery
- Decision: reject for now.
- Rationale: The current lane-specific scripts are an intentional evidence surface. Consolidation before a runner taxonomy pass would likely reduce clarity and traceability.
- Duplicates merged: none
- Next step: none unless runner duplication becomes an observed maintenance problem.

## [R-05] `HAL-E-*` Is Undefined

- Status: rejected
- Source: `docs/REVIEW_20260309.md` sections `CB-6`, `CB-11`
- Additional sources: `EH`
- Summary: Parts of the review claim the `HAL-E-*` family is undefined.
- Why it matters: maintainability
- Decision: reject that claim.
- Rationale: `HAL-E-*` is already defined in source and spec. The valid remaining work is to catalog it more clearly, which is captured in `P-07`.
- Duplicates merged: `CB-6`, `CB-11`, `EH`
- Next step: none beyond `P-07`.
