# scripts/

- `meta-check.ps1`: one-command readiness check for this repository.
- `docs-check.ps1`: verifies required top-level docs exist.
- `run-smoke.ps1`: executes the smoke VBA sample through the CLI.
- `run-conformance.ps1`: runs MVP conformance corpus and checks against golden expectations.
- `run-matrix.ps1`: executes required matrix cells for the declared ladder profile and writes gate artifacts.
- `run-formal.ps1`: runs manifest-driven formal obligations in non-blocking mode and writes markdown/csv reports.
- `run-formal-kani-wsl.ps1`: strict formal run helper that executes Kani obligations through WSL.
- `run-formal-kani-async.ps1`: async manager for long-running Kani/profile formal runs (`Start`/`Status`/`Tail`/`Wait`/`Stop`).
- `async-task-runner.ps1`: helper invoked by async managers to execute commands and persist exit status.
- `run-bench.ps1`: captures baseline-vs-optimized VM timing evidence for the active stabilization profile.
- `setup-kani.ps1`: verifies or installs Kani toolchain and prints activation instructions for required formal mode.
- `test-path-stability.ps1`: validates scripts/tests behave correctly when executed from non-root working directories.
- `validate-divergences.ps1`: validates structural fields required in divergence records.
- `validate-language-coverage.ps1`: validates language coverage index structure/status vocabulary and duplicate keys.
