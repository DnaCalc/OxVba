# PROFILE_LADDER_2026-03-01_MACH1000_V147_V166_NON_HAL_COMPLETION.md

## Objective

Complete all currently identified non-HAL language/runtime/library work that is implementable without blocking on external empirical (Excel/VBA oracle) validation.

## Scope Boundaries

In scope:
- Non-HAL language semantics and runtime/library implementation gaps.
- Removal of projection/stub behavior in deterministic core where feasible.
- Coverage and gate hardening for VM+JIT parity on in-scope surfaces.
- Formal/deferred gate integration (non-blocking formal policy unchanged).

Out of scope (explicit exclusions for this ladder):
- HAL-adjacent host-sensitive behavior (`Shell`, `Environ`, `Dir` parity work).
- COM/external automation parity beyond deterministic bridge.
- UI interaction (`MsgBox`, `InputBox`).
- Stateful host file-I/O parity.
- Any topic requiring oracle validation to unblock forward implementation.

## Authoritative Source Sets

Language source map:
- `docs/FOUNDATION_SPEC_REFERENCE.md`
- `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/conformance_items.jsonl`

Library source map:
- `docs/FOUNDATION_SPEC_REFERENCE.md`
- `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/conformance_items.jsonl`

Current implementation backlog:
- `docs/evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md`

Deferred oracle gate register:
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`

## Ladder Profiles

| Profile | Focus | Primary Deliverables |
|---|---|---|
| `v147` | Gap baseline lock | Freeze non-HAL gap baseline from `SPEC_CHECKLIST`, `COVERAGE_INDEX`, `LIBRARY_CHECKLIST` and publish profile status contract. |
| `v148` | `Err` surface expansion I | Add additional `Err` properties/state fields and parser/binder/runtime routing for direct usage. |
| `v149` | `Err` surface expansion II | Complete `Err` lifecycle transitions on procedure entry/exit and success-path clearing points where non-oracle-blocking. |
| `v150` | String runtime completion I | Replace remaining projection placeholders in core string operations with concrete semantics for current type domain. |
| `v151` | String runtime completion II | Tighten `vbNullString`/String sentinel semantics in non-boundary execution paths. |
| `v152` | UDT/value semantics | Strengthen UDT copy/assignment/init behavior in runtime and typechecker where deterministic rules are available. |
| `v153` | Coercion edge normalization | Unify non-HAL coercion/error behavior for Null/Empty/Error paths in deterministic subset. |
| `v154` | Financial functions I | Implement real algorithms for `NPV/IRR/MIRR` replacing projection placeholders. |
| `v155` | Financial functions II | Implement real algorithms for `Rate/NPer` and add numeric stability handling. |
| `v156` | Financial tolerance model | Define deterministic tolerance/iteration policy and error signaling for non-convergence. |
| `v157` | Diagnostics timing pass | Harden compile-time vs runtime diagnostic phase timing for in-scope constructs. |
| `v158` | VM parity expansion | Extend interpreter execution coverage for newly concrete runtime behaviors. |
| `v159` | JIT parity expansion | Ensure JIT fallback/supported-op parity for newly added behaviors and add targeted tests. |
| `v160` | Corpus expansion I | Add/refresh conformance fixtures for `Err`, string sentinel, UDT copy, and coercion edge paths. |
| `v161` | Corpus expansion II | Add/refresh conformance fixtures for financial algorithm behavior and tolerance/error modes. |
| `v162` | Formal obligations update | Add formal/Kani obligations for newly introduced unsafe-sensitive runtime paths (non-blocking gate policy). |
| `v163` | Evidence reconciliation | Update `COVERAGE_INDEX`, `LIBRARY_CHECKLIST`, `SPEC_CHECKLIST` for achieved non-HAL statuses. |
| `v164` | Deferred-oracle sync | Move remaining oracle-dependent uncertainties into `DEFERRED_ORACLE_GATES` with clear foldback notes. |
| `v165` | Integrated non-HAL gate | Run matrix + conformance + bench + formal lanes; publish integrated gate artifacts for this ladder. |
| `v166` | Terminal closure | Publish terminal profile status and milestone closure doc for non-HAL completion tranche. |

## Gate Policy

- Formal failures remain non-blocking for this ladder unless unsoundness/data-corruption risk is detected.
- Oracle-dependent semantics are non-blocking; they must be tracked in deferred-oracle gates.
- HAL-adjacent tasks are not blockers for this ladder.

## Exit Criteria (`v166`)

1. No remaining `partial/planned` items in non-HAL rows of:
- `docs/evidence/language/COVERAGE_INDEX.csv`
- `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
- `docs/evidence/SPEC_CHECKLIST.md`

2. All unresolved non-HAL oracle-dependent topics are explicitly represented in:
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`

3. VM/JIT conformance corpus remains green for in-scope surfaces with updated evidence.
