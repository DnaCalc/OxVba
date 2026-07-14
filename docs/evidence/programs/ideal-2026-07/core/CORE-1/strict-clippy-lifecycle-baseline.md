# CORE-1 strict Clippy and runtime lifecycle baseline

Date: 2026-07-14
Bead: `bd-59co.2.2.3`
Effect: support
Canonical row: `CORE-READINESS/CORE-BASELINE-UNSAFE-CLIPPY`

## Result and truth boundary

The integrated Rust workspace has a clean formatting, all-target strict-Clippy,
ordinary-test and runtime-lifecycle baseline after the bounded CORE-1 delivery
repairs. No `allow` or `expect` attribute was added to suppress a finding. The
Rust workspace tested at `c16e5fa0` is byte-identical through `f110db1f`; later
commits in that range change only beads, documentation, validation data and
PowerShell control/evidence tooling.

This is the aggregate support result required before the platform profiles run.
It does not verify a Core capability row. The Windows x64 development run,
pinned Linux x64 CI run and terminal reconciliation remain respectively
`bd-59co.2.2.10`, `bd-59co.2.2.11` and `bd-59co.2.2.12`, so all five CORE-1
matrix rows remain `planned`.

## Accepted commands

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass; no formatting delta |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass in 34.9 seconds; zero warnings and zero suppressions |
| `cargo test --workspace` | pass in 496 seconds; all workspace unit, integration and doc tests passed |
| `cargo test -p oxvba-differential --lib` | pass in the default parallel harness; 1,145 tests passed |
| `cargo test -p oxvba-differential --lib -- --test-threads=1` | pass in the serial harness; the same 1,145 tests passed |
| `cargo test -p oxvba-differential --test balance_fixture_protocol` | pass; all three versioned subprocess-protocol tests passed |
| `cargo test -p oxvba-runtime live_counters` | pass; same-thread leak detection and sibling-thread isolation tests passed |
| `cargo test -p oxvba-host --test sqliteforexcel_declare_integration` | pass; both VM3/JIT-decline integration tests passed |
| `./scripts/check-governance.ps1` | pass through line endings, environment and fixture manifests, contract disposition, closure/trace/rollout, 26 negative-validator cases and repeatable derived summaries |

The integrating controller reran formatting, strict all-target Clippy and the
complete workspace test serially on the current tree after the two Windows
handoff commits. The combined lane passed in 596.1 seconds. A path-filtered Git
comparison confirmed that Rust and Cargo inputs remain byte-identical to the
accepted `c16e5fa0` source set.

The workspace test initially exposed one obsolete SQLiteForExcel assertion. Its
exact successor `bd-59co.2.2.33` was delivered before this gate resumed: VM3
executes the real `sqlite3.dll` demonstration, while JIT returns the current
structured M4-9 native/COM diagnostic and the Engine does not fall back to VM.
Full JIT native `Declare` delivery remains open under `bd-59co.3.10`; this
baseline accepts only the exact current decline. The post-repair 496-second
workspace run is the accepted result.

## Lifecycle and unsafe-boundary coverage

The clean aggregate follows the delivery sequence rather than concealing it:

- SafeArray and `vba_record` ownership audits, HAL dead-code cleanup, the
  `oxvba-rt-abi` raw-pointer boundary and the JIT/VM3/symbol/project/binder
  strict-Clippy tranches were repaired without public-interface broadening.
- The named policy-error BSTR leak now preserves raised error 5 and complete
  `FinalErr` while repeated serial and parallel subprocess runs report zero
  BSTR, object, SAFEARRAY, record and total carrier drift.
- Ordinary synchronous differential balance samples are thread-scoped, so
  sibling test activity cannot contaminate a fixture. The independent child
  protocol still proves process-global accounting: a sibling allocation is
  process `+1`, parent thread `0`, and both rebalance after join.
- Stop-statement and RaiseEvent fixtures no longer allocate expected strings
  inside the measured interval. Error, event, recursion and cleanup ordering
  remain covered by neighboring differential and backend tests.
- Line endings and generated inputs are deterministic. Required downstream
  fixture, WIN-14 and derived-summary digests were regenerated and independently
  reviewed; all 57 certification cases remain blocked and carry no capability
  credit.

## Six-axis evidence

| Axis | Observation |
|---|---|
| result | Format, strict Clippy, ordinary workspace tests, parallel/serial differentials and governance all pass. |
| full Err | Error-bearing fixtures preserve the complete `FinalErr` shape; the policy denial remains error 5 and SQLite JIT decline remains the exact structured M4-9 diagnostic. |
| side effects | No snapshot bless, warning suppression, VM fallback, matrix capability promotion or release certification occurred. Generated truth changes were limited to reviewed dependency digests and trace counts. |
| lifecycle/order | Allocation, invoke/error/event handling, writeback, cleanup and final balance ordering are exercised in focused and neighboring runtime tests. Owned fixture children are awaited or terminated and reaped by the canonical runner. |
| transport | Default-parallel and single-thread Rust harnesses, versioned subprocess reports, VM3 execution and explicit JIT diagnostic transport agree with their declared lanes. |
| balance | Thread-local synchronous samples and independent process-global child evidence both return to zero for BSTR, object, SAFEARRAY, record and total carrier counts. |

## Review and residuals

Each bounded delivery slice received non-author review; two reviews found real
balance/oracle-boundary defects that were repaired and re-reviewed before the
aggregate gate. Final controller review verified the command/result record, the
six-axis claims, the unchanged Rust source identity after `c16e5fa0`, and the
explicit support-versus-platform closure boundary; no remaining finding was
accepted.

No unfinished warning, failing Rust test or known carrier imbalance remains
under this bead. Cross-platform certification remains open under `.10`, `.11`
and `.12`, while full JIT native `Declare` remains `bd-59co.3.10`; any failure
in the platform lanes must receive an exact delivery successor rather than
reopening this support record by assertion.
