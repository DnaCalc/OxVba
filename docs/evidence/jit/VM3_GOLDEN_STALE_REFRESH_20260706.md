# VM3 Golden Stale Corpus Refresh

Date: 2026-07-06

Scope: `bd-h4oh.9.2` support refresh for `crates/oxvba-differential/vm3_golden.snap`.

## Trigger

`cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture` exposed a
stale first drift at
`conformance/jit_v2/tracer_bullets/tb05_safearray_foreach_bounds.bas`. The
snapshot expected success for `For Each v As Long In a()` over a typed array, but
the current binder emits `For Each control variable must be Variant or Object`.

## Compatibility Check

The diagnostic matches Microsoft VBA documentation:
`https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/for-each-control-variable-must-be-variant-or-object`.

That page states that array `For Each` control variables must be `Variant`.

## Staleness Check

A clean detached HEAD worktree at `a9f38990` reproduced the same first golden
drift before the M4-7 record-layout changes:

`CARGO_TARGET_DIR=/tmp/oxvba-head-target cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture`

## Checks

- `OXVBA_BLESS_GOLDEN=1 cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture`
- `cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture`

This is a support-only snapshot truth repair. It does not close M4-7 record
layout behavior by itself.
