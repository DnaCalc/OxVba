# Workset: Clean-Stack Language Services and IDE Baseline

Date: 2026-07-10
Owner: unassigned
Status: proposed; bead rollout not yet performed
Type: language-service capability delivery
Source review: [`OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`](../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md)

## 1. Outcome

Deliver a first-class OxVba language-service baseline for IDEs without creating a second compiler:

- one compiler-owned semantic fact model used by compilation and editor queries;
- project-aware, versioned source workspaces with in-memory overlays;
- diagnostics, symbols, navigation, completion, signature help, hover, semantic tokens, references and safe rename;
- consistent coverage of VBA source, the VBA library, referenced VBA projects, verified OxImage compiled references, host references, Declare symbols and COM typelibs;
- a direct Rust API for embedded hosts;
- a thin Language Server Protocol transport aligned with current LSP practice;
- a working editor smoke path;
- explicit tests, performance bounds and honest documentation.

The goal is the basic language intelligence an IDE needs. Advanced refactorings and product-specific UX can mature later. The workset closes only when the basic surface uses current clean-stack compiler facts across every accepted reference kind and is validated end to end.

## 2. Current status determination

### 2.1 No active implementation

The current workspace contains no active `oxvba-languageservice` or `oxvba-lsp` crate and no `SemanticModel`/workspace query/session API.

Git history records the transition:

- `f69ec0b2` (2026-06-07) removed the former language-service/LSP/tooling cluster from the clean build and stated that it must be reimplemented over `oxvba-symbol` and `oxvba-syntax`;
- `b2773030` (2026-06-18) deleted the harvest copy.

The in-repo VS Code extension still tries to launch `oxvba-lsp` and tells the user to build a crate that does not exist. Current language-service public-interface, host-boundary, showcase, workset and validation documents still claim APIs, methods and tests from the deleted implementation.

Current status is therefore `not implemented on the clean stack`. Historical tests/designs remain recoverable inputs, not present capability.

### 2.2 Reusable clean-stack foundation

The rebuild can reuse:

- lossless green/red CST and token offsets;
- parser recovery for incomplete editor text;
- symbol IDs, scopes, declaration spans, visibility, signatures and types;
- project/reference closure loading;
- project, referenced-project, VBA-library, host, COM typelib and Declare providers;
- conditional-compilation preprocessing that preserves byte length;
- shared diagnostic DTOs;
- Core IR call/type/dispatch facts;
- a bounded programmatic `ProjectExportSurface` callable/coclass scaffold; this is not yet a canonical verified `.oxi` reference loader and cannot reconstruct cross-project public fields;
- historical language-service/LSP behavior and tests from git history;
- the existing VS Code shell as packaging reference after its missing-binary path is repaired.

### 2.3 Missing foundation

- stable document/workspace/snapshot identity;
- immutable semantic snapshots;
- compiler-owned binding facts for every source use site;
- typed expression/member/call facts at source ranges;
- source-located symbol and bind diagnostics;
- reference/use-site indices;
- dependency-aware invalidation;
- asynchronous cancellation and stale-result suppression;
- metadata definitions for non-source symbols;
- direct query/session API;
- active LSP server and integration tests.

## 3. Scope

### 3.1 In scope

#### Consumed compiler semantic facts

- declaration, definition and use-site identity;
- source ranges and provenance;
- scope/visibility and case-insensitive name resolution;
- expression/member/call/result types;
- properties/default members and assignment intent;
- overload/accessor/invoke-kind decisions;
- Optional, named, omitted, ParamArray and ByRef facts;
- diagnostics and related locations;
- inactive conditional regions and generated/normalized source maps.

#### Workspace model

- `.basproj`, bounded `.vbp` and convention projects through `oxvba-project`;
- `.bas`, `.cls`, `.frm` code documents;
- referenced-project closure;
- verified OxImage export-surface references after CORE-3.4/CORE-4 establish public-data representation, artifact ingestion/versioning and provenance; historical `Bundle` metadata is not an interchangeable product artifact;
- COM typelibs and host references;
- open/change/save/close overlays;
- target/conditional constants, reference order and project options;
- reload, invalidation and snapshot versioning.

#### Direct language-service API

- workspace/document lifecycle;
- diagnostics;
- document and workspace symbols;
- semantic classification/tokens;
- hover;
- completion and completion resolve;
- signature help;
- definition, type definition and implementation where applicable;
- references and document highlights;
- prepare rename and safe workspace edits;
- basic diagnostics-driven code actions;
- folding and selection ranges;
- metadata/virtual-document navigation.

