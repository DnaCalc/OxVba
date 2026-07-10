# Workset: Ideal Language-Service and IDE Foundation Realization

Date: 2026-07-10
Owner: unassigned
Status: accepted; directed bead rollout in progress under `bd-59co`
Type: architecture and language-service capability delivery
Profile: `PROFILE-IDE-001`
Source review: [`../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`](../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md)

## 1. Outcome

Build the clean-stack language-service foundation in its ideal architecture: compiler-owned strict/tolerant analysis facts, immutable semantic snapshots, real project/reference workspaces, complete basic semantic queries, consistent source/OxImage/VBA-library/host/COM/Declare/generated reference coverage, a stable direct Rust API and a thin standards-aligned LSP projection.

The language service must be an index and query product over the real compiler, not a revived second compiler. The profile is complete only when one embedded host and one editor client use the same facts end to end and every advertised transport feature is equivalent to its direct result.

Authority:

- system clauses `PROFILE-IDE-001`, `COMP-ANALYSIS-001`, `PROJ-REF-001`, `LS-*`, `DEBUG-MAP-001`, `CONF-*`;
- [`../spec/OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md`](../spec/OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md);
- compiler, OxImage and Windows metadata producer contracts.

## 2. Honest entry state

No active `oxvba-languageservice` or `oxvba-lsp` crate exists. The previous implementation was removed from the clean build and deleted; the VS Code extension and older docs still reference that deleted surface and are now explicitly deprecated.

Reusable foundations are the lossless CST, declaration/scope/signature infrastructure, project/reference closure, providers, diagnostics DTO, Core IR facts and historical test/design corpus. Missing are the complete compiler AnalysisResult, semantic snapshots, overlays, indices, invalidation, query API, LSP server and runnable editor path.

Historical language-service code/tests may be recovered only through explicit port beads that adopt current compiler/project/artifact contracts. They are not current capability evidence.

## 3. Target scope

### Required basic surface

- project/workspace/document/snapshot lifecycle;
- diagnostics;
- document/workspace symbols;
- semantic classification/tokens;
- hover, completion/resolve and signature help;
- definition, type definition and implementation;
- references and highlights;
- prepare rename, safe versioned edits and bounded code actions;
- folding and selection ranges;
- read-only virtual metadata documents;
- direct Rust API;
- thin LSP 3.18.x projection;
- embedded-host and VS Code smoke paths;
- performance, cancellation, invalidation and lifecycle gates.

### Required reference kinds

- active project source;
- referenced source projects including public data;
- verified OxImage export surfaces through the sealed loader;
- complete typed VBA library;
- versioned host providers;
- authoritative COM typelibs;
- source Declare declarations;
- generated/normalized source provenance.

### Explicitly deferred

Complete VBA IDE, forms designer, debugger/DAP, formatter parity, call/type hierarchy, inlay hints, broad refactoring, multi-root/multi-workspace process semantics, notebooks, remote/offline indexes and polished client-specific UX.

Deferred features do not block the basic profile. A missing basic reference kind or compiler disagreement does.

## 4. Consumed producer contracts

| consumer need | producer gate |
|---|---|
| source/provenance and original/virtual maps | CORE-2 |
| compiler AnalysisResult/use sites/types/diagnostics | CORE-3 |
| referenced-source public data and source/OxImage equivalent surface | CORE-3 public-surface delivery |
| Declare identity/signature/call legality | CORE-3 Declare compiler rows |
| complete VBA library metadata | stable CORE-LIB inventory/signature slices |
| verified OxImage loading, schema, provenance and exports | CORE-4 verified loader/schema/metadata |
| stable COM raw metadata and resolver digest | WIN-1 authoritative resolver/handoff |

The service can develop against stable slices, but its terminal reference matrix is rerun after every producer gate closes.

## 5. Architectural transformation

| current state | required state | clauses |
|---|---|---|
| deleted service and stale docs | new clean-stack direct service and honest indexes | `LS-BASIC-001`, `DOC-*` |
| compiler lacks complete fact output | versioned strict/tolerant AnalysisResult producer | `COMP-ANALYSIS-001` |
| no semantic snapshot identity | immutable snapshots, opaque handles and logical keys | `LS-FACT-001` |
| no overlay/invalidation service | real project closure and dependency-aware workspace | `LS-WORKSPACE-001` |
| metadata/reference gaps | parity across all production reference kinds | `PROJ-REF-001`, `LS-WORKSPACE-001` |
| no direct API | complete basic semantic result surface | `LS-BASIC-001` |
| no server/editor path | thin negotiated LSP and real smoke integration | `LS-LSP-001` |

