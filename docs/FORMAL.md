# FORMAL.md

## Scope
Formal artifacts in OxVBA are currently scaffolded for staged adoption and profile-scoped execution.

## Lean scaffold
Location: `formal/lean/`

Current files:
- `OxVba/VarType.lean`
- `OxVba/Coerce.lean`
- `OxVba/Arithmetic.lean`
- `OxVba/RefCount.lean`

## Kani scaffold
Kani harness placeholders are introduced in runtime and VM code under `#[cfg(kani)]` blocks and expanded as unsafe-heavy paths mature.

## Executable formal model checks
Profile-scoped formal obligations may also run as deterministic, reduced-domain model checks via `cargo test` when external provers are unavailable. These are tracked in the same obligation manifest and reported by the formal runner.

## Formal tracking
- Manifest: `docs/evidence/formal/MANIFEST.md`
- Obligation index (machine-readable): `docs/evidence/formal/obligations.csv`
- Inventory: `docs/evidence/formal/INVENTORY.md`
- Deferred formal gates register: `docs/evidence/formal/DEFERRED_GATES.md`
- Extended non-blocking backlog: `docs/evidence/formal/EXTENDED_TODO.md`
- Runner: `./scripts/run-formal.ps1` (non-blocking by current ladder policy)
- Optional strict mode: `./scripts/run-formal.ps1 -RequireKani` or `OXVBA_REQUIRE_KANI=1`
- Kani setup helper: `./scripts/setup-kani.ps1` (`-Install` to bootstrap locally)
- Windows+WSL strict helper: `./scripts/run-formal-kani-wsl.ps1` (runs Kani obligations inside WSL while keeping report paths in this repo)
- Async strict helper for long profile runs: `./scripts/run-formal-kani-async.ps1` (`Start`/`Status`/`Tail`/`Wait`/`Stop`)
- Latest async lane evidence: `docs/evidence/formal/ASYNC_KANI_V83.md`
- Active typing-ladder async lane evidence: `docs/evidence/formal/ASYNC_KANI_V83.md`
- Current install note: native Windows Kani install may fail in this environment; use WSL path above for strict Kani runs.

## Deferred-gate policy (DG)
For long-running Kani in profile ladders, formal completion may be deferred without blocking the active profile gate if all of the following are true:
1. Async run is started and reproducible (`state.json`, stdout/stderr, command script present).
2. DG entry is recorded in `docs/evidence/formal/DEFERRED_GATES.md`.
3. A foldback profile is assigned where results will be reconciled.

DG lanes are still required work, but they are reconciled asynchronously and folded into formal reports once complete.
