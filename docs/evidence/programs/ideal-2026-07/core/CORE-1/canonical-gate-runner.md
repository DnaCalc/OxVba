# CORE-1 Versioned Core-Profile Gate Runner

Date: 2026-07-11

Bead: `bd-59co.2.2.9`

Base: `d51bb1ffc302f7d4066cf2e32cd8af1eceb59d6e`

Initial implementation: `7f43d4477b8ebe56e034807898c579c9909f1d15`

Trust-boundary hardening: `4e27cb1560a1dc054ec0657c2301c1a8c92e4fb6`

Implementation tree: `4127e3cefb6fcc2a7a0a511ffeb23850ac96816f`

Clause: `CONF-QUALITY-001`

Matrix route: `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES`

## Result and claim boundary

The repository has one versioned, portable Core-profile gate plan at
`ci/core-profile/gates-v1.json` and one entry point at
`scripts/run-core-profile-gates.ps1`. The runner is now an x64-only,
fail-closed trust boundary rather than a command loop: it binds execution to a
clean committed source tree, exact tool and command identities, an owned
process tree, and immutable terminal evidence.

This bead proves the runner contract and its adversarial failure behavior on
the Windows x64 development host. It does **not** claim that the canonical Core
gates have passed. It also does not claim Linux runtime execution, Excel/VBA
compatibility, or terminal matrix advancement. `bd-59co.2.2.10` owns the
Windows development transcript, `bd-59co.2.2.11` owns execution in the pinned
Linux x64 environment, and `bd-59co.2.2.12` owns the reconciled cross-platform
terminal baseline.

## Sealed implementation identity

The implementation commit contains these exact raw-file SHA-256 identities:

| surface | SHA-256 |
|---|---|
| `ci/core-profile/gates-v1.json` | `44ab21919ce3f7b64bfd5d2b9e082ee237417ffc246a96ac2223844ed323aba5` |
| `scripts/run-core-profile-gates.ps1` | `6244db82e13c3b455f43f269196f873a3a9b65d26cd1b64d651bdfb4f3cad926` |
| `scripts/test-core-profile-gates.ps1` | `969d5988e55c9b0ef57aa44df5b40c6cd8182b340c4202a3bff2c05e7d05ed15` |
| `scripts/core-gate-process-supervisor.cs` | `19f0ca6949cef9159b87b56497f1dbdd6254d92331521798ad16586e1b7b2b1f` |
| `scripts/core-gate-linux-supervisor.sh` | `5505dee95e09c8f6dccdec36cd7f25ade74af9ccc1796cbb379123c57f09b29e` |

The manifest digest is also its strict UTF-8/LF-canonical digest. The Linux
supervisor is tracked with mode `100755`.

## Versioned plan

The plan fixes order, explicit Windows/Linux x64 applicability, arguments,
environment actions, deadlines, Cargo-lock participation, evidence paths and
the supervision transport:

| order | gate | platform disposition | deadline | Cargo lock | evidence directory |
|---:|---|---|---:|---|---|
| 1 | `linux-runtime-environment` | Linux x64 only; explicit N/A on Windows | 180 s | no | `commands/001-linux-runtime-environment` |
| 2 | `windows-environment-ledger` | Windows x64 only; explicit N/A on Linux | 180 s | no | `commands/002-windows-environment-ledger` |
| 3 | `meta-fast-no-artifacts` | Windows/Linux x64 | 7,200 s | yes | `commands/003-meta-fast-no-artifacts` |
| 4 | `differential-default-parallel` | Windows/Linux x64; removes inherited `RUST_TEST_THREADS` | 3,600 s | yes | `commands/004-differential-default-parallel` |
| 5 | `differential-single-thread` | Windows/Linux x64; sets `RUST_TEST_THREADS=1` | 3,600 s | yes | `commands/005-differential-single-thread` |
| 6 | `truth-reconciliation` | Windows/Linux x64 | 600 s | no | `commands/006-truth-reconciliation` |

