# Project Integration Deferred + Unclear Spec Notes v1

Date: `2026-03-03`

This register captures integration-scope items intentionally deferred or awaiting clearer host/spec decisions.

| Integration Topic | Current State | Tracking | Why Deferred / Unclear |
|---|---|---|---|
| Project startup metadata and entrypoint selection beyond `Sub Main` | Deferred | `ODG-043`, `CCT-045`, `INTP-012` | Host-dependent startup semantics are not fully modeled yet in current project execution path. |
| Host project extension module lifecycle (open host project model) | Deferred beyond bounded subset | `ODG-040` (closed), `CCT-042` (closed), `INTP-013` | Bounded initial-scope host-extension attach behavior is now evidenced by `host_extension_oracle_20260326T144800Z`: supported `ThisWorkbook` attachment, missing-target failure, and overwrite-on-occupied-target behavior match Excel. Broader add/remove lifecycle and other host-specific extension behavior remain deferred under `INTP-013`. |
| Stateful file statement parity (`Open`, `Input#`, `Print#`, `Write#`) | Deferred | `ODG-032`, `CCT-033`, `INTP-014` | A supported host-backed statement subset now exists (`Output`/`Print`/`Close`/`Input`/`Line Input` roundtrip), but broader stateful file semantics remain HAL-adjacent and oracle-sensitive. |
| Class graph semantics for `Implements` | Partial (baseline compile + runtime prefixed flow implemented) | `CCT-040`, `INTP-008`, `ODG-038` | Compile-time coverage legality is implemented and deterministic runtime prefixed-member execution is covered; multi-interface oracle edge matrix remains deferred. |
| Event model semantics for `RaiseEvent`/`WithEvents` | Partial (compile legality + static runtime dispatch baseline) | `CCT-041`, `INTP-009`, `DIV-0004`, `ODG-039` | Compile-time class/event legality and deterministic static dispatch are implemented; true instance-level subscription/reassignment lifecycle semantics remain deferred. |
| Full COM/type-library runtime parity in integrated project lane | Deferred/partial | `ODG-041`, `CCT-043` | Deterministic scaffolding exists, and a baseline file-backed `.tlb` oracle lane is proved; broader importlib/reference-resolution behavior and full bridge breadth remain open. |

## Policy

- Deferred items remain non-blocking for active integration lane pass.
- All deferred items must remain explicitly linked to `ODG`/`CCT` records.
- Any transition from deferred to active must include deterministic fixture(s) and a catalog row update.