#### LSP transport

- lifecycle and capability negotiation;
- text synchronization;
- diagnostic transport;
- the direct API query set above where standardized;
- position-encoding negotiation;
- cancellation, progress and partial results where useful;
- one honest project/workspace root per server session initially;
- protocol transcript and editor-client tests.

#### Validation and integration

- direct API tests;
- current LSP protocol behavior;
- VS Code smoke path;
- embedded-host example;
- performance/invalidation/cancellation;
- reference-kind parity;
- Windows COM metadata tests;
- current docs and matrices.

### 3.2 Explicitly deferred

These are not required to close the basic baseline:

- a complete VBA IDE;
- forms designer;
- debugger/DAP delivery;
- formatter parity;
- code lens, call hierarchy and type hierarchy;
- inlay hints;
- broad automated refactoring catalog;
- multi-root/multi-workspace process semantics;
- notebook support;
- remote workspace indexing;
- LSIF/offline index generation;
- AI-specific editor features;
- client-specific polished UI.

Deferred features may be added only after basic correctness and host use reveal a need.

## 4. Standards and practice baseline

The transport baseline is the official [Language Server Protocol 3.18 specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/), which the official site listed as current and under development on 2026-07-10. The method matrix and generated DTOs must pin the exact 3.18.x meta-model/spec revision or commit used; “latest 3.18” is not a reproducible protocol contract.

Relevant baseline practices:

- negotiate client/server capabilities rather than assuming every method;
- negotiate position encoding, defaulting to UTF-16 when omitted;
- maintain versioned document state;
- honor `$/cancelRequest` and avoid publishing stale results;
- support diagnostic pull (`textDocument/diagnostic` and, where claimed, `workspace/diagnostic`), with a deliberate push fallback for clients that need it;
- attach result IDs for unchanged diagnostic/semantic-token responses;
- support semantic-token full and delta only when correctness and cache identity are proven;
- use partial results/progress for potentially long workspace operations;
- provide read-only virtual metadata through `workspace/textDocumentContent` plus refresh when supported, with a declared client fallback;
- request diagnostic and semantic-token refresh after project/reference/provider changes where negotiated;
- handle closed-file/project changes through watched-file notifications or an explicit extension reload command;
- define root precedence (`workspaceFolders` over `rootUri`), extra-root rejection and workspace-folder change behavior for the one-root baseline;
- declare static versus dynamic registration policy;
- return exactly one response for every still-open cancelled request; normal/partial results remain legal, `RequestCancelled` is used when client cancellation is answered as an error, `ServerCancelled` is used only for methods that permit server cancellation/retrigger data, and a cancellation arriving after the response is a harmless race;
- use versioned `WorkspaceEdit.documentChanges` and reject stale edits;
- keep stdout protocol-clean, compute byte-accurate `Content-Length`, and test shutdown/exit status plus `MethodNotFound` for unadvertised methods;
- keep JSON-RPC/LSP DTOs at the transport edge;
- make single-root behavior explicit rather than simulating multi-root by replacing state silently.

The direct Rust API remains the semantic authority. LSP is a projection, not a second parser, binder, workspace or project model.

## 5. Architecture target

`oxvba-project workspace/reference closure
      + document overlays/options/target
                |
                v
    compiler analysis pipeline
    syntax -> symbol -> bind facts
                |
                v
      immutable SemanticSnapshot
      - syntax trees
      - symbols/scopes/signatures
      - definition/use bindings
      - expression/call/member types
      - diagnostics/source maps
      - reference provenance
                |
          +-----+------+
          |            |
          v            v
 direct Rust API   thin oxvba-lsp
 embedded IDEs    JSON-RPC/LSP clients`

The compiler exposes an `AnalysisResult { facts, diagnostics, core_program: Option<_> }`-style contract or equivalent. Strict compilation accepts only an error-free result with a CoreProgram. Editor analysis retains compiler-owned poison/unknown facts for incomplete text, but malformed text cannot reach code generation. Valid-source facts must be identical in strict and editor use. No service path may rerun a heuristic binder.

## 6. Identity and provenance contract

Every queryable symbol/use must carry:

- workspace and snapshot version;
- project identity;
- document/module identity;
- source or virtual metadata URI;
- source span in original document coordinates;
- a snapshot-bound opaque symbol handle that cannot be used with another snapshot;
- a deterministic logical `SymbolKey` for equivalence/cache reuse across unchanged snapshots, including provider identity/version and reference provenance;
- symbol kind, namespace, visibility and declared type/signature;
- origin:
  - active source;
  - referenced source project;
  - verified OxImage export;
  - VBA base library;
  - COM typelib;
  - host-injected reference;
  - Declare/native declaration;
  - generated compiler source;
