# JIT M4-0 Baseline Evidence

Date: 2026-07-03

## Environment

- Host: Linux dna-koderbot 6.17.0-19-generic x86_64
- Rust: `rustc 1.95.0 (59807616e 2026-04-14)`
- Cargo: `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- Beads tool: `br 0.2.16`

## Criterion Baseline

Command:

```text
cargo bench -p oxvba-differential --bench jit_m4_baseline -- --quiet
```

Final run notes:

- Every `vm3_execution_precompiled_oxir` fixture includes a per-iteration live-handle balance assertion.
- The COM fixture uses a fixed-count portable dispatch pair: `CallByName(obj, "Count", VbGet)` for the late route and direct `obj.Count` for the early route against the controlled `OxVba.TestDispatch` typelib fixture.
- Compile and image-load groups intentionally measure fast-path operations and are not scaled to the 50-200 ms VM execution band.

### VM3 Execution, Precompiled OxIR

| fixture | time |
|---|---:|
| `scalar_loop` | [62.918 ms 64.275 ms 66.429 ms] |
| `string_concat` | [73.769 ms 75.089 ms 77.712 ms] |
| `array_loop` | [55.606 ms 56.270 ms 56.703 ms] |
| `udt_fields` | [52.476 ms 53.045 ms 53.676 ms] |
| `call_overhead` | [61.518 ms 62.675 ms 63.710 ms] |
| `error_loop` | [50.798 ms 53.898 ms 56.524 ms] |
| `collection_ops` | [53.642 ms 53.955 ms 54.221 ms] |
| `com_late_vs_early` | [54.390 ms 55.143 ms 55.866 ms] |

### Source To OxIR Compile

| fixture | time |
|---|---:|
| `scalar_loop` | [139.91 us 145.99 us 152.29 us] |
| `string_concat` | [168.29 us 171.15 us 173.91 us] |
| `array_loop` | [211.12 us 214.69 us 218.44 us] |
| `udt_fields` | [195.70 us 196.89 us 198.48 us] |
| `call_overhead` | [198.49 us 199.99 us 202.04 us] |
| `error_loop` | [139.52 us 141.80 us 143.42 us] |
| `collection_ops` | [149.27 us 152.47 us 156.33 us] |
| `com_late_vs_early` | [233.76 us 238.17 us 243.71 us] |

### Image Load

| fixture | time |
|---|---:|
| `image_load_json_parse/from_bytes_validate` | [33.179 us 34.519 us 36.040 us] |

## Corpus Wall Clock

Warm command:

```text
/usr/bin/time -p cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture
```

Result:

- `vm3_golden_snapshot`: passed.
- test body: `0.31s`
- wall clock: `real 0.62`, `user 0.40`, `sys 0.23`

The post-change golden snapshot remained stable after adding the portable projection-name lowering path.

## NumericMode Census

Artifacts:

- `docs/evidence/perf/JIT_M4_NUMERIC_MODE_CENSUS_20260703.md`
- `docs/evidence/perf/JIT_M4_NUMERIC_MODE_CENSUS_20260703.csv`

Summary:

| metric | value |
|---|---:|
| programs elaborated | 263 |
| failed inputs | 66 |
| checked arithmetic ops | 67 |
| widening arithmetic ops | 126 |
| checked arithmetic share | 34.72% |
| widening arithmetic share | 65.28% |
| compare ops | 120 |

Follow-up bead filed from census headroom:

- `bd-h4oh.17` - Increase NumericMode Checked coverage for provable fixed numeric lanes.

## Backend Selection

Commands:

```text
cargo run -q -p oxvba-cli --bin oxvba-cli -- run conformance/tests/smoke.bas --dump-values --backend vm3
cargo run -q -p oxvba-cli --bin oxvba-cli -- run conformance/tests/smoke.bas --backend jit
cargo test -p oxvba-cli parse_run_args -- --nocapture
cargo test -p oxvba-host --test jit_m4_com_projection -- --nocapture
cargo test --workspace --no-run
```

Results:

- `--backend vm3`: exit 0, `VALUES:i16:15`
- `--backend jit`: exit 1, `RUN-E-JIT-NOT-IMPLEMENTED`
- CLI backend parser: passed, including rejection of the noncanonical `--backend vm` alias.
- Portable COM projection regression: passed; `CallByName(obj, "Count", VbGet)` lowers through fixture typelib metadata and matches direct `obj.Count` under the Windows-headless runtime profile.
- Workspace no-run gate: passed after tightening Linux test-target cfg for fixture COM tests (`oxvba-com` catalog tests and `oxvba-hal` Windows-only raw COM helpers).

## Formal Lane Status

Commands:

```text
bash scripts/run-miri.sh
MIRIFLAGS=-Zmiri-disable-isolation bash scripts/run-miri.sh
cargo +nightly miri test -p oxvba-jit -- --nocapture
```

Results:

- Initial broad Miri run failed at setup because nightly `miri` was missing; installed with `rustup +nightly component add miri`.
- Broad runtime Miri with default isolation failed in `proptest` setup on isolated `getcwd`.
- Broad runtime Miri with `-Zmiri-disable-isolation` progressed into runtime proptests but was interrupted after several minutes in proptest RNG; no OxVBA assertion failure was observed before interruption.
- Focused `oxvba-jit` Miri lane passed: `jit_api_reports_not_implemented`.

Formal failures remain non-blocking under the current ladder policy; the unresolved broad-runtime Miri cost is recorded here rather than hidden.
