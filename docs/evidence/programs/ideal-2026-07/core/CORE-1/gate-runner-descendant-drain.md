# CORE-1 Bounded Gate-Runner Descendant Drain Window

Date: 2026-07-18

Bead: `bd-59co.2.2.38`

Clause: `CONF-QUALITY-001`, `SEC-BOUNDARY-001`

Matrix route: `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES`

## Result and claim boundary

The canonical Core-profile gate runner no longer trips on ordinary
process-tree teardown after a successful direct child. Both polling loops
(Windows Job `ActiveProcesses`, Linux owned-tree `LiveProcessCount`) now allow
a bounded 3,000 ms drain window, inside the existing execution cutoff, before
declaring `descendant-processes-remained-after-direct-exit`. A descendant that
survives beyond the window still fails closed with the unchanged reason string
and the unchanged kill-on-close / pidfd-subreaper cleanup.

This is a runner-robustness delivery. It does not weaken any containment
property: persistent descendants still trip and are killed, escaped-descendant
detection is untouched, and total gate deadlines are unchanged. It advances no
Windows COM/native, language-service, VM3/JIT conformance or terminal
environment row. The cross-platform Core baseline remains `in-progress` under
`bd-59co.2.2.10`, `.11` and `.12`; the Linux drain logic changes identically
but is verified here only by code parity, with platform verification owned by
`bd-59co.2.2.11`.

## Characterization

Found by `bd-59co.2.2.10` attempt 6 (2026-07-18): gate
`differential-default-parallel` failed with
`descendant-processes-remained-after-direct-exit` after a fully green suite
(exit code 0, 804 s). Investigation:

- A Job-context probe of the identical gate shape (same sealed cargo, same
  supervisor, same owned-file stdout/stderr) completed clean with `active=1`
  draining in under 100 ms after direct exit.
- A microprobe of `cargo --version` (two-process rustup-proxy chain)
  reproduced `active=1` for 17 ms after direct exit: the sealed proxy exits
  while the toolchain cargo is still tearing down.
- No crash, Windows Error Reporting report, escaped descendant, or persistent
  process exists after drain. The observed residue is ordinary out-of-order
  teardown of the rustup-proxy/toolchain chain; the runner's fixed 100 ms
  grace was intermittently insufficient on a loaded development host.

## Change

- `scripts/run-core-profile-gates.ps1`: new `$script:DescendantDrainMs = 3000`
  replaces the hardcoded 100 ms grace in both loops. The drain wait remains
  inside the `executionCutoffMs` polling loop, so a direct exit near the
  cutoff still degrades to `total-deadline-exceeded` with cleanup; deadline
  semantics are unchanged.
- `scripts/test-core-profile-gates.ps1`: new Core-phase case
  `descendant-drain` — a grandchild that exits 500 ms after its direct parent
  (beyond the old 100 ms grace, inside the new window) now passes and is gone
  afterward. The persistent-descendant trip is retained; the descendant and
  Linux escaped-session fixture gate timeouts were raised 5 s to 10 s because
  the 3,000 ms drain must fit inside their execution cutoffs; the retained
  retained-pipe and descendant duration bounds were adjusted for the same
  arithmetic; the All-phase descendant counter is 2 on Windows and 3 on Linux.
- `docs/validation/IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv`: one traceability
  row for this bead.

## Verification

`pwsh -NoLogo -NoProfile -File scripts/test-core-profile-gates.ps1 -Phase All`
passed in 902,926 ms with zero stderr and no owned-process residue:

```text
test-core-profile-gates: ok (phase=All x64=1 exact-success=1 failures=1 timeouts=1 descendants=2 evidence-tamper=6 source-tool-seals=5 path-confinement=2 manifest-mutations=27 cargo-concurrency=2)
```

The new `descendant-drain` case proves a short-lived descendant is accepted;
the retained persistent-descendant case proves a 30 s grandchild still trips
with `descendant-processes-remained-after-direct-exit` and is killed; all 27
manifest mutations, six evidence-tamper cases, source/tool seals, path
confinement and Cargo serialization are unchanged and green.

One Extended-phase identity-race fixture failed once under parallel agent load
(the admission pause did not fire and the run completed instead); the same
exact All phase passed on an unloaded rerun. The identity path was not
modified by this bead.

## Review

Fresh-eyes non-author review (explore subagent, 2026-07-18) returned
changes-required with one blocking finding: the persistent-descendant and
escaped-session fixture gate timeouts (5 s) could not contain the 3,000 ms
drain inside their 4.5 s execution cutoffs. The timeouts were raised to 10 s
before the accepted All-phase run; all other checks (constant usage in both
loops, deadline semantics, drain-test discrimination, CSV schema, earlier
digest/fmt commits) passed clean.

## Residuals

- Linux drain behavior is code-parity only until `bd-59co.2.2.11` executes the
  pinned Linux x64 CI baseline.
- The Extended identity-race flake under load is environmental; if it recurs
  on an unloaded host it receives its own exact successor.
