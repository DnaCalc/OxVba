# OxVba Language-Service Architecture V1

Date: 2026-07-10
Status: current destination architecture; implementation absent on the clean stack
System clauses: `PROFILE-IDE-001`, `LS-*`, `COMP-ANALYSIS-001`, `PROJ-REF-001`
Supersedes: `LANGUAGE_SERVICE_SPEC_V1.md`, `LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md` and root language-service guidance for current architecture

## 1. Target state

The OxVba language service consumes compiler-owned analysis results into immutable semantic snapshots. It adds workspace lifecycle, indexing, query APIs, invalidation and transport; it does not add another parser, binder, project model or type system.

The direct Rust API is the semantic product surface for embedded hosts. LSP is a thin negotiated projection over the same results. Source projects, verified OxImage references, the VBA library, host providers, COM typelibs, Declare declarations and generated provenance participate through production provider contracts.

## 2. Compiler analysis boundary

The service consumes the versioned AnalysisResult defined by the compiler contract:

- CST and preprocessing context;
- declarations, scopes and signatures;
- definition/use-site bindings;
- expression/member/call/result types;
- argument/accessor/default-member facts;
- diagnostics and related locations;
- project/reference/provider provenance;
- optional CoreProgram, which the service never requires for malformed editor text.

Valid strict compilation and editor analysis produce identical facts. Incomplete source may retain poison/unknown facts but cannot reach code generation. The service never binds substrings or invents a semantic recovery path that compilation cannot represent.

## 3. Identity model

Every workspace, project, document, snapshot, provider and metadata revision has explicit identity. A query uses a snapshot-bound opaque symbol handle that is invalid in any other snapshot.

A deterministic logical SymbolKey supports equivalence and cache reuse across unchanged snapshots. Keys include project/document or provider identity, provider version/digest, declaration identity, target/conditional context and reference provenance. Name alone is never identity.

Rename and workspace edits carry the originating SnapshotId and document versions. Stale results or edits are rejected rather than heuristically rebased.

## 4. Workspace model

One service session initially owns one project/reference closure. It loads `.basproj`, the supported `.vbp` subset and convention projects through `oxvba-project`, preserving project options, target constants, source encodings and reference order.

Open/change/save/close overlays replace document text for a version; they do not replace project or compiler semantics. Immutable snapshots support concurrent reads while dependency-aware invalidation rebuilds changed documents, affected modules/projects and provider/reference dependents.

Closed source, project files, typelibs, host metadata and OxImage references have explicit revision/digest watches or reload commands. Cancellation prevents stale results from publishing but never corrupts the current snapshot.

## 5. Reference-kind parity

The basic query set covers:

- active project source;
- referenced source projects, including public data;
- verified OxImage export surfaces loaded through the production sealed loader;
- the typed VBA base-library inventory;
- versioned host-injected globals/classes/interfaces/events;
- authoritative COM typelib libraries/types/members/events;
- source-owned Declare identifiers and call sites;
- generated compiler source through mapped or explicit virtual provenance.

Read-only metadata provides stable virtual locations/content. Source-owned Declare identifiers may navigate and rename; Lib/Alias literals and external target metadata remain read-only. Missing DLL/export availability is a runtime issue, not a compiler diagnostic.

Compiler-internal structural intrinsics and generated helper names never leak into completion, symbols or metadata documents.

## 6. Direct semantic API

The direct API provides immutable result DTOs for:

- syntax/symbol/project/reference diagnostics;
- document and workspace symbols;
- semantic classification/tokens;
- hover;
- completion and lazy resolve;
- signature help;
- definition, type definition and implementation;
- references and document highlights;
- prepare rename and versioned workspace edits;
- bounded diagnostic-driven code actions;
- folding and selection ranges;
- read-only virtual metadata content.

Results include snapshot/version, identity, provenance, ranges and read-only/renameability status. Ordering and deduplication are deterministic.

## 7. Diagnostics and incomplete code

Syntax, symbol, bind, project/reference and artifact diagnostics retain compiler codes, phases, primary ranges and related locations. The service deduplicates presentation without changing compiler meaning.

Common mid-edit states return stable partial facts and bounded diagnostics without panic or cascading noise. Inactive conditional regions and generated/normalized source keep correct original or virtual mappings.

Diagnostic result identities support unchanged responses and refresh after document, project, reference or provider changes.

## 8. Navigation, completion and editing

Navigation follows symbol identity across modules, projects and metadata. References classify reads, writes, calls, implementation and property/default-member roles where compiler facts provide them.

Completion and signature help use typed scope/member/call facts, visibility, reference precedence, properties/default members, Optional/named/ParamArray and With contexts. Arbitrary late-bound Object receivers do not promise members unless a known host/typelib projection exists.

Rename edits only writable source definitions and bound references. It rejects collisions, stale versions and metadata edits, and handles accessor groups and Implements according to compiler identity. Code actions are diagnostic-code-driven and recompile their proposed result.

## 9. LSP projection

The server pins an exact LSP 3.18.x meta-model/spec revision. It advertises only features backed by the direct API and proves direct-result versus decoded-LSP-result equivalence.

The transport owns:

- initialize/shutdown/exit and honest capabilities;
- one-root workspaceFolders/rootUri precedence and changes;
- full/incremental versioned text synchronization;
- UTF-16 default and negotiated position encodings;
- pull diagnostics and deliberate push fallback;
- semantic-token full/delta result identities;
- virtual textDocumentContent and refresh with client fallback;
- diagnostic/semantic refresh after external changes;
- watched files or explicit reload commands;
- versioned WorkspaceEdit.documentChanges;
- progress, partial results and exactly-one-response cancellation semantics;
- URI/path normalization;
- protocol-clean stdout and byte-accurate framing.

For client cancellation, a still-open request receives exactly one legal response. RequestCancelled is used when cancellation is answered as an error; ServerCancelled is limited to methods that support retrigger semantics. Late-cancel and partial-result races remain valid and tested.

## 10. Host and editor integration

Embedded hosts create workspaces and call the direct API without JSON-RPC. The VS Code extension launches/packages the real LSP server and uses project/reference helper commands outside semantic protocol methods.

At least one embedded host and one editor client perform open, diagnostics, completion, hover, navigation, references, rename, tokens and project/reference reload over the same semantic snapshots.

The IDE profile does not claim debugger, forms designer, formatter or broad refactoring delivery.

## 11. Performance and robustness

The service measures cold workspace load, local edits, dependent invalidation, provider/reference reload and each query family on representative workspaces. Budgets are evidence-backed and do not permit stale or semantically partial answers.

Memory and handles remain bounded across open/change/close/reload cycles. Concurrent reads are deterministic, obsolete work cancels, protocol and edit positions are fuzzed, and invalid positions/messages never panic the server.

## 12. Completion evidence

Language-service completion requires:

- compiler strict/editor fact parity;
- reference-kind matrix rerun after all producer contracts stabilize;
- incomplete-code and Unicode/CRLF position corpora;
- direct API tests for every feature/reference kind;
- decoded-LSP/direct-result equivalence and negative capabilities;
- virtual content/refresh, roots/watch/reload and cancellation races;
- stale rename/edit rejection;
- Windows x86/x64 COM-reference target contexts;
- embedded-host and editor smoke paths;
- performance, cancellation and lifecycle evidence;
- no active documentation or extension claim referring to deleted language-service crates.
