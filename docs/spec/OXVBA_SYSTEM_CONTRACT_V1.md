# OxVba System Contract V1

Date: 2026-07-10
Status: normative destination contract
Authority: subordinate to `CHARTER.md` and `OPERATIONS.md`; authoritative for OxVba target architecture and capability claims
Current realization: [`../ARCHITECTURE.md`](../ARCHITECTURE.md)

## 1. Purpose and use

This contract states what a completed OxVba system is. It separates durable architectural intent from current implementation status, delivery sequencing and historical design exploration.

The clauses in this document are destination requirements. A clause is not a claim that the current repository already satisfies it. Current realization and gaps belong in `docs/ARCHITECTURE.md`; delivery belongs in active worksets; compatibility evidence belongs in canonical validation matrices and reproducible artifacts.

Subsystem specifications refine these clauses but may not contradict them. When an older specification or guidance document conflicts with this contract or current architecture, the older document is historical unless it is explicitly reconciled.

## 2. Compatibility authority

### VBA behavior — `AUTH-VBA-001`

OxVba matches real VBA 7 compile-time and run-time behavior. Public specifications, the real VBA type library and reproducible black-box Excel/VBA observations are the semantic authorities. Historical OxVba behavior, legacy fallbacks and implementation convenience are not compatibility targets.

Where public specifications are incomplete or appear to disagree with reproducible Office behavior, the discrepancy is recorded with its authority, environment and project decision. Uncertainty remains an open compatibility row; it is not silently resolved in favor of the current implementation.

### Clean-room boundary — `AUTH-CLEAN-001`

All design and conformance work uses public documentation, published research and reproducible black-box observation. Proprietary implementation material, decompilation and reverse engineering of Office internals are outside the project method.

### VBA semantic references — `AUTH-SPEC-001`

The in-repo grammar, type-system, expression/call and project/reference specifications organize OxVba's clean-room semantic model:

