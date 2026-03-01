# IMPLEMENTATION_DEFINED.md

This register captures implementation-defined behavior choices in OxVBA so conformance work can clearly separate specified semantics from local policy decisions.

## Current entries

| ID | Area | Decision | Why | Test/Tracking |
|---|---|---|---|---|
| ID-001 | Host-sensitive intrinsics | Shell/Environ/Dir use deterministic capability fallback semantics in non-HAL mode. | Enables portable deterministic tests before host HAL parity work. | docs/evidence/runtime/LIBRARY_CHECKLIST.csv, ODG-033 |
| ID-002 | COM/dispatch bridge | CreateObject/DispatchInvoke run deterministic projection subset in non-HAL mode. | Full COM behavior is HAL/interop scoped and host-dependent. | docs/evidence/runtime/LIBRARY_CHECKLIST.csv, ODG-030, ODG-031 |
| ID-003 | Time/random runtime | Now/Date/Time/Timer and Rnd/Randomize use deterministic subset behavior in non-HAL mode. | Keeps CI reproducible while oracle parity is deferred. | ODG-026, ODG-027 |
| ID-004 | File library | FreeFile/EOF/LOF/Seek expression subset is implemented; stateful file statements are deferred. | Stateful I/O requires HAL-backed policy and host conformance probes. | docs/evidence/runtime/LIBRARY_CHECKLIST.csv, ODG-032 |
| ID-005 | Error-tag encoding | CVErr values are normalized into reserved internal error-tag range for deterministic VM/JIT semantics. | Avoids host-dependent tagging details while preserving predicate/type behavior in scope. | conformance/tests/coercion_cverr_range_predicates.bas, FTODO-V175-001 |

## Governance
- Related conformance topic: CCT-036.
- Related deferred gate: ODG-034.
- Any new implementation-defined decision should be added here with evidence links before landing.
