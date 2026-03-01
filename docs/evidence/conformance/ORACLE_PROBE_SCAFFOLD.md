# ORACLE_PROBE_SCAFFOLD.md

This document defines the initial deferred oracle probe scaffold introduced in profile `v174`.

## Purpose

- Prepare reusable artifacts for later differential checks against external VBA hosts.
- Keep oracle work non-blocking for current hardening ladder execution.
- Generate queue rows that can be filled by future host-capture runs.

## Script

- `scripts/oracle-probe.ps1`

## Usage

Generate scaffold rows for all conformance tests:

```powershell
./scripts/oracle-probe.ps1
```

Generate scaffold rows for selected tests:

```powershell
./scripts/oracle-probe.ps1 -TestPath conformance/tests/coercion_cverr_range_predicates.bas,conformance/tests/error_nested_mode_transitions.bas -OutputCsvPath docs/evidence/conformance/oracle_probe_queue_subset.csv
```

## Output Shape

The script writes CSV rows with:
- `probe_id`
- `test_file`
- `host`
- `status` (`pending` in scaffold mode)
- `observed_slots`
- `notes`
- `captured_at_utc`
- `generated_at_utc`

## Policy

- This scaffold is explicitly deferred oracle work and remains non-blocking.
- Results from real host captures should be folded back into deferred oracle gate tracking.
