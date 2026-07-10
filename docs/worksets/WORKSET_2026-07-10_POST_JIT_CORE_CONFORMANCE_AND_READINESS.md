# Workset: Post-JIT Core Conformance and Readiness

Date: 2026-07-10
Owner: unassigned
Status: proposed; bead rollout not yet performed
Type: capability and conformance delivery
Source review: [`OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`](../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md)

## 1. Outcome

Bring the clean OxVba compiler, shared executable package, VM3 and the non-Windows-dependent JIT surface to an evidence-backed VBA-conforming baseline suitable for:

- use as the trusted language/runtime base for DNA Calc;
- execution through either VM3 or JIT without semantic qualification for the accepted core scope;
- loading and executing a verified standalone `OxImage` artifact;
- long-lived embedded/package sessions;
- current-stack Excel/VBA validation on Windows;
- handoff to the separate Windows COM/native and language-service worksets.

This workset is not complete when individual defects, a bounded corpus, documentation, or an audit is complete. It closes only when the scoped compiler/package/VM/JIT behavior is parity-complete, required evidence is green, and every residual accepted gap has been delivered rather than merely recorded.

## 2. Scope

### 2.1 In scope

- source ingestion, lexing, parsing and conditional compilation;
- compiler diagnostics and source maps;
- VBA declarations, typing, binding, calls, coercions and project-reference semantics;
- VBA base-library behavior not intrinsically dependent on real Windows COM/native calls;
- Core IR and Core IR-to-OxIR elaboration;
- `OxProgram`/`OxImage` verification, versioning, loading, linking and provenance;
- runtime carriers, shared evaluation semantics and rt-abi safety;
- VM3 completeness and reference-runtime trust;
- JIT support for the full accepted platform-neutral OxIR/language/runtime/library surface;
- JIT error handling, recursion, sessions, cache and lifecycle;
- VM3/JIT differential observability and corpus coverage;
- test reliability, cross-platform line-ending behavior and safety lanes;
- Windows and Excel/VBA oracle validation of core language/compiler/runtime behavior;
- architecture, specs, matrices and status-truth reconciliation.

### 2.2 Explicitly out of scope

These are owned by [`WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md`](WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md):

- real COM activation, `IUnknown`/`IDispatch` or native vtable calls in JIT code;
- COM connection points and native event sinks;
- VBA classes served as COM servers through the JIT;
- Windows `Declare` execution and pointer/callback lowering in the JIT;
- native DLL/EXE export and AOT PE packaging.

These are owned by [`WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md`](WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md):

- editor workspace/session APIs;
- semantic snapshot indexing/query APIs and editor caches;
- LSP transport and editor integration.

This workset owns compiler analysis production: correctness, declaration/use-site bindings, expression/member/call facts, typed signatures, original/virtual source provenance and source-located diagnostics. It must expose an `AnalysisResult`-style fact sink that can retain diagnostics and partial/unknown facts when no valid CoreProgram can be emitted. The language-service workset consumes those facts into immutable snapshots, indices and queries. The two worksets may not create parallel binders.

Also outside this basics workset are the forms runtime/designer, debugger/debug protocol, broader product security model, Office object-model implementation, polished IDE UX, and macOS/browser/WASM/Tauri certification. Their status is not changed or implied by this workset.

### 2.3 Target boundary

The target is VBA 7 language, runtime and base-library behavior as specified publicly and observed reproducibly in Excel/VBA. The certification targets for this workset are:

- Linux x64 VM3 and JIT execution for platform-neutral behavior;
- Windows x64 VM3 and JIT execution for the same core behavior;
- Windows x86 and x64 compile-time/VBA conditional semantics, with VM3 runtime coverage where pointer width affects the core;
- 32-bit and 64-bit Excel/VBA compile/runtime oracle evidence for width-sensitive core rows.

Windows x86 JIT session/codegen certification is a prerequisite owned by the Windows workset. macOS and browser/WASM are explicit unassessed targets, not an implied green row. Any narrower certification target requires a user-approved workset amendment and an explicit successor owner; it may not be hidden as an unsupported test skip.

## 3. Binding invariants

