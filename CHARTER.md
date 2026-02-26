# CHARTER.md — OxVBA Charter

## 1. Mission
OxVBA is a full-fidelity VBA 7 runtime engine in Rust, designed for compatibility-first execution with rigorous correctness and high performance.

OxVBA is part of the DNA Calc universe and follows Foundation doctrine, while operating as its own project with its own planning and delivery cadence.

## 2. Values Ordering
When values conflict, higher-ranked values win.

1. Robustness
2. Compatibility
3. Performance
4. Runtime size
5. Development environment quality

## 3. Clean-room Rule (Non-negotiable)
OxVBA development uses only:
- public specifications/documentation,
- published research,
- reproducible black-box observation of Office/VBA behavior.

Excluded:
- proprietary/restricted sources,
- reverse engineering of internals,
- decompilation/disassembly of Office internals.

## 4. Scope
Initial focus:
- Full VBA language/runtime core and execution pipeline.
- COM-compatible object/runtime semantics on Windows.
- Host-aware runtime APIs (host can inject root objects such as `Application` at engine initialization).
- Forms runtime (Rust implementation).

In scope but not currently active:
- Runtime security model.
- Debugging protocol.
- IDE features.
- Forms Designer.
- Non-Windows COM library interop completeness.

Out of scope:
- Spreadsheet engine implementation (DNA Calc domain).
- VBA IDE implementation.
- Office application object model implementation (host-provided).

## 5. Document Model
Top-level guidance precedence for OxVBA:
1. `CHARTER.md`
2. `OPERATIONS.md`
3. `MACH1000_PLAN.md`

`MACH1000_PLAN.md` is the full architecture and phased implementation plan. This charter defines mission, values, and scope boundaries.

## 6. Relationship to MACH-1000 Plan
`MACH1000_PLAN.md` contains the detailed architecture, formal strategy, testing approach, and implementation sequencing. If details in the plan drift from this charter, this charter is authoritative and the plan must be updated.
