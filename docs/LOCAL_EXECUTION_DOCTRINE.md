# Local Execution Doctrine

Status: `active`  
Date: 2026-03-02

## Purpose

Capture execution-process rules that reduce avoidable operator errors during high-volume profile ladder runs.

This doctrine complements:
- `CHARTER.md`
- `OPERATIONS.md`
- `MACH1000_PLAN.md`

## Lessons Applied

## 0) Worksets execute through beads

Active worksets must be decomposed into bead subtrees before substantial execution.

Method references:
- `docs/methods/beads/BEADS_WORKING_METHOD.md`
- `docs/methods/beads/BEADS_UTILITIES_CHEAT_SHEET.md`
- `docs/methods/beads/BEADS_BREAKDOWN_EXAMPLE.md`

Operational rules:
- worksets remain the milestone/umbrella planning unit,
- epics are the major execution lanes within a workset,
- beads are the near-atomic execution unit,
- do not treat broad workset prose as sufficient execution tracking,
- work from ready beads by default,
- if a bead reveals required missing work, create a new bead for it before closing the current bead.

Required hierarchy:
- active execution should normally read as `workset -> epic -> bead`,
- the first execution epic may be a workset-initiation epic if the epic set still has to be rolled out,
- the first bead of an epic may be a bead-creation / rollout bead when that epic still needs its executable child set defined.
- use `docs/templates/WORKSET_EPIC_BEAD_ROLLOUT_TEMPLATE.md` when creating a new workset rollout from scratch.

Completion discipline:
- a bead is only complete when its stated outcome exists and its completion evidence has been verified,
- a workset is not complete while required beads remain open,
- narrative progress text is never a substitute for bead state.
- a workset is not considered properly rolled out until its necessary epics exist explicitly, even if some later child beads are still to be created by epic rollout beads.
- do not mutate the bead graph concurrently; serialize `br` mutations through `scripts/invoke-br-serialized.ps1`.

## 1) Scaffold determinism is a gate, not a convenience

Generated profile artifacts (`workset`/`profile-status`/`integrated_gate`) must follow strict naming and multiline structure.

Failure mode observed:
- malformed names (`WORKSET_...__V...`) and collapsed one-line files.

Policy:
- generated docs are not accepted until scaffold validation is green.

## 2) Profile/policy/runtime-class are distinct axes

Do not overload one field to carry all host behavior choices.

Keep separate:
- runtime profile identity,
- runtime class,
- policy preset and overrides.

This is required for deterministic reproducibility and future host-runner configuration.

## 3) Spec drift checks must run alongside conformance checks

When host-sensitive mapping changes (compiler/VM/host gates), spec docs must be updated in the same cycle.

Minimum expectation:
- update HAL spec docs,
- update uncertainty/implementation-defined registers if behavior boundary moved,
- keep conformance plan mapping current.

Additional rule:
- if a behavior claim is only true for a subset, the updated docs must name that subset explicitly.

## 4) Non-GUI behavior is first-class, not fallback

Headless/console UI behavior must be explicit and deterministic.

For Linux and headless profiles:
- no hidden GUI dependencies,
- policy + virtualization path must be specified and testable.

## 5) Runner bootstrap is a formal contract boundary

Policy/profile selection at process startup (CLI/env/config precedence) must be deterministic, validated, and auditable.

Until fully implemented:
- API-driven configuration remains valid,
- external bootstrap remains tracked as a formal uncertainty/work item.

## 6) COM coverage requires split lanes by design

Keep two independent COM client lanes:
- registrationless controlled lane (deterministic floor, always required),
- registered external lane (real host-registration behavior, opt-in).

Do not collapse them into one lane. They catch different failure classes.

## 7) Registered COM lanes must be explicit and serialized

Registered external COM tests are ignored-by-default and must be run intentionally through scripts.

Operational requirements:
- run with `--ignored`,
- force `--test-threads=1`,
- capture structured evidence (`csv`/`md`/logs under `docs/evidence/conformance/com/`),
- keep COM activation test configuration on explicit string ProgIDs and registered-lane env knobs; do not introduce selector-mapping APIs or numeric `CreateObject(...)` shims.

## 8) Deferred formal lanes need explicit anti-drift reconciliation

Remote Kani is asynchronous and long-running; DG metadata must be reconciled regularly so local planning does not diverge from live runner state.

