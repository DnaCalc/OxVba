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
- do not treat `selected_count=0` no-op lanes as formal pass evidence.

## Required Local Checks (Doc-Heavy Ladder Runs)

1. Validate profile scaffold integrity:

```powershell
./scripts/validate-profile-scaffold.ps1 -FromVersion <start> -ToVersion <end>
```

2. Validate HAL clause/doc drift when HAL spec surface is touched:

```powershell
./scripts/check-hal-clause-drift.ps1
```

3. Run targeted tests for touched crates and host/hal paths.

4. Ensure referenced artifacts actually exist before commit.

## Commit Discipline for Ladder Docs

- Commit only after scaffold checks pass.
- Keep one coherent commit for a ladder block when practical.
- If generation errors occur, fix names/content before adding more steps.
