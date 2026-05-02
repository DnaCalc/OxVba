# Correctness Corpus And Oracle Stress Workset

Status: `complete` (recovered 2026-05-02)
Date: 2026-04-30; recovery update 2026-05-02
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

## Evidence

- Corpus matrix and naming convention:
  [`../evidence/native_ready/CORRECTNESS_CORPUS_MATRIX_2026-05-01.md`](../evidence/native_ready/CORRECTNESS_CORPUS_MATRIX_2026-05-01.md)
- Numeric stress cases:
  [`../evidence/native_ready/NUMERIC_STRESS_CASES_2026-05-01.md`](../evidence/native_ready/NUMERIC_STRESS_CASES_2026-05-01.md)
- Coercion/error stress cases:
  [`../evidence/native_ready/COERCION_ERROR_STRESS_CASES_2026-05-01.md`](../evidence/native_ready/COERCION_ERROR_STRESS_CASES_2026-05-01.md)
- UDT semantic/layout non-claim cases:
  [`../evidence/native_ready/UDT_LAYOUT_STRESS_CASES_2026-05-01.md`](../evidence/native_ready/UDT_LAYOUT_STRESS_CASES_2026-05-01.md)
- Oracle foldback packet:
  [`../evidence/native_ready/ORACLE_FOLDBACK_PACKET_2026-05-01.md`](../evidence/native_ready/ORACLE_FOLDBACK_PACKET_2026-05-01.md)
- Executable recovery proof:
  [`../evidence/native_ready/CORRECTNESS_CORPUS_RECOVERY_EXECUTABLE_STRESS_2026-05-02.md`](../evidence/native_ready/CORRECTNESS_CORPUS_RECOVERY_EXECUTABLE_STRESS_2026-05-02.md)

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

Rolled out on 2026-05-01 under bead epic `bd-9xmu.4`:

- `bd-9xmu.4.1` / `stress-000`: roll out this executable bead path.
- `bd-9xmu.4.2` / `stress-001`: create corpus matrix and fixture naming
  convention. Done 2026-05-01; the matrix schema, fixture naming convention,
  and initial row IDs are recorded.
- `bd-9xmu.4.3` / `stress-002`: add numeric rounding/overflow/truncation
  cases. Recovered 2026-05-02; `numeric_stress_rounding_overflow_truncation_edges`
  now runs one test and passes.
- `bd-9xmu.4.4` / `stress-003`: add string-number/Null/Empty/Error coercion
  cases. Recovered 2026-05-02;
  `coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing` now
  runs one test and passes.
- `bd-9xmu.4.5` / `stress-004`: add UDT semantic and non-claim cases.
  Recovered 2026-05-02; `nested_udt` now runs three host tests and passes.
- `bd-9xmu.4.6` / `stress-005`: add Office/VBA oracle capture packet for
  selected rows. Done 2026-05-01; `NR-ORACLE-001` has VBA source,
  instructions, output schema, and headless-CI skip rationale.
- `bd-9xmu.4.7` / recovery: restore executable correctness corpus stress tests
  after RuntimeValue-heavy test deletion. Recovered 2026-05-02 with nonzero
  `NR-NUM-001/002`, `NR-COERCE-001`, and `NR-UDT-001` tests.

## Terminal Gate

This workset returns to complete when the corpus can act as a native-readiness
tripwire: it must be broad enough that hidden numeric/coercion/UDT skeletons are
likely to fail before native code generation begins, and every cited row must be
backed by an executing test or an explicit non-executing oracle packet.

Recovery result: the previously zero-test numeric, coercion, mixed numeric, and
UDT filters now run nonzero and pass. Evidence:
[`../evidence/native_ready/CORRECTNESS_CORPUS_RECOVERY_EXECUTABLE_STRESS_2026-05-02.md`](../evidence/native_ready/CORRECTNESS_CORPUS_RECOVERY_EXECUTABLE_STRESS_2026-05-02.md).