- writable/renameable status;
- target/profile/conditional context.

Name-only identity is insufficient. Case-insensitive equality must not erase original spelling or project/reference provenance.

Every rename/workspace edit also carries its originating SnapshotId and document versions. The service rejects stale handles and stale edits rather than rebasing them heuristically.

## 7. Canonical truth artifacts

Rollout must replace or supersede the stale matrix with:

1. `docs/validation/LANGUAGE_SERVICE_BASELINE_MATRIX_V2.csv`
2. `docs/validation/LANGUAGE_SERVICE_REFERENCE_KIND_MATRIX_V1.csv`
3. `docs/validation/LSP_3_18_METHOD_MATRIX_V1.csv`
4. `docs/validation/LANGUAGE_SERVICE_PERFORMANCE_MATRIX_V1.csv`

The baseline matrix records direct API status. The LSP matrix records transport projection separately. A green LSP row cannot compensate for missing direct semantics.

Every row must identify:

- supported subset;
- source/reference kinds;
- incomplete-code behavior;
- direct API test;
- LSP transcript/client test where applicable;
- Windows/COM dependency;
- performance/cancellation state;
- residual owner.

## 8. Binding invariants

1. Production compiler facts are the semantic authority.
2. Source is parsed once per snapshot/version.
3. Language-service queries do not perform substring parsing or source rewriting.
4. Editor overlays replace source text, not compiler semantics.
5. Referenced projects, verified OxImages, VBA library, host and COM metadata use their production providers.
6. A source symbol and metadata symbol never collide by name alone.
7. Diagnostics, navigation and rename operate on original document coordinates.
8. Incomplete text returns partial facts/diagnostics without panics.
9. Stale requests cannot publish results for a newer document version.
10. Rename and edits never modify read-only metadata references.
11. LSP transport contains no independent project discovery or symbol rules.
12. Claims remain basic and subset-labelled; advanced features do not block the baseline.

## 9. Execution epics

### LS-0 — Truth reset, historical recovery and rollout

Type: support

Required outcomes:

- create the workset root and all `LS-*` epics;
- create rollout beads and first delivery beads;
- mark current language-service docs/matrices/showcases as stale or historical;
- update the VS Code extension status so it does not claim a runnable server;
- recover former APIs/tests from git history into an inventory without reactivating legacy dependencies;
- classify historical tests as reusable behavior, obsolete architecture or advanced/deferred;
- seed the four canonical matrices;
- define crate/API ownership before code is created.

First bead candidates:

| candidate | type | outcome | close evidence |
|---|---|---|---|
| LS-0.1 | support | roll out all language-service epics | delivery-ready bead graph |
| LS-0.2 | support | stale truth correction | no active doc claims missing crates/APIs |
| LS-0.3 | support | historical behavior/test inventory | git commit/path and disposition for each feature family |
| LS-0.4 | support | seed V2 matrices and reference-kind map | every basic feature/reference kind has an owner |
| LS-0.5 | support | compiler/service/transport ownership decision | reviewed architecture note |

Close condition: current absence is honest and the clean-stack rebuild has a concrete delivery graph.

### LS-1 — Compiler analysis-contract acceptance and semantic snapshot ingestion

Type: delivery
Dependencies: CORE-2.5, CORE-3.1, CORE-3.5 and CORE-3.8

Required outcomes:

- accept and version the compiler-owned AnalysisResult/fact contract delivered by the core workset;
- ingest declaration/use-site, expression/member/call/result, accessor/default-member, argument mapping and provenance facts without rebinding;
- convert compiler identities into snapshot-bound handles and deterministic logical SymbolKeys;
- retain poison/unknown facts and diagnostics for incomplete input when `core_program` is absent;
- prove valid-source facts are identical in strict compilation and editor analysis;
- index source-located diagnostics and original/virtual source provenance;
- ensure compiler output and facts originate in one analysis operation;
- serialize only facts needed by artifact/debug consumers; keep editor caches outside `OxImage` unless architecturally required.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-1.1 | compiler fact-contract acceptance | versioned mapping and invariants against CORE-3.8 |
| LS-1.2 | snapshot handles, logical keys and fact index | source fixtures round-trip without cross-snapshot handle reuse |
| LS-1.3 | valid strict/editor parity | compiler facts and service facts are identical |
| LS-1.4 | incomplete poison/unknown ingestion | useful facts remain while malformed input cannot reach codegen |
| LS-1.5 | source-located diagnostic/provenance index | original and virtual source snapshots agree |
| LS-1.6 | one-analysis proof | instrumentation proves no heuristic second binder |

