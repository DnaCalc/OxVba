# HAL Time Semantics V1

Status: `working-draft`  
Step: `v194`  
Date: 2026-03-02

## Objective

Complete the date/time integration contract from VBA intrinsic surface through HAL and host behavior.

## Surface

VBA intrinsics:
- `Date()`
- `Time()`
- `Now()`
- `Timer()`

HAL operations:
- `date_serial_now()`
- `time_serial_now()`
- `timer_ticks()`

## Current Contract Baseline

Deterministic lanes:
- fixed deterministic token outputs for date/time/timer.

Host-backed lanes (host-matching, non-deterministic policy):
- system-time-derived token outputs are permitted.

## Required Completion Items

1. `Now()` representation:
- current runtime projects `Now()` to date token in the current value-token model.
- target: explicit combined date-time semantics, with deterministic and host-backed definitions.

2. Timezone/locale determinism:
- deterministic lanes must not vary with host timezone/locale.
- host-backed lanes may vary, but error shape and token domain constraints remain stable.

3. `Timer()` stability:
- define reset/wrap contract and day-boundary behavior expectations.

## Clause Candidates

| Clause | Statement | Verification Layer |
|---|---|---|
| `HAL-TIME-V1-001` | Deterministic lanes return fixed contract values for date/time/timer. | HAL tests/conformance |
| `HAL-TIME-V1-002` | Host-backed lanes return non-negative system-derived values. | HAL conformance |
| `HAL-TIME-V1-003` | `Now()` combined semantics are explicit and non-ambiguous. | compiler/vm/host integration |
| `HAL-TIME-V1-004` | Runtime error handling for unsupported time capability is stable. | host integration |

## Planned Implementation Direction

- retain `Date/Time/Timer` host-intrinsic path via `TimeLocaleHal`.
- introduce explicit representation path for `Now()` in runtime value model.
- keep deterministic policy controls as first-class gate for reproducible runs.

## Open Items

- exact encoding for combined date-time at current VM boundary.
- locale-sensitive formatting hooks versus pure numeric serial semantics.