1. Real VBA behavior is the compatibility target.
2. VM3 remains the reference interpreter; the JIT is proven against it and Excel/VBA, not against historical OxVba behavior.
3. Both backends consume the same verified `OxImage` and accepted OxIR vocabulary.
4. No product JIT path silently falls back to VM3.
5. Every accepted `OxInst` either executes in both backends or is rejected by the package verifier for the declared target.
6. External images use bounded decoding with declared length/resource limits before large allocation, then sealed semantic verification before linking, code generation or execution.
7. Panics, malformed artifacts and unsupported targets return stable diagnostics; they do not abort or silently mis-execute.
8. Source locations remain traceable through preprocessing, project normalization, binding, elaboration and runtime errors.
9. Old oracle captures remain target expectations until the current clean stack replays them.
10. Support-only beads do not close capability epics.

## 4. Current entry evidence

The 2026-07-10 review established:

- 843 focused syntax/symbol/bind/project tests pass;
- 348 bundle/eval/OxIR/rt-abi/runtime/VM3 tests pass;
- 164 JIT crate tests pass;
- a single-thread differential run passes 1,138 tests and fails two deterministic gates;
- the full workspace run is red;
- Clippy is red;
- the captured 56-case oracle replay is green but narrow;
- live Excel/COM/native-JIT validation was not run;
- architecture and validation truth are materially stale.

The workset begins from `in-progress`, not from an assumed complete baseline.

## 5. Canonical truth artifacts

The rollout must create or designate:

1. `docs/validation/CORE_COMPILER_VM_JIT_READINESS_MATRIX_V1.csv`
   - independently closable rows split whenever backend, target, evidence authority or residual status differs;
   - compiler, VM3, JIT, Windows/Excel oracle, formal/safety and test anchors;
   - explicit subset and residual ownership.
2. `docs/validation/VBA_BASE_LIBRARY_PARITY_MATRIX_V1.csv`
   - one independently closable row per public member/overload and host profile;
   - exact signature/return, compiler, VM3, JIT, side-effect/Err and Excel/spec evidence.
3. `docs/validation/OXIR_BACKEND_SUPPORT_MATRIX_V1.csv`
   - every instruction and terminator;
   - verifier, elaborator, VM3, JIT, fault/lifecycle and test status.
4. `docs/validation/OXIMAGE_PACKAGE_CONTRACT_MATRIX_V1.csv`
   - format/header, verification, ABI/layout, capabilities, provenance, source map, linker and malformed-input rows.
5. `docs/validation/CURRENT_STACK_EXCEL_ORACLE_MATRIX_V1.csv`
   - compile and runtime rows tied to clean-stack source fixtures and captured results.
6. a generated summary derived from those matrices; no competing hand-maintained completion summary.

Existing matrices should be migrated, superseded or archived rather than silently abandoned.

Matrix and bead granularity is binding: split rows whenever target, backend, observable, evidence authority or residual owner differs. Broad candidates such as grammar completion, call legality, full verification, corpus coverage and oracle sweeps are epic seeds; rollout must split them into reviewable fixture/type/metadata/member tranches. Every prepared bead must state type, direct dependencies, matrix row IDs, touched truth surfaces, exact commands and residual behavior.

## 6. Execution epics

### CORE-0 — Initiation, truth reset and bead rollout

Type: support

Why separate: every later closure claim depends on a trustworthy matrix and removal of deleted architecture from active status.

Required outcomes:

- create the workset root and one child epic per `CORE-*` lane;
- create a rollout bead under each epic and a believable first delivery path;
- inventory active/deleted/stale specs, worksets, blockers, matrices and test paths;
- publish all canonical matrix skeletons, including the member-level library and current-stack oracle matrices;
- identify every open `bd-aprs`/legacy-ladder item that must be rehomed, superseded or retained;
- update `docs/ARCHITECTURE.md` to the current pipeline before implementation closure language is used.

First bead candidates:

