# scripts/

- `meta-check.ps1`: one-command readiness check for this repository.
- `docs-check.ps1`: verifies required top-level docs exist.
- `run-smoke.ps1`: executes the smoke VBA sample through the CLI.
- `run-conformance.ps1`: runs MVP conformance corpus and checks against golden expectations.
- `run-matrix.ps1`: executes required matrix cells for the declared MVP profile and writes gate artifacts.
- `run-formal.ps1`: runs profile-scoped formal obligations in non-blocking mode and writes a report.