## 6. Binding invariants

1. Production compiler facts are semantic authority.
2. Valid strict/editor facts are identical.
3. Incomplete text may produce poison/unknown facts but never executable Core IR.
4. Source is parsed once per snapshot/version.
5. No substring parser, editor binder or duplicate project model exists.
6. Snapshot handles never cross versions; logical keys include provider/provenance identity.
7. Overlays replace text, not compiler semantics.
8. Production reference providers serve compiler and language service.
9. Diagnostics/navigation/edits use original or explicit virtual coordinates.
10. Stale requests and edits cannot affect newer versions.
11. Read-only metadata is never renamed or edited.
12. LSP advertises only green direct features and contains no VBA/project policy.

## 7. Canonical artifacts

1. `LANGUAGE_SERVICE_BASELINE_MATRIX_V2.csv`
2. `LANGUAGE_SERVICE_REFERENCE_KIND_MATRIX_V1.csv`
3. `LSP_3_18_METHOD_MATRIX_V1.csv`
4. `LANGUAGE_SERVICE_PERFORMANCE_MATRIX_V1.csv`

The baseline matrix owns direct API truth. The LSP matrix records projection/equivalence separately. Rows identify feature, reference kinds, incomplete-code behavior, direct test, transport/client test, producer dependency, target context, performance/cancellation and residual owner.

## 8. Execution epics

### LS-0 — Authority reset, historical recovery and rollout

Type: support
Clauses: `DOC-*`, `CONF-MATRIX-001`

Deliver workset/epic/bead graph; mark deleted-stack docs/specs/evidence historical; repair extension status; inventory historical APIs/tests by reusable behavior versus obsolete architecture; seed four matrices; freeze compiler/service/transport ownership and producer dependencies.

First beads: rollout; stale truth/extension correction; history inventory; matrix seed; crate/API ownership note.

Close: current absence is honest and every basic feature/reference kind has a delivery-ready owner.

### LS-1 — Compiler fact contract acceptance and snapshot core

Type: delivery
Clauses: `COMP-ANALYSIS-001`, `LS-FACT-001`
Dependencies: CORE-2 provenance; CORE-3 AnalysisResult/diagnostics

Deliver:

- accept/version compiler AnalysisResult mapping;
- ingest declaration/use-site/expression/member/call/argument/accessor/provenance facts without rebinding;
- snapshot-bound handles and deterministic logical SymbolKeys;
- poison/unknown facts for incomplete input;
- strict/editor valid-source equality;
- diagnostic/source-provenance index;
- instrumentation proving one analysis operation.

First beads: contract mapping; snapshot IDs/handles/keys; declaration/use index; typed call/member index; incomplete facts; diagnostic/provenance index; one-analysis proof.

Close: every source-queryable compiler fact is indexed without a second semantic model.

### LS-2 — Project workspace, overlays and incremental snapshots

Type: delivery
Clauses: `LS-WORKSPACE-001`, `PROJ-REF-001`

Deliver new service crate; Workspace/Project/Document/Snapshot IDs; real basproj/vbp/convention loading; original encodings/maps; open/change/save/close overlays; immutable concurrent snapshots; dependency invalidation; reference/option/target rebuild; cancellation/stale suppression; provider/file watches/reload; repeated lifecycle ownership.

First beads: identity model; canonical project load; overlay lifecycle; dependency graph; provider/reference invalidation; cancellation races; reload/drop stress.

Close: a real project closure remains semantically correct under edits and external changes.

### LS-3 — Diagnostics and incomplete-code analysis

Type: delivery
Clauses: `COMP-DIAG-001`, `LS-BASIC-001`

Deliver merged syntax/symbol/bind/project/reference/artifact diagnostics; stable code/severity/source/ranges/related/help; active/inactive regions; useful incomplete declarations/statements/expressions; cascade/dedupe policy; broken references; generated-source mapping; version/result identity and cancellation.

First beads: result DTO; phase merge; incomplete corpus by grammar family; conditional/generated mapping; project/reference errors; dedupe/cascade; unchanged/result IDs.

Close: common editing states never panic and presentation remains compiler-identical.

### LS-4 — Symbols, navigation and semantic classification

