# PROFILE_LADDER_2026-03-05_MACH1000_V407_V466_COM_EARLY_BINDING_TYPELIB

## Range

- Ladder span: `v407..v466`
- Implementation block-I closure gate (current approved run): `v426`
- Full implementation and conformance closure gate: `v466`

## Objectives

1. Deliver a complete formalized plan for COM early binding and type-library consumption with source-clause traceability.
2. Implement PMR+HAL+binder+IR+runtime support for the approved early-binding subset.
3. Establish deterministic conformance + formal lanes and produce repeatable evidence.

## Step map

| Step | Focus | Deliverables |
|---|---|---|
| `v407` | Source baseline lock | online/local source set lock, anchor index, uncertainty baseline |
| `v408` | PMR contract expansion | reference identity schema (`importlib/libid/version/lcid`) spec + diagnostics draft |
| `v409` | HAL typelib contract | trait draft for resolve/load/invalidate with profile behavior matrix |
| `v410` | Binder model draft | early-bound type/member resolution algorithm and precedence matrix |
| `v411` | IR/runtime contract draft | early-bound IR intent forms + runtime pre/post/failure contracts |
| `v412` | Cache/invalidation model | cache key, fingerprint, invalidation triggers and determinism clauses |
| `v413` | Dual-interface policy model | vtable/dispatch strategy policy with explicit fallback constraints |
| `v414` | Formal property set | Kani/property/Lean-ready property catalog and harness mapping |
| `v415` | Conformance lane plan | E0..E6 lane matrix and artifact schema freeze |
| `v416` | Planning workset closure | integrated plan review and open-question register update |
| `v417` | PMR schema implementation I | additive project/reference fields and serialization model updates |
| `v418` | PMR resolver implementation II | resolver path for libid/version/importlib and deterministic error codes |
| `v419` | HAL Windows resolver I | Windows typelib resolve/load scaffold with deterministic unsupported on non-Windows |
| `v420` | HAL cache substrate II | metadata cache store + invalidation command surface |
| `v421` | Binder integration I | declared external type resolution (`Dim x As MyLib.MyThing`) |
| `v422` | Binder integration II | early-bound member lookup and signature binding subset |
| `v423` | `As New` integration | `Dim x As New MyLib.MyThing` binding and auto-instantiation path |
| `v424` | IR lowering I | early-bound call/property IR and VM contract wiring |
| `v425` | Runtime execution I | initial early-bound invoke lane (dispatch-backed first subset) |
| `v426` | Design-to-code gate | planning closure, docs sync, compile/tests green |
| `v427` | Dual strategy runtime II | explicit policy-controlled vtable/dispatch strategy scaffolding |
| `v428` | Diagnostics hardening | stable bind/runtime diagnostic families + taxonomy mapping |
| `v429` | Test server typelib v1 | controlled COM server with dual interface + typelib fixture |
| `v430` | E0 ingest lane | metadata ingest and schema validation tests |
| `v431` | E1 PMR lane | reference resolution conformance tests |
| `v432` | E2 binder lane | compile-time early-bind fixtures and diagnostics tests |
| `v433` | E3 runtime lane A | method/property early-bound runtime smoke tests |
| `v434` | E3 runtime lane B | error path tests (missing member/signature mismatch/byref mismatch) |
| `v435` | E4 cache lane | invalidation and deterministic replay tests |
| `v436` | E5 end-to-end lane A | project integration fixtures mixing project+typelib refs |
| `v437` | E5 end-to-end lane B | `As New` + reference precedence + mixed early/late bind tests |
| `v438` | Formal lane setup | E6 harness registration and deferred gate tracking updates |
| `v439` | Formal lane run I | first reduced-state Kani/property run set |
| `v440` | Formal lane foldback | moderate-fix pass + backlog/deferred updates |
| `v441` | Performance baseline | metadata/cache overhead instrumentation and baseline capture |
| `v442` | Performance pass I | member lookup index/cache optimizations |
| `v443` | Performance pass II | call-site handle cache and hot path tightening |
| `v444` | Robustness pass I | fuzz/property stress for resolution and diagnostics totality |
| `v445` | Robustness pass II | negative corpus expansion and deterministic failure checks |
| `v446` | Oracle prep I | Excel/VBA oracle fixture mapping for early-binding topics |
| `v447` | Oracle prep II | deferred oracle gate definitions + script scaffolding |
| `v448` | Compatibility sweep | cross-check with existing late-bound and PMR behavior |
| `v449` | Regression matrix update | integration suite matrix and docs evidence sync |
| `v450` | Docs/spec normalization | source crosswalk and clause ownership completeness |
| `v451` | CI lane integration | add conformance lane orchestration scripts |
| `v452` | CI lane stabilization | flaky lane hardening and deterministic artifact paths |
| `v453` | Runtime/JIT parity check | parity checks for early-bound lowering subset |
| `v454` | HAL/policy gate check | compile-time/runtime unsupported policy behavior coverage |
| `v455` | Security/safety review | unsafe/FFI boundary review checklist + tests |
| `v456` | Formal lane run II | second Kani/property round with tuned scopes |
| `v457` | Formal lane foldback II | unresolved formal deltas deferred with rationale |
| `v458` | Conformance rerun | full E0..E5 rerun and evidence refresh |
| `v459` | Documentation closure I | implementation-defined/uncertainty register final update |
| `v460` | Documentation closure II | status tour + plan-to-implementation trace report |
| `v461` | Gate prep I | checklist and evidence integrity validation |
| `v462` | Gate prep II | drift checks (`meta-check`, clause drift, fixture lint) |
| `v463` | Integrated gate rehearsal | dry-run of full gate command set |
| `v464` | Integrated gate run | final verification and conformance summary capture |
| `v465` | Closure write-up | final profile evidence artifact + outstanding deferred list |
| `v466` | Terminal gate | ladder complete and ready for next series |

## Constraints

1. Non-Windows COM early-binding runtime behavior remains deterministic unsupported.
2. Formal verification failures are non-blocking unless memory-safety unsoundness is indicated.
3. No hidden fallback between early-bound and late-bound lanes; strategy transitions must be explicit and policy-controlled.