Policy:
- use `./scripts/run-formal-kani-sync.ps1` as the default operator entrypoint,
- reconcile before and after each deferred dispatch start,
- during active runs, reconcile at least every 30 minutes (or at each cycle boundary),
- do not treat `selected_count=0` no-op lanes as formal pass evidence,
- keep a durable remote-monitor trace during long runs (`-StatusSnapshotJsonl` / `-MonitorSnapshotJsonl`; `*Ndjson` aliases retained) so stall/rate triage is based on time-series evidence.

## 9) Final validation should not mutate tracked evidence

Use no-artifact validation for pre-commit confidence checks so `LATEST` files do not churn during staging.

Policy:
- prefer `./scripts/meta-check.ps1 -Fast -NoArtifacts` before commit,
- use artifact-producing runs only when intentionally refreshing evidence.

## 10) Conformance and perf runs must share a stable run-id per cycle

Artifact-heavy lanes should resolve run-id through lock-aware run context (`scripts/lib-run-context.ps1`) so repeated calls within one cycle reuse the same identifier and avoid duplicate `RUN_*` churn.

## 11) Keep evidence history bounded

Timestamped evidence files are valuable, but unbounded growth hurts repository operations.

Policy:
- keep `LATEST` pointers plus a bounded number of timestamped runs,
- use `./scripts/prune-evidence-artifacts.ps1 -KeepCount <N>` as housekeeping.

## 12) Guard profile artifact scope before commit

Do not unintentionally mutate historical profile artifacts when closing a newer gate.

Policy:
- run `./scripts/validate-profile-artifact-scope.ps1 -Mode staged` before commit,
- if historical backfill is intentional, pass explicit allow-list versions.

## 13) Validation truth must carry subset boundaries

Do not allow active truth artifacts to collapse materially different support states into one broad feature claim.

Required practice:
- split rows when subsets differ materially,
- prefer `implemented-subset` / `in-progress` over broad `implemented`,
- archive or rewrite active artifacts that cannot express the needed precision,
- keep compiler, interpreter, JIT, oracle, and formal-model status visible where relevant.

The standing canary for this rule is `For Each`:
- array iteration support and object-enumerator support must not share one closure label unless both are actually complete for the scoped target.

Additional practice:
- validation beads should map to canonical matrix files or rows in `docs/validation/MATRIX_BEAD_TRACEABILITY_2026-03-29.csv` or a successor traceability artifact,
- generated summaries should come from canonical matrices, not independent status prose,
- recurring reconciliation should be run with `scripts/run-truth-reconciliation.ps1`.

## Required Local Checks (Doc-Heavy Ladder Runs)

1. Validate profile scaffold integrity:

```powershell
./scripts/validate-profile-scaffold.ps1 -FromVersion <start> -ToVersion <end>
```

2. Validate active ladder gate coherence before or during runs:

```powershell
./scripts/validate-active-ladder-sync.ps1
```

3. Validate HAL clause/doc drift when HAL spec surface is touched:

```powershell
./scripts/check-hal-clause-drift.ps1
```

4. Run targeted tests for touched crates and host/hal paths.

5. Ensure referenced artifacts actually exist before commit.

6. Validate staged commit scope split:

```powershell
./scripts/check-staged-commit-scope.ps1
```

7. Validate profile artifact scope:

```powershell
./scripts/validate-profile-artifact-scope.ps1 -Mode staged
```

8. Prefer scaffold generation for large profile slices:

```powershell
./scripts/new-profile-slice.ps1 -FromVersion <start> -ToVersion <end> -LadderPath <ladder> -WorksetPath <workset>
```

## Minimal Bead Loop

For active workset execution:

1. create or refresh the bead subtree for the workset,
2. ensure the workset has the necessary epic set rolled out,
3. inspect the ready bead set,
4. mark one bead in progress,
5. complete the bead outcome and verify its evidence,
6. create follow-up beads immediately for newly discovered required work,
7. if the epic itself now needs further expansion, create or refresh an epic rollout bead,
8. close the bead only when the stated outcome is actually complete,
9. commit bead-state and code-state together.

## Commit Discipline for Ladder Docs

- Commit only after scaffold checks pass.
- Keep one coherent commit for a ladder block when practical.
- If generation errors occur, fix names/content before adding more steps.
