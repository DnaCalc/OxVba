# CHARTER.md — OxVBA Charter

## 1. Mission
OxVBA is a full-fidelity VBA 7 runtime engine in Rust, designed for compatibility-first execution with rigorous correctness and high performance.

Compatibility means matching real VBA compile-time and run-time behavior. Existing
OxVBA behavior, legacy fallbacks, and internal convenience paths are not project
goals except as explicitly documented temporary gaps on the way to VBA parity.

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
3. `docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md`
4. `docs/ARCHITECTURE.md`
5. Current subsystem specifications
6. Accepted active worksets, status files, validation matrices, and evidence artifacts

This charter defines mission, values, and scope boundaries. The system contract
defines the durable destination and capability profiles. Current implementation
truth lives in `docs/ARCHITECTURE.md`; active execution and proof live in
worksets, canonical validation matrices, status files, and evidence artifacts.

## 6. Relationship to Architecture And Worksets
`docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md` states what completed OxVba must be.
`docs/ARCHITECTURE.md` contains the current realization and gap map. Active
worksets contain scoped execution plans and gates. If downstream documents drift
from this charter or claim a different destination without amending the system
contract, the higher authority wins and the drift must be corrected.
