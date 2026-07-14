# CORE-1 Identity-Bound Gate Inputs

Date: 2026-07-14

Bead: `bd-59co.2.2.23`

Base: `481ec3304098a93d092ea55be31a57a0bca4a60f`

Implementation: `07bcf2d6463381f33b9ebfaef0f269787c54b678`

Implementation tree: `f93ecc13bfed73174863b95505edc80fa876b15f`

Verification: `a79e4a71b35c4070a9117cabe45ebad06416ac53`

Verification tree: `6a2a390acfa0a9209f0f728cd29400f8edd5a9df`

Clause: `CONF-QUALITY-001`

Matrix route: `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES`

## Result and claim boundary

The Core-profile runner now consumes the exact admitted instances of every
direct gate or tool-probe launch input. A successful hash/path admission is no
longer followed by an unprotected pathname reopen for the executable,
PowerShell command, manifest, native/shell supervisor, Bash or `setsid`.

On Windows x64, retained nonreplaceable file, reparse-entry, target and ancestor
directory handles keep the admitted instance stable through suspended process
creation and for the complete Job lifetime. On Linux x64, retained directory
and source descriptors, sealed read-only `memfd` snapshots and child-only
`posix_spawn` descriptor duplication make execution independent of later path
replacement. Existing Windows Job and Linux pidfd/subreaper containment remain
the process-lifecycle authority.

This is a runner trust-boundary result. It does not claim that the canonical
Core gate payloads have passed on the pinned Linux CI image or close the
cross-platform terminal baseline. Those environment and terminal transcripts
remain owned by `bd-59co.2.2.11` and `bd-59co.2.2.12`.

## Sealed implementation identity

The implementation and verification commits contain these raw-file SHA-256
identities:

| surface | SHA-256 |
|---|---|
| `ci/core-profile/gates-v1.json` | `08d045e0dc3072691a047f2ee360d19026e219b7d32198799a8aa386b467b377` |
| `scripts/core-gate-linux-supervisor.sh` | `649600ad996d2511c0d2fe397a23692f43a5f6ae804b0b50c8bd305e61f3d1c3` |
| `scripts/core-gate-process-supervisor.cs` | `10a1e8ba86114b6c078ce9d73328f50e103fdc52093f8aa5af333022abe4f7ba` |
| `scripts/run-core-profile-gates.ps1` | `615ee212c865e1082d2eae71053a4b9eb32913444e95e5eaddd93e553f15b51f` |
| `scripts/test-core-profile-gates.ps1` | `2553eae4d2fb60ad576e4919f75fb1d1d77b89bbbdefed0ded773588537cfc2c` |

No canonical matrix, `AUTORUN_STATE`, generated summary or `.beads` surface is
changed by this bead.

## Windows x64 binding contract

`OxVbaCoreGateWindowsJob.Start` receives the complete admitted input path and
digest set before `CreateProcessW`:

- Every final entry is opened without write or delete sharing. A reparse entry
  is retained separately from its followed regular-file target.
- Every ancestor directory is opened as a non-reparse directory without delete
  sharing. The target final path is checked again after the ancestor locks are
  acquired.
- SHA-256 is computed from the retained target handle, not by reopening its
  pathname.
- The exact executable must occur in the bound set. All retained handles remain
  owned until the Job has drained and is disposed.
- Existing `STARTUPINFOEX` handle allowlisting, suspended creation,
  assign-before-resume and Job kill-on-close remain intact. Bound input handles
  stay in the parent and are not ambient child capabilities.

This is the platform-appropriate counterpart to execute-by-descriptor: Windows
process creation still names the executable, but the admitted entry, target and
all rename-relevant ancestors are nonreplaceable for the entire check/use
interval.

## Linux x64 binding contract

`OxVbaCoreGatePosixChild` owns the Linux direct-child launch:

- Repository inputs are opened relative to the retained working-directory fd
  with `openat2(RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS |
  RESOLVE_NO_SYMLINKS)`. External tools are opened below a retained root fd
  after canonical resolution.
- The source fd is retained for provenance. Its admitted bytes are copied into
  an executable `memfd` with `F_SEAL_SEAL`, `F_SEAL_SHRINK`, `F_SEAL_GROW` and
  `F_SEAL_WRITE`, then reopened read-only and rehashed.
- All source, snapshot, root, work, gate-directory and transport fds remain
  `FD_CLOEXEC` in the parent. The immediately retained launch pidfd is checked
  for the same invariant.
- `posix_spawn_file_actions_adddup2` maps only the required descriptors to
  reserved child fd numbers. The child's argv and environment name only those
  child descriptors. There is no process-wide `F_SETFD` inheritance window and
  no class-local lock pretending to serialize unrelated process creation.
- The exact sealed `setsid`, Bash, supervisor, executable, command and manifest
  bytes survive later rename, ancestor replacement or hostile symlink
  installation. The shell changes directory through the retained work fd and
  enters the executable fd with the admitted original `argv[0]`.
- Ready, acknowledgement, stdout and stderr are opened below one retained gate
  directory. Parent reads and writes use the exact retained fds. Captured
  stdout/stderr bytes must still equal the confined evidence-path bytes, so a
  later evidence-path replacement fails closed.

The versioned transport is:

`setsid-fd-posix-spawn-pidfd-subreaper-v6:child-dup2-bound-inputs;no-ambient-parent-inheritance;pinned-glibc-x64-abi;direct-ready;builtin-ack-poll;parent-freeze;pidfd-kill;owned-file-stdout-stderr`