Type: delivery
Clauses: `LS-BASIC-001`, `LS-FACT-001`

Deliver document/workspace symbols; definition/type-definition/implementation; classified references/highlights; semantic token legend; case-preserving identity; virtual definitions for VBA library/OxImage/COM/host/generated metadata; stable ordering.

First beads: document symbols; workspace symbols; definition/type/implementation; classified references; semantic tokens; virtual content identities.

Close: navigation/classification uses stable identity across every available reference kind.

### LS-5 — Completion, signature help and hover

Type: delivery
Clauses: `LS-BASIC-001`

Deliver scope/keyword/snippet completion; typed member completion across project/library/OxImage/host/COM; visibility/reference precedence/Option Private; dot/bang/default-member/With contexts; procedure/property/library/Declare/COM signatures; named/Optional/ParamArray active parameter; provenance/documentation hover; incomplete recovery; lazy resolve; deterministic ranking.

First beads: lexical/scope; typed member/default/With; signature matrix; hover/provenance; recovery; ranking/resolve.

Close: results are compiler-fact queries, not name-list/regex approximations.

### LS-6 — Reference-kind parity

Type: delivery
Clauses: `PROJ-REF-001`, `LS-WORKSPACE-001`, `LS-BASIC-001`

For each reference kind, prove symbols, completion, hover/signature, definition/virtual content, source-owned references, rename/read-only policy, precedence/ambiguity, compile-time availability diagnostic and revision invalidation.

Deliver separate tranches:

- referenced source projects after CORE public-data delivery;
- verified OxImage references after CORE schema/loader/verifier/provenance;
- VBA library after CORE-LIB inventory/signatures;
- versioned/digested host providers;
- COM metadata after WIN resolver/handoff;
- Declare after CORE compile legality, with runtime DLL/export absence not a compiler error;
- generated-source mapping/virtual provenance;
- collision/precedence matrix.

First beads correspond one-to-one to those tranches.

Close: no basic query silently omits a supported production reference kind and the full matrix is rerun after producers stabilize.

### LS-7 — Safe rename and bounded code actions

Type: delivery
Clauses: `LS-BASIC-001`

Deliver prepare rename; case-insensitive bound definition/reference edits; multi-document/project edits; collisions/visibility/qualification/public-surface analysis; property group/Implements; read-only metadata conflicts; trivia/attributes/encoding preservation; SnapshotId/document-version edits; diagnostic-driven declare local, Option Explicit, PtrSafe/pointer correction, qualification and reference-planning actions; recompile validation.

First beads: local/module rename; cross-module/project; stale edit rejection; read-only/external safety; property/Implements; bounded action families.

Close: every edit is compiler-validated, version-safe and provenance-aware.

### LS-8 — CST-derived document structure

Type: delivery
Clauses: `SYN-CST-001`, `LS-BASIC-001`

Deliver folding for procedures/properties/blocks/types/regions/multiline constructs and selection ranges from token through module, including CRLF, Unicode, continuations and incomplete blocks. No independent parser.

First beads: folding families; selection hierarchy; Unicode/CRLF/incomplete corpus.

Close: structural operations are stable CST projections.

### LS-9 — Scheduling, performance and robustness

Type: delivery
Clauses: `CONF-QUALITY-001`, `LS-WORKSPACE-001`

Deliver representative workspace/typelib/OxImage corpora; evidence-backed cold/edit/invalidation/query budgets; obsolete-work cancellation; bounded memory/handles; no global locks over I/O/provider work; deterministic concurrent reads; edit/position/protocol fuzz; host-policy telemetry.

Planning budgets to validate: local update p95 <20 ms, common document query p95 <100 ms, medium workspace/provider rebuild p95 <1 s, no stale publish and flat lifecycle memory.

First beads: corpus/budgets; incremental engine; concurrency/cancellation; memory lifecycle; fuzz.

Close: measured responsiveness never weakens semantic correctness.

### LS-10 — Thin pinned LSP projection

Type: delivery
Clauses: `LS-LSP-001`

Pin exact LSP 3.18.x meta-model/spec revision. Deliver lifecycle, one-root/root precedence, versioned text sync, UTF-16/negotiated positions, diagnostic pull/push policy, every direct query projection, semantic full/delta, virtual textDocumentContent/refresh/fallback, diagnostic/token refresh, watched/reload, versioned WorkspaceEdit, progress/partial results, exactly-one-response cancellation semantics, URI/path normalization, clean framing/stdout, shutdown/exit and MethodNotFound/negative capabilities.

