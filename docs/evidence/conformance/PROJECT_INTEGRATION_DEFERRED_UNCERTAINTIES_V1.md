# Project Integration Deferred + Unclear Spec Notes v1

Date: `2026-03-03`

This register captures integration-scope items intentionally deferred or awaiting clearer host/spec decisions.

| Integration Topic | Current State | Tracking | Why Deferred / Unclear |
|---|---|---|---|
| Project startup metadata and entrypoint selection beyond `Sub Main` | Deferred | `ODG-043`, `CCT-045`, `INTP-012` | Host-dependent startup semantics are not fully modeled yet in current project execution path. |
| Host project extension module lifecycle (open host project model) | Deferred | `ODG-040`, `CCT-042`, `INTP-013` | Requires HAL/project-catalog capabilities and host-environment contracts not finalized. |
| Stateful file statement parity (`Open`, `Input#`, `Print#`, `Write#`) | Deferred | `ODG-032`, `CCT-033`, `INTP-014` | Current support is expression-level file introspection subset; statement-level semantics remain HAL-adjacent and oracle-sensitive. |
| Class graph semantics for `Implements` | Active limit (expected compile-time gate) | `CCT-040`, `INTP-008`, `DIV-0003` | Design/model work is in progress; current behavior is intentionally gated with stable diagnostic contract. |
| Event model semantics for `RaiseEvent`/`WithEvents` | Active limit (expected compile-time gate) | `CCT-041`, `INTP-009`, `DIV-0004` | Full class/event graph ordering and legality remain deferred pending class-model expansion. |
| Full COM/type-library runtime parity in integrated project lane | Deferred/partial | `ODG-041`, `CCT-043` | Deterministic scaffolding exists, but registered-host oracle parity and full bridge behavior remain open. |

## Policy

- Deferred items remain non-blocking for active integration lane pass.
- All deferred items must remain explicitly linked to `ODG`/`CCT` records.
- Any transition from deferred to active must include deterministic fixture(s) and a catalog row update.