Close condition: every source-queryable compiler row is indexed from the compiler-owned fact pipeline, valid strict/editor facts agree, and malformed analysis cannot reach code generation.

### LS-2 — Workspace, documents and incremental snapshots

Type: delivery

Required outcomes:

- add `oxvba-languageservice` or an explicitly named successor crate;
- define WorkspaceId, ProjectId, DocumentId, SnapshotId and version semantics;
- load real `.basproj`/`.vbp`/convention projects through `oxvba-project`;
- represent modules/classes/forms and original encodings/source maps;
- maintain open/update/save/close overlays;
- build immutable snapshots safe for concurrent reads;
- invalidate changed documents and affected dependents;
- rebuild when references/options/conditional target change;
- reject detached shadow documents or make them explicit scratch workspaces;
- support one project-reference closure per session initially;
- provide cancellation and stale-result checks;
- stress reload/reset/close lifecycle.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-2.1 | workspace/document/snapshot identity model | deterministic lifecycle tests |
| LS-2.2 | canonical project loading | basproj/vbp/convention fixtures use one loader |
| LS-2.3 | versioned overlay lifecycle | open/change/save/close and restore tests |
| LS-2.4 | dependency invalidation | cross-module/reference edits refresh exact dependents |
| LS-2.5 | cancellation/stale-result suppression | race tests never publish old versions |
| LS-2.6 | repeated reload/drop ownership | no leaked snapshots, typelibs or file handles |

Close condition: the direct service can maintain a real project closure under edits without semantic or identity drift.

### LS-3 — Diagnostics and resilient incomplete-code analysis

Type: delivery

Required outcomes:

- merge syntax, symbol, bind, project/reference and relevant package diagnostics;
- preserve stable diagnostic code, severity, source, range, related locations and help;
- distinguish active and inactive conditional code;
- recover useful syntax/symbol facts from incomplete declarations, statements and expressions;
- avoid cascades and duplicate diagnostics;
- report broken/missing references with project/reference location;
- map generated-source failures back to user source;
- define debounce/cancellation policy outside semantic correctness;
- provide full and unchanged-result diagnostics with snapshot IDs.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-3.1 | unified diagnostic result DTO | multi-phase ordered snapshots |
| LS-3.2 | incomplete-code recovery corpus | common mid-edit states return stable partial facts |
| LS-3.3 | conditional/generated source mapping | original ranges and inactive-region behavior |
| LS-3.4 | reference/project diagnostics | broken, ambiguous, reordered and missing reference tests |
| LS-3.5 | diagnostic dedupe/cascade policy | minimized error corpus |

Close condition: basic editing never panics and diagnostic ranges match the production compiler’s facts.

### LS-4 — Core symbols, navigation and semantic classification

Type: delivery

Required outcomes:

- document symbols for modules, procedures, properties, types, enums, fields, events and declarations;
- workspace symbol search with project/reference provenance;
- definition for declarations and source references;
- type definition for object/interface/UDT types;
- implementation for `Implements` and property accessor groups where meaningful;
- references with read/write/call/implementation classification;
- document highlights;
- semantic token/classification legend for VBA syntax and semantic kinds;
- folded/case-preserving names;
- virtual metadata definitions for non-source symbols;
- stable result ordering.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-4.1 | document/workspace symbols | multi-module/class/property fixture |
| LS-4.2 | definition/type-definition/implementation | source/project/interface matrix |
| LS-4.3 | classified references/highlights | read/write/call/default-member tests |
| LS-4.4 | semantic classification | token snapshots under shadowing and inactive code |
| LS-4.5 | virtual metadata documents | VBA library/verified OxImage/COM navigation |

Close condition: navigation and classification use stable symbol identity across every accepted source/reference kind.

### LS-5 — Completion, signature help and hover

Type: delivery

Required outcomes:

- scope-aware declarations, keywords and snippets;
- member completion from project classes, interfaces, VBA library, verified OxImage exports, host roots and COM typelibs;
- correct visibility, reference precedence and Option Private behavior;
- completion after dot/bang/default-member/With contexts;
- signature help for procedures, properties, default members, builtins, Declare and COM calls;
- named/Optional/ParamArray and active-parameter behavior;
- hover with kind, declared type, signature, property group, documentation/provenance and constant value where safe;
- incomplete-code and error-recovery behavior;
- lazy resolve for expensive documentation/metadata;
- deterministic ranking and deduplication.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-5.1 | lexical/scope completion | locals/params/module/project shadowing |
| LS-5.2 | member/default/With completion | project and dynamic/typed contexts |
| LS-5.3 | signature help | named/omitted/Optional/ParamArray/property matrix |
| LS-5.4 | hover | source and metadata type/provenance snapshots |
| LS-5.5 | recovery and ranking | mid-edit corpus and deterministic ordering |

