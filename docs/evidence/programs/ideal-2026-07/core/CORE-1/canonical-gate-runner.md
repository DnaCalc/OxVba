# CORE-1 Versioned Core-Profile Gate Runner

Date: 2026-07-11
Bead: `bd-59co.2.2.9`
Base: `d51bb1ffc302f7d4066cf2e32cd8af1eceb59d6e`
Implementation commit: `7f43d447`
Clause: `CONF-QUALITY-001`
Matrix route: `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES`

## Result and claim boundary

The repository now has one versioned, portable Core-profile gate plan at
`ci/core-profile/gates-v1.json` and one runner at
`scripts/run-core-profile-gates.ps1`. The plan fixes command order, explicit
Windows/Linux x64 applicability, command arguments and environment changes,
per-command timeouts, Cargo-lock participation and evidence paths. The runner
validates the complete plan before it lists or executes anything.

This bead proves the runner contract and its failure behavior on the Windows
x64 development host. It does **not** claim that the canonical Core gates have
passed. `bd-59co.2.2.10` owns the Windows development transcript,
`bd-59co.2.2.11` owns execution inside the pinned Linux x64 environment, and
`bd-59co.2.2.12` owns the reconciled cross-platform terminal baseline. No Linux
execution result, VBA compatibility result or terminal matrix advancement is
claimed here.

## Versioned plan

The canonical manifest SHA-256, using strict UTF-8 and LF-canonical text, is:

```text
1d7d3e9df01bfd7c6fb378d6a78eb92e355179d83a4adc68b0fc65e68d7e5fee
```

Its order is:

| order | gate | platform disposition | timeout | Cargo lock | evidence directory |
|---:|---|---|---:|---|---|
| 1 | `linux-runtime-environment` | Linux x64 only; explicit not-applicable row on Windows | 180 s | no | `commands/001-linux-runtime-environment` |
| 2 | `windows-environment-ledger` | Windows x64 only; explicit not-applicable row on Linux | 180 s | no | `commands/002-windows-environment-ledger` |
| 3 | `meta-fast-no-artifacts` | Windows/Linux x64 | 7,200 s | yes | `commands/003-meta-fast-no-artifacts` |
| 4 | `differential-default-parallel` | Windows/Linux x64; removes inherited `RUST_TEST_THREADS` | 3,600 s | yes | `commands/004-differential-default-parallel` |
| 5 | `differential-single-thread` | Windows/Linux x64; sets `RUST_TEST_THREADS=1` | 3,600 s | yes | `commands/005-differential-single-thread` |
| 6 | `truth-reconciliation` | Windows/Linux x64 | 600 s | no | `commands/006-truth-reconciliation` |

The meta gate invokes `meta-check.ps1 -Fast -NoArtifacts`, so current
governance, format, strict workspace Clippy and ordinary workspace tests retain
their existing authoritative composition. The two differential gates invoke
the same crate/target command; only their explicit scheduler environment
differs. Truth reconciliation is check-only and never refreshes derived truth.

## Invocation contract

Manifest validation is side-effect free:

```powershell
./scripts/run-core-profile-gates.ps1 -Mode ValidateManifest
./scripts/run-core-profile-gates.ps1 -List
./scripts/run-core-profile-gates.ps1 -DryRun
```

`-List` and `-DryRun` project the same deterministic, timestamp-free plan. They
include every gate, its `run` or `not-applicable` disposition, reason, platform
set, timeout, Cargo-lock flag, command/arguments/environment and future evidence
path. Platform-specific work therefore cannot disappear as a silent skip.

Execution is deliberately a distinct mode and requires an explicit bounded
run identity:

```powershell
./scripts/run-core-profile-gates.ps1 -Mode NoArtifacts -RunId <lowercase-id>
```

The runner refuses a pre-existing run directory rather than mixing current and
stale evidence. It writes only below:

```text
temp/no-artifacts/core-profile-gates/<run-id>/
```

