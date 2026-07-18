# CORE-1 Manifest-Declared Ambient Toolchain Descendants in the Gate Runner

Date: 2026-07-18

Bead: `bd-59co.2.2.39`

Clause: `CONF-QUALITY-001`, `SEC-BOUNDARY-001`

Matrix route: `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES`

## Result and claim boundary

The canonical Core-profile gate runner no longer fails a green gate when the
only surviving job members are manifest-declared ambient toolchain helpers.
After the `bd-59co.2.2.38` bounded drain window expires with residuals, the
runner now enumerates residual process identities and judges them: a residual
set made up only of declared ambient names is recorded in `result.json`
(`ambient_descendants`) and terminated through the existing kill-on-close /
pidfd-subreaper cleanup, and the gate keeps its direct exit code. Any other
surviving descendant — including an unresolvable `"pid:?"` image — still fails
closed with `descendant-processes-remained-after-direct-exit`.

This amends the absolute wording of
[`gate-runner-descendant-drain.md`](gate-runner-descendant-drain.md): a
survivor beyond the drain window is no longer automatically "a genuine leak";
it is judged by identity against the versioned declaration.

This is a runner-robustness delivery. It advances no Windows COM/native,
language-service, VM3/JIT conformance or terminal environment row. The
cross-platform Core baseline remains `in-progress` under `bd-59co.2.2.10`,
`.11` and `.12`; Linux applies the same manifest field by code parity with
platform verification owned by `bd-59co.2.2.11`.

## Characterization

`bd-59co.2.2.10` attempts 8 and 9 (2026-07-18) failed gates
`differential-default-parallel` and `meta-fast-no-artifacts` with
`descendant-processes-remained-after-direct-exit` even with the 3,000 ms drain
window. A process-lifecycle capture (250 ms whole-system polling across the
full run) identified the survivor unambiguously:

- `vctip.exe` (Visual C++ telemetry helper, VS 18 Insiders MSVC 14.51.36231),
  spawned by `link.exe` during rustc builds as a normal job member at build
  start (23:00:47) and still alive when the gate's cargo exited (23:09:30),
  i.e. more than 8 minutes — far beyond any reasonable drain window.
- `conhost.exe`, the OS console host, is a job residual whenever a console
  descendant survives, because it serves that descendant's console. In the
  captured canonical run it drained early; in fixture runs it persists with
  its client. Declaring it is required for the exemption to work in reality.

Both are ambient OS/toolchain infrastructure with no evidence interaction:
the gate's exit code was 0, every test passed, and no evidence bytes were
touched. Suppression probes on this host showed `VSCMD_SKIP_SENDTELEMETRY=1`
and `HKCU\SOFTWARE\Microsoft\VSCommon\18.0\SQM\OptIn=0` (reverted) do **not**
stop direct `link.exe` spawns on VS 18 Insiders; machine-level registry edits
or deleting the toolchain binary were rejected as hostile environment
mutations.

## Change

- `ci/core-profile/gates-v1.json`: versioned
  `supervision.ambient_descendant_names = ["vctip.exe", "conhost.exe"]`.
- `scripts/core-gate-process-supervisor.cs`:
  `OxVbaCoreGateWindowsJob.GetMemberImageNames()` enumerates job members via
  `JOBOBJECT_BASIC_PROCESS_ID_LIST` (info class 3, verified empirically on
  Windows x64 — class 2 is rejected with `ERROR_BAD_LENGTH`) and resolves
  images via `OpenProcess` + `QueryFullProcessImageNameW`;
  `OxVbaCoreGatePosixOwnedTree.GetLiveProcessNames()` is the Linux
  `/proc/<pid>/comm` parity surface. Unresolvable members are recorded as
  `"pid:?"`, which matches no declared name and therefore fails closed.
- `scripts/run-core-profile-gates.ps1`: schema, validation (present JSON
  array, at most 16 plain image names), `Test-AllAmbientDescendants`, and the
  post-drain judgment branch in both polling loops. Windows terminates the
  accepted residue explicitly; Linux terminates it through `TerminateAll`.
  `ambient_descendants` is recorded in every gate `result.json` and in
  non-executed result rows for schema uniformity.
- `scripts/test-core-profile-gates.ps1`: new `ambient-descendant` case (a 30 s
  sleeper named `vctip.exe` with its real `conhost.exe` console host passes
  with the identity recorded and the process terminated) and three new
  manifest mutations (scalar names, extensionless name, 17 names) for a total
  of 30.

## Name-only matching trade-off

Residual judgment matches executable image names, not signed identities: a
renamed binary called `vctip.exe` or `conhost.exe` would be accepted. This is
a reviewed, recorded trade-off — every accepted residual is recorded with its
full `pid:path` in the gate `result.json`, the declared name list is versioned
in the manifest, and the runner still fails closed on any undeclared or
unresolvable image. The ambient set is deliberately limited to the two names
observed as ordinary toolchain/OS infrastructure on the certification
environments.

## Verification

`pwsh -NoLogo -NoProfile -File scripts/test-core-profile-gates.ps1 -Phase All`
passed in 939,530 ms with zero stderr and no owned-process residue:

```text
test-core-profile-gates: ok (phase=All x64=1 exact-success=1 failures=1 timeouts=1 descendants=2 ambient-descendants=1 evidence-tamper=6 source-tool-seals=5 path-confinement=2 manifest-mutations=30 cargo-concurrency=2)
```

## Review

Fresh-eyes non-author review (explore subagent, 2026-07-18) returned one
blocking finding — the initial `JOBOBJECT_BASIC_PROCESS_ID_LIST` info class
(2) is rejected with `ERROR_BAD_LENGTH`, deadening both the new accept path
and the pre-existing trip — which was repaired to class 3 and re-verified
end-to-end before the accepted All-phase run. Its should-fix findings were all
addressed in the accepted change: the `conhost.exe` declaration is recorded
here and in the bead/CSV truth surfaces; the bead record carries proper
acceptance criteria; stale drain-window comments were amended; Windows
unresolvable members emit `"pid:?"` for Linux parity and cannot hot-spin the
poll loop; `New-NonExecutedResult` carries `ambient_descendants`; three
manifest mutations cover the new validation; this artifact exists.

## Residuals

- Linux behavior is code-parity only until `bd-59co.2.2.11` executes the
  pinned Linux x64 CI baseline.
- A gate leaking a renamed binary matching a declared name would pass on the
  name-only judgment; the full `pid:path` record is the audit trail. Tightening
  to signed path prefixes is a possible follow-up but was not required to
  bound the observed toolchain residue.
