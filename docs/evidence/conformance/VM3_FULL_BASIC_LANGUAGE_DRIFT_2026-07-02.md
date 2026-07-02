# VM3 Full Basic-Language Drift Split

Date: 2026-07-02
Bead: `bd-5kqj`
Split from: `bd-9sed.18`

## Context

After the W10 pending-row oracle refresh promoted all 29 originally pending
fixtures, the broader default conformance gate still failed:

- Command: `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language`
- Current result: 70 mismatches over 192 files.

This drift predates the final W10 pending-row promotions and is not being
treated as a VBA compatibility target or as accepted legacy behavior. It is now
tracked explicitly in `bd-5kqj`.

## Observed Shape

The failure surface includes retained array/object dump-shape drift, scalar
subtype drift such as `i16` vs `i32`, several status mismatches, and selected
error-state/value mismatches. Each row still needs classification against real
VBA behavior or an explicitly documented retained-value surface decision.

During fresh-eyes review of the W10 Rnd probes, a `Single` retained-value
formatter bug was fixed (`Single` no longer prints as `f64:0`). After that fix,
the full gate still reports 70 mismatches over 192 files; `bd-5kqj` starts from
that current surface.

## Required Next Evidence

`bd-5kqj` should classify every mismatch and either produce a passing intended
full `basic-language` gate or split real behavior gaps into delivery beads with
no hidden unresolved mismatch left in this drift lane.
