# Formal Proof Inventory

This inventory tracks formal artifacts and proof-adjacent harnesses by profile.

Canonical inventory source:
- `docs/evidence/formal/obligations.csv`

Human-readable status:
- `docs/evidence/formal/latest_run.md`
- `docs/evidence/formal/latest_run.csv`
- `docs/evidence/formal/KANI_MODEL_CHECKING_REVIEW_2026-03-04.md`
- `docs/evidence/formal/KANI_OBLIGATION_POLICY_V1.csv`

## Notes
- Profiles `v2`..`v67` are represented in the obligation index.
- Kani-backed obligations are recorded as `skipped` when `cargo-kani` is unavailable.
- Executable model-check tests are used to keep formal cadence active when external provers are unavailable.
- Kani obligation-tier governance is tracked in `KANI_OBLIGATION_POLICY_V1.csv` and validated by `scripts/validate-kani-obligation-policy.ps1`.
