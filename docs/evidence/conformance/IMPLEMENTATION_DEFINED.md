# IMPLEMENTATION_DEFINED.md

This register captures implementation-defined behavior choices in OxVBA so conformance work can clearly separate specified semantics from local policy decisions.

## Current entries

| ID | Area | Decision | Why | Test/Tracking |
|---|---|---|---|---|
| ID-001 | Host-sensitive intrinsics | Shell/Environ/Dir use real host behavior in native host-backed mode; non-HAL/default mode keeps deterministic capability fallback semantics. | Separates proved Windows host behavior from portable CI-safe fallback behavior without overstating Excel policy parity. | docs/evidence/runtime/LIBRARY_CHECKLIST.csv, docs/evidence/conformance/oracle_captures/host_sensitive_oracle_20260326T074730Z/summary.md, ODG-033 |
| ID-002 | COM/dispatch bridge | CreateObject/DispatchInvoke run deterministic projection subset in non-HAL mode. | Full COM behavior is HAL/interop scoped and host-dependent. | docs/evidence/runtime/LIBRARY_CHECKLIST.csv, ODG-030, ODG-031 |
| ID-003 | Time/random runtime | Now/Date/Time/Timer and Rnd/Randomize use deterministic subset behavior in non-HAL mode. | Keeps CI reproducible while oracle parity is deferred. | ODG-026, ODG-027 |
| ID-004 | File library | FreeFile/EOF/LOF/Seek expression subset is implemented; stateful file statements are partial, with the host-backed Output/Print/Close/Input/Line Input plus `EOF` / `LOF` / `Seek` input-position subset now evidenced. | Broader file I/O semantics still require HAL-backed policy and host conformance probes before strong parity claims. | docs/evidence/runtime/LIBRARY_CHECKLIST.csv, docs/evidence/conformance/oracle_captures/file_io_oracle_20260326T160900Z/summary.md, ODG-032 |
| ID-005 | Error-tag encoding | CVErr values are normalized into reserved internal error-tag range for deterministic VM/JIT semantics. | Avoids host-dependent tagging details while preserving predicate/type behavior in scope. | conformance/tests/coercion_cverr_range_predicates.bas, FTODO-V175-001 |
| ID-006 | COM early-binding execution model | Implemented early-binding subset lowers through deterministic `CreateObject`/`DispatchInvoke` transport rather than direct vtable calls by default. | Preserves deterministic compatibility while early-bound call ABI and wider member coverage are still staged. | docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md, docs/evidence/conformance/com_early/COM_EARLY_CONFORMANCE_LATEST.csv |
| ID-007 | COM strategy policy default | Host policy default for COM invocation strategy is `dispatch-only`; `prefer-vtable` is opt-in and currently scoped to controlled test lanes. | Avoids hidden transport-mode shifts while preserving explicit experimentation path. | crates/oxvba-hal/src/model.rs, crates/oxvba-hal/src/adapters/standard.rs |
| ID-008 | COM early oracle parity | Excel/VBA parity for dual-interface fallback behavior and typelib version/broken-reference handling remains deferred; the supported `As New Scripting.Dictionary` subset is now captured separately. | These behaviors are host-dependent and still need empirical oracle capture before strong parity claims. | ODG-045, ODG-046 |

## Governance
- Related conformance topic: CCT-036.
- Related deferred gate: ODG-034.
- Any new implementation-defined decision should be added here with evidence links before landing.