- [`VBA_GRAMMAR_V1.md`](VBA_GRAMMAR_V1.md);
- [`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md);
- [`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md);
- [`VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md);
- [`PROJECT_MODULE_REFERENCE_SPEC_V1.md`](PROJECT_MODULE_REFERENCE_SPEC_V1.md).

These documents refine VBA semantics. They do not own product architecture, implementation status or completion claims.

## 3. Capability profiles

### Core VBA toolchain — `PROFILE-CORE-001`

The core profile contains project/source ingestion, conditional compilation, syntax, compiler analysis, Core IR, OxIR/OxImage, exact runtime carriers, the VBA base library, host abstractions, VM3 and the platform-neutral JIT surface. It is the minimum profile for claiming a complete dual-runtime VBA toolchain.

### Windows VBA compatibility — `PROFILE-WIN-001`

The Windows profile adds native COM client, server and event behavior; authoritative typelib/reference resolution; `Declare`; pointers and callbacks; x64 Windows ABI behavior; and real 64-bit Excel/VBA evidence. Windows-specific VM3 and JIT rows are both part of this profile.

The accepted Windows profile is x64-only. x86/32-bit Office, WOW64, ARM64 and other Windows architectures are outside the active target and carry no implied support or active successor program.

### IDE foundation — `PROFILE-IDE-001`

The IDE profile adds compiler-fact semantic snapshots, project-aware workspace analysis, a direct language-service API, reference-kind parity and a thin LSP projection. It does not imply a complete VBA IDE, forms designer or debugger UI.

### Standalone tooling — `PROFILE-TOOL-001`

The standalone tooling profile adds verified distributable OxImage packages, wrappers, COM-server artifacts and genuine native DLL/EXE outputs with explicit external ABIs. Wrapped and native outputs remain distinct claim classes.

### Extended product surfaces — `PROFILE-EXT-001`

Forms runtime/designer, semantic debugging/DAP, runtime security policy, browser/WASM, desktop shells and non-Windows COM are explicit extended profiles. They remain part of the charter destination where stated, but they do not become implicit completion requirements or implicit completed capabilities of the core profile.

No profile may be described as complete while a required clause or matrix row in that profile remains `in-progress`, `implemented-subset` or `planned`.

## 4. Whole-system architecture

### One semantic pipeline — `SYS-PIPE-001`

OxVba has one compiler-owned semantic pipeline:

```text
project/source
  -> target-aware preprocessing
  -> lossless CST of the supplied active-source view
  -> compiler analysis facts and diagnostics
  -> CoreProgram
  -> OxIR / verified OxImage
       |-- VM3
       \-- Cranelift JIT
```

Production compilation, editor analysis, VM execution and JIT compilation do not maintain competing parsers, binders or reconstructions of VBA meaning. Consumer-specific lowering may choose physical representation and code shape, but it may not rediscover language semantics from source or names.

### One executable artifact — `SYS-ART-001`

OxImage is the shared serialized executable-semantic artifact for a complete project/reference closure. It contains verified OxPrograms plus the types, descriptors, imports, exports, capabilities, source provenance and ABI facts required by consumers.

The historical `Bundle`/`Op`/`.oxb` machine is not a product execution artifact. Any remaining use is a bounded internal metadata source that must either migrate into current typed contracts or remain explicitly non-authoritative.

### Two runtime consumers — `SYS-DUAL-001`

VM3 and the JIT consume the same verified semantic artifact and expose the same host/session observable for every shared capability row. VM3 is the permanent readable reference interpreter; the JIT is a native-code implementation of the same contract.

Neither backend silently invokes the other. A backend may reject an incompatible target before execution, but an accepted row must execute with equivalent VBA-observable behavior.

### Explicit ownership — `SYS-OWN-001`

Each semantic decision has one owning layer. The compiler owns source meaning; OxIR/OxImage own executable meaning and metadata; runtime/eval/rt-abi own value and helper semantics; VM3 and the JIT own execution strategy; HAL owns host capability and policy; COM owns Windows COM boundary behavior; build/host layers own orchestration and packaging; language services own indexing and query projection over compiler facts.

Boundary adaptation does not justify duplicate semantic models or lossy canonical values.

## 5. Source, project and compiler contract

### Source identity and encoding — `SRC-ID-001`

Every input module has stable project, module, document and encoding identity. File decoding is explicit and diagnostic; valid supported source never panics. Project normalization, exported-module preambles, generated startup code and conditional compilation retain original-source coordinates where applicable and explicit virtual/generated provenance otherwise.

### Conditional compilation — `SRC-CC-001`

Conditional compilation is target-aware and fail-closed. It evaluates the VBA conditional environment, preserves source coordinates for active text and produces compile diagnostics for malformed directives or expressions. It never selects a branch by ignoring syntax errors.

### Lossless syntax — `SYN-CST-001`

The compiler and language service share one lossless CST for the preprocessed module view. Trivia, continuations, attributes and incomplete syntax remain representable. Recovery returns bounded partial structure and diagnostics rather than panicking or inventing executable semantics.

### Project and reference closure — `PROJ-REF-001`

Project loading constructs one deterministic reference closure across source projects, verified OxImage references, the VBA library, host references, COM typelibs and `Declare` declarations. Reference identity, order, visibility, `Option Private`, ambiguity, broken-reference state and provenance are explicit.

Source and compiled project references expose the same VBA-visible public surface, including public data and callable/class metadata. Compiled references do not become name-only facades.

### Compiler analysis result — `COMP-ANALYSIS-001`

The compiler exposes the closed public mode enum `AnalysisMode::{Strict, Editor}` and produces one immutable `AnalysisResultV1` containing lossless syntax/CST, stable project/module/document/provider identities, scopes, declarations and resolved use sites, expression/member/call/result types, argument mapping, dispatch/accessor/default-member decisions, diagnostics, provenance and an optional CoreProgram.

Every compiler span is a half-open UTF-8 byte range in an identified supplied active-view document, with versioned maps to original, normalized or explicit generated/virtual documents. Strict compilation accepts only an error-free result with a CoreProgram. Editor analysis may retain poison or unknown facts for incomplete source, but malformed source cannot reach code generation. Valid-source facts are identical in Strict and Editor use.

### Typed binding — `COMP-BIND-001`

Binding preserves declared types and signatures across local code, referenced projects, the VBA library, host providers, COM metadata and `Declare`. ByVal, ByRef, Optional, named, omitted, ParamArray, property, default-member, object/interface, array and UDT rules are decided once and represented explicitly in Core IR.

### Diagnostics — `COMP-DIAG-001`

Syntax, symbol, binding, project and package diagnostics have stable codes, phase, severity, primary location and related locations. Compiler diagnostics describe VBA compile-time behavior; runtime availability failures such as a missing DLL export remain runtime diagnostics rather than being recast as compile errors.

## 6. Semantic IR and artifact contract

### Core IR — `IR-CORE-001`

Core IR is the resolved, source-independent semantic tree emitted by the compiler. It makes places, assignment intent, coercions, calls, properties/default members, error statements, arrays, records, objects, events and external descriptors explicit without committing to interpreter or native-code storage.

### OxIR — `IR-OXIR-001`

OxIR is the typed backend-neutral control-flow representation elaborated from Core IR. Its instruction, terminator, metadata and effect vocabulary is total: every verified operation has an explicit VM3 and JIT disposition for the declared target.

Backend-specific procedure lowering may plan registers, stack slots, ABI calls and cleanups, but it consumes OxIR facts and does not become another language IR.

### Verified program and image — `IMAGE-VERIFY-001`

External artifacts use bounded decoding before large allocation and sealed semantic verification before linking, code generation or execution. Production APIs accept verified program/image handles rather than raw deserialized OxProgram values.

Verification covers identity uniqueness, entry points, CFG structure, types, ranks, signatures, operands/results, error edges, imports/exports, classes, records, arrays, events, external descriptors, target capabilities and ownership/effect invariants.

### Artifact identity and compatibility — `IMAGE-ABI-001`

OxImage records a content digest, schema version, target/profile requirements, helper ABI, carrier/layout ABI, source/debug maps and build/reference provenance. Cache and loader compatibility are deterministic. Incompatible images fail before execution with stable diagnostics.

Detailed artifact requirements live in [`OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md).

