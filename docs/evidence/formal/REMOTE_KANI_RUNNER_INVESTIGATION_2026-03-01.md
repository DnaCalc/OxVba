# Remote Kani Runner Investigation (2026-03-01)

## Scope

Investigate why remote deferred Kani appears slow (low visible queue progress over many hours), verify whether the runner is healthy, and propose next-step strategy.

Target host/context:

- Host: `ubuntu@94.72.99.81`
- Isolated path: `/home/ubuntu/.dnacalc_remote` (no writes outside this path)
- Control script: `scripts/run-formal-kani-remote.ps1`

## Probe Snapshot

### Queue and dispatch state

From `./scripts/run-formal-kani-remote.ps1 -Action Status`:

- Jobs:
  - `20260228T221909Z_lane-smoke-v81` running
  - `20260228T222237Z_deferred-dispatch` running
- Deferred lanes:
  - `v81-kani` running
  - `v82-kani` running
- Dispatch meta:
  - `mode=cumulative`
  - `recommended_concurrency=2`
  - `concurrency=1` (configured)
  - versions list contains `81 82 83 87 ... 106`
  - remote repo commit pinned in this dispatch:
    - `1f99b897fe1528fbd4aec7adc491e2afe6c65986` (old v106-era commit)

Dispatch log currently has only:

- `skip lane=v81-kani reason=already-running`
- `started lane=v82-kani pid=...`

### Capacity probe

From `./scripts/run-formal-kani-remote.ps1 -Action ProbeCapacity`:

- `cpu=16`
- `mem_gib=49`
- `recommended_concurrency=2`

### Process-level liveness

Remote process snapshot:

- Two `cbmc` processes each at ~`99.9%` CPU.
- Associated commands:
  - both are solving harness:
    - `pc_progression_is_safe_for_valid_jump_target`
  - one under `v81-kani` target dir
  - one under `v82-kani` target dir
- Memory: each `cbmc` about `8.6-8.7%` of system RAM (roughly ~5.3 GiB each on 62 GiB host).
- System load: ~`2.x` on 16 cores, matching ~2 fully busy workers (~13% host CPU).

30-second /proc delta probe on both `cbmc` PIDs:

- `delta_utime=2999` ticks over 30s for each process
- `delta_stime=1`
- RSS pages increased

Interpretation: both solver processes are actively progressing (not hung).

### Artifact/log behavior

- `state/deferred_lanes/v82-kani/run.log` exists but remains `0` bytes.
- `state/jobs/.../run.log` for active Kani jobs also `0` bytes.
- Target build artifacts under
  - `work/targets/v81-kani/...`
  - `work/targets/v82-kani/...`
  are present and large (~220 MiB per target tree).

Reason:

- `run_formal_lane.py` uses `subprocess.run(..., stdout=PIPE, stderr=STDOUT)` and writes logs only after each command completes.
- During long Kani/CBMC commands, there is effectively no incremental log output in lane `run.log`.

## Root-Cause Findings

1. Runner is healthy but opaque.
- The workload is running; low visible progress is mostly observability deficiency.

2. Current queue shape is inefficient.
- Active deferred mode is `cumulative`.
- Active Kani obligations currently are only the early set (`v2..v4`) in the obligations manifest.
- Therefore `v81` and `v82` cumulative lanes execute the same Kani command sequence, duplicating expensive solves.

3. Dispatch parallelism is constrained.
- Dispatch set to `concurrency=1`; one lane at a time from dispatcher.
- An externally started lane (`v81`) plus dispatch lane (`v82`) yields 2 concurrent solvers total.

4. Remote run is on an older repo commit.
- Current dispatch synced to `1f99b89` (v106-era), not current local head.
- This may not match current deferred-gate intent (v120+/v146 tracking).

## Is This Expected?

Partly yes, partly no:

- Yes: CBMC on certain harnesses can run for hours and consume a full core each.
- No: repeated cumulative-lane duplication + buffered-only logs + old-commit queueing is not an efficient strategy for the current deferred set.

## Recommendations

### Immediate (high impact)

1. Reshape execution model from per-version cumulative Kani to deduplicated command runs.
- Run each unique Kani command once per commit/toolchain tuple.
- Map results back to all profiles that include that obligation.
- This removes repeated solving across `v81/v82/...`.

2. For per-version lanes, use `exact` mode unless a specific cumulative audit is needed.
- Current active Kani obligations are early-profile only; later exact lanes would skip quickly.
- If cumulative evidence is required, run one cumulative lane at highest target version (not one per version).

3. Run against current commit before launching long deferred batch.
- Ensure dispatch syncs to latest intended gate commit (currently local work is past v106).

### Observability (must-have)

4. Add heartbeat/progress files per lane.
- Persist:
  - current obligation id
  - command start time
  - elapsed seconds
  - completed count / selected count
  - last-output timestamp/bytes

5. Stream logs incrementally.
- Replace `subprocess.run(..., stdout=PIPE)` with streamed line write to `run.log` plus per-obligation log.
- Even if CBMC is quiet, heartbeat still proves progress.

6. Add per-obligation timeout and classified outcome.
- e.g. `timeout` -> `deferred-timeout` with automatic continuation to next item.
- Prevent single harness from monopolizing lane indefinitely.

### Capacity/pacing

7. Increase concurrency cautiously after dedup + observability.
- Current host can likely handle >2 workers (memory headroom exists).
- Start with `3` workers; if stable, test `4`.
- Without dedup, higher concurrency just burns more CPU on duplicated work.

8. Share build cache where possible.
- Per-lane `CARGO_TARGET_DIR` duplicates compilation outputs.
- Use shared target dir for identical commit/toolchain/harness cohorts.

## Practical Next Step Proposal

1. Do not assume stall; classify current run as `running-but-opaque-and-duplicative`.
2. Introduce dedup + heartbeat update in remote runner scripts.
3. Restart deferred Kani with:
- latest commit sync
- deduplicated command queue
- concurrency `3` initial
- timeout policy enabled
4. Keep `Status` polling cadence (10 min) but report per-obligation heartbeat, not only process liveness.
