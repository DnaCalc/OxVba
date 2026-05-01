# Correctness Corpus And Oracle Stress Workset

Status: `planned`
Date: 2026-04-30
Parent: `WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`

## Purpose

Build the correctness corpus that is likely to expose hidden numeric, coercion,
UDT, array, and error-state bugs before direct native compilation depends on
those semantics.

## Scope

In scope:

- Numeric edge tests: rounding, overflow, truncation, sign, division, `Mod`,
  exponentiation, `Currency`, `Decimal`, `Date`, and Boolean truth.
- Coercion edge tests: string-to-number, Empty/Null/Error, `CVErr`,
  assignment timing, argument passing, and comparison.
- UDT tests: nested fields, whole-copy, same-shape cross-type rejection,
  initialization, arrays where supported, and explicit native-layout non-claims.
- Oracle capture plans for Office/VBA where available.
- Spec-backed or implementation-defined classifications where oracle capture is
  unavailable.

Out of scope:

- Performance interpretation.
- Direct native execution.
- Broad Office object model parity.

## Execution Epics

1. **Corpus Matrix Design**
   - Close condition: matrix records case ID, area, source, expected result,
     oracle/spec status, backends, and claim boundary.
2. **Numeric Stress Lane**
   - Close condition: edge numeric cases run through VM/JIT reference lanes.
3. **Coercion And Error Lane**
   - Close condition: coercion/error-state edge cases cover failure timing and
     runtime state.
4. **UDT And Layout Lane**
   - Close condition: UDT semantic subset is covered and native-layout residuals
     are explicit.
5. **Oracle Foldback Lane**
   - Close condition: Office/VBA-observable cases have capture scripts or
     recorded skip rationale.

## First Beads

- `stress-001`: create corpus matrix and fixture naming convention.
- `stress-002`: add numeric rounding/overflow/truncation cases.
- `stress-003`: add string-number/Null/Empty/Error coercion cases.
- `stress-004`: add UDT semantic and non-claim cases.
- `stress-005`: add Office/VBA oracle capture packet for selected rows.

## Terminal Gate

This workset is complete when the corpus can act as a native-readiness tripwire:
it must be broad enough that hidden numeric or UDT skeletons are likely to fail
before native code generation begins.