## 7. Runtime, library and host contract

### Exact value carriers — `RUNTIME-VALUE-001`

OxVba runtime values use exact VBA/OLE-compatible carrier families where the platform contract defines them: BSTR, VARIANT, SAFEARRAY, IUnknown-compatible object identity and numeric primitives. Declared VBA types and Variant subtypes remain distinct semantic facts even when they share storage.

Ownership, clone, move, ByRef alias, erase, preserve, termination and cross-boundary rules are explicit for scalars, strings, objects, arrays, records and procedure references. Exact carrier layout is not a COM-only projection.

### Shared semantic kernel — `RUNTIME-EVAL-001`

Value operations, coercion, comparison, string behavior, errors and lifecycle operations have one semantic owner used by both VM3 and JIT. Backend-local implementations are permitted only where execution mechanics differ and must be differentially proven against the shared contract.

### Runtime ABI — `RUNTIME-ABI-001`

The runtime-helper ABI has one versioned descriptor catalog with stable helper identities, typed signatures, ownership, allocation, error, reentrancy, target, apartment and panic-containment contracts. VM3, JIT and Windows adapters generate registration from that catalog rather than private tables. Raw-pointer entry points are explicitly unsafe behind typed internal wrappers. Panics never unwind across foreign or generated-code boundaries and always seat deterministic internal diagnostics.

### VBA base library — `LIB-VBA-001`

The VBA base library is a complete typed library surface, not a collection of incidental opcodes. Every public member and overload has a declared signature, compiler binding, VM3/JIT execution route, host/locale policy where relevant and Excel/spec evidence.