Close condition: core query results are compiler-consistent and reference-complete, not regex/name-list approximations.

### LS-6 — Reference-kind parity

Type: delivery

Why separate: the review request specifically requires language/library/project/compiled-artifact/COM reference gaps to close.

Required reference kinds:

1. active VBA project source;
2. referenced VBA project source;
3. referenced verified OxImage export surface;
4. VBA base library modules, classes, constants and source-visible intrinsics; compiler-internal `__oxvba_*`/structural machinery is never queryable;
5. host-injected globals/classes/interfaces;
6. COM typelib libraries, coclasses, interfaces, members, enums, records and events;
7. Declare procedures and native library aliases;
8. generated compiler modules with mapped provenance.

For each kind, prove:

- symbol listing/search;
- completion;
- hover/signature;
- definition or virtual metadata location;
- references where source-owned;
- renameability/read-only policy;
- precedence/ambiguity;
- broken/unavailable reference diagnostic where VBA compile-time reference resolution applies; runtime DLL/export absence remains an invocation-time error and may only be shown as non-compiler informational status;
- cache invalidation when metadata/reference changes.

Host-provider metadata carries a version/revision digest and stable provider identity so an injection change invalidates affected diagnostics and queries. Generated modules are queryable only through mapped user-source or explicit read-only virtual provenance; they never appear as writable physical source.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-6.1 | referenced source-project parity | depends on CORE-3.4 public-data delivery; full query matrix |
| LS-6.2 | verified OxImage export parity | depends on CORE-3.4 and CORE-4.1/4.2/4.3/4.5; sealed loader/version/provenance/unavailable-definition fixture plus public-data coverage |
| LS-6.3 | VBA library parity | depends on stable CORE-LIB inventory/signature slices; constants/intrinsics/classes/properties |
| LS-6.4 | host reference parity | versioned/digested injected root/object/type/event metadata and invalidation |
| LS-6.5 | COM typelib parity | cross-platform fixture metadata plus Windows live typelib |
| LS-6.6 | Declare/native symbol parity | depends on CORE-3.3; source-owned identifiers/call sites navigate and rename, `Lib`/`Alias` targets remain read-only, and missing runtime DLL/exports are not compiler diagnostics |
| LS-6.7 | precedence/ambiguity matrix | active/reference/library/host/COM collisions |
| LS-6.8 | generated-source provenance | mapped user navigation or explicit read-only virtual document, never a phantom writable file |

Close condition: no basic feature silently omits a supported reference kind.

### LS-7 — Safe rename and basic code actions

Type: delivery

Required outcomes:

- prepare rename with symbol/range and rejection reason;
- case-insensitive rename over definitions and bound references;
- multi-document/project edits for writable source;
- collision, visibility, qualification and public-surface analysis;
- property group/accessor and Implements handling;
- block edits to VBA-library, OxImage-only, host and COM metadata;
- expose external/read-only references as conflicts or non-editable references;
- preserve trivia, attributes and original encoding;
- basic diagnostic-driven actions:
  - declare undeclared local where unambiguous;
  - add `Option Explicit`;
  - add `PtrSafe` or pointer-width correction only when compiler facts justify it;
  - qualify an ambiguous name;
  - add/import a known project/COM reference through project-helper planning, not raw LSP file edits;
- no broad refactoring claim.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-7.1 | prepare rename and local/module rename | collision and case tests |
| LS-7.2 | multi-module/project rename | reference graph edits |
| LS-7.3 | read-only/external reference safety | no metadata mutation |
| LS-7.4 | property/Implements rename rules | grouped accessor/interface tests |
| LS-7.5 | bounded code-action families | diagnostic-code-driven edits and recompile |

Close condition: every emitted edit is compiler-validated and safe for its reference provenance.

### LS-8 — Folding, selection and basic document structure

Type: delivery

Required outcomes:

- folding ranges for procedures, properties, blocks, types/enums, regions and multiline constructs;
- selection ranges from token to expression/statement/block/procedure/module;
- source ranges remain correct with CRLF, Unicode and continued lines;
- incomplete blocks produce bounded useful ranges;
- no semantic divergence or independent parser.

Close condition: baseline structural editor operations are stable and CST-derived.