| candidate | type | outcome | close evidence |
|---|---|---|---|
| CORE-0.1 | support | roll out all workset epics and delivery beads | bead tree, dependencies and next ready delivery beads exist |
| CORE-0.2 | support | replace active architecture with CoreProgram -> OxIR/OxImage -> VM3/JIT truth | architecture review plus governance check |
| CORE-0.3 | support | reconcile obsolete front-end/JIT/VM/language matrix paths | explicit supersede/archive/rehoming map |
| CORE-0.4 | support | seed canonical readiness/package/backend/oracle matrices | every review finding and current residual has an owner |

Close condition: truth surfaces agree on current architecture and every required capability lane has an open delivery path.

### CORE-1 — Restore deterministic repository gates

Type: delivery/support mix

Why separate: semantic work cannot close against a red or platform-sensitive baseline.

Required outcomes:

- fix `jit_scope.snap` and related snapshots so line endings are platform-independent;
- add a repository EOL policy;
- make handle-balance tests isolated and fixture-addressable;
- minimize and resolve the policy-error BSTR imbalance, distinguishing real leak from measurement lifetime;
- remove parallel interference from process-global carrier counters;
- update stale host tests to assert stable diagnostic codes rather than obsolete message text;
- repair the stale `New Collection` unsupported test;
- make Windows native-import tests pass while the JIT still honestly returns a stable unsupported diagnostic;
- fix all Clippy failures and the dead-code warning;
- repair the machine-readable AutoRun terminal-gate drift so governance and meta checks execute beyond gate sync;
- add one canonical cross-platform gate runner with exact commands, filters, environment, timeout and evidence-output paths for default-parallel and single-thread execution;
- require ordinary `cargo test --workspace` and strict Clippy to pass on Linux and Windows.

First bead candidates:

| candidate | type | outcome | close evidence |
|---|---|---|---|
| CORE-1.1 | delivery | cross-platform snapshot normalization and `.gitattributes` policy | Windows and Linux snapshot gates byte/line equivalent |
| CORE-1.2 | delivery | carrier-balance harness isolation with fixture identity | parallel and serial differential runs agree |
| CORE-1.3 | delivery | policy-error BSTR imbalance minimized and fixed | minimized regression plus zero balance |
| CORE-1.4 | delivery | stale host/JIT unsupported assertions reconciled | host unit/native lanes green without weakening decline checks |
| CORE-1.5 | delivery | strict Clippy and unsafe-contract ratchet restored | workspace all-target Clippy green |
| CORE-1.6 | delivery | canonical core gate runner and CI entry | the same checked-in command surface runs on Linux and Windows and records artifacts |

Close condition: format, strict Clippy, ordinary workspace tests and serial/parallel differential baseline are green on Linux and Windows.

### CORE-2 — Source, lexer, parser and preprocessor hardening

Type: delivery

Required outcomes:

- make all byte traversal UTF-8 safe and panic-free;
- define supported VBA source encodings, code-page behavior and Unicode identifier policy;
- support or deterministically diagnose exported Office module encodings;
- require total expression parsing when used as a compile-time expression;
- fail closed on malformed `#If`, `#ElseIf`, `#Else`, `#End If` and `#Const`;
- preserve original source offsets through conditional blanking;
- correct case-insensitive file extension and Attribute parsing;
- verify class preamble, startup shim and top-level-mainline source mapping;
- anchor every accepted grammar row to a fixture and route proof;
- add parser/lexer fuzz and malformed-input no-panic gates.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-2.1 | UTF-8-safe lexer and invalid-text diagnostic | arbitrary valid UTF-8 lexer properties; arbitrary-byte/code-page cases at the file-decoding boundary |
| CORE-2.2 | total expression API | trailing-token cases reject with source spans |
| CORE-2.3 | fail-closed conditional compilation | negative directive matrix matches Excel compile behavior |
| CORE-2.4 | source encoding and exported-module loader contract | Windows code-page fixtures plus documented supported set |
| CORE-2.5 | project normalization source-map preservation | diagnostics point to original files/lines after every rewrite |
| CORE-2.6 | grammar matrix fixture completion | no accepted row lacks a current clean-route test |

Close condition: accepted source forms never panic, malformed compile-time syntax cannot select code silently, and every compiler diagnostic maps to original source where applicable or carries explicit virtual/generated-source provenance.