Pure library behavior belongs in shared library/runtime semantics. Filesystem, environment, time, interaction and other host-sensitive members delegate through explicit host capabilities without changing their VBA-visible errors or side effects.

### Host abstraction and policy — `HOST-HAL-001`

HAL owns capability discovery, profile selection, policy and delegation to filesystem, UI, environment, time, native loading, COM and event services. It does not own compiler semantics, canonical value representation or COM wire rules.

Host policy may deny a capability, but denial produces a stable VBA-compatible outcome and is distinct from missing implementation. Deterministic test adapters and real platform adapters implement the same host contract.

### Product sessions — `HOST-SESSION-001`

Hosts consume a backend-neutral project runtime session: load a verified image, select a backend, initialize, invoke, retain project state, reset/reload and dispose. Globals, class singletons, live objects, events, Err state, host policy and apartment/thread rules have explicit session lifetimes.

VM3 and JIT sessions expose equivalent behavior. Compiled code, metadata and runtime state are session-owned or safely shared; repeated create/use/drop cycles do not leak process-lifetime allocations.

## 8. VM3 contract

### Reference interpreter — `VM3-REF-001`

VM3 is the complete typed-CFG interpreter and executable reference for verified OxIR. It executes every operation admitted for its declared target, including errors, lifecycle, host, COM/native and session behavior where the profile requires them.

VM3 does not define VBA behavior merely by being first. Its observable is validated against public specifications and Excel/VBA; discrepancies are resolved toward VBA and become permanent regression evidence.

### Interpreter robustness — `VM3-SAFE-001`

VM3 uses bounded heap-owned execution state for VBA frames, deterministic error propagation and explicit cleanup. Malformed images are rejected before execution; source recursion reaches VBA-compatible limits without relying on the process native stack.