### LS-9 — Performance, scheduling and robustness

Type: delivery

Required outcomes:

- define representative small, medium and large VBA workspaces;
- measure cold load, single-document edit, dependent invalidation and each query family;
- set budgets before optimization;
- cancel obsolete long operations;
- bound memory across edit/reload cycles;
- avoid holding global locks during filesystem/typelib work;
- make query results deterministic under concurrent reads;
- fuzz edits and positions;
- collect opt-in diagnostics/telemetry only through host policy.

Initial budgets to validate during rollout:

- keystroke-local syntax/update p95 under 20 ms on the medium corpus;
- common document query p95 under 100 ms;
- workspace/reference rebuild p95 under 1 s on the medium corpus;
- no stale response after a newer document version;
- flat memory after repeated open/change/close/reload cycles.

These are planning targets, not claims. Adjust only with recorded measurements and user-impact rationale.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-9.1 | benchmark corpus and budgets | checked-in reproducible harness |
| LS-9.2 | dependency-aware incremental update | p95 and invalidation correctness |
| LS-9.3 | cancellation/concurrency | race and stress tests |
| LS-9.4 | memory/lifecycle stability | repeated-session profile |
| LS-9.5 | edit/position fuzzing | no panic and minimized regressions |

### LS-10 — Thin LSP 3.18 transport

Type: delivery

Required baseline:

- `initialize`, `initialized`, `shutdown`, `exit`;
- honest server capabilities and one-root policy;
- `didOpen`, versioned `didChange`, `didSave` and `didClose`;
- position-encoding negotiation with UTF-16 default and tested UTF-8;
- diagnostics:
  - document pull;
  - workspace pull only if implemented with partial results;
  - push fallback only by explicit client capability/policy;
- document/workspace symbols;
- hover;
- completion and resolve;
- signature help;
- definition/type definition/implementation;
- references and highlights;
- prepare rename/rename;
- code actions;
- semantic tokens full and delta after result-ID correctness;
- folding and selection ranges;
- `workspace/textDocumentContent`/refresh for virtual metadata, with a declared fallback for clients without that 3.18 capability;
- diagnostic/semantic-token refresh after project/reference/provider changes where negotiated;
- watched closed-file changes or an explicit extension reload command;
- exact `workspaceFolders`/`rootUri` precedence, extra-root rejection and folder-change behavior;
- explicit static/dynamic registration policy;
- exactly-one-response cancellation semantics, `RequestCancelled` error use, method-limited `ServerCancelled`, partial-result and late-cancel races;
- versioned `WorkspaceEdit.documentChanges` and stale edit rejection;
- progress, partial results and protocol errors;
- protocol-clean stdout, byte-accurate framing and shutdown/exit behavior;
- URI/path normalization and original document identity;
- thin conversion only—no parser, provider or project policy in this crate.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-10.1 | server lifecycle/capability/stdio shell | JSON-RPC transcript tests |
| LS-10.2 | versioned sync and position encoding | Unicode/CRLF UTF-16/UTF-8 tests |
| LS-10.3 | diagnostic pull/push policy | result IDs, unchanged and stale-version tests |
| LS-10.4 | query projection tranche | method matrix and transcript tests |
| LS-10.5 | semantic tokens full/delta | client capability and delta equivalence |
| LS-10.6 | cancellation/progress/errors | still-open request gets exactly one legal response; partial/late races leave server usable |
| LS-10.7 | one-root/reload policy | explicit error instead of shadow workspace |
| LS-10.8 | virtual content and refresh | read-only VBA/OxImage/COM documents plus fallback client path |
| LS-10.9 | watches, roots and registration | closed-file reload, root precedence and capability matrix |
| LS-10.10 | versioned edits and stdio hardening | stale edits reject; framing/exit/unadvertised-method transcripts |

Close condition: every advertised capability is backed by direct-result versus decoded-LSP-result equivalence plus protocol tests; unimplemented capabilities are not advertised.

### LS-11 — Direct-host and editor smoke integration

Type: delivery

Required outcomes:

- publish stable direct Rust session/query DTOs;
- provide an embedded-host example using the direct API;
- repair the VS Code extension to launch the real server;
- package or resolve the server binary predictably;
- support the basic language/file/project associations;
- run an editor smoke path:
  - open project;
  - publish diagnostics;
  - completion;
  - hover;
  - definition;
  - references;
  - rename;
  - semantic tokens;
  - project/reference change and reload;
