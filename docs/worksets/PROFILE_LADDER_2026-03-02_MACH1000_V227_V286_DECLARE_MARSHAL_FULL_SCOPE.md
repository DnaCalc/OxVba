# PROFILE_LADDER_2026-03-02_MACH1000_V227_V286_DECLARE_MARSHAL_FULL_SCOPE

## Range
- Ladder span: `v227..v286`
- Focus: complete `Declare` + marshaling specification, contracts, conformance, implementation, formal lanes, and performance closure.

## Objective

Deliver full-scope `Declare`/marshaling capability under HAL governance with:
- explicit per-profile operating envelopes,
- deterministic diagnostics and error mapping,
- executable conformance lanes tied to clause IDs,
- formal lane integration (non-blocking policy retained),
- staged performance hardening.

## Scope Boundaries

In scope:
- external declaration parsing/binding/descriptor model,
- dynamic-link runtime path and policy gating,
- marshaling layers (`M0` deterministic token subset, `M1` Automation-compatible, `M2` native ABI lane),
- loader integration (`LoadLibrary`/`GetProcAddress`, `dlopen`/`dlsym`) with deterministic failure behavior,
- clause-driven conformance and evidence.

Out of scope for this ladder:
- non-Windows COM parity beyond declared bridge scope,
- host features unrelated to Declare/marshaling unless directly required for boundary correctness.

## Authoritative Sources

- `CHARTER.md`
- `OPERATIONS.md`
- `MACH1000_PLAN.md`
- Foundation canonical references:
  - `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/*.jsonl`
  - `../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/*.jsonl`
  - `../Foundation/reference/runs/20260301-ms-dtyp-pass02/outputs/*.jsonl`

## Profile Steps

