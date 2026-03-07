# Project Integration Deferred + Unclear Spec Notes v1

Date: `2026-03-03`

This register captures integration-scope items intentionally deferred or awaiting clearer host/spec decisions.

| Integration Topic | Current State | Tracking | Why Deferred / Unclear |
|---|---|---|---|
| Project startup metadata and entrypoint selection beyond `Sub Main` | Deferred | `ODG-043`, `CCT-045`, `INTP-012` | Host-dependent startup semantics are not fully modeled yet in current project execution path. |
| Host project extension module lifecycle (open host project model) | Deferred | `ODG-040`, `CCT-042`, `INTP-013` | Requires HAL/project-catalog capabilities and host-environment contracts not finalized. |
| Stateful file statement parity (`Open`, `Input#`, `Print#`, `Write#`) | Deferred | `ODG-032`, `CCT-033`, `INTP-014` | Current support is expression-level file introspection subset; statement-level semantics remain HAL-adjacent and oracle-sensitive. |
| Class graph semantics for `Implements` | Partial (compile-time legality/coverage implemented) | `CCT-040`, `INTP-008`, `DIV-0003` | Compile-time interface coverage and legality checks are implemented; runtime dispatch parity and advanced edge cases remain deferred. |
| Event model semantics for `RaiseEvent`/`WithEvents` | Partial (compile-time legality implemented) | `CCT-041`, `INTP-009`, `DIV-0004` | Compile-time class/event legality is implemented; full event graph ordering/subscription runtime semantics remain deferred. |
| Full COM/type-library runtime parity in integrated project lane | Deferred/partial | `ODG-041`, `CCT-043` | Deterministic scaffolding exists, but registered-host oracle parity and full bridge behavior remain open. |

## Policy

- Deferred items remain non-blocking for active integration lane pass.
- All deferred items must remain explicitly linked to `ODG`/`CCT` records.
- Any transition from deferred to active must include deterministic fixture(s) and a catalog row update.
