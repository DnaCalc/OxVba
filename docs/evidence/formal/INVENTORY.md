# Formal Proof Inventory

This inventory tracks formal artifacts and proof-adjacent harnesses by profile.

Canonical inventory source:
- `docs/evidence/formal/obligations.csv`

Human-readable status:
- `docs/evidence/formal/latest_run.md`
- `docs/evidence/formal/latest_run.csv`

## Notes
- Profiles `v2`..`v26` are represented in the obligation index.
- Kani-backed obligations are recorded as `skipped` when `cargo-kani` is unavailable.
- Executable model-check tests are used to keep formal cadence active when external provers are unavailable.