| Step | Focus | Deliverables |
|---|---|---|
| `v227` | Scope baseline lock | Freeze current M0 Declare subset baseline and clause references. |
| `v228` | Clause expansion | Add `HAL-DYN-011..020` slots and verification mapping placeholders. |
| `v229` | Failure taxonomy | Deterministic diagnostic/error-family matrix for Declare/marshal failures. |
| `v230` | Profile/runtime matrix | Dynamic-link + ABI support matrix per profile/runtime class. |
| `v231` | Calling convention policy | Explicit convention policy table and defaults by profile. |
| `v232` | Selection policy formalization | External name/alias selection policy fields and semantics. |
| `v233` | Declare grammar completion | Parser coverage for Declare forms in scope. |
| `v234` | Identifier normalization | Deterministic canonicalization for lib/alias/proc identifiers. |
| `v235` | Ordinal alias rules | Strict `#ordinal` validation and canonical representation. |
| `v236` | Declaration-shape restrictions | Explicit acceptance/rejection matrix with deterministic diagnostics. |
| `v237` | Type-surface restrictions | v1 supported type subset enforcement and diagnostics. |
| `v238` | Resolve/compile diagnostics | Clause-linked compile-time diagnostic hardening. |
| `v239` | External descriptor model | Add explicit external call descriptor model and stable IDs. |
| `v240` | Descriptor metadata enrichment | Convention + marshaling lane metadata in descriptor model. |
| `v241` | Descriptor-driven runtime path | VM/host path uses external descriptors for execution decisions. |
| `v242` | Compile-time preflight expansion | Preflight gates descriptor/policy/profile constraints. |
| `v243` | Runtime gating parity | Runtime mode failure behavior aligned with compile-time policy. |
| `v244` | Deterministic descriptor stability | Stable descriptor ordering/hash behavior and tests. |
| `v245` | Trait evolution design | Draft versioned `bind/prepare/invoke` trait split. |
| `v246` | Trait evolution implementation | Backward-compatible implementation path with shim coverage. |
| `v247` | Windows loader baseline | Host-backed Windows symbol bind/load lane implementation. |
| `v248` | Linux loader baseline | Host-backed Linux symbol bind/load lane implementation. |
| `v249` | Unsupported-profile contract | Deterministic unsupported behavior for `macos`/`wasm`/`null`. |
| `v250` | Loader lifecycle invariants | Handle/resource lifecycle and failure cleanup guarantees. |
| `v251` | Marshaling M1 numeric | Automation-compatible scalar numeric mapping path. |
| `v252` | Marshaling M1 `VARIANT` legality | `VT_BYREF` and discriminant legality enforcement. |
| `v253` | Marshaling M1 `SAFEARRAY` legality | Array shape/element legality checks for supported subset. |
| `v254` | Marshaling M1 `BSTR` | String boundary ownership/shape handling. |
| `v255` | Marshaling M1 interface pointers | `IUnknown*`/`IDispatch*` legality and deterministic failures. |
| `v256` | Marshaling M1 failure matrix | Deterministic failure contract for illegal OAUT shapes. |
| `v257` | Marshaling M2 scalar ABI | Native scalar ABI mapping rules and tests. |
| `v258` | Marshaling M2 pointer strings | `LPSTR`/`LPWSTR` encoding + length/termination contracts. |
| `v259` | Marshaling M2 ByRef writeback | ByRef writeback semantics and determinism checks. |
| `v260` | Marshaling M2 complex shapes | UDT/struct/callback policy (implemented or deterministic reject). |
| `v261` | `AddressOf`/callback policy | Explicit callback boundary contract and unsupported rules. |
| `v262` | Lane selection engine | Policy/profile/declaration-driven lane selection model. |
| `v263` | COM invoke output obligations | `VarResult`/`ExcepInfo`/`ArgErr` obligations in COM-enabled lanes. |
| `v264` | Dispatch marshaling parity | In-scope dispatch argument/result marshaling checks. |
| `v265` | Runtime error pipeline hardening | Stable `Err.Number` mapping and error-routing invariants. |
| `v266` | Diagnostics payload closure | Stable payload schema for all boundary failures. |
| `v267` | Conformance expansion I | Implement executable checks for `HAL-DYN-002..010` closure items. |
| `v268` | Conformance expansion II | Implement executable checks for `HAL-DYN-011..020`. |
| `v269` | Matrix execution coverage | Windows/Linux supported lanes + negative lanes. |
| `v270` | Unsupported-platform conformance | Explicit wasm/null/macos negative conformance evidence. |
| `v271` | Remote Linux evidence integration | Integrate remote Linux lane evidence in formal artifacts. |
| `v272` | Clause-coverage gate | Coverage regression guard for conformance-scoped clauses. |
| `v273` | Formal lane design | Define Kani targets and decomposition for declare/marshal contracts. |
| `v274` | Formal proofs I | Alias/declaration normalization invariants. |
| `v275` | Formal proofs II | `VARIANT` legality invariants. |
| `v276` | Formal proofs III | `SAFEARRAY` legality invariants. |
| `v277` | Formal proofs IV | Error-code and routing determinism invariants. |
| `v278` | Async formal orchestration | Queue/retry/priority integration for long-running formal jobs. |
| `v279` | Perf baseline capture | Baseline microbenchmarks for Declare/marshal hot paths. |
| `v280` | Perf optimization I | Descriptor/symbol cache and deterministic invalidation model. |
| `v281` | Perf optimization II | Marshaling hot-path tuning without contract drift. |
| `v282` | Perf gate integration | Performance regression gates and evidence publication. |
| `v283` | Oracle backlog closure prep | Enumerate remaining implementation-defined parity probes. |
| `v284` | Oracle harness scaffolding | Probe harness and divergence capture workflow updates. |
| `v285` | Final doc synchronization | Spec/evidence/crosswalk/registry consistency closure. |
| `v286` | Terminal integrated gate | Full integrated closure gate for `v227..v286`. |

## Gate Policy

- Formal lane failures are non-blocking unless unsoundness/data-corruption risk is found.
- Kani runs are async/deferred-gate eligible; liveness/progress evidence must still be captured.
- Implementation-defined behavior is allowed only when explicitly registered and clause-linked.

## Exit Criteria (`v286`)

1. Declare/marshaling clauses in catalog are either:
- `implemented-verified`, or
- `implemented-partial` with explicit tests/evidence and deferred-oracle linkage.

2. Supported lanes:
- Windows and Linux dynamic-link execution are operational for declared supported subset.

3. Unsupported lanes:
- `macos`/`wasm`/`null` behavior is deterministic and verified.

4. Conformance and gates:
- clause drift guard passes,
- declare-focused compiler/host/hal test lanes pass,
- integrated gate evidence exists under `docs/evidence/profiles/v286/`.
