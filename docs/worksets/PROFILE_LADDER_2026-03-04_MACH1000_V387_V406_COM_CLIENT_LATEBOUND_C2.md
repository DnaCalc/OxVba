# PROFILE_LADDER_2026-03-04_MACH1000_V387_V406_COM_CLIENT_LATEBOUND_C2

## Range

- Ladder span: `v387..v406`
- Terminal gate for executed run: `v406`
- Focus: late-bound COM client C2 contract closure and executable runway.

## Objectives

1. Freeze a rigorous C2 contract for late-bound COM client behavior (activation, member resolution, invoke packing, deterministic failures).
2. Align COM scope/conformance docs and HAL clause catalogs with explicit pre/postconditions.
3. Publish executable planning artifacts so the next implementation block can proceed without design ambiguity.

## Steps

| Step | Focus | Deliverables |
|---|---|---|
| `v387` | C2 step baseline lock | scope and source-anchor lock for late-bound client C2 closure |
| `v388` | activation contract | `CreateObject` string/token surface contract, policy matrix, deterministic failure set |
| `v389` | member resolution contract | member-name/token resolution contract, case policy, DISPID lookup model |
| `v390` | invoke contract | `DISPPARAMS`/`VarResult`/`ExcepInfo`/`ArgErr` translation rules and deterministic mapping |
| `v391` | HAL clause uplift | add C2-targeted `HAL-COM-*` clauses with verification plan wiring |
| `v392` | spec closure gate | publish closure evidence + profile statuses + index/control updates |
| `v393` | bridge + lifetime contract lock | explicit VBA late-bound bridge contract + native COM object identity/lifetime hardening in HAL |
| `v394` | activation runway hardening | durable activation/object-token ownership path prepared for ProgID-text lane |
| `v395` | deterministic COM error taxonomy | machine-readable COM error mapping + adapter labeling/tests for native failure families |
| `v396` | conformance + process scaffold | C2 late-bound fixture scaffold, gate-drift guard, and uncertainty->work mapping updates |
| `v397` | `CreateObject` string path implementation | runtime surface supports ProgID string activation lane |
| `v398` | member-name invoke implementation | name-based dispatch path with deterministic cache semantics |
| `v399` | invoke packing implementation I | positional arguments and scalar VARIANT coercion subset |
| `v400` | fixture expansion II | failure-path fixtures and `On Error Resume Next` checks |
| `v401` | lane script scaffold | COM client lane runner scripts and artifact model |
| `v402` | lane run I | registration-free lane evidence |
| `v403` | lane run II | registered lane smoke evidence or deferred gate entry |
| `v404` | VM/JIT parity sweep | parity checks on late-bound client fixtures |
| `v405` | integrated gate prep | matrix/formal/conformance artifact alignment |
| `v406` | closure gate | C2 implementation closure report and handoff |

## Constraints

- Windows-first COM behavior only; non-Windows COM remains deterministic unsupported.
- Deterministic error surface is mandatory for unsupported/policy-denied/native failures.
- Formal lanes (Kani) remain non-blocking under current policy but must be tracked.