### CORE-3 — Typed binder, calls, diagnostics and project references

Type: delivery

Required outcomes:

- preserve declared return types through referenced-project, VBA-library, host, intrinsic and Declare bindings;
- define a single typed callable signature contract used by compiler, package and language-service facts;
- complete ByVal/ByRef, array, UDT, object/interface, Optional and ParamArray legality/coercion matrices;
- validate Declare arity/type/call-site legality at the VBA compile-time boundary;
- implement cross-project public module-variable and class-field access;
- reject duplicate/ambiguous references deterministically;
- make all syntax, symbol and bind diagnostics carry module/file/span and stable code;
- publish compiler-owned declaration/use-site, expression/member/call, argument-mapping, accessor/default-member and provenance facts through one analysis result;
- separate tolerant analysis from strict compilation: malformed/incomplete input may produce facts and diagnostics, but only an error-free result may expose a CoreProgram to code generation;
- replace arbitrary default-member depth limits with cycle-aware behavior or oracle-backed limits;
- implement `DefDec`; define `Option Compare Database` through an explicit host-supplied collation contract plus Access/VBA oracle evidence, or leave it open through an approved target amendment;
- cover Option Private, broken references, diamonds, qualification, visibility and public-type leakage.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-3.1 | typed callable/return contract reaches Core IR | typed cross-project/library overflow and coercion differentials |
| CORE-3.2 | full ordinary argument compatibility matrix | compiler/runtime timing matches current Excel oracle |
| CORE-3.3 | Declare compile-time legality matrix | VM/JIT-independent compiler cases match VBA |
| CORE-3.4 | cross-project public data import/export | source and serialized referenced-project tests |
| CORE-3.5 | source-located compiler diagnostics | message/code/span snapshots and editor-ready DTOs |
| CORE-3.6 | default-member cycle semantics | recursive graph terminates with VBA-compatible result/error |
| CORE-3.7 | residual declaration/options completion | `DefDec` plus host-collation-backed `Option Compare Database` delivered and Access/VBA-proven, or an approved open target split |
| CORE-3.8 | compiler analysis/fact sink | valid strict/editor facts are identical; malformed analysis cannot reach codegen |

Close condition: the canonical language matrix has no accepted compiler row dependent on static-type erasure, missing project data or locationless diagnostics.

### CORE-LIB — VBA base library and portable-host parity

Type: delivery

Why separate: library completeness cannot be inferred from language corpora or from a count of intrinsic tests. Every public member and overload needs an independently closable compile-time and runtime row.

Required outcomes:

- inventory the declared VBA base-library modules, classes, constants, enums, members, overloads, defaults and aliases against public specifications and the selected Office/VBA oracle;
- preserve exact typed signatures and return types through providers, binding, Core IR and both backends;
- cover positional, named, omitted, Optional, ParamArray and ByRef behavior for every applicable member;
- assign each member to shared evaluation, runtime, HAL/host policy, file/settings/environment, date/locale, interaction or explicitly Windows-owned implementation;
- prove value, Err, side effect, state/lifecycle and host-call ordering under VM3 and JIT;
- define locale, calendar, code-page, filesystem, environment, time, randomness and interaction test profiles;
- complete the known open CCT-033 stateful file-I/O rows, including richer `Input #`, modes, encodings and error paths;
- reject or route host-denied operations with VBA-compatible errors rather than backend-specific convenience behavior;
- capture current Excel/VBA evidence for observable member families and public-spec evidence for non-oracleable host contracts;
- derive the library status report from the canonical matrix rather than a hand-maintained “broad subset” claim.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-LIB.1 | authoritative member/signature inventory | every public member has independent compiler/VM3/JIT/oracle/host-policy rows |
| CORE-LIB.2 | pure scalar/string/conversion/math families | typed call and structural differential tranches |
| CORE-LIB.3 | date/time/locale/calendar/random families | deterministic profiles plus non-default-locale Excel evidence |
| CORE-LIB.4 | collection/object/array and stateful families | state, identity, side-effect and cleanup evidence |
| CORE-LIB.5 | file/settings/environment/interaction host families | allowed/denied/error matrices, including CCT-033 |
| CORE-LIB.6 | library terminal sweep | no missing, inferred-only or VM3/JIT-subset member row |