Detailed VM and OxIR requirements are part of [`OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md).

## 9. JIT contract

### Typed compiled core — `JIT-CORE-001`

The JIT compiles verified OxIR through an inspectable backend-owned lowering plan that makes physical slots, calls, fault edges, ownership, cleanups and helper requirements explicit. Statically known procedures use typed primary entries and direct typed calls where VBA semantics permit.

A universal Variant-frame invocation thunk remains available for late binding, reflection, host entry, COM and genuinely dynamic calls. It is an adapter around the typed compiled core rather than the only internal calling shape.

### JIT semantic parity — `JIT-PARITY-001`

The JIT accepts the complete declared OxIR/profile surface or rejects the target before partial code generation. It does not silently fall back to VM3. Results, full Err state, structural values, side effects, event/lifecycle order, transport facts and carrier balance match VM3 and Excel/VBA where observable.

### JIT sessions and cache — `JIT-CACHE-001`

Compiled modules live in persistent project sessions and may be cached by verified image digest, target ISA/CPU features, helper ABI, carrier/layout ABI, host capability profile and relevant compilation settings. Cache admission, invalidation, eviction and code lifetime are bounded and deterministic.

### JIT and native-code continuity — `JIT-AOT-001`

The same verified lowering and runtime ABI contracts support in-memory JIT execution and later object/blob/native packaging. Native output may add entry/export thunks and a loader, but it may not reconstruct compiler semantics or relabel a runtime-backed wrapper as a native program.

Detailed JIT requirements live in [`OXVBA_JIT_ARCHITECTURE_V1.md`](OXVBA_JIT_ARCHITECTURE_V1.md).

## 10. Windows COM and native interop contract

### Authoritative Windows metadata — `WIN-META-001`

Windows typelib/reference resolution produces stable library, type, member and event identities from registered or file-backed authoritative metadata. GUID/version/LCID/platform selection, reference order, aliases, inherited/default/source interfaces, coclass activation and broken-reference state are explicit and reusable by compiler, runtime, build and language-service consumers.

### Shared interop plan — `WIN-PLAN-001`

Verified compiler/package descriptors elaborate into one backend-neutral interop call plan containing the exact signature, transport, marshalling temporaries, ownership, cleanup, ByRef writeback order, error mapping and reentrancy policy. VM3 and JIT use execution adapters for the same plan.

### COM client — `COM-CLIENT-001`

OxVba supports late-bound IDispatch and early-bound native-vtable COM calls, activation/GetObject, properties/default members, named/optional/ByRef arguments, enumeration, object identity, arrays/records and VBA-compatible HRESULT/EXCEPINFO/IErrorInfo behavior. Early-bound rows remain early-bound and are proven by transport evidence.

### COM events — `COM-EVENT-001`

OxVba consumes and emits COM connection-point events through typed source-interface metadata. Event delivery preserves ordering, identity, reentrancy, lifecycle and synchronous ByRef writeback before the native caller returns across the declared apartment/process matrix.

### COM serving — `COM-SERVE-001`

Exported OxVba classes can be activated and consumed as late-bound and early/dual COM objects with stable IUnknown identity, type information, generated vtables, Implements, errors and outgoing events. Registration, class factories, apartments, proxy/marshalling strategy, local-server lifetime and unload behavior are explicit for x64 Windows.

### Native import and callbacks — `NATIVE-IMPORT-001`

VBA7 `Declare`, pointer helpers and AddressOf callbacks follow the exact x64 Windows calling convention, layout, string/buffer, array/record, ByRef, lifetime, loader-policy and LastDllError behavior. Compiler legality and runtime availability remain distinct phases.

Detailed Windows requirements live in [`OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`](OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md).

## 11. Build and deployment contract

### Package-first builds — `BUILD-PACKAGE-001`

Every build target starts from a verified OxImage and versioned metadata. Wrappers embed or deploy that artifact with a compatible runtime; COM servers additionally carry authoritative class/interface/event and registration metadata.

### Honest output classes — `BUILD-CLASS-001`

OxImage, wrapper executable, wrapper library, wrapped COM server, native DLL and native EXE are distinct output classes with distinct manifests and evidence. A wrapper is not called native merely because it contains generated code or hides the runtime.

### Native export ABI — `BUILD-NATIVE-001`

Native outputs select exported procedures through explicit project metadata and define a versioned external ABI for names/ordinals, types, ownership, errors, concurrency and initialization. DLL initialization performs no JIT compilation, COM initialization or blocking work under loader lock.

## 12. Language-service and IDE contract

### Compiler-fact snapshots — `LS-FACT-001`

The language service consumes compiler-owned `AnalysisResultV1` values into immutable, versioned semantic snapshots without parsing, rebinding or identity reconstruction. Snapshot-bound handles are never reused across versions; deterministic logical symbol keys support equivalence and cache reuse with provider/version provenance.

### Workspace and references — `LS-WORKSPACE-001`

The service maintains real project/reference closures with versioned document overlays, dependency-aware invalidation, cancellation and stale-result suppression. Source projects, verified OxImage exports, the VBA library, host providers, COM metadata, Declare declarations and generated-source provenance participate consistently.

### Basic semantic surface — `LS-BASIC-001`

The direct API provides diagnostics, symbols, semantic classification, hover, completion, signature help, definition/type-definition/implementation, references/highlights, safe rename, bounded code actions, folding, selection ranges and read-only virtual metadata documents.

Queries never parse substrings, rebuild a second symbol table or edit read-only metadata. Incomplete source returns compiler-consistent partial facts.

### Thin LSP projection — `LS-LSP-001`

LSP is a negotiated transport projection over the direct API, pinned to an exact protocol/meta-model revision. It alone converts compiler UTF-8 byte spans to/from the negotiated client position encoding using the exact snapshot and document version. It owns JSON-RPC framing, document synchronization, position conversion, cancellation responses, result IDs, refresh, virtual content, versioned edits and capability advertisement—not VBA semantics or project discovery.

Detailed language-service requirements live in [`OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md`](OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md).

## 13. Diagnostics, debugging and forms

### Cross-layer source and debug maps — `DEBUG-MAP-001`

Compiler analysis, Core IR, OxIR, VM3, JIT and packaged/native outputs retain enough provenance to map diagnostics, runtime faults, stack frames and breakpoints to original or explicit generated source.

### Semantic debugger — `DEBUG-CORE-001`

The future debugger is an OxVba-owned semantic session over the same project runtime session and compiler source maps. Direct hosts and DAP consume one debugger core; neither transport defines independent stepping, breakpoint, expression-evaluation or object-inspection semantics.

Debugger absence does not weaken core compiler/runtime claims, but no debugger profile is complete without equivalent VM3/JIT behavior for supported operations.

### Forms runtime — `FORMS-RUNTIME-001`

The forms runtime implements VBA-compatible form/class/event/lifecycle behavior over OxVba runtime values and host UI capabilities. Forms Designer is a separate IDE surface over the same form/project metadata, not a source-rewriting compiler path.

Forms and designer status remain explicit extended-profile rows until implemented and evidenced.

## 14. Portability and security

### Portable semantic core — `PORT-CORE-001`

Project loading, compiler analysis, Core IR, OxIR, exact portable carrier semantics, VM3 and language services are platform-neutral unless a VBA behavior is intrinsically host-specific. Platform adapters expose capability differences explicitly.

Browser/WASM and desktop-shell targets consume the same verified image and host contracts. They do not imply native Windows COM; portable COM-shaped fixtures are not evidence of Windows COM compatibility.

### Security and resource policy — `SEC-BOUNDARY-001`

External artifacts, native descriptors, COM metadata and protocol inputs are bounded, validated and panic-safe. Host policy controls filesystem, UI, environment, native loading, COM activation and callback capabilities without altering compiler truth.

Unsafe boundaries document ownership and lifetime, contain panics/unwinds, reject incompatible metadata and clean up on success, VBA error, host denial, reentrancy and cancellation.

## 15. Evidence and completion contract

### Canonical matrices — `CONF-MATRIX-001`

Each capability profile has canonical independently closable rows split whenever semantic subset, backend, target, evidence authority or residual owner differs. Narrative status is derived from those rows rather than maintained as a competing truth source.

### Differential observable — `CONF-DIFF-001`

Shared VM3/JIT rows compare structural results, complete Err state, side-effect journals, lifecycle/event ordering, transport facts and carrier/resource balance. Tag-only or compiled/declined snapshots are coverage aids, not parity evidence.

### Excel/VBA oracle — `CONF-ORACLE-001`

Every VBA-observable semantic row has current-stack Excel/VBA evidence or an authoritative public-spec reason. Compile checks use the VBE compile command and owned-process UI automation; runtime captures record Office build/bitness, locale, source, result and cleanup.

### Safety, lifecycle and performance — `CONF-QUALITY-001`

Malformed-input, fuzz/property, repeated-session, leak/balance, sanitizer and appropriate formal lanes protect the artifact, runtime and interop boundaries. Performance is measured for cold load/compile/first call and warm repeated use without weakening semantic gates.

### Completion language — `CONF-DONE-001`

A profile or capability is complete only when its required implementation, tests, evidence and documentation agree and no required delivery residual remains open. Documentation, audits, fixtures, ignored tests or a useful subset do not close a broader capability.

## 16. Documentation contract

### Authority separation — `DOC-AUTH-001`

The charter owns mission and scope; operations owns execution doctrine; this system contract owns destination architecture; `docs/ARCHITECTURE.md` owns current realization and gaps; current subsystem specs refine contracts; active worksets own delivery; matrices and evidence own proof.

Historical plans, handoffs, reports and superseded specifications remain provenance only. They carry an explicit deprecation notice and cannot be cited as current capability or architecture authority.

### Traceability — `DOC-TRACE-001`

Subsystem specs, architecture deltas, workset epics and canonical matrices cite stable contract clause IDs. A new architectural direction changes this contract or a refining current spec before implementation claims are broadened.

The deprecation and successor map is maintained in [`DEPRECATION_LEDGER_2026-07-10.md`](DEPRECATION_LEDGER_2026-07-10.md).
