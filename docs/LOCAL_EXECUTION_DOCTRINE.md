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
- treat selector mapping as policy-level config first (engine/HAL override API), env fallback second.

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

## Commit Discipline for Ladder Docs

- Commit only after scaffold checks pass.
- Keep one coherent commit for a ladder block when practical.
- If generation errors occur, fix names/content before adding more steps.