- keep project creation/reference editing in project-helper/extension commands, not semantic LSP methods;
- avoid claiming debugger or complete extension UX.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| LS-11.1 | direct embedded-host example | executable integration test |
| LS-11.2 | VS Code launch/package repair | extension-host smoke |
| LS-11.3 | basic feature click-through | captured client/server log and screenshots if useful |
| LS-11.4 | project/reference reload | command changes project and LS refreshes facts |

Close condition: at least one direct host and one LSP client consume the same semantics end to end.

### LS-12 — Windows COM-reference validation

Type: delivery/conformance

Dependency: authoritative raw COM metadata/resolver deliverables WIN-1.6 and WIN-1.7; COM runtime/serving/export completion is not a prerequisite for metadata queries.

Required outcomes:

- load registered and file-backed typelibs through the production resolver;
- update queries when COM reference selection/version/order changes;
- navigate to stable virtual metadata for libraries, coclasses, interfaces, members, enums, records and events;
- complete signature/completion/hover for early-bound COM types and late-bound receivers with a known typelib/host projection; arbitrary runtime `Object` receivers do not promise member completion;
- distinguish unavailable runtime activation from available compile-time metadata;
- cover both x86 and x64 target contexts for typelib/reference differences; the server process itself need not be 32-bit;
- test broken/missing/unregistered references;
- cross-check names, signatures, attributes/default members and events with Excel/VBE Object Browser or public typelib facts where reproducible.

Close condition: the COM reference column in the reference-kind matrix is green for the basic query set.

### LS-13 — Terminal docs, matrix and baseline release

Type: support/conformance

Required outcomes:

- update language-service spec to the clean architecture and LSP 3.18 baseline;
- replace stale public-interface, host-boundary and showcase claims;
- update architecture, contributing, building and testing docs;
- generate capability summaries from canonical matrices;
- document direct versus LSP consumption;
- document single-root and advanced-feature deferrals;
- run direct API, protocol, editor smoke, performance, Windows COM-reference and governance gates;
- perform fresh-eyes API, code, protocol and user-path review;
- create delivery beads for any uncovered required work before closing.

Close condition: docs, advertised capabilities, matrices, crates and runnable editor behavior agree.

## 10. Dependency graph

| epic | hard prerequisites | closure dependencies/notes |
|---|---|---|
| LS-0 | CORE-1 green workspace prerequisite | establishes honest truth, ownership and matrices |
| LS-1 | CORE-2.5, CORE-3.1, CORE-3.5, CORE-3.8 | accepts compiler facts and builds snapshot indices; may proceed alongside LS-2 interface work |
| LS-2 | LS-0, compiler/project identity contracts | workspace and overlay substrate |
| LS-3 | LS-1, LS-2 | diagnostics/incomplete analysis consume compiler facts and versions |
| LS-4 | LS-1, LS-2 | navigation/classification consume fact indices and provenance |
| LS-5 | LS-1, LS-2, LS-4 | completion/signature/hover consume typed/member facts |
| LS-6 | LS-1, LS-2, LS-4, LS-5 | source references require CORE-3.4; OxImage requires CORE-3.4 and CORE-4.1/4.2/4.3/4.5; library requires CORE-LIB; Declare requires CORE-3.3; COM requires WIN-1.6/1.7 |
| LS-7 | LS-1, LS-2, LS-4, LS-6 | rename/edit safety requires complete identity/provenance graph |
| LS-8 | LS-2 and shared CST | can run independently of rich semantic features |
| LS-9 | LS-2 initially | harness is continuous and closes only after all required feature lanes |
| LS-10 | LS-2 for lifecycle/sync | each advertised method additionally depends on its direct LS feature; transport shell may start early |
| LS-11 | LS-10 plus all features advertised in the smoke path | direct host and editor consume the same snapshots |
| LS-12 | LS-4/5/6, WIN-1.6, WIN-1.7 | real Windows metadata certification, independent of COM runtime completion |
| LS-13 | LS-1 through LS-12 and CORE-1 green gates | terminal truth after every required delivery lane closes |

## 11. Test plan

### 11.1 Unit and semantic parity

- CST/text/range and Unicode position conversion;
- declaration/use/type/call/accessor fact snapshots;
- compiler diagnostic code/range equality;
- incomplete/malformed edit corpus;
- conditional compilation and generated-source maps;
- snapshot-handle isolation and logical SymbolKey equivalence across unchanged snapshots;
- invalidation and reference precedence;
- safe rename/collision/read-only analysis.

### 11.2 Reference-kind integration

For every basic query:

