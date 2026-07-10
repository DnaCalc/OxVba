# CONTRIBUTING.md

## Workflow
1. Read `CHARTER.md`, `OPERATIONS.md`, `docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md`, and `docs/ARCHITECTURE.md`; then open only the current workset/spec/matrix rows relevant to the change. `MACH1000_PLAN.md` is historical vision context, not current execution authority.
2. Implement changes with tests.
3. Run `./scripts/meta-check.ps1 -Fast -NoArtifacts`.
   - For long Kani/profile formal runs, use `./scripts/run-formal-kani-async.ps1` and attach status/log artifacts.
   - For profile ladder doc generation/edits, also run `./scripts/validate-profile-scaffold.ps1 -FromVersion <start> -ToVersion <end>`.
   - If HAL clause/spec surface changed, run `./scripts/check-hal-clause-drift.ps1`.
4. Update docs for any behavior/plan changes.
5. Open PR with clear scope and evidence notes.

## Compatibility claims
Any Office/VBA compatibility claim must point to reproducible evidence (test case, harness output, or documented spec source).

## Clean-room rule
Do not introduce proprietary or reverse-engineered sources. Use only public specs/research/reproducible observation.