Each candidate above is an epic seed. Rollout must split it by coherent member family and observable; it is not one oversized delivery bead.

Close condition: every in-scope VBA base-library member has exact typed compiler facts and green VM3/JIT/Excel or public-spec-backed behavior for the declared host profile.

### CORE-4 — Verified OxImage and full OxIR contract

Type: delivery

Required outcomes:

- introduce opaque `VerifiedOxProgram` handles contained by `VerifiedOxImage`, or an equivalent sealed verified-state type hierarchy;
- verify every program at deserialize, build, link, host-session and JIT entry;
- prevent production VM3/JIT APIs from accepting raw `OxProgram`; keep any unchecked constructor test-only and visibly named;
- bound lengths, nesting and allocation during decode before semantic verification;
- honor the declared image entry rather than positional convention;
- validate program entry/global initializer, CFG/block IDs, operands/results, types, ranks, arity/signatures, fault edges, records, classes, externals, events, imports and exports;
- reject duplicate case-folded unit/export identities;
- verify target/profile/capability compatibility before execution;
- add content digest, helper ABI version, carrier/layout version, source maps and build/reference provenance;
- make malformed/hostile images panic-free and resource-bounded;
- define compatibility/migration behavior for older image versions;
- separate synthetic VBA-library Bundle metadata from the product executable artifact in code/docs.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-4.1 | package v3 contract and migration decision | reviewed spec and matrix rows |
| CORE-4.2 | sealed verified program/image loader | raw programs/images cannot reach production VM3/JIT/host APIs |
| CORE-4.3 | full instruction/type/metadata verification | mutation tests for every verifier family |
| CORE-4.4 | declared entry and unique linker tables | non-last entry executes correctly; duplicates diagnose |
| CORE-4.5 | ABI/layout/capability/source/provenance metadata | round-trip and incompatible-version tests |
| CORE-4.6 | hostile artifact fuzzing | no panic, UAF or unbounded allocation on fuzz corpus |

Close condition: no public path executes or compiles an unverified image, and the package matrix is complete for the accepted target.

### CORE-5 — Runtime, VM3 ownership and ABI hardening

Type: delivery

Required outcomes:

- make VM3 implement or verifier-reject every accepted OxIR instruction, including `AddRef`, `Release` and `DrainTerminations`;
- generate backend-support exhaustiveness from the OxIR enum;
- unify VM3/JIT runtime-class/interface descriptor projection;
- replace process-lifetime `Box::leak` and leaked host/image ownership with session-owned arenas/Arc graphs;
- make raw-pointer rt-abi functions explicitly unsafe with documented contracts or private behind safe typed wrappers;
- use RAII for drain/reentrancy/bridge state;
- seat deterministic internal faults when helper code panics;
- complete shared semantic ownership for error, lifecycle, array, object and call operations where duplication risks drift;
- stress repeated compile/load/activate/invoke/reset/drop;
- validate Collection-selector and string-coercion risk cases against Excel.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-5.1 | generated OxIR backend/verifier support table | build fails when a new instruction lacks an explicit disposition |
| CORE-5.2 | VM3 lifecycle instruction parity | direct OxIR and source-lowered tests |
| CORE-5.3 | shared class/interface descriptor arena | VM3/JIT use one implementation |
| CORE-5.4 | bounded session ownership | repeated-session memory/handle stress is flat |
| CORE-5.5 | sound rt-abi boundary | safety docs, Miri-capable tests and Clippy green |
| CORE-5.6 | panic/fault RAII hardening | injected panics leave stable Err and reusable session state |
| CORE-5.7 | oracle-sensitive evaluation edges | Collection/Err/string cases match Excel |

Close condition: VM3 is complete for verified OxIR, runtime metadata is not leaked per session, and the ABI/safety gates are green.

### CORE-6 — Complete platform-neutral JIT semantics

Type: delivery

Required outcomes:

