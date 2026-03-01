# PROFILE_LADDER_2026-03-01_MACH1000_V167_V186_NON_HAL_HARDENING.md

## Objective

Second-batch non-HAL run after `v166`:
- harden semantics implemented in `v147..v166`,
- improve performance and formal coverage on completed non-HAL surfaces,
- prepare clean foldback interface to future oracle/HAL phases.

## Prerequisite

- `v166` terminal gate passed from:
  - `docs/worksets/PROFILE_LADDER_2026-03-01_MACH1000_V147_V166_NON_HAL_COMPLETION.md`

## Profiles

| Profile | Focus | Primary Deliverables |
|---|---|---|
| `v167` | Post-completion audit | Enumerate any residual non-HAL partials and enforce closure or explicit deferral. |
| `v168` | Runtime perf instrumentation | Add benchmarks around newly concrete `Err`, string, and financial paths. |
| `v169` | Financial hot-path perf | Optimize financial intrinsic implementations while preserving deterministic tolerance behavior. |
| `v170` | String path perf | Optimize string/sentinel code paths and verify no semantic regressions. |
| `v171` | Coercion matrix hardening | Expand coercion matrix tests for newly supported non-HAL edge paths. |
| `v172` | Error-model hardening | Stress-test nested error mode transitions and `Err` state interactions. |
| `v173` | JIT lowering robustness | Improve JIT lowering/fallback robustness for new operations. |
| `v174` | Differential scaffolding prep | Prepare reusable oracle probe harness scaffolding (without running oracle-dependent gates). |
| `v175` | Formal lane expansion I | Add new Kani/Lean obligations where practical for completion-phase semantics. |
| `v176` | Formal lane expansion II | Resolve moderate-complexity formal failures or defer with explicit unblock paths. |
| `v177` | Documentation normalization | Align architecture/testing docs with completed non-HAL semantics. |
| `v178` | Coverage matrix normalization | Ensure checklist/indexes contain no stale notes from removed projection subsets. |
| `v179` | Regression corpus growth | Add regression fixtures from `v147..v178` bug discoveries. |
| `v180` | Integrated perf gate | Run benchmark + conformance gate and publish trends vs `v166`. |
| `v181` | Integrated correctness gate | Full VM/JIT + matrix + docs checks pass with updated evidence. |
| `v182` | Deferred-oracle hygiene | Verify deferred-oracle list remains complete and deduplicated. |
| `v183` | Divergence hygiene | Update divergence records for any intentional non-HAL behavior deltas. |
| `v184` | Terminal stabilization pass | Final bugfix/stability sweep for second batch scope. |
| `v185` | Release-candidate gate | End-to-end non-HAL candidate gate with artifacts. |
| `v186` | Batch-2 closure | Publish terminal status and handoff packet to oracle/HAL phases. |

## Policy

- Same non-blocking formal policy as earlier ladders unless unsoundness risk appears.
- No HAL-adjacent expansion in this ladder.
- Oracle probes may be scaffolded but do not block closure.
