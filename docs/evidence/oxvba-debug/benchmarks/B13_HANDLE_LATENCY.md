# B13 oxvba-debug handle latency benchmark summary

Command: `cargo bench -p oxvba-debug --features bench --bench handle_latency -- --quiet`

Date: 2026-05-23

Environment note: run on the local Windows development machine; Criterion used the plotters backend because `gnuplot` was not installed. `target/criterion/` remains build output and is not committed.

## Criterion summary

| Benchmark | Mean interval |
|---|---:|
| `debug_handle_latency/step_into_round_trip` | `[2.0672 ms 2.1682 ms 2.4052 ms]` |
| `debug_handle_latency/set_source_breakpoint_round_trip` | `[2.0980 ms 2.2573 ms 2.5170 ms]` |
| `debug_handle_latency/evaluate_watches_round_trip` | `[2.0802 ms 2.0880 ms 2.0974 ms]` |

## Stress stability evidence

Focused stress lane was run three times without flake:

`cargo test -p oxvba-debug --test stress_attach_detach --test stress_concurrent_sessions --test stress_sequential_commands --test bench_handle_latency`

Covered tests:
- `one_hundred_sequential_attach_detach_cycles_stable`
- `one_hundred_concurrent_sessions_complete_without_crosstalk_or_leak`
- `one_thousand_sequential_commands_stay_bounded`
- benchmark catalog mirror tests for step-into, set-breakpoint, and evaluate-watches round trips
