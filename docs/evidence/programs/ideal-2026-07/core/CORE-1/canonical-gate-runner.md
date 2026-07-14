# CORE-1 Versioned Core-Profile Gate Runner

Date: 2026-07-14

Bead: `bd-59co.2.2.9`

Implementation: `b7e3f5f40489260373d7a4f304671e7ea1d7c63a`

Implementation tree: `96dbaaa1f7176baf45c657c2c88991accb969038`

Clause: `CONF-QUALITY-001`

Matrix route: `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES`

## Result and claim boundary

The repository has one versioned, portable Core-profile gate plan at
`ci/core-profile/gates-v1.json` and one entry point at
`scripts/run-core-profile-gates.ps1`. The runner is an x64-only, fail-closed
trust boundary. It binds execution to a clean committed source tree, sealed
tool and command identities, exact owned process trees, bounded deadlines and
immutable terminal evidence.

This bead proves that contract and its adversarial failure behavior on the
Windows x64 development host. Native WSL Ubuntu x64 checks additionally prove
the Linux pidfd/subreaper ownership helper, including the pre-confirmation abort
path, and the child-free shell readiness transport. It does **not** claim that
the canonical six Core gates have passed and does not advance the terminal Core
matrix. `bd-59co.2.2.10` owns the Windows development transcript,
`bd-59co.2.2.11` owns the pinned Linux x64 execution, and `bd-59co.2.2.12` owns
the reconciled cross-platform terminal baseline.

## Sealed implementation identity

Implementation commit `b7e3f5f40489260373d7a4f304671e7ea1d7c63a`
contains these exact raw-file SHA-256 identities:

| surface | SHA-256 |
|---|---|
| `ci/core-profile/gates-v1.json` | `eddfff47d6d24076fba24f8d03cae83c52ef9de9e851bbf2c8d35fecade14eca` |
| `scripts/run-core-profile-gates.ps1` | `d8132f5dce592acfcf0ca4f2a0e8d43f748dd0b43cdf4d3eed98d87ee42199f8` |
| `scripts/test-core-profile-gates.ps1` | `5fa9ab54d8b713f2818ece0b52179479a47e1332695a976e146db4d9b33ef1fd` |
| `scripts/core-gate-process-supervisor.cs` | `5aa0abefe4e0a3bf9dd279cc0f16715019561d26998e2ead20d028c4b75ce665` |
| `scripts/core-gate-linux-supervisor.sh` | `95d7f8e057ef8161b8782ab00722637a2074ffb8002ea8fc5708b610c23ba001` |

The manifest digest is also its strict UTF-8/LF-canonical digest. The Linux
supervisor is tracked with mode `100755` and contains LF-only line endings.

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

### x64, source and tool identity

Every mode requires `OSArchitecture=x64`, `ProcessArchitecture=x64` and
`Is64BitProcess=true`; the tuple is recorded in plan and run evidence. A
fail-only injected x86 identity proves the negative path.

Before `NoArtifacts` execution, the runner requires a valid tracked `HEAD` and
exact tree identity, a clean staged/working/untracked state, tracked runner and
command surfaces, and no reparse/symlink component in their ancestry. It
rechecks the committed tree, manifest, source/command bytes and tool byte/link
identities before and after each selected gate and at terminal validation.

Candidate paths and bytes are resolved without executing the candidate. The
repo-native C# supervisor is loaded only after those seals exist. All later
version and Git queries run under the same Windows Job or Linux pidfd ownership
used by product gates and write output to confined owned files rather than
inherited pipes. The sealed executable set is Git, the active PowerShell Core
process and Cargo; Linux additionally seals exact `/usr/bin/setsid` and
`/usr/bin/bash`. The v4 Linux supervisor no longer executes or seals `mv` or
`sleep`.

Each plan row records executor path/hash, command or script hash, arguments,
environment actions and a deterministic digest over that complete shape.
PowerShell gates invoke sealed `pwsh`; Cargo gates invoke sealed Cargo. Mid-run
source, command, manifest or tool replacement fails before another gate runs.

### Exact process ownership and one gate deadline

