# V0.2 Date-String VM/JIT/Host/Conformance Evidence

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.4.4`
Parent: `bd-bqm8.4`
Status: complete

## Delivered Evidence

- Added `conformance/tests/stdlib_date_string_policy.bas`.
- Added golden expectation:
  `stdlib_date_string_policy.bas, ok, 2000,1,1,1,0`.
- The fixture covers:
  - `Year(DateValue("1 Jan 2000"))`
  - `Month(CDate("Jan. 1, 2000"))`
  - `Day(DateValue("January 1, 2000"))`
  - `IsDate("1 Jan 2000")`
  - `IsDate("February 30, 2000")`

## Verification

Passed:

- `cargo run -q -p oxvba-cli -- run conformance/tests/stdlib_date_string_policy.bas --dump-slots`
- `cargo run -q -p oxvba-cli -- run conformance/tests/stdlib_date_string_policy.bas --dump-slots --jit`
- `./scripts/run-conformance.ps1 -Backend vm -IncludePattern stdlib_date_string_policy.bas`
- `./scripts/run-conformance.ps1 -Backend jit -IncludePattern stdlib_date_string_policy.bas`
- `cargo test -p oxvba-host formal_v50_isdate_string_policy_subset --lib`

## Boundary

This evidence covers the accepted V0.2 grammar and deterministic invalid-date
behavior. It does not claim the explicitly unsupported locale-sensitive,
two-digit-year, time-suffix, or localized-month rows.