Each run contains `plan.json`, `run-manifest.json`, `summary.txt`, and one
directory per executed gate with `stdout.log`, `stderr.log` and `result.json`.
The child receives the run ID, gate ID, evidence root, plan path, plan digest
and manifest digest through reserved `OXVBA_CORE_GATE_*` variables. The runner
also propagates the bound manifest path, revalidates the live manifest before
and after every selected gate, and rechecks it with the plan, run manifest and
per-gate result files before reporting success.

## Fail-closed and ownership behavior

The manifest parser rejects malformed UTF-8/JSON, comments, trailing commas,
duplicate properties, unknown or mis-cased keys, wrong scalar types or JSON
container kinds, non-x64 or missing platform coverage, non-contiguous/duplicate
identities, missing or escaping commands, unsafe environment names, invalid
timeouts, unlocked Cargo commands, and non-canonical/colliding evidence paths.
Array kinds are checked from `System.Text.Json` before PowerShell can coerce a
scalar object or string into a one-element collection.

Command and environment surfaces reject snapshot mutation/acceptance routes.
Before every child starts, the runner removes all inherited `OXVBA_BLESS*`,
snapshot-update/acceptance, `INSTA_UPDATE` and `UPDATE_EXPECT` environment
families. The closed manifest environment allowlist cannot add them back.
There is no update switch or alternate command catalog in the runner. A
PowerShell gate reuses the exact active `pwsh` executable; a missing Cargo tool
fails before command execution. Nonzero exit, start failure, lock-acquisition
failure, timeout, changed live-manifest or plan bytes, malformed evidence,
missing evidence or a stale run root all fail the run. Timeouts kill the owned
child process tree.

Every gate marked `cargo_workspace=true` acquires a named cross-process mutex
derived from the canonical repository root and the versioned lock prefix. The
lock is released in `finally`; abandoned ownership is recovered explicitly.
Commands still execute one at a time within a runner, and concurrent runners
cannot overlap workspace Cargo gates for the same checkout.

## Checks executed

```text
./scripts/run-core-profile-gates.ps1 -List
PASS: exact six-row Windows x64 projection; Linux lane visible as not-applicable.

./scripts/run-core-profile-gates.ps1 -Mode ValidateManifest
PASS: canonical closed-schema manifest and all referenced commands.

./scripts/test-core-profile-gates.ps1
PASS: two identical lists, two identical dry-runs, one positive no-artifact run,
      command failure, hard timeout, stale evidence, evidence tampering, missing
      Cargo, a mid-run manifest replacement, 22 invalid/mutated manifests, and
      two concurrent Cargo-lock runs.
PASS: the concurrent command intervals did not overlap and lock-wait evidence
      showed serialization.
PASS: hostile parent values for both known OxVba bless variables plus generic
      snapshot-update and `INSTA_UPDATE` variables were absent in every child.

./scripts/check-governance.ps1
PARTIAL: Linux-contract mutations, docs and the following program validators
         passed; the inherited isolated `.8` base then stopped at its already
         stale generated PMR diagnostic snippet. This bead does not own or edit
         that generator/artifact, and the controller must rerun governance on
         the integrated primary truth before acceptance.

PowerShell parser
PASS: runner and test harness parse without errors.
```

The test harness creates process-unique repositories under the system temporary
root and deletes only a resolved directory whose owned prefix matches. It
constructs real child processes for failure, timeout and concurrency tests; it
does not mock the runner's ordering, lock or evidence implementation.

## Residual execution

- `bd-59co.2.2.10` must execute the manifest on the Windows x64 development
  host and retain the resulting transcript/evidence summary.
- `bd-59co.2.2.11` must execute the same manifest in the exact digest-pinned
  Linux x64 CI environment and retain its transcript/evidence summary.
- `bd-59co.2.2.12` must compare the platform results, resolve any divergence,
  and keep the matrix planned until all terminal baseline gates are green.
