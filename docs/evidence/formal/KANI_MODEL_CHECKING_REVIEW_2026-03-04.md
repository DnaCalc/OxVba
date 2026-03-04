# Kani Model Checking Review Cycle (2026-03-04)

## Scope

This review cycle covers both:
- model-checking value assessment (what is worth keeping strict), and
- remote-runner plumbing hardening (resource-aware monitoring/guarding).

Commands used:
- `./scripts/run-formal-kani-remote.ps1 -Action Status`
- `./scripts/run-formal-kani-remote.ps1 -Action ProbeCapacity`
- `./scripts/run-formal-kani-remote.ps1 -Action Tail -Lane <lane>`
- `rg`/`Get-Content` over `DEFERRED_GATES.md`, `EXTENDED_TODO.md`, `latest_run.md`

## Baseline Snapshot

At review start:
- remote host probe: `cpu=16`, `mem_available_gib=18`, `recommended_concurrency=1`
- active heavy lanes: `v89-kani`, `v89-kani-extra`
- recent strict lane outcomes:
  - `v87-kani`: fail (`selected=4`, `failures=2`, `timeouts=1`)
  - `v88-kani`: fail (`selected=4`, `failures=2`, `timeouts=1`, explicit CBMC OOM message)
  - `v146-kani`: fail (`selected=4`, `failures=2`, `timeouts=2`)
  - `v287-kani-pinned-fast`: fail overall (`selected=3`, two timeouts, one pass)
- deferred-gate register counts: `dg-folded=13`, `dg-pass=3`, `dg-fail=1`, `dg-deferred=3`, `dg-not-started=25`

Observed failure class is predominantly resource/state-space blow-up (timeouts/OOM), not consistent semantic counterexamples.

## Have Any Kani Checks Succeeded?

Yes.

Strict Kani pass evidence exists in historical lanes and foldbacks (for example `DG-V67-001` .. `DG-V78-001` as `dg-folded` pass in `DEFERRED_GATES.md`).

Recent runs are failure-heavy because the active queue currently hits high-complexity obligations and some runner-selection mismatch/no-op lanes, not because Kani is uniformly failing on all obligations.

## Value Decision Framework (Applied)

Decision rubric used:
1. Keep strict: obligation targets safety/property that tests cannot cheaply guarantee.
2. Demote/slice: obligation repeatedly times out/OOM without actionable counterexample.
3. Keep as smoke: obligation is environment/toolchain liveness only.

Output artifact:
- `docs/evidence/formal/KANI_OBLIGATION_POLICY_V1.csv`

This policy classifies all active `cargo kani` obligations into `high|medium|low` signal tiers and defines next action per obligation.

## Plumbing Changes Implemented

Primary file:
- `scripts/run-formal-kani-remote.ps1`

### 1) Resource telemetry
- Added remote `resource_snapshot.sh` (installed by `-Action Ensure`).
- `Status` now emits:
  - memory used percent,
  - swap/load,
  - active `cbmc`/`kani` process counts,
  - top memory-consuming Kani/CBMC processes.

### 2) Active monitor action
- Added `-Action Monitor` with looping controls:
  - `-MonitorDurationSeconds`
  - `-MonitorIntervalSeconds`
  - `-MonitorAutoResume`
- Supports pressure response by threshold/action.

### 3) Memory-pressure guardrails in queue dispatch
- `StartDeferred` now accepts and forwards:
  - `-MemorySoftUsedPercent` (default 85)
  - `-MemoryHardUsedPercent` (default 92)
  - `-HardPressureAction` (`pause|halt-one|halt-all|none`, default `pause`)
- Dispatch now:
  - pauses new lane starts when above soft threshold,
  - applies configured hard action above hard threshold,
  - uses pause flags (`PAUSE_NEW_LANES.auto`, optional manual flag),
  - records guard actions in dispatch logs.

### 4) Validation and governance scaffold
- Added `scripts/validate-kani-obligation-policy.ps1`.
- Wired it into `scripts/meta-check.ps1`.
- This enforces policy coverage and drift detection for all active Kani obligations.

### 5) Live validation samples (runner side)
- `Status` now returns resource telemetry and top memory consumers.
- Forced-threshold monitor sample (`soft=65`, `hard=68`) correctly emitted:
  - `monitor_action=pause`
  - `pause_flag_auto=present`
- Recovery sample (`soft=85`, `hard=92`, auto-resume enabled) correctly emitted:
  - `monitor_action=resume`
  - `pause_flag_auto=absent`

### 5) Documentation updates
- Updated:
  - `docs/evidence/formal/REMOTE_KANI_RUNNER.md`
  - `scripts/README.md`
  - `docs/evidence/formal/EXTENDED_TODO.md`

## Outstanding Risks / Next Pass

1. No-op/mismatch lanes still require explicit selection-preflight logic (tracked in `FTODO-KANI-REVIEW-001`).
2. High-signal heavy harnesses still need slicing (`FTODO-KANI-REVIEW-002`).
3. New memory guard defaults need burn-in/tuning on live queue load (`FTODO-KANI-REVIEW-003`).

## Deferred Register After Reconciliation

After this cycle's register refresh:
- `dg-folded=13`
- `dg-pass=3`
- `dg-fail=4`
- `dg-running=1`
- `dg-deferred=3`
- `dg-not-started=21`

## Recommended Operating Policy (Current Host)

- Use `recommended_concurrency` from probe unless strong evidence says otherwise.
- On this host, keep `DeferredConcurrency=1` as default for heavy lanes.
- Run monitor loop during long batches:

```powershell
./scripts/run-formal-kani-remote.ps1 `
  -Action Monitor `
  -MonitorDurationSeconds 600 `
  -MonitorIntervalSeconds 30 `
  -MemorySoftUsedPercent 85 `
  -MemoryHardUsedPercent 92 `
  -HardPressureAction pause `
  -MonitorAutoResume $true
```