After separately bounded Cargo-lock acquisition, one gate deadline covers the
direct process, descendants, output closure, termination and reaping. The
product runner contains no `Process.Kill`, numeric PID signal or numeric
process-group signal path.

On Windows:

- `STARTUPINFOEX` supplies an exact stdin/stdout/stderr handle allowlist; an
  inheritable parent-owned event excluded from that list proves no ambient
  handle leak;
- the direct process is created suspended, assigned to a kill-on-close Job
  Object and resumed; start/assignment failure terminates it while suspended;
- direct exit cannot pass while the Job still reports active descendants.

On Linux:

- the runner becomes a child subreaper and starts exact `setsid` plus Bash;
- immediately after `Process.Start`, it opens and retains the exact root pidfd
  **before** any fallible `/proc`, parent or start-tick confirmation;
- after reading `/proc` and confirming the direct parent, it sends signal zero
  through that retained pidfd before setting `_rootConfirmed`, preventing PID
  reuse from attaching the retained authority to a different numeric task;
- every pre-confirmation failure remains unconfirmed and takes a dedicated
  abort path that STOP/KILLs only the retained root pidfd; the caller then
  reaps its `Process` handle and requires zero retained pidfds;
- the Bash supervisor creates no external helper or background child before
  acknowledgement. It writes a newline-terminated readiness record directly,
  uses Bash `read` and `EPOCHREALTIME` for a bounded built-in acknowledgement
  poll, and only then `exec`s the gate;
- readiness binds nonce, PID, process group, session and `/proc` start ticks;
- confirmed descendants are retained with `pidfd_open` after exact ancestry
  and start-tick revalidation. Cleanup freezes parents via pidfd `SIGSTOP`,
  repeats discovery until the exact stopped set is stable, then sends `SIGKILL`
  through retained pidfds until the deadline;
- adopted zombies are reaped with exact `waitpid`; the direct root is reaped by
  `System.Diagnostics.Process`; success requires no live owned process and zero
  retained pidfds.

The Linux transport descriptor is
`setsid-bash-pidfd-subreaper-v4:direct-ready;builtin-ack-poll;parent-freeze;pidfd-kill;owned-file-stdout-stderr`.
A selected row can pass only with status `passed`, exit code `0`, complete tree
cleanup, ownership readiness and the exact platform containment descriptor.

The controlled fake-Cargo fixture exits after spawning a descendant that
retains stdout/stderr. The runner reports
`descendant-processes-remained-after-direct-exit`, empties the owned tree, and
the test proves that descendant gone while an unrelated live sentinel remains.

### Immutable evidence, closed schema and serialization

Execution requires a new bounded run ID and refuses an existing evidence root.
It writes only below `temp/no-artifacts/core-profile-gates/<run-id>/`. Plan and
initial run-manifest bytes remain immutable between gates. Children cannot
create terminal summaries/digests early or alter earlier result/log bytes. Each
row records SHA-256 for stdout, stderr and result JSON.

Terminal success is reconstructed from immutable in-memory results. Selected
rows must be exact passed/exit-0/clean-tree results; nonselected rows must be
exact `not-applicable` results. The runner byte-compares plan, run manifest,
summary, digest and every row artifact, then performs one final input seal
before printing `core-profile-gates: ok`.

The closed manifest rejects malformed UTF-8/JSON, duplicate or unknown keys,
wrong casing/types, non-x64 or incomplete platform coverage, escaping command
paths, unsafe environment names, invalid deadlines, unlocked Cargo commands,
noncanonical evidence paths and changed containment descriptors. The runner
removes inherited bless/snapshot-update environment families. Every
`cargo_workspace=true` row acquires one repository-derived cross-process mutex;
concurrent runners cannot overlap Cargo gates for the same checkout.

## Invocation contract

Validation and projection are side-effect free:

```powershell
./scripts/run-core-profile-gates.ps1 -Mode ValidateManifest
./scripts/run-core-profile-gates.ps1 -List
./scripts/run-core-profile-gates.ps1 -DryRun
```

Execution is distinct and requires a lowercase bounded identity:

