# V0.2 Date-String Parser Implementation

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.4.3`
Parent: `bd-bqm8.4`
Status: complete

## Delivered

- Shared string-date parsing now validates calendar correctness before
  returning a packed date.
- `DateValue`, `CDate`, and `IsDate` therefore share the accepted grammar plus
  leap-year/day-of-month validation.
- `IsDate("1 Jan 2000")` now succeeds through the same parser used by
  `DateValue` and `CDate`.
- `IsDate("February 30, 2000")` and `DateValue("February 30, 2000")` reject
  deterministically instead of accepting a shape-only match.

## Code

- `crates/oxvba-vm/src/semantics.rs`
  - `parse_string_date_to_packed` now calls `packed_date_components` before
    returning success.
  - direct tests cover valid string date classification and invalid calendar
    rejection for both runtime-value and retained-`Variant` paths.
- `crates/oxvba-host/src/engine.rs`
  - host formal coverage now exercises accepted and invalid string `IsDate`
    policy through source execution.

## Verification

Passed:

- `cargo test -p oxvba-vm date --lib`
- `cargo test -p oxvba-host formal_v50_isdate_string_policy_subset --lib`
- `cargo test -p oxvba-host formal_v48_datevalue_string_month_name_subset --lib`
- `cargo test -p oxvba-host formal_v48_datevalue_string_month_first_subset --lib`
- `cargo test -p oxvba-host formal_v48_cdate_string_month_dot_subset --lib`

## Boundary

The unsupported grammar rows from
`V02_DATE_STRING_GRAMMAR_POLICY_2026-04-27.md` remain unsupported and explicit:
two-digit years, localized month names, time suffixes, weekday prefixes, and
host-locale ambiguous `m/d/y` parsing are not claimed by this implementation
bead.
