# PMR DG-V287 Foldback (2026-03-03)

Scope: `DG-V287-001` (`FO-V287-001..003`)

## Lane

- Runner: remote Kani (`/home/ubuntu/.dnacalc_remote`)
- Pinned workspace: `/home/ubuntu/.dnacalc_remote/work/OxVba_v287_pinned`
- Lane: `v287-kani-pinned-fast`
- Job: `20260303T201117Z_manual-v287-kani-pinned-fast`
- Mode: `exact`
- Timeout policy: `600s`, retries `0`

## Selection and outcome

- `selected_count=3`
- `status=fail`
- `failures=2`
- `timeouts=2`

| Obligation | Result | Notes |
|---|---|---|
| `FO-V287-001` | `timeout` | host PMR harness timed out at 600s while deep in CBMC unwind (`memcmp` loop) |
| `FO-V287-002` | `timeout` | host PMR harness timed out at 600s while deep in CBMC unwind (`memcmp` loop) |
| `FO-V287-003` | `pass` | HAL dynlink contract harness verified successfully |

## Decision

- Rewrite-bridge fallback is **not retired**.
- `compile_project` keeps module-aware path as default with rewrite fallback retained.
- Retirement criterion remains: `DG-V287-001` must fold back without unresolved regressions/timeouts on required PMR obligations.

## Next actions

1. Keep `DG-V287-001` open as formal follow-up (`timeout` class).
2. Run a dedicated host-harness strategy pass (harness slicing/assumptions/bounds) for `FO-V287-001/002`.
3. Re-run remote `v287` lane and re-evaluate bridge retirement only after host PMR harnesses clear.