### Explicit libc constraint

The active Linux release profile is pinned Debian glibc x64. The implementation
uses that profile's 80-byte public `posix_spawn_file_actions_t` ABI. Before any
file-actions storage is allocated, `gnu_get_libc_version` must be present and
must yield a bounded numeric version. The resulting `glibc-N.N-x64` identity is
recorded in plan and gate evidence and checked across launch. Musl, an unknown
libc or a malformed identity fails before memory is allocated; this bead makes
no broader libc-portability claim.

## Failure and lifecycle behavior

The direct Linux pidfd is retained immediately after a successful
`posix_spawn`. Any subsequent `Start` failure signals only that pidfd, reaps the
exact child with `waitpid`, unlinks only the same ready/ack inodes and closes all
owned descriptors. The forced failure test occurs after pidfd retention and
proves the gate never receives acknowledgement or runs.

If `pidfd_open` itself reports that the child has already exited, the exact
child is reaped. If the pidfd API fails unexpectedly before authority can be
retained, the implementation does not fall back to numeric-PID signaling. The
sealed supervisor remains in its child-free pre-ack loop, expires after five
seconds, and is reaped through the exact `waitpid` parent relationship within
the bounded failure path. This is a fail-closed API limit, not a waived recheck.

Early exit, timeout and later descendant cleanup continue through the existing
Job or pidfd/subreaper implementation. Unrelated files, handles, processes and
sentinels are never cleanup targets.

## Adversarial verification

The permanent tests cover:

- write, final-file rename and atomic ancestor-directory rename after admission
  on Windows; all are blocked while an unrelated file rename succeeds;
- ancestor rename followed by hostile symlink replacement on Linux; exact
  sealed original executable/command bytes run and later pathname reconciliation
  fails closed;
- exact manifest immutability, including a gate attempting to append to the
  admitted instance;
- a concurrent unrelated Bash launch at the gate launch boundary. It exits with
  a dedicated failure code if an admitted supervisor descriptor is inherited;
- forced post-spawn failure after launch pidfd retention, with exact child reap,
  balanced parent descriptors, no gate execution, no ready/ack residue and an
  unrelated sleep sentinel still alive;
- Windows inherited-handle allowlisting, owned probe-descendant cleanup,
  reparse rejection, evidence-root swap confinement, 27 manifest mutations,
  abandoned Cargo-lock recovery and two-run Cargo serialization.

The executable race pauses on the gate-unique admitted manifest, not a brittle
count of earlier Cargo version probes. This point is reached only after the
gate's Cargo executable and every other gate input are retained.

## Execution transcripts

### Focused Windows x64 phases

| command | result | elapsed | stderr |
|---|---|---:|---:|
| `pwsh -NoLogo -NoProfile -File scripts/test-core-profile-gates.ps1 -Phase Core` | pass | 392.1 s | 0 bytes |
| `pwsh -NoLogo -NoProfile -File scripts/test-core-profile-gates.ps1 -Phase Extended` | pass | 241.2 s | 0 bytes |

Core passed deterministic plan/architecture, exact success, fast exit, command
failure, total deadline, complete tree ownership, six evidence-tamper cases and
the source/command/manifest seal. Extended passed both identity races, the
Windows handle allowlist, exact tool sealing, owned probe descendants, path
confinement, all 27 mutations and Cargo serialization (`max_wait_ms=1878`).

### Native WSL Linux x64 harness

The native C# harness links the same supervisor source and exercised the actual
Linux syscalls after the final libc and CLOEXEC assertions:

```text
bd23-linux-fd-bound: ok root=1113 source-ancestor=renamed symlink=hostile output=ORIGINAL retained=0 sentinel=1110:alive
bd23-linux-post-spawn-abort: ok child=1117:reaped fds=78:balanced sentinel=1110:alive probe=no-inheritance
```

This is a focused development proof, not the pinned Linux CI terminal
transcript.

### Exact terminal test phase

The first unified attempt preserved a useful test-harness failure: after
417,723 ms it reached the identity race, but its unrelated sentinel had
naturally completed a 30-second sleep before the gate-unique admission point.
Core was green; stderr was 350 bytes containing only that assertion. No owned
process remained. The sentinel bound was changed to 180 seconds while its
PID-scoped `finally` cleanup remained unchanged.

The exact rerun passed:

```text
pwsh -NoLogo -NoProfile -File scripts/test-core-profile-gates.ps1 -Phase All
EXIT_CODE=0
ELAPSED_MS=612329
STDERR_BYTES=0
core-profile-gates Cargo serialization: ok (max_wait_ms=2234)
test-core-profile-gates: ok (phase=All x64=1 exact-success=1 failures=1 timeouts=1 descendants=1 evidence-tamper=6 source-tool-seals=5 path-confinement=2 manifest-mutations=27 cargo-concurrency=2)
```

Post-run PID-scoped audits reported:

```text
WINDOWS_OWNED_PROCESS_MATCHES=0
WSL_OWNED_PROCESS_MATCHES=0
```

The All-phase identity assertion and the native Linux proof both confirmed that
their unrelated sentinels remained alive until the test-owned cleanup block.

## Status doctrine

This delivery bead closes the exact check/use-race residual for direct
Core-profile gate and tool-probe inputs. It advances no Windows COM/native,
language-service, VM3/JIT conformance or terminal environment row by itself.
The cross-platform Core baseline remains `in-progress` until the separately
owned pinned Linux and reconciled Windows/Linux terminal transcripts pass and
the canonical truth surfaces are updated by their owning beads.