The meta gate retains the repository's authoritative format, strict workspace
Clippy, ordinary tests and governance composition through
`meta-check.ps1 -Fast -NoArtifacts`. The two differential gates differ only in
their explicit scheduler environment. Truth reconciliation is check-only.

## Execution trust contract

### x64 and committed-source identity

Every mode requires `OSArchitecture=x64`, `ProcessArchitecture=x64` and
`Is64BitProcess=true`. The architecture tuple is recorded in both plan and run
evidence. A fail-only injected x86 identity proves the negative path.

Before `NoArtifacts` execution, the runner requires:

- a valid tracked `HEAD` and its exact tree identity;
- no staged, working-tree or untracked drift according to Git status;
- every runner, manifest, supervisor and PowerShell command file to be tracked;
- no reparse/symlink component in repository, manifest, runner, supervisor or
  command ancestry.

The runner rechecks committed source identity, the versioned manifest, every
command/source byte hash and every tool byte/link identity before and after
each selected gate and again at terminal validation.

### Exact tools and command plans

Git, the active PowerShell Core process, Cargo and, on Linux, exact
`/usr/bin/setsid` are resolved once. Their absolute paths, raw hashes, versions
and link targets are recorded. All subsequent probes and gate launches use
those exact paths rather than a fresh `PATH` lookup. The child `PATH` puts the
sealed tool directories first.

Each plan row records executor path/hash, command or script hash, arguments,
environment actions and a deterministic digest over that complete command
shape. PowerShell gates invoke the sealed `pwsh`; Cargo gates invoke the sealed
Cargo executable. Mid-run source, command, manifest or tool replacement fails
before another gate can run.

### Complete process-tree ownership and one gate deadline

After the separately bounded Cargo-lock acquisition, one gate deadline covers
direct-process execution, descendants, output-handle closure, termination and
reaping:

- Windows uses a kill-on-close Job Object. The direct process is created
  suspended with owned stdout/stderr files, assigned to the job, and only then
  resumed. Assignment/start failure terminates the suspended process.
- Linux starts the tracked supervisor through exact `setsid`, verifies that the
  child PID is the new process-group identity, redirects to owned files, and
  performs bounded group `TERM`, then `KILL`, before the deadline.
- Direct-process exit cannot pass while descendants remain. A short bounded
  observation window converts that state into an explicit failure and cleans
  the complete job/group.
- A selected row can pass only with status `passed`, exit code `0`, and
  `tree_cleanup=complete`. Transport and total deadline are recorded per row.

The adversarial grandchild fixture starts a 30-second child, keeps the direct
parent alive for more than one second, then lets the parent exit. The runner
fails the row as `descendant-processes-remained-after-direct-exit`, records the
platform transport, empties the Job Object and verifies that the published
grandchild PID no longer exists.

### Immutable evidence and exact terminal success

Execution requires a new bounded run ID and refuses an existing evidence root.
It writes only below:

```text
temp/no-artifacts/core-profile-gates/<run-id>/
```

The plan and initial `running` run manifest are constructed in memory and
written as exact UTF-8 bytes. Between gates, those bytes must remain identical;
children may not create the terminal summary or digest early, nor alter any
prior log/result bytes. Each executed row records SHA-256 for `stdout.log`,
`stderr.log` and `result.json`.

Terminal success is reconstructed from immutable in-memory results. Selected
rows must be exact passed/exit-0/clean-tree results; nonselected rows must be
exact `not-applicable` results with `platform:<current-x64-platform>`. The
runner then writes and byte-compares:

- `plan.json`;
- `run-manifest.json`, including architecture, source, tools, commands,
  supervision, run status/failure and the exact result list;
- `summary.txt`, whose digest is bound into the run manifest;
- `run-manifest.sha256`, binding the final manifest digest and relative name;
- all per-row logs/results against their recorded content hashes.

Only after that validation and one final input-identity check does the runner
print `core-profile-gates: ok`. If terminal validation fails, it rewrites the
run and summary state as failed and never prints the marker.

### Mutation and serialization boundaries

