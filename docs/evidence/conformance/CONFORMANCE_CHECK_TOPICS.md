# CONFORMANCE_CHECK_TOPICS.md

Topic / oracle / gate register for behavior that is semantically uncertain or historically tricky in VBA.
This file is intentionally not the primary implementation-truth surface.
Canonical implementation truth lives in the domain validation matrices under `docs/validation/` and the
topic-to-matrix map at `docs/validation/CONFORMANCE_TOPIC_MATRIX_MAP_2026-03-29.csv`.

Purpose:
- Capture high-risk semantic topics, oracle probes, and gate states.
- Preserve a bounded tracking surface for deferred or differential evidence.
- Route implementation truth to the canonical validation matrices, not this file.
- Fold oracle outcomes back into OxVBA semantics, tests, and docs through matrix-backed rows.

Machine-readable source:
- `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
- Deferred-oracle gate register:
  - `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
  - `docs/evidence/conformance/DEFERRED_ORACLE_GATES.md`

Matrix ownership:
- `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv`
- `docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv`
- `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv`
- `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv`
- `docs/validation/VALIDATION_CANONICAL_OWNERSHIP_MAP_2026-03-29.md`
- `docs/validation/CONFORMANCE_TOPIC_MATRIX_MAP_2026-03-29.csv`

This register may track:
- topic identity and probe shape
- oracle status and foldback state
- gate status and evidence links

This register must not be treated as:
- the authoritative source of feature support state
- the canonical source for subset boundaries
- the source of truth for implementation parity

Status values:
- `planned`: topic identified, probes not yet executed against VBA oracle.
- `in-progress`: probes authored and/or oracle runs underway.
- `resolved`: oracle behavior confirmed and reflected in implementation/tests.
- `deferred`: intentionally postponed with rationale.

## Recommended Execution Order

1. `P0` control-flow and error-state topics first.
2. `P0` type/coercion edge semantics.
3. `P1` object/dispatch/property and array edge semantics.
4. `P1/P2` stdlib host-sensitive and locale/time/file topics.

## Oracle Strategy

Primary oracle candidates:
- VBA7 in Excel (Windows x64) as default runtime baseline.
- Optional secondary host checks (Access/Word) for host-specific behaviors.

For each topic:
1. Write minimal probe macro(s) with explicit output capture (cells/Immediate/CSV).
2. Run probe in real VBA host and record observed output.
3. Mirror probe as OxVBA conformance fixture.
4. Add/update divergence record if behavior differs.
5. Update the relevant canonical matrix row(s) first; keep this register synchronized as a topic/gate view.
6. Record the topic state here as a gate/oracle status, not as the implementation truth itself.

## High-Semantic Focus Areas

- Unstructured control flow: `GoTo`, line-number targets, and interaction with nested blocks.
- Error model: `On Error` mode transitions, `Resume` target semantics, `Err` lifecycle and clearing points.
- ByRef and coercion subtleties: temporary copies, copy-back behavior, parentheses effects, and Variant/object edges.
- Array semantics at boundaries: bounds, `Option Base`, `ReDim Preserve`, rank/dimension constraints.
- Late binding / default members / property procedures with object dispatch.
- Locale/time/string/format semantics where host and locale matter.

## Output Artifacts (planned)

- Oracle run logs per topic batch.
- Differential summary report (pass/mismatch) by topic ID.
- Updated conformance fixtures + divergence records.
- Matrix-backed topic mapping updates when scope ownership changes.