- require verified images at JIT compilation;
- implement line-number tracking and exact `Erl` behavior;
- implement writable Err fields and complete `Err.Raise`/`Error` source, description, help file/context and dynamic-number semantics;
- implement safe deep recursion/error 28 without relying on native-stack survival;
- generalize internal call/return coercion and ByRef copy-in/out from allowlists to the typed signature matrix;
- make every accepted platform-neutral OxIR operation compile or return a target-level rejection before partial codegen;
- version and validate the helper ABI;
- decide and document direct OxIR-to-CLIF versus a real lowering IR;
- split the monolithic JIT into reviewable lowering, compiler, ABI/runtime-helper, session/cache and test modules;
- define supported host/architecture/layout policy explicitly;
- remove milestone-number text from durable product diagnostics.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-6.1 | verified-input JIT boundary | malformed image tests return stable diagnostics |
| CORE-6.2 | line/Erl/Err field semantics | VM3/JIT/Excel error matrix green |
| CORE-6.3 | complete dynamic `Err.Raise` metadata | nested handler and propagation corpus |
| CORE-6.4 | non-native-stack recursion model | deep recursive source reaches VBA error 28 safely |
| CORE-6.5 | typed call/coercion generalization | canonical signature matrix has no allowlist-only row |
| CORE-6.6 | helper ABI version and panic diagnostics | incompatible helper version rejects deterministically |
| CORE-6.7 | JIT architecture decision and module split | spec, code boundaries and dependency checks agree |
| CORE-6.8 | explicit target/layout policy | Linux/Windows x64 gates and unsupported-target tests |

Close condition: all accepted platform-neutral language/runtime/library rows execute under the JIT with VM3/Excel parity and no milestone-specific exception list.

### CORE-7 — JIT sessions, cache and standalone package use

Type: delivery

Required outcomes:

- add persistent JIT-backed project/package sessions;
- load and compile a verified `.oxi` without recompiling source;
- retain globals, objects, class singletons, events and Err state according to session rules;
- support invoke/reset/reload/drop through the same host-facing session contract as VM3;
- define cache keys over image digest, target ISA, helper ABI, carrier/layout version, host policy/profile and relevant compile settings;
- make compiled code/session ownership thread/apartment safe for its declared target;
- provide cache invalidation, bounded eviction and diagnostics;
- measure cold load+compile+first call and warm repeated calls.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-7.1 | JIT `ProjectRuntimeSession` contract | repeated invokes preserve and reset state correctly |
| CORE-7.2 | verified `.oxi` JIT load path | source-free package integration tests |
| CORE-7.3 | deterministic cache key/invalidation | mutation and incompatible-ABI tests |
| CORE-7.4 | bounded compiled-image cache | eviction and repeated-workspace stress |
| CORE-7.5 | product performance evidence | cold/warm VM3/JIT benchmark report |

Close condition: embedded and standalone package consumers can select either backend with equivalent session semantics.

### CORE-8 — Differential, conformance and safety evidence

Type: delivery

Required outcomes:

- replace status-only scope evidence with VM3/JIT semantic comparison;
- compare structural arrays, records, objects, ProcRefs and object identity rather than tags alone;
- capture the promised observables: results, full Err, side-effect journal, lifecycle ordering, carrier balance and host transport facts where applicable;
- ensure every canonical corpus row is executed or has an explicit matrix-owned unsupported target;
- generate scope/coverage summaries from test manifests;
- make handle counters scoped or serialize every allocation-capable test process;
- add property/fuzz generation for scalar, Variant, calls, control, arrays, records and error edges;
- run ASAN and appropriate Miri/Kani lanes;
- turn every divergence into a minimized permanent fixture;
- establish release-size and performance regressions without weakening correctness.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-8.1 | versioned full observable | VM3/JIT runner emits all required axes |
| CORE-8.2 | structural carrier comparison | nested array/record/object differentials catch payload drift |
| CORE-8.3 | complete corpus manifest | every accepted matrix row maps to a fixture and both backends |
| CORE-8.4 | scoped balance/safety harness | parallel reproducibility plus ASAN/Miri evidence |
| CORE-8.5 | property/fuzz tranche | shrinking and permanent regression workflow |
| CORE-8.6 | derived support/performance report | no hand-maintained competing status |