- active module;
- sibling module/class;
- referenced source project;
- verified OxImage-only project;
- VBA library;
- host-injected reference;
- synthetic COM typelib on all platforms;
- registered/file-backed COM typelib on Windows;
- Declare/native declaration;
- generated-source provenance;
- negative checks that compiler-internal structural intrinsics never appear.

### 11.3 LSP protocol

- raw JSON-RPC transcripts;
- lifecycle order and invalid-state errors;
- capability negotiation;
- full and incremental changes;
- UTF-16 default and UTF-8 negotiation;
- CRLF and astral Unicode positions;
- diagnostic result IDs/unchanged reports;
- semantic token full/delta equivalence;
- cancellation and partial results;
- every still-open cancelled request receives exactly one legal response; error, partial/normal-result and late-cancel races follow the pinned spec and leave the server usable;
- stale-version suppression;
- versioned rename/workspace-edit stale rejection;
- direct-result versus decoded-LSP-result equivalence for every advertised query;
- negative capability and `MethodNotFound` behavior;
- virtual document content, refresh and fallback;
- diagnostic/semantic-token refresh after provider/reference change;
- root precedence, extra-root rejection, watched-file/reload and registration policy;
- URI/path/case behavior on Windows and Linux;
- byte-accurate framing, protocol-clean stdout, shutdown/exit codes, malformed messages and server recovery.

### 11.4 Client/use path

- VS Code extension-host smoke;
- direct embedded-host smoke;
- file/project/reference edits and reload;
- server restart and workspace close;
- missing/broken project and degraded-state behavior.

### 11.5 Performance and robustness

- cold/warm workspaces;
- repeated edits;
- cross-project invalidation;
- large typelib and OxImage metadata;
- concurrent reads and cancellations;
- fuzzed edits/positions/protocol packets;
- memory and handle stability.

## 12. Required checks

### Per feature bead

- direct semantic unit/integration test;
- reference-kind neighbor test;
- LSP projection test if advertised;
- matrix row update;
- performance check for hot-path changes;
- fresh-eyes review.

### Merge gate

- CORE-1 canonical workspace/governance gate is green;
- workspace format/strict Clippy/tests;
- language-service direct suite;
- LSP transcript suite;
- direct-result versus decoded-LSP-result equivalence for every advertised method;
- deterministic cross-platform tests;
- governance/meta checks;
- no advertised method without a green direct row.

### Release gate

- Linux and Windows;
- both Windows x86 and x64 reference target contexts;
- VS Code and embedded-host smoke;
- Windows COM-reference matrix;
- full reference-kind matrix rerun after CORE-3.3/3.4, CORE-LIB, CORE-4.1/4.2/4.3/4.5 and WIN-1.6/1.7 producers reach their required stable gates;
- performance/cancellation/memory;
- fuzz/no-panic;
- docs/matrix truth reconciliation.

## 13. Terminal condition

This workset is complete only when:

1. all required `LS-*` delivery epics are closed;
2. production compilation and editor analysis emit facts through the same compiler-owned analysis pipeline, with the language service owning only snapshots, indices and queries;
3. a real project/reference workspace supports versioned overlays, invalidation and cancellation;
4. diagnostics, symbols, navigation, completion, signature, hover, references, semantic tokens, safe rename, basic code actions, folding and selection ranges are green for the declared baseline;
5. active source, referenced source, verified OxImage, VBA library, host, COM, Declare and generated-source reference kinds are covered consistently after their producer contracts close and the full matrix is rerun;
6. the direct Rust API is stable and tested;
7. the LSP 3.18 transport advertises only implemented capabilities and passes protocol tests;
8. one direct host and one editor client pass an end-to-end smoke path;
9. performance, stale-result, Unicode/range, incomplete-code and lifecycle gates are green;
10. virtual-content/refresh, roots/watch/reload, cancellation-response, negative-capability and stale-edit gates are green;
11. current docs/matrices no longer claim deleted capability or hide basic gaps.

Advanced deferred features do not block closure. A missing basic reference kind, compiler disagreement, locationless diagnostic, stale response or transport-only heuristic does.

## 14. Bead-preparation handoff

After acceptance:

1. create the workset root and epics `LS-0` through `LS-13`;
2. create rollout beads for every epic;
3. create the listed first delivery beads with dependencies;
4. map every validation bead to V2 baseline, reference-kind, LSP and performance rows;
5. classify delivery versus support;
6. recover historical tests only through explicit port beads—never copy the old compiler dependency graph back into the workspace;
7. leave at least one unblocked direct-semantic delivery bead after LS-0;
8. do not close the workset on an LSP shell, docs, matrices or editor packaging without the compiler-owned direct feature set.
