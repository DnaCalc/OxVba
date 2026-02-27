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
- Extended non-blocking backlog: `docs/evidence/formal/EXTENDED_TODO.md`
- Runner: `./scripts/run-formal.ps1` (non-blocking by current ladder policy)
- Optional strict mode: `./scripts/run-formal.ps1 -RequireKani` or `OXVBA_REQUIRE_KANI=1`
- Kani setup helper: `./scripts/setup-kani.ps1` (`-Install` to bootstrap locally)
