# Specification and Guidance Deprecation Ledger

Date: 2026-07-10
Status: current authority migration ledger
System clause: `DOC-AUTH-001`

## 1. Rule

Deprecated documents remain available for provenance, historical evidence and recovery of useful tests. They are not current architecture, implementation or capability authority.

Where a deprecated document conflicts with its successor, the successor wins. Historical implementation snapshots cannot be cited as proof for the current clean stack without current-route replay.

## 2. Architecture migrations

| deprecated/historical document or family | reason | current successor |
|---|---|---|
| `docs/ARCHITECTURE.md` before 2026-07-10 | described VM2, Bundle/linearize and a stub JIT | rewritten `docs/ARCHITECTURE.md` plus `OXVBA_SYSTEM_CONTRACT_V1.md` |
| `OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md` | conflated Core IR with bytecode/package and recorded removed frontend architecture | `OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md` |
| `HIR_RESOLUTION_ENVIRONMENT_V1.md` | superseded HIR-era resolution plan | compiler contract V2 |
| `EXECUTABLE_SEMANTIC_PACKAGE_V1.md` | Bundle/bytecode-centered target predates OxIR/OxImage | `OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md` |
| `EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md` | implementation map targets retired package/VM stages | OxIR/Image contract plus core workset/matrices |
| `BYTECODE_VM_SEMANTIC_CONTRACT_V1.md` | bytecode/VM2 terminology no longer describes the product interpreter | OxIR/Image contract |
| `VMR06_DESCRIPTOR_DRIVEN_BEHAVIOR_SELECTION_V1.md` | historical VM/package delivery slice | OxIR/Image contract and current workset |
| `JIT_V2_IMPLEMENTATION_DESIGN_V1.md` | planning design predates the implemented direct OxIR JIT and current ideal typed/session target | `OXVBA_JIT_ARCHITECTURE_V1.md` |
| `JIT_V2_PROC_LOWERING_IR_V1.md` | proposed IR was not implemented as described; useful design input only | JIT architecture §3 |
| `JIT_V2_HELPER_ABI_CATALOG_V1.md` | planning catalog is not the current runtime-helper contract | JIT architecture §7 and `RUNTIME-ABI-001` |
| `JIT_V2_SEMANTIC_CONTRACT_AND_FACT_PACK_V1.md` | planning fact-pack shape is not the current artifact | system/OxIR/JIT contracts |
| `JIT_V2_RUN_PROTOCOL_V1.md` | last-program entry and leaked-descriptor rules are current gaps, not destination rules | OxIR/Image and JIT contracts |
| `JIT_V2_DIFFERENTIAL_HARNESS_V1.md`, tracer plan | planning/status detail is stale; test ideas remain useful | `CONF-DIFF-001` and core workset |
| `docs/OXVBA_JIT_PLAN.md` | historical milestone ledger and stale opening status | JIT architecture plus current core/Windows worksets |

## 3. Language-service migrations

| deprecated/historical document | reason | current successor |
|---|---|---|
| `LANGUAGE_SERVICE_SPEC_V1.md` | claims a bounded active implementation that was removed/deleted | `OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md` |
| `LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md` | builds on the deleted V1 service and stale matrix truth | language-service architecture plus current LS workset |
| `docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md` | describes deleted APIs/crates | language-service architecture §§2-6 |
| `docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md` | describes deleted host/service integration | language-service architecture §§4, 10 |
| `docs/LANGUAGE_SERVICE_SHOWCASE.md` | showcase is not runnable on the clean stack | current LS workset/editor smoke gate |
| `OXIDE_DIRECT_HOST_SESSION_FACADE_V1.md` | claims an implemented facade over deleted service/runtime assumptions | language-service architecture and future host workset |
| `OXVBA_EMBEDDED_BUILD_RUN_CONTRACT_V1.md` | cites deleted workspace/session APIs as current substrate | system contract `HOST-SESSION-001`, current architecture and future host work |
| older LSP/OxIde worksets and v0.2 evidence | historical execution/evidence for removed stack | current LS workset; reuse only through explicit port/replay beads |

## 4. Runtime, VM and build migrations

| deprecated/historical document | reason | current successor |
|---|---|---|
| `docs/VM3_COMPLETION_AND_VM2_RETIREMENT_PLAN.md` | retirement plan is historical; VM3 is now sole interpreter | current architecture §7 and OxIR/Image contract |
| `docs/OXIR_VM3_ORACLE_HANDOFF.md` | handoff snapshot, not current completion authority | dated review, core workset and canonical matrices |
| `.oxb`/Bundle-centered build guidance | product artifact is now `.oxi`; bounded Bundle metadata remains internal | architecture §§5, 10 and OxIR/Image contract |
| `NATIVE_READY_*` implementation snapshots | useful provenance but predate current JIT and package realization | system/JIT/Windows contracts and current worksets |

## 5. Debugger, web and extended profiles

Debugger, DAP, immediate evaluator, forms, browser/WASM, web-host and OxIde documents remain design inputs for `PROFILE-EXT-001` unless a new accepted workset promotes them. Any statement that their crates or product paths are currently implemented must be read as historical.

In particular, `OXVBA_DEBUG_HANDLE_DESIGN.md`, its test catalog and older debugger/DAP handoffs do not describe active workspace crates as of 2026-07-10.

## 6. Historical worksets and ladders

Worksets dated before the three 2026-07-10 readiness worksets are not automatically active. MACH-1000 ladders, IP-08/IP-08B sequences and earlier frontend/JIT/language-service programs remain history unless a current workset explicitly imports a residual row.

This does not erase completed implementation or evidence. It removes those plans as competing present-tense authority.

## 7. Deprecation presentation

High-risk misleading documents receive an in-file deprecation notice. Other historical files are governed by this ledger and the current indexes. Moving files is optional and should occur only when link migration is worth the churn; explicit authority is more important than directory location.

New documents must link current successors rather than extending a deprecated architecture family.