The closed manifest schema rejects malformed UTF-8/JSON, duplicate properties,
unknown or mis-cased keys, scalar/array confusion, non-x64 or missing platform
coverage, missing/escaping commands, unsafe environment names, invalid
deadlines, unlocked Cargo commands, and noncanonical/colliding evidence paths.

Command and environment surfaces reject snapshot mutation or acceptance. The
runner removes inherited OxVba bless families, snapshot-update/acceptance
families, `INSTA_UPDATE` and `UPDATE_EXPECT`; the allowlist cannot add them
back. Every `cargo_workspace=true` row acquires a repository-derived named
cross-process mutex. Concurrent runners remain source-read-safe and cannot
overlap Cargo gates for the same checkout.

## Invocation contract

Manifest validation and deterministic projection are side-effect free:

```powershell
./scripts/run-core-profile-gates.ps1 -Mode ValidateManifest
./scripts/run-core-profile-gates.ps1 -List
./scripts/run-core-profile-gates.ps1 -DryRun
```

Execution is a distinct mode and requires a lowercase bounded identity:

```powershell
./scripts/run-core-profile-gates.ps1 -Mode NoArtifacts -RunId <run-id>
```

## Checks executed

```text
./scripts/run-core-profile-gates.ps1 -Mode ValidateManifest
PASS: six-row closed-schema manifest on Windows x64.
PASS: manifest SHA-256 44ab21919ce3f7b64bfd5d2b9e082ee237417ffc246a96ac2223844ed323aba5.

./scripts/run-core-profile-gates.ps1 -List
PASS: exact six-row Windows x64 projection; Linux lane remains visible as N/A.

PowerShell parser + Add-Type compilation
PASS: runner/test parse; C# Windows/POSIX ownership helper compiles.

./scripts/test-core-profile-gates.ps1
PASS in 200.6 s: default All phase.
PASS: x64 injection, exact positive plan/run/summary/digest and content hashes,
      command failure, one-second timeout, long-lived grandchild cleanup,
      stale evidence and hostile inherited environment.
PASS: six independent evidence attacks: plan, run status, early summary, prior
      log, prior result, and consistent forged result/run state. No failing
      case printed the terminal success marker.
PASS: dirty source, mid-run command replacement, manifest drift, mutable exact
      Cargo found through PATH, missing Cargo and reparse command ancestry.
PASS: 25 strict manifest mutations.
PASS: two concurrent runners; command intervals did not overlap and the
      observed maximum Cargo-lock wait was 1,763 ms.
PASS: process-unique temporary mini-repositories and published test PIDs were
      removed/absent after completion.

git diff --check / staged mode check
PASS: no whitespace errors; Linux supervisor is mode 100755.

./scripts/check-governance.ps1
PARTIAL: Linux contract validation and 20 mutations, docs, active-program sync,
         divergence, deferred-oracle, PMR follow-up and project integration
         checks passed. The command then stopped at the inherited stale
         docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md, the same unrelated
         base-state drift recorded before this hardening.

Downstream governance checks run directly after that inherited stop
PASS: PMR diagnostic sync, validation ownership, Windows x64 surfaces,
      contract-clause disposition, environment manifest, legacy migration,
      closure taxonomy, bead traceability, workset rollout, 24 negative
      validator cases and derived-summary check.
```

The Linux environment validator ran in contract-only mode on Windows; this is
not Linux runtime evidence. The canonical six gate commands were deliberately
not executed by this bead.

## Residual and limitation

- `bd-59co.2.2.10` must execute the sealed manifest on the Windows x64
  development host and retain the transcript/evidence summary.
- `bd-59co.2.2.11` must execute the same manifest in the exact digest-pinned
  Linux x64 CI environment and retain its transcript/evidence summary.
- `bd-59co.2.2.12` must compare both platform results, resolve divergence, and
  keep the matrix planned until every terminal baseline gate is green.
- The remaining local limitation is a non-hostile atomic filesystem race: a
  concurrent process could replace a validated path in the narrow interval
  between the last identity check and use. Persistent drift and reparse ancestry
  are rejected. Fully removing this check/use interval needs a future
  handle-relative/open-by-identity launch path; another path recheck cannot
  eliminate it.