Close condition: no accepted core row is represented only by “compiled/raised,” tag equality or VM3-minted evidence.

### CORE-9 — Windows and Excel/VBA core oracle certification

Type: delivery/conformance

Boundary: no real COM/native-JIT implementation; those rows route to the Windows workset.

Required outcomes:

- replay current clean compiler, VM3 and JIT against real Excel/VBA for all accepted core rows;
- capture compile-time diagnostics, selected token/line and runtime value/error behavior;
- cover malformed conditional compilation, declaration legality, argument typing, shadowing, default members, source encodings, line numbers, error/Erl, arrays/records/classes, project references and base-library edges;
- test both 32-bit and 64-bit VBA conditional targets where behavior differs;
- validate locale-sensitive date/string/number behavior under an explicit locale matrix;
- preserve captured source, environment, Excel build/bitness, result and cleanup evidence;
- fold every divergence into the canonical matrix and an owning delivery bead.

Oracle execution must follow the repository’s Excel/VBA modal protocol:

- VBE visible for compile checks;
- Debug -> Compile VBAProject;
- UI Automation scoped to the owned Excel/VBE process;
- capture dialog text, selected token and full selected line;
- PID-scoped dialog dismissal and cleanup;
- never treat `Application.Run` as a compile check.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| CORE-9.1 | current-stack oracle harness/environment manifest | reproducible owned Excel run with modal interception |
| CORE-9.2 | compiler diagnostic matrix | code/message/token/line parity |
| CORE-9.3 | runtime/error/core-library matrix | VM3/JIT/Excel observable parity |
| CORE-9.4 | project/reference/source-encoding matrix | real exported modules and multi-project fixtures |
| CORE-9.5 | locale and 32/64-bit matrix | explicit environment coverage and residuals |

Close condition: every VBA-observable semantic/library matrix row has current clean-stack Excel evidence or authoritative public-spec evidence. Package, IR, cache, safety and hostile-artifact rows close against their own specified engineering authorities, not a fictitious Excel observation.

### CORE-10 — Terminal truth, documentation and release gate

Type: support/conformance

Required outcomes:

- update architecture, package, frontend, VM3, JIT, testing, conformance and building docs;
- archive/supersede stale reports and worksets;
- reconcile blockers, implementation log, canonical matrices and bead state;
- publish a derived readiness report;
- run full Linux and Windows gates, current Excel oracle, safety and performance lanes;
- perform fresh-eyes code, docs and runnable-path review;
- file every discovered required residual as a delivery bead before any parent closes.

Close condition: all required child delivery beads are closed, matrices show the same truth as tests/docs, and the workset terminal gate is demonstrably green.

## 7. Dependency graph

| epic | hard prerequisites | closure dependencies/notes |
|---|---|---|
| CORE-0 | none | establishes truth, target ledger, matrices and bead graph |
| CORE-1 | CORE-0 | establishes the green/canonical runner prerequisite for every later merge gate |
| CORE-2 | CORE-1 | may run in parallel with CORE-4 and CORE-8 scaffolding |
| CORE-3 | CORE-2 | compiler analysis/fact contract also gates language-service LS-1/3/4/5 |
| CORE-LIB | CORE-2, stable CORE-3 signature slices | member families may deliver incrementally; closes only after VM3/JIT/oracle completion |
| CORE-4 | CORE-1 | package contract can begin before CORE-2/3; final metadata must consume typed compiler facts |
| CORE-5 | CORE-4 | VM/runtime hardening consumes sealed verified programs |
| CORE-6 | CORE-3, CORE-4, CORE-5 | JIT semantics consumes typed calls, verified input and hardened ABI/runtime |
| CORE-7 | CORE-4, CORE-5, CORE-6 | persistent package sessions/cache require stable package/helper contracts |
| CORE-8 | CORE-1 | harness scaffolding starts early; closure depends on CORE-2/3/LIB/4/5/6/7 |
| CORE-9 | stable CORE-2/3/LIB/6 slices | oracle capture may proceed per stable row; closure depends on all VBA-observable core rows |
| CORE-10 | CORE-1 through CORE-9 and CORE-LIB | terminal truth/release only after every required delivery lane closes |

