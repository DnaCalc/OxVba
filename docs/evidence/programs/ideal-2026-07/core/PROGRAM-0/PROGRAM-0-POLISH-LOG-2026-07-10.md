# PROGRAM-0 Graph Polish and Capacity Log

Date: 2026-07-10  
Program: `ideal-2026-07`  
Control epic: `bd-59co.1`

## Outcome

Seven directed-state review passes reached a provisional steady state. Passes 6 and 7 were independently clean, but the first writable AutoRun run then exposed a stricter clause-owner/matrix witness defect. Pass 8 repaired premature completion wording. Pass 9 then found that the first repair still conflated accountable producers with consumers, permitted one-way leaf trace coverage, and encoded hidden certification dependencies in several matrix routes. The semantic routing model, graph and validators were repaired, and the final independent semantic and documentation rereads were both clean. No planned capability is credited as implemented by this control work.

## Pass record

| Pass | Review shape | Result | Material findings and disposition |
|---|---|---|---|
| 1 | Sol architecture and feature coverage | Material | Reconciled the active target to Windows x64/64-bit Excel only, separated the three profile closure surfaces, and made missing system-contract outcomes explicit. |
| 2 | Terra dependency and bead sizing | Material | Reworked broad milestone work into 42 executable epics with one-day leaf policy, explicit producer/consumer dependencies, legacy dispositions, and rollout acceptance. |
| 3 | Luna tests, evidence, and acceptance | Material | Seeded 15 canonical matrices, exact evidence environments and observable axes, negative validator coverage, and leaf-specific commands and artifacts. |
| 4 | Sol cross-workset interfaces and hidden risk | Material | Removed Windows-to-Core closure leakage; repaired CORE-4/CORE-5 producer inversion; made the compiler, sealed-artifact, helper-catalog, session, interop-plan, and LS/LSP boundaries exact; split incorrectly combined LS reference tranches. |
| 5 | Terra concurrency and ready-queue optimization | Material | Repaired terminal ordering and coarse dependency edges, added precise slice-gate requirements, enforced Rust-writer and serialized-resource limits, and made `br ready` the sole claim source. |
| 6 | Independent fresh-context GPT-5.5 review | Clean in Directed mode | Confirmed x64 scope, independent profile ownership, exact contracts, then-current 42/15/187/75/209 totals, fail-closed LSP, legacy dispositions, zero cycles, and a current-only queue. Read-only SQLite limitations were cross-checked against JSONL. |
| 7 | Independent fresh-context high-tier review | Clean in Directed mode | Reconfirmed the Pass-6 invariants plus closure taxonomy, resource guards, direct/LSP equivalence, environment separation, and the then-current 13 negative cases. Conservative whole-epic rollout blockers were judged non-material because precise child slicing is mandatory before delivery. |
| 8 | Independent post-repair GPT-5.5 review | Material control-state repair | The 188-row/210-trace repair, x64 scope, exact contracts, typelib producer and semantic clause routes were clean. The review correctly found that volatile state/evidence described PROGRAM-0 as completed while `.1.4` and `.1` were still active; state was changed to finalizing PROGRAM-0.4. Its read-only concern that other rollout leaves might be ready was resolved by the writable certified command: ancestor epic blockers are honored and `br ready -l ideal-2026-07 -t task` returns only `bd-59co.2.1.1`. |
| 9A/9B | Independent control audit plus Core/Windows/IDE semantic audits | Material semantic-model repair | Control state and the actual ready queue were clean, but clause routes overclaimed matrix rows, 20 leaves had bead clauses absent from their trace unions, and consumer evidence was recorded as producer ownership. Exact row clauses were rebuilt; accountable `owner_epics` were separated from `consumer_epics`; producer traces now cover full rows while focused evidence/projection traces may be selective; reverse leaf parity, declared parent/matrix routes and exhaustive witnesses are enforced. A final cross-profile audit then routed common session ownership through WIN-2 and VM3/dual-runtime certification through WIN-14. |
| 9C/9D | Independent control and semantic-route rereview | Material semantic dependency repair, then clean final certification | The control rereview was clean. The semantic rereview found hidden CORE-9/CORE-10 and CORE-LIB/CORE-9 certification cycles, an incomplete WIN-2 shared-plan boundary, missing portable `.oxi` tooling ownership, overbroad `WIN-META-001` carriers, and conflated clean-room/clean-VM authority. The repair added a pre-terminal portable Core certification row, a dedicated early library-oracle leaf and distributable OxImage row; made WIN-2 the exact shared-plan owner/adapter; narrowed carrier metadata and authority routes; and added COM metadata to LS reference coverage. Independent current-tree semantic and documentation rereads then found no material defect. |

