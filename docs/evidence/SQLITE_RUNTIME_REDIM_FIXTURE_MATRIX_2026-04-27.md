# SQLite Runtime ReDim Fixture Matrix

Date: 2026-04-27
Beads: `bd-sql1.16.4`, `bd-sql1.16`

## Purpose

Publish the post-`ReDim buf(length - 1)` SQLiteForExcel fixture boundary after
the runtime-sized dynamic-array base-slot and byte-buffer bridge slices were
validated.

## Matrix

| Row | Command | Result | Boundary |
| --- | --- | --- | --- |
| Host integration matrix | `cargo test -p oxvba-host --test sqliteforexcel_declare_integration -- --nocapture` | pass, 6/6 | Normalized core and demo compile past the old runtime-sized `ReDim` boundary; bounded normalized demo completes in VM and JIT. |
| Raw demo, CLI VM | `cargo run -p oxvba-cli -- run-project .external\sqliteforexcel\fixtures\Demo64\SQLiteForExcelDemo64.basproj` | pass | Raw `_64` fixture reaches `----- All Tests Complete -----`, including full `TestBinding` 100k-loop evidence. |
| Raw demo, CLI JIT | `cargo run -p oxvba-cli -- run-project .external\sqliteforexcel\fixtures\Demo64\SQLiteForExcelDemo64.basproj --jit` | pass | Raw `_64` fixture reaches `----- All Tests Complete -----` in JIT, including full `TestBinding` 100k-loop evidence. |
| Bounded normalized demo, CLI VM | `cargo run -p oxvba-cli -- run-project .external\sqliteforexcel\fixtures\Demo64NormalizedBounded\SQLiteForExcelDemo64NormalizedBounded.basproj --allow-filesystem-mutation true` | pass | Bounded normalized fixture reaches `----- All Tests Complete -----`. |
| Bounded normalized demo, CLI JIT | `cargo run -p oxvba-cli -- run-project .external\sqliteforexcel\fixtures\Demo64NormalizedBounded\SQLiteForExcelDemo64NormalizedBounded.basproj --allow-filesystem-mutation true --jit` | pass | Bounded normalized fixture reaches `----- All Tests Complete -----` in JIT. |

## Noise Row

A first bounded normalized CLI VM attempt was launched while the raw CLI VM row
was still executing. It failed in `Kill ...` with Windows `os error 32` because
both runs used the shared temp database path. The same row passed when rerun
sequentially, so this is the already-documented fixture concurrency noise, not
a language/runtime boundary.

## Closure

The scoped `bd-sql1.16` runtime-sized `ReDim` lane is complete. The old
compile-time boundary at `ReDim buf(length - 1)` no longer remains for the raw
or normalized SQLiteForExcel fixture rows covered by this lane.
