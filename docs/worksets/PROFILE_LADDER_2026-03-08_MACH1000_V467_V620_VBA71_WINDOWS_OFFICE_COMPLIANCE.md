# PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE

## Range

- Ladder span: `v467..v620`
- Program theme: VBA 7.1 + Windows Office COM full compliance closure
- Terminal gate: `v620`

## Objectives

1. Close all known semantic gaps required for a strict VBA 7.1 parity claim on Windows.
2. Implement full COM parity for Office-style hosting, including full property/default-member semantics.
3. Produce a complete evidence package supporting a strict "100%" claim contract.

## Decision gates (must lock early)

1. `DG-01` COM HAL v2 invoke contract (`method/get/put/putref/named args/lcid/excepinfo`).
2. `DG-02` Runtime value model for object/reference semantics and COM argument bundles.
3. `DG-03` Default-member and `Set`/`Let` assignment semantic model.
4. `DG-04` Claim contract language and residual-scope policy.

## Step map

| Step | Focus | Deliverables |
|---|---|---|
| `v467` | Program bootstrap | compliance claim contract draft + ladder/workset registration |
| `v468` | Source baseline lock | clause/source snapshot and uncertainty registry freeze |
| `v469` | Domain inventory | gap matrix for language/runtime/property/com/event/host domains |
| `v470` | Divergence triage | open divergences classified by severity + closure owner map |
| `v471` | Decision gate prep | design alternatives for COM HAL v2 + runtime value model |
| `v472` | `DG-01` lock | approved COM HAL v2 invoke contract and adapter obligations |
| `v473` | `DG-02` lock | approved runtime value transport/object reference model |
| `v474` | `DG-03` lock | approved `Set`/`Let` and default-member semantic model |
| `v475` | `DG-04` lock | approved "100%" claim contract and residual-scope policy |
| `v476` | Planning closure gate | phase decomposition + ownership + gate scripts update |
| `v477` | Spec closure I | property/default-member normative spec draft |
| `v478` | Spec closure II | COM invoke/dispatch flag semantics spec draft |
| `v479` | Spec closure III | COM marshalling/byref/out semantics spec draft |
| `v480` | Spec closure IV | Host Project + root object semantic spec update |
| `v481` | Spec closure V | event parity addendum for COM-EVT-A/B and non-COM parity |
| `v482` | Clause catalog sync | full clause map update and ownership closure |
| `v483` | Diagnostics map sync | diagnostics taxonomy extension for new semantic surfaces |
| `v484` | Formal obligation plan | deep obligation set design for new lanes |
| `v485` | Conformance lane plan | lane matrix `CP-L0..CP-L12` specification |
| `v486` | Spec gate | governance check + clause drift + plan docs closure |
| `v487` | Binder model I | explicit assignment intent model (`Set`/`Let`) in bound IR |
| `v488` | Binder model II | property target intent classification and diagnostics |
| `v489` | Binder model III | default-member read/write resolution rules and disambiguation |
| `v490` | Binder model IV | call-context default-member + call-vs-value parity |
| `v491` | Binder model V | named/optional argument parity in late/early binding |
| `v492` | Binder model VI | property get/let/set signature enforcement |
| `v493` | Project rewrite model I | replace hardcoded dispatch member tables with metadata model |
| `v494` | Project rewrite model II | typelib-driven member kind/default marker mapping |
| `v495` | Project rewrite model III | compile-time diagnostics for unsupported metadata mismatches |
| `v496` | Binder integration gate | compiler tests + project-level conformance sweep |
| `v497` | Runtime assignment I | runtime reference semantics for `Set` assignments |
| `v498` | Runtime assignment II | runtime `Let` semantics separation and coercion paths |
| `v499` | Runtime property I | property get/let/set runtime operation model |
| `v500` | Runtime property II | indexed/default property runtime behavior |
| `v501` | Runtime error model I | COM/property failure to VBA error state mapping |
| `v502` | Runtime error model II | resume/handler interaction for COM invoke failures |
| `v503` | Runtime object lifecycle I | RC and deterministic termination parity checks |
| `v504` | Runtime object lifecycle II | stress teardown and graph edge cleanup |
| `v505` | Runtime integration gate | VM/runtime semantic parity suite green |
| `v506` | COM HAL v2 scaffold | trait/API expansion and adapter migration scaffolding |
| `v507` | COM invoke path I | `DISPATCH_PROPERTYGET` and method path hardening |
| `v508` | COM invoke path II | `DISPATCH_PROPERTYPUT` support with named args |
| `v509` | COM invoke path III | `DISPATCH_PROPERTYPUTREF` support with named args |
| `v510` | COM invoke path IV | LCID propagation and excepinfo diagnostics mapping |
| `v511` | COM marshalling I | scalar VARIANT and object pointer shape coverage |
| `v512` | COM marshalling II | SAFEARRAY and byref payload coverage |
| `v513` | COM marshalling III | out/inout mutation parity and copyback rules |
| `v514` | COM dual-interface I | dispatch/vtable strategy parity and policy locks |
| `v515` | COM dual-interface II | parity tests across invocation strategies |
| `v516` | COM client gate | controlled COM client lane closure |
| `v517` | Typelib ingestion I | member kind + dispid + invoke kind metadata |
| `v518` | Typelib ingestion II | default member (`UserMemId`/`DISPID_VALUE`) metadata |
| `v519` | Typelib ingestion III | optional/named parameter metadata and flags |
| `v520` | Typelib ingestion IV | event metadata closure (`A/B` paths, arg shapes) |
| `v521` | Early-bind integration I | compiler uses full member metadata for rewrite/lowering |
| `v522` | Early-bind integration II | compile-time arity/kind/putref diagnostics closure |
| `v523` | Early-bind integration III | default member and indexed property compile-time parity |
| `v524` | Late-bind integration I | runtime late-bound default-member parity improvements |
| `v525` | Late-bind integration II | named/optional invoke packing parity |
| `v526` | Metadata integration gate | binder/runtime/typelib integrated lane green |
| `v527` | COM server model I | server-side dispatch contract completeness |
| `v528` | COM server model II | GetIDsOfNames/Invoke parity details |
| `v529` | COM server model III | property put/putref behavior parity server-side |
| `v530` | COM server model IV | excepinfo/HRESULT mapping parity |
| `v531` | COM server model V | class/type exposure and registration behavior |
| `v532` | COM server model VI | controlled server fixture expansion |
| `v533` | COM server gate | client/server roundtrip parity lane green |
| `v534` | Event parity I | non-COM runtime graph parity completion |
| `v535` | Event parity II | host ingress parity and teardown/reassignment closure |
| `v536` | Event parity III | COM-EVT-A parity completion |
| `v537` | Event parity IV | COM-EVT-B parity completion (or approved residual policy) |
| `v538` | Event parity V | callback signature/default-member/event-arg edge matrix |
| `v539` | Event gate | integrated event parity lane gate |
| `v540` | Host model I | Host Project semantics and root object parity |
| `v541` | Host model II | global/default exposure and workbook/add-in style behavior |
| `v542` | Host model III | host service/HAL policy semantics under callbacks |
| `v543` | Host model IV | project tooling and host contract evidence closure |
| `v544` | Host gate | host integration lane closure |
| `v545` | Oracle plan freeze | Office differential corpus and matrix freeze |
| `v546` | Oracle lane I | property/default-member/set-let oracle captures |
| `v547` | Oracle lane II | COM invoke flags/named args/optional params captures |
| `v548` | Oracle lane III | COM events A/B callback behavior captures |
| `v549` | Oracle lane IV | lifecycle/error mapping captures |
| `v550` | Oracle foldback I | divergence reconciliation and clause updates |
| `v551` | Oracle foldback II | diagnostics/evidence sync and deferred gate cleanup |
| `v552` | Oracle gate | required Office matrix green |
| `v553` | Formal lane I | Kani harness implementation for new invariants |
| `v554` | Formal lane II | property/default-member semantic invariant proofs |
| `v555` | Formal lane III | COM invoke/flag/named-arg invariant proofs |
| `v556` | Formal lane IV | RC/lifecycle/event invariant proofs |
| `v557` | Formal lane V | async full run + foldback for unresolved items |
| `v558` | Formal gate | formal obligations complete and reconciled |
| `v559` | Perf/robustness I | hot-path checks under full semantics |
| `v560` | Perf/robustness II | fuzz/stress expansion and deterministic replay |
| `v561` | Perf/robustness III | VM/JIT parity sweeps for compliance lanes |
| `v562` | Robustness gate | stability and nondeterminism checks green |
| `v563` | Documentation closure I | spec set finalized and cross-linked |
| `v564` | Documentation closure II | conformance/formal/evidence indexes finalized |
| `v565` | Governance closure I | governance script and artifact checks pass |
| `v566` | Governance closure II | no drift in diagnostics/clauses/catalogs |
| `v567` | Integrated rehearsal I | dry-run of full terminal gate command set |
| `v568` | Integrated rehearsal II | fix/foldback from rehearsal failures |
| `v569` | Integrated gate I | full integrated run pass #1 |
| `v570` | Integrated gate II | full integrated run pass #2 (repeatability) |
| `v571` | Divergence finalization I | close all in-scope divergence records |
| `v572` | Divergence finalization II | close all in-scope deferred gate rows |
| `v573` | Claim package I | compliance report draft |
| `v574` | Claim package II | matrix + formal + diagnostics appendices |
| `v575` | Claim package III | reviewer checklist and signoff packet |
| `v576` | Release prep I | branch protection + CI profile lock |
| `v577` | Release prep II | reproducible rerun scripts and artifact integrity hashes |
| `v578` | Terminal rehearsal | final pre-terminal dry run |
| `v579` | Terminal execution I | terminal gate command run |
| `v580` | Terminal execution II | repeat run to confirm deterministic closure |
| `v581` | Post-terminal audit I | spot checks and evidence verification |
| `v582` | Post-terminal audit II | claim language consistency audit |
| `v583` | Closure corrections | bounded cleanup from audit findings |
| `v584` | Closure freeze | freeze compliance dossier artifacts |
| `v585` | Program signoff prep | final stakeholder review package |
| `v586` | Program signoff | signoff record with approved claim text |
| `v587` | Contingency buffer I | reserved for blocker resolution |
| `v588` | Contingency buffer II | reserved for blocker resolution |
| `v589` | Contingency buffer III | reserved for blocker resolution |
| `v590` | Contingency buffer IV | reserved for blocker resolution |
| `v591` | Contingency buffer V | reserved for blocker resolution |
| `v592` | Contingency buffer VI | reserved for blocker resolution |
| `v593` | Contingency buffer VII | reserved for blocker resolution |
| `v594` | Contingency buffer VIII | reserved for blocker resolution |
| `v595` | Contingency buffer IX | reserved for blocker resolution |
| `v596` | Contingency buffer X | reserved for blocker resolution |
| `v597` | Matrix rerun lock I | scheduled rerun for long-tail confidence |
| `v598` | Matrix rerun lock II | scheduled rerun for long-tail confidence |
| `v599` | Formal rerun lock | scheduled rerun for long-tail confidence |
| `v600` | Governance rerun lock | scheduled rerun for long-tail confidence |
| `v601` | Evidence archive prep | archive packaging and reproducibility index |
| `v602` | Evidence archive finalize | final artifact archive set |
| `v603` | Compliance dossier publish | publish consolidated dossier |
| `v604` | Post-publish check | integrity and link validation pass |
| `v605` | External review window I | reviewer feedback intake |
| `v606` | External review window II | reviewer feedback foldback |
| `v607` | External review closure | resolve/record all reviewer items |
| `v608` | Final rerun command pack | complete command pack rerun |
| `v609` | Final rerun evidence pack | rerun artifacts and hashes |
| `v610` | Final drift check | no doc/spec/diagnostic drift |
| `v611` | Final blocker sweep | blocker register must be empty |
| `v612` | Terminal packet freeze | immutable final packet |
| `v613` | Executive summary draft | one-page compliance summary |
| `v614` | Executive summary finalize | approved summary text |
| `v615` | Claim publication prep | release-note and claim publication draft |
| `v616` | Claim publication finalize | approved publication text |
| `v617` | Terminal gate precheck | final checklist pass |
| `v618` | Terminal gate run | full compliance gate execution |
| `v619` | Terminal gate verification | verify deterministic rerun equivalence |
| `v620` | Terminal gate close | ladder complete; full-compliance claim eligible |

## Constraints

1. Clean-room constraints remain non-negotiable; oracle behavior must be black-box observed and reproducible.
2. "100%" claim is forbidden unless all four claim model conditions are satisfied (coverage/divergence/deferred/oracle+formal).
3. No silent fallback across COM strategy lanes; policy transitions must be explicit and observable.
4. All behavior-affecting changes require synchronized diagnostics, conformance topics, and governance checks.

## Companion workset

- `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