First beads: server shell/framing; capabilities/root policy; sync/positions; diagnostics; query projection tranches; semantic delta; virtual content/refresh; watches/reload; edits; cancellation/progress/errors.

Close: every advertised method has direct-result/decoded-LSP equivalence and all unimplemented capabilities are absent.

### LS-11 — Direct-host and editor integration

Type: delivery
Clauses: `LS-BASIC-001`, `LS-LSP-001`

Deliver stable direct session/result DTOs; embedded-host example/test; VS Code server launch/package; language/project associations; open/diagnostics/completion/hover/navigation/references/rename/tokens/reference reload smoke; project/reference helper commands outside semantic LSP; server restart/close/degraded behavior.

First beads: embedded host; extension launch/package; basic click-through; project/reference reload; restart/degraded state.

Close: one direct host and one LSP editor consume the same snapshots end to end.

### LS-12 — Windows COM-reference certification

Type: delivery/conformance
Clauses: `WIN-META-001`, `LS-WORKSPACE-001`, `LS-BASIC-001`
Dependencies: WIN authoritative resolver/raw metadata handoff

Deliver registered/file typelibs; reference version/order changes; stable virtual libraries/coclasses/interfaces/members/enums/records/events; early-bound and known-projection late-bound completion/signatures/hover; runtime activation distinction; x64 target contexts; broken/unregistered references; Excel Object Browser/public metadata cross-check.

First beads: resolver integration; virtual content; query matrix; revision invalidation; x64; broken refs; Excel metadata cross-check.

Close: COM column is green for every basic query without requiring COM runtime/serving completion.

### LS-13 — Terminal architecture and IDE-profile release

Type: support/conformance
Clauses: `CONF-DONE-001`, `DOC-*`

Reconcile system/compiler/language-service contracts, architecture, code, extension, matrices and docs; remove deleted-stack claims; generate capability summary; document direct/LSP/single-root/deferred boundaries; run direct/protocol/editor/performance/COM/governance gates and final API/protocol/user-path fresh-eyes review.

Close: every required delivery epic is closed and runnable behavior matches advertised capability.

## 9. Dependency graph

| epic | hard prerequisites |
|---|---|
| LS-0 | accepted workset and CORE-1 green authority/gate baseline |
| LS-1 | CORE-2 provenance, CORE-3 AnalysisResult/diagnostics |
| LS-2 | LS-0 plus compiler/project identity contracts |
| LS-3 | LS-1, LS-2 |
| LS-4 | LS-1, LS-2 |
| LS-5 | LS-1, LS-2, LS-4 |
| LS-6 | LS-1/2/4/5 plus the producer gates in §4 |
| LS-7 | LS-1, LS-2, LS-4, LS-6 |
| LS-8 | LS-2 and shared CST |
| LS-9 | starts after LS-2; closes after all feature lanes |
| LS-10 | shell after LS-2; each method after its direct feature |
| LS-11 | LS-10 plus all smoke-path features |
| LS-12 | LS-4/5/6 plus WIN raw metadata handoff |
| LS-13 | every required delivery epic and producer rerun |

## 10. Checks and terminal condition

Per feature bead: direct semantic test, reference-kind neighbor, transport equivalence if advertised, matrix/contract update, performance check for hot paths and fresh-eyes review.

Merge gate: CORE-1 green workspace/governance baseline; direct suite; LSP transcripts/equivalence; deterministic Linux/Windows tests; no advertised method without a green direct row.

Release gate: Linux/Windows x64 reference target contexts, VS Code and embedded host, Windows COM metadata, full producer-dependent reference rerun, performance/cancellation/memory, fuzz/no-panic and docs/matrix truth.

This workset is complete only when compiler and editor facts share one pipeline; real workspaces/overlays/invalidation/cancellation work; every basic feature/reference kind is green; direct API and pinned LSP are equivalent; one host and editor pass; robustness/performance gates are green; and no current doc or extension claims the deleted stack.

## 11. Bead-preparation handoff

Create LS-0 through LS-13 epics and rollout beads, then materialize the first candidates above. Every bead names contract clauses, producer dependencies, reference kinds, matrix rows, direct/transport evidence, target context, performance/lifecycle impact and residual behavior. Historical code/tests enter only through explicit port beads. An LSP shell, docs or editor packaging cannot close a semantic capability epic.
