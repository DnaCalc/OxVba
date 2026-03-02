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
