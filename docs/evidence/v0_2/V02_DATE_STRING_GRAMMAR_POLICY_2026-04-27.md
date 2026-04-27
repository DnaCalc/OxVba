# V0.2 Date-String Grammar And Locale Policy

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.4.2`
Parent: `bd-bqm8.4`
Status: complete

## Scope

This policy defines the accepted V0.2 string-date grammar for:

- `DateValue`
- `CDate`
- `IsDate`

The policy is intentionally deterministic. It does not claim full VBA
locale-sensitive parsing parity.

## Accepted Grammar

Accepted inputs are trimmed three-part date strings. Separators may be comma,
dot, hyphen, slash, or whitespace. Multiple separators collapse to whitespace.

Accepted shapes:

- `yyyy month day`, where `month` is numeric or a supported English month
  token.
- `month day yyyy`, where `month` is a supported English month token.
- `day month yyyy`, where `month` is numeric or a supported English month
  token.

Supported English month tokens are case-insensitive:

- `jan`, `january`
- `feb`, `february`
- `mar`, `march`
- `apr`, `april`
- `may`
- `jun`, `june`
- `jul`, `july`
- `aug`, `august`
- `sep`, `sept`, `september`
- `oct`, `october`
- `nov`, `november`
- `dec`, `december`

Calendar validity is required before a string is considered accepted:

- month must be `1..12`
- day must fit the year/month, including leap-year handling
- four-digit year is required for string inputs

## Function Semantics

- `DateValue(text)` returns the date component as a date-subtyped result with
  the time component floored to midnight.
- `CDate(text)` returns a date-subtyped result for accepted strings.
- `IsDate(text)` returns true only when the same accepted grammar and calendar
  validation succeed.

Numeric compatibility carriers remain a separate existing lane:

- packed `yyyymmdd` numeric carriers continue to coerce as dates when valid.
- non-packed numeric carriers continue to coerce as OLE Automation serial dates
  when range-valid.

## Locale Policy

V0.2 uses invariant parsing instead of host-locale probing:

- year-first numeric strings are interpreted as `yyyy-mm-dd`.
- all-numeric non-year-first strings are interpreted deterministically as
  `day-month-year`.
- month-first strings require an English month token, not a numeric month.

This avoids hiding locale-sensitive behavior behind whichever host happens to
run the test suite.

## Explicit Unsupported Rows

These inputs remain unsupported for V0.2 unless a later bead explicitly
delivers them with evidence:

- two-digit years
- single-token compact string dates such as `20260427`
- time-only strings
- date strings with time suffixes
- localized non-English month names
- weekday prefixes
- arbitrary host-locale `m/d/y` or `d/m/y` ambiguity
- relative date words such as `today` or `tomorrow`

Unsupported strings must produce deterministic false/error behavior instead of
being described as implemented.

## Current Gap To Implementation

The current `IsDate` string path rejects plain strings instead of sharing the
accepted grammar. The implementation bead must reconcile `IsDate(text)` with
the accepted parser while preserving deterministic unsupported boundaries.