The requested `gpt-5.6-sol` CLI invocation for Pass 7 was attempted, but the installed Codex client rejected that model as requiring a newer client. The pass was therefore reassigned to an isolated fresh-context high-tier reviewer. This changed reviewer transport, not acceptance scope or independence.

## AutoRun transition repair

Switching the control file to AutoRun activated full owner-and-matrix coverage checks and found 153 missing clause witnesses. The ledger was sound except for one obsolete `PROFILE-WIN-001 -> EXCEL-ORACLE` route; that Core-only matrix cannot certify Windows delivery. Exact support/consumer traces and their bead contracts were augmented instead of weakening the rule. The repair also exposed a real missing producer surface: WIN-1 promised raw typelib resolution and stable metadata identities but had no canonical producer row. `WAC-TYPELIB-METADATA` and its exact WIN-1 trace now cover registered/file resolution, GUID/version/LCID/platform/reference order, stable identities, broken-reference facts, resolver digest and compiler/package/language-service handoff without claiming COM execution.

The producer/consumer/matrix rule now applies in Directed and AutoRun modes. The clause ledger distinguishes accountable owners from non-owning consumers, each trace must use a declared profile, parent and matrix, and every declared route has a witness. Producer traces cover the complete target row; focused evidence/projection traces remain exact to what they prove. Reverse leaf parity prevents a bead contract from carrying untraced clauses, and traces may attach only to execution leaves. The 19 negative cases include independent owner-witness, matrix-witness, undeclared-consumer, undeclared-matrix, reverse-parity and non-leaf failures. The post-repair totals are 189 rows, 224 traces and 19 negative cases.

## Accepted graph

- 1 umbrella, 1 control epic, 3 profile roots, and 42 execution epics;
- 42 rollout leaves and 76 current execution leaves in total;
- 15 canonical matrices with 189 required rows: Core 55, Windows x64 57, IDE 77;
- 224 exact trace relationships covering every required row and every execution leaf;
- 60 contract-clause dispositions: 57 in scope and 3 explicitly deferred extended features;
- 42 legacy dispositions: 5 imported, 24 superseded/split, 6 already satisfied, 5 tracker tombstones, and 2 `PROFILE-EXT` deferrals;
- zero dependency cycles;
- sole certified claim command: `br ready -l ideal-2026-07 -t task`;
- initial post-control ready leaf: `bd-59co.2.1.1` (`CORE-0` rollout).

The ready result is taken from the writable bead database, not reconstructed from a leaf's direct JSONL edges alone. Rollout leaves under blocked parent epics are not ready even when their own only direct relationship is `parent-child`.

## Resource admission

Every executable leaf has a resource label. At most two `resource-rust-writer` leaves may run together. Workspace-wide Cargo, Excel/VBE automation, registry mutation, certification-VM provisioning, and the aggregate large JIT/VM3/differential/rt-abi writer lane are serialized. A large-module writer must also be a Rust writer. `bv` is used only for topology and capacity; it is never a work-claim source.

## Capacity snapshot

The pre-delivery `bv` snapshot reported zero cycles, maximum structural parallelism of 58, and an advanced critical path of eight leaf/epic steps:

`bd-59co.2.1 -> bd-59co.2.2 -> bd-59co.2.3 -> bd-59co.2.4 -> bd-59co.3.10 -> bd-9sed.17 -> bd-59co.3.11.2 -> bd-59co.3.11.4`

The final pre-delivery three-agent capacity view (`2026-07-11T01:19:44Z`, graph hash `16562c4cc446269c`) reported 124 open nodes, 94,442 estimated minutes, 11,805 serial minutes, 82,637 parallel minutes, and 87.50% structurally parallelizable work. Its 81.98-day estimate is not a calendar commitment: rollout children are not yet fully materialized, and `bv` classifies some non-claimable epics as actionable. The full 12-node structural critical path is `bd-59co -> bd-59co.2 -> bd-59co.2.1 -> bd-59co.2.2 -> bd-59co.2.3 -> bd-59co.2.4 -> bd-59co.3.10 -> bd-59co.3.11 -> bd-59co.3.11.1 -> bd-9sed.17 -> bd-59co.3.11.2 -> bd-59co.3.11.4`. Capacity and critical path must be refreshed at every epic boundary.

## Residuals

No PROGRAM-0 blocker remains. The existing `projection_member_token_by_name` dead-code warning is retained as baseline delivery work under CORE-1. Certification environments and capability rows remain truthfully `planned` with current owners.