```powershell
./scripts/run-core-profile-gates.ps1 -Mode NoArtifacts -RunId <run-id>
```

## Checks executed

```text
PowerShell AST parse + Add-Type compilation
PASS: runner and test parse; native Windows/POSIX supervisor compiles.

./scripts/run-core-profile-gates.ps1 -Mode ValidateManifest
PASS: six-row closed-schema manifest on Windows x64.
PASS: digest eddfff47d6d24076fba24f8d03cae83c52ef9de9e851bbf2c8d35fecade14eca.

Static authority and ordering audit
PASS: no Process.Kill, SignalGroup, GroupExists or native numeric kill path.
PASS: root confirmation order is /proc read, retained-pidfd signal zero, then
      _rootConfirmed=true.
PASS: the pre-ack shell has no mv/sleep/helper child and uses a built-in bounded
      EPOCHREALTIME poll.

./scripts/test-core-profile-gates.ps1 -Phase All
PASS on the final implementation candidate before the narrow post-/proc pidfd
      liveness amendment: 583,681 ms, exit 0, empty stderr.
PASS summary: x64=1 exact-success=1 failures=1 timeouts=1 descendants=1
      evidence-tamper=6 source-tool-seals=5 path-confinement=2
      manifest-mutations=27 cargo-concurrency=2.
PASS: maximum observed Cargo-lock wait 2,286 ms.

./scripts/test-core-profile-gates.ps1 -Phase Core
PASS on exact implementation b7e3f5f4: 402,478 ms, exit 0, empty stderr.
PASS: final ordering assertion, deterministic/x64 gate, exact success, fast
      exit, nonzero command, timeout, descendant cleanup, evidence attacks and
      committed source/command/manifest seals.

WSL Ubuntu x64 native .NET 10 confirmed-tree harness
PASS: core9-pidfd-harness: ok root=1053 escaped=1254 retained=1 reaped=1
      non_target=1052:alive.
PASS: confirmed ArmRoot path, 200-child short-exit storm, escaped setsid
      descendant, adopted-zombie reap, zero retained pidfds and unrelated
      process preservation on exact implementation b7e3f5f4.

WSL Ubuntu x64 forced unconfirmed-root harness
PASS: core9-unconfirmed-root: ok root=1257 retained_before=1 retained_after=0
      reaped=1 non_target=1256:alive gate=not-run.
PASS: failure after pidfd retention and before acknowledgement; exact root
      abort/reap, zero retained pidfds, gate not executed, unrelated sentinel
      preserved on exact implementation b7e3f5f4.

WSL exact shell readiness/ack harness
PASS: core-gate-linux-supervisor-handshake: ok pid=1285
      hostile-path=not-executed.
PASS: direct complete readiness, built-in acknowledgement poll and hostile PATH
      non-execution on exact implementation b7e3f5f4.

git diff --check / staged scope check / executable-mode check
PASS: no whitespace errors; implementation/evidence scopes separated; Linux
      supervisor is mode 100755 and LF-only.
```

The WSL environment has native .NET 10 but no usable Linux `pwsh`, so the full
PowerShell runner was not executed there. The native pidfd and shell slices are
not a substitute for `bd-59co.2.2.11`'s pinned Linux run. The canonical six gate
commands were deliberately not executed by this bead.

## Residual and limitation

- `bd-59co.2.2.10` must execute the sealed manifest on the Windows x64
  development host and retain the transcript/evidence summary.
- `bd-59co.2.2.11` must execute the same manifest in the exact digest-pinned
  Linux x64 CI environment and retain its transcript/evidence summary.
- `bd-59co.2.2.12` must reconcile both platform results and keep the matrix
  planned until every terminal baseline gate is green.
- `bd-59co.2.2.23` narrowly owns the remaining pathname check/use race. Current
  reparse/path/hash checks fail closed at explicit boundaries, but a later named
  open/exec is not yet bound to a retained nonreplaceable Windows handle or
  Linux directory/file descriptor. A concurrent pathname replacement can race
  that interval; another path recheck alone cannot remove it.