Cross-workset edges:

- language-service LS-1/3/4/5 consume CORE-2.5, CORE-3.1, CORE-3.5 and CORE-3.8;
- language-service referenced-source public data consumes CORE-3.4;
- language-service compiled-artifact references consume CORE-3.4 and CORE-4.1/4.2/4.3/4.5;
- language-service VBA-library and Declare rows consume stable CORE-LIB slices and CORE-3.3 respectively;
- Windows interop cannot build external codegen before CORE-4/5/7 establish verified package, ABI and session contracts;
- Windows x86 JIT certification feeds this workset's final declared-target summary.

## 8. Required checks

### Fast per-bead

- format and strict Clippy for touched crates;
- targeted unit/integration tests;
- relevant matrix validation;
- relevant VM3/JIT differential for executable semantics, or a matrix-recorded `N/A` with reason for compile-only/docs/tooling work;
- fresh-eyes review.

### Core merge gate

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `pwsh -File ./scripts/run-core-readiness.ps1 -Mode Differential -TestThreads Default`
- `pwsh -File ./scripts/run-core-readiness.ps1 -Mode Differential -TestThreads 1`
- `./scripts/check-governance.ps1`
- `./scripts/meta-check.ps1 -Fast -NoArtifacts`
- `pwsh -File ./scripts/run-core-readiness.ps1 -Mode Truth`

`CORE-1.6` must deliver this runner before later epics use it. The script owns exact package filters, environment, timeouts and evidence paths so Linux and Windows invoke one versioned gate rather than prose approximations.

### Scheduled/release gate

- Linux x64 and Windows x64 core matrices, Windows x86 compiler/VM3 target rows, and the Windows-workset x86 JIT prerequisite;
- current Excel/VBA oracle;
- ASAN over carrier/ABI/differential paths;
- Miri over suitable runtime/ABI units;
- formal lanes where applicable, with unresolved non-blocking failures tracked;
- cold/warm performance and artifact-size report;
- repeated session/load/drop stress.

No test may be weakened, skipped or re-blessed merely to pass a gate. A changed snapshot requires a reviewed semantic explanation or a cross-platform normalization proof.

## 9. Terminal condition

This workset is complete only when:

1. all `CORE-*` epics and required delivery beads are closed;
2. ordinary workspace tests, strict Clippy, serial/parallel differentials and governance gates are green for the explicit Linux x64, Windows x64 and Windows x86 target rows above;
3. accepted source never panics and malformed compile-time syntax fails closed;
4. compiler typing, calls, diagnostics and project references are complete for the declared scope, and the compiler-owned analysis/fact result is shared with language services;
5. every load/execution path consumes a verified image and honors its declared entry/profile;
6. VM3 implements the complete verified OxIR vocabulary and passes zero-balance lifecycle stress;
7. the JIT implements all accepted platform-neutral language/runtime/library semantics, including error/Erl and safe recursion;
8. every in-scope VBA base-library/portable-host member has typed compiler, VM3/JIT and oracle/spec evidence;
9. VM3 and JIT expose equivalent persistent package/session behavior;
10. the full differential observable and current-stack Excel/VBA semantic oracle are green;
11. architecture, specs, matrices, worksets, blockers and tests agree.

If any accepted row remains `implemented-subset`, `planned` or `in-progress`, this workset remains `in-progress` unless the residual is explicitly transferred to one of the other two worksets and is genuinely outside this workset’s boundary.

## 10. Bead-preparation handoff

The next action after accepting this workset is bead preparation, not implementation:

1. create the workset root;
2. create epics `CORE-0` through `CORE-10` plus `CORE-LIB`;
3. create one rollout bead beneath each epic;
4. create the listed first delivery beads with explicit dependencies;
5. attach the canonical matrix rows and touched truth surfaces to each bead;
6. mark every bead as delivery or support;
7. ensure at least one unblocked delivery bead exists after `CORE-0`;
8. do not close a capability epic on rollout, audit or documentation beads alone.
9. split every epic-sized candidate into beads that name type, dependencies, matrix rows, touched truth surfaces, exact commands and residual behavior before execution.
