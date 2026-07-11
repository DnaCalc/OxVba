# CORE-1 Executable Rollout Acceptance

Date: 2026-07-11
Rollout bead: `bd-59co.2.2.1`
Status: accepted.

## Outcome and truth boundary

CORE-1 now has an executable path for every baseline item named by the workset.
The earlier graph covered only unsafe-code cleanup. The expanded graph separates
line endings/snapshots, isolated carrier balances, the named policy-error leak,
host/JIT expectation repair, current unsafe ownership, pinned Linux CI, the
canonical runner, platform executions and terminal reconciliation.

The Core readiness matrix now contains five CORE-1 rows. All remain `planned`:

| Row | Delivery path | Current residual owner |
|---|---|---|
| `CORE-BASELINE-EOL-SNAPSHOT` | `.4`, then both platform baselines | `bd-59co.2.2.12` |
| `CORE-BASELINE-BALANCE-LIFECYCLE` | `.5` and `.6`, then both platform baselines | `bd-59co.2.2.12` |
| `CORE-BASELINE-HOST-JIT-DIAGNOSTICS` | `.7`, then both platform baselines | `bd-59co.2.2.12` |
| `CORE-BASELINE-UNSAFE-CLIPPY` | `bd-2cjy`, `.2`, `.13`, `.3`, then both platform baselines | `bd-59co.2.2.12` |
| `CORE-BASELINE-CROSS-PLATFORM-GATES` | `.8` through `.12` | `bd-59co.2.2.12` |

`linux-x64-ci-pending-v1` is now owned by executable leaf
`bd-59co.2.2.8`, not the epic. It remains planned and blocking until its
immutable image/toolchain/fixture identities are sealed.

## Executable graph

| ID | Effect | Bounded outcome | Primary acceptance/evidence |
|---|---|---|---|
| `bd-2cjy` | delivery | repair the six current SafeArray strict-Clippy regressions | `cargo clippy -p oxvba-runtime --all-targets -- -D warnings`; `safe-array-unsafe-audit.md` |
| `.2.2.2` | support | certify the already integrated `vba_record` audit | focused `vba_record` tests; `vba-record-unsafe-audit.md` |
| `.2.2.3` | support | strict workspace Clippy and ordinary workspace tests | workspace format/Clippy/tests; `strict-clippy-lifecycle-baseline.md` |
| `.2.2.4` | delivery | deterministic EOL and snapshots | fail-closed EOL validator, `git ls-files --eol`, JIT snapshot; `eol-snapshot.md` |
| `.2.2.5` | delivery | named subprocess balance protocol | balance protocol/isolation tests; `balance-isolation.md` |
| `.2.2.6` | delivery | repair the harness-identified policy-error BSTR leak | golden plus named balance test; `policy-error-bstr.md` |
| `.2.2.7` | delivery | current host behavior and structured JIT diagnostics | host/native-declare tests; `host-jit-expectations.md` |
| `.2.2.13` | delivery | repair the known HAL dead-code warning without suppression | strict HAL Clippy/tests; `hal-clippy-cleanup.md` |
| `.2.2.8` | delivery | pinned Linux x64 CI environment | environment validators; `linux-ci-environment.md` |
| `.2.2.9` | delivery | versioned portable gate runner | runner plan/self-tests; `canonical-gate-runner.md` |
| `.2.2.10` | support | Windows x64 development baseline | Windows runner lane; `windows-x64-baseline.md` |
| `.2.2.11` | support | pinned Linux x64 CI baseline | Linux runner lane; `linux-x64-baseline.md` |
| `.2.2.12` | support | terminal reconciliation and successor handoff | truth/governance/path/lint/cycles; `terminal.md` |

Every leaf is at most 360 minutes, has explicit resource/risk/model labels,
contract clauses, one or more exact trace rows, typed commands, an artifact,
observable axes and residual behavior. `bd-59co.2.2.2` no longer repeats work
already integrated in `37811fd5`; `bd-2cjy` no longer claims roughly 101 live
findings when the current regression is six SafeArray findings.

## Current characterization incorporated

- `vm3_golden_snapshot` identifies a policy-denied error-5 path with `bstrs: +1`.
- `jit_scope_snapshot` is a checkout LF/CRLF mismatch.
- the stale host test expects `New Collection` to fail although it now succeeds.
- runtime strict Clippy exposes six current SafeArray findings; workspace Clippy
  also exposes the existing HAL dead-code warning.
- Linux CI currently uses mutable runner/toolchain identifiers and therefore is
  not release evidence.

These are planned delivery inputs, not completion evidence.

## Observable and closure policy

All applicable leaves record result, full Err, side effects, lifecycle/order,
transport and balance. No snapshot may be blessed implicitly; no warning may be
suppressed instead of repaired; local Linux output cannot substitute for pinned
CI; the Windows development host cannot certify Windows release scope. CORE-1
can close only after all five rows are verified, both platform manifests agree
where portable, and every uncovered residual has a delivery owner.

The EOL, balance, unsafe-Clippy and host-diagnostic delivery leaves advance implementation but
do not own cross-platform completion. Their matrix evidence/residual owner is
the terminal `.12`; `.10` and `.11` must contribute explicit Windows and pinned
Linux evidence first. A skipped Windows-only test receives no verification
credit.

## Acceptance record

The final independent Core review is clean. It specifically rechecked the
unsafe-row delivery/terminal split, Windows and pinned-Linux evidence routes,
exact command parity, HAL successor ownership, rollout-versus-terminal row
ownership and the non-oracle host-policy wording. Truth reconciliation,
governance, path stability, documentation, traceability, rollout, ownership,
clause disposition, environment, closure, lint and dependency-cycle checks
pass. The 24 negative validator cases pass. The currently observed HAL warning
remains planned delivery in `.13`; it is not represented as a green result.
