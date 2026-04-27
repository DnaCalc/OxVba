# V0.2 Date-String Final Checklist

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.4.5`
Parent: `bd-bqm8.4`
Status: complete

## Scope

This checklist closes the V0.2 date-string parsing/coercion lane after the
grammar policy, parser/coercion delivery, and VM/JIT/host/conformance evidence
beads.

Accepted rows covered by the lane:

- `DateValue("1 Jan 2000")`
- `DateValue("January 1, 2000")`
- `CDate("Jan. 1, 2000")`
- `IsDate("1 Jan 2000")`
- deterministic rejection for invalid calendar dates such as
  `February 30, 2000`

Unsupported boundaries remain explicit in
`V02_DATE_STRING_GRAMMAR_POLICY_2026-04-27.md`: locale-sensitive numeric
ambiguity, two-digit years, localized month names, time suffixes, and ordinal
suffixes are not claimed by V0.2.

## Verification

Passed:

- docs scan for `DateValue`, `CDate`, `IsDate`, `date-string`, `unsupported`,
  and `V02_DATE_STRING` across `docs/CONFORMANCE.md`, V0.2 date-string
  evidence, and the active workset
- `cargo test -p oxvba-vm date --lib`
- `cargo test -p oxvba-host formal_v48_datevalue_string_month_name_subset --lib`
- `cargo test -p oxvba-host formal_v48_datevalue_string_month_first_subset --lib`
- `cargo test -p oxvba-host formal_v48_cdate_string_month_dot_subset --lib`
- `cargo test -p oxvba-host formal_v50_isdate_string_policy_subset --lib`
- `./scripts/run-conformance.ps1 -Backend vm -IncludePattern stdlib_date_string_policy.bas`
- `./scripts/run-conformance.ps1 -Backend jit -IncludePattern stdlib_date_string_policy.bas`
- `cargo check -p oxvba-vm -p oxvba-jit -p oxvba-host`

## Closure Decision

`bd-bqm8.4` is complete for the bounded V0.2 date-string parsing/coercion lane.
The capability lane closes on executable VM/JIT/host/conformance evidence, not
on documentation alone.
