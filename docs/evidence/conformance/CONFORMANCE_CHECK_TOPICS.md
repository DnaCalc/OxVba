# CONFORMANCE_CHECK_TOPICS.md

Targeted backlog for behavior that is semantically uncertain or historically tricky in VBA.

Purpose:
- Capture high-risk semantic topics now.
- Implement language/library features first.
- Then run differential checks against real VBA/Office hosts and fold outcomes back into OxVBA semantics, tests, and docs.

Machine-readable source:
- `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`

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
5. Promote topic status (`planned` -> `resolved`) once behavior is encoded and tested.

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
