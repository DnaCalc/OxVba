# OxVba Architecture

Date: 2026-07-10
Status: current implementation realization and gap map
Destination authority: [`spec/OXVBA_SYSTEM_CONTRACT_V1.md`](spec/OXVBA_SYSTEM_CONTRACT_V1.md)
Evidence review: [`OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`](OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md)

## 1. Role of this document

The system contract states what completed OxVba must become. This document states what the repository is now, how the active crates fit together and where implementation diverges from that destination.

Current code and executable tests are implementation truth. A green subset is not a completed capability claim. The canonical worksets and validation matrices own delivery/evidence status; historical plans and reports are not architecture authority.

## 2. Current system summary

The active production path is:

```text
.basproj/.vbp/source
  -> oxvba-project project/reference closure
  -> oxvba-symbol target-aware conditional preprocessing
  -> oxvba-syntax lossless CST of the supplied active-source view
  -> oxvba-symbol declarations/providers/resolution environment
  -> oxvba-bind typed binding and CoreProgram
  -> oxvba-oxir::elaborate typed CFG OxProgram
  -> OxImage (.oxi project-closure artifact)
       |-- oxvba-vm3 interpreter
       \-- oxvba-jit Cranelift compiler
```

This is the sole production compiler/execution stack. The legacy compiler, HIR/front-end fallback, source-rewrite compiler, VM2, CoreProgram-to-Bundle linearizer and `.oxb` product execution path have been removed or retired.

The architecture is broad and real but not complete. The compiler/binder is the most mature layer. VM3 and the JIT cover a large platform-neutral language/runtime/library surface. The JIT does not yet implement native COM/Declare/pointer execution, and there is no active clean-stack language-service implementation.

## 3. Workspace ownership

### Compiler and project

- `oxvba-project` parses `.basproj`, the supported `.vbp` subset and convention projects, loads modules and constructs referenced-project closures.
- `oxvba-syntax` provides the lossless lexer/parser and green/red CST.
- `oxvba-symbol` owns conditional compilation, declaration scanning, scopes, symbols, signatures and providers for projects, referenced surfaces, the VBA library, host references, COM typelibs and Declare descriptors.
- `oxvba-bind` performs typed binding and emits `oxvba_bundle::coreir::CoreProgram`.
- `oxvba-diagnostics` provides the shared cross-layer diagnostic DTO; producing crates retain ownership of semantic error meaning.

### Executable semantics and runtime

- `oxvba-bundle` now primarily owns Core IR plus bounded legacy Bundle/Op types used by the synthetic VBA-library metadata path. It is not the current executable package producer.
- `oxvba-oxir` elaborates Core IR into typed CFG OxIR, defines OxProgram/OxImage, normalization/analysis passes and the current program verifier.
- `oxvba-runtime` owns Variant, BStr, SafeArray, ObjectRef, record and related value/lifecycle carriers.
- `oxvba-eval` owns shared value-semantic kernels used by VM3 and JIT for the extracted operation families.
- `oxvba-rt-abi` owns VM/JIT-neutral runtime cells and helper decisions used across the backend boundary.
- `oxvba-lib` implements VBA base-library bodies over runtime values and HAL services.
- `oxvba-vm3` is the sole typed-CFG interpreter and the JIT reference backend.
- `oxvba-jit` directly lowers linked OxProgram sets to Cranelift native code with a hard no-fallback boundary.
- `oxvba-differential` owns VM3 golden and VM3/JIT/oracle comparison harnesses.

### Host, platform and outputs

- `oxvba-hal` owns host capability/profile/policy and adapter contracts.
- `oxvba-com` owns Windows-first COM metadata, invocation, dynamic-object and wire-boundary services.
- `oxvba-host` orchestrates compiler/backend execution and VM3-backed package sessions.
- `oxvba-build` emits OxImage and wrapper/COM-server artifacts.
- `oxvba-comhost` is the reusable Windows in-process COM-server host, currently VM3-backed.
- `oxvba-cli` exposes source/project run and build commands.

No active `oxvba-languageservice`, `oxvba-lsp`, `oxvba-debug` or forms-runtime crate exists in the current workspace.

## 4. Source and compiler realization

### Current shape

For each supplied module, `oxvba-symbol` applies target-aware conditional preprocessing and calls the shared parser. Length-preserving blanking keeps active token offsets relative to the supplied module text. Parsed modules are retained in one resolution environment and scanned into project/provider surfaces before binding.

The binder walks resolved CST once, infers types, inserts coercions and emits Core IR with explicit places, assignment intent, calls, properties/default members, arrays, records, objects, events, errors and import/export descriptors.

The provider architecture is the right target shape: the binder asks what a symbol means rather than branching on whether it came from source, the VBA library, a host or COM metadata.

### Material gaps against the contract

- identifier scanning can panic on valid non-ASCII UTF-8 input;
- malformed conditional expressions/directives can fail open;
- original file provenance is incomplete after some project normalization/generation;
- referenced-project public data fields are not represented across compiled surfaces;
- declared return types are erased on important referenced/library/native call routes;
- ByVal/ByRef, arrays/UDTs and Declare legality checks are incomplete;
- many symbol/bind diagnostics lack source locations;
- grammar/language matrices do not yet provide current-route evidence for the full surface;
- the compiler does not yet publish the complete AnalysisResult/use-site fact contract required by language services.

The current detailed target is [`spec/OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md`](spec/OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md).

## 5. Core IR, OxIR and OxImage realization

Core IR is the resolved semantic tree emitted by the binder. `oxvba-oxir::elaborate` converts it into OxProgram: typed locals/places, basic blocks, instructions, terminators, fault edges, functions, globals, classes, records, external descriptors, COM interfaces, imports and exports.

OxImage serializes a project closure as pretty JSON `.oxi` with a schema magic/version, program list and entry index. VM3-backed host/build paths can load it and create package sessions.

### Material gaps against the contract

- `OxImage::validate` checks only basic header/count/entry conditions and does not seal fully verified programs;
- production VM3/JIT APIs can still receive raw OxProgram values;
- VM3 linking follows a last-program convention instead of treating image entry as fully authoritative;
- the verifier is incomplete for several ID, type, arity, descriptor, export, event and effect families;
- duplicate/ambiguous case-folded link identities are not comprehensively rejected;
- OxImage lacks content digest, helper/carrier ABI, target/capability requirements, full provenance and source/debug maps;
- a few OxIR operations have divergent VM3/JIT dispositions;
- product consumers do not yet share a sealed `VerifiedOxProgram`/`VerifiedOxImage` boundary.

The current target is [`spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md).

## 6. Runtime, library and host realization

Variant is the canonical execution carrier. BStr uses BSTR-shaped UTF-16 storage; SafeArray represents typed/dynamic arrays; ObjectRef provides IUnknown-shaped identity; records use descriptor-backed storage. The representation direction is exact VBA/OLE carrier layout rather than boundary-only projections.

`oxvba-eval` shares a meaningful value core across VM3 and JIT, while runtime, library, array/object/call/lifecycle/error behavior is still distributed among runtime, rt-abi, VM3, JIT, lib and host. `oxvba-lib` exposes a broad VBA-library implementation; host-sensitive operations delegate to HAL.

### Material gaps against the contract

- the shared semantic-kernel extraction is incomplete;
- VM3 and rt-abi duplicate some class/interface descriptor projection;
- descriptor and host/image session paths contain process-lifetime leaks;
- some public rt-abi functions hide raw-pointer safety contracts behind safe Rust signatures;
- panic/fault and manual drain/reentrancy state need RAII hardening;
- the base library has no member-by-member typed/compiler/VM3/JIT/oracle completion matrix;
- stateful file I/O, locale and several host-sensitive families remain bounded subsets;
- host denial, unsupported implementation and VBA runtime failure are not consistently separated in all paths.

The exact layout doctrine remains [`spec/OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md`](spec/OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md); library/host completion is governed by system clauses `RUNTIME-*`, `LIB-VBA-001` and `HOST-*`.

## 7. VM3 realization

VM3 interprets typed OxIR with heap-owned frames, typed places, ByRef aliases, error/Resume routing, class lifecycle, project events and broad library/runtime support. It is the sole product interpreter and the JIT reference backend.

Focused VM/runtime suites are broad, and the VM3 golden contains hundreds of value/error rows. This is useful regression evidence, not automatic VBA authority.

### Material gaps against the contract

- the current golden gate has a reproducible BSTR-balance failure on a policy error path;
- VM3 does not implement every verifier-accepted OxIR operation;
- loader verification and explicit image-entry handling are incomplete;
- class/interface descriptor ownership and repeated-session lifetime need hardening;
- some value/error edges still need live Excel/VBA clarification;
- the differential observable does not yet structurally compare every carrier/lifecycle family.

VM3 destination behavior is specified with OxIR/OxImage in [`spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md).

## 8. JIT realization

The JIT is a real Cranelift backend. It directly lowers linked OxProgram blocks, compiles whole accepted program sets without VM fallback and supports broad control flow, calls, values, arrays, records, project classes, lifecycle and project events.

Its current primary entry shape is a universal dynamic ABI:

```text
unsafe extern "C" fn(*mut JitRun, *mut RawExecState) -> i32
```

Static calls can invoke local compiled functions, but Variant-backed frames and helpers still materialize much of the call state. Source/manifest invocation recompiles; `prepare_image_session` remains VM3-only.

### Material gaps against the contract

- the public compiler does not require sealed verified image/program input;
- no inspectable procedure-lowering plan currently separates semantic OxIR from physical/codegen decisions;
- there is no typed-primary-entry family with a universal thunk as the dynamic adapter;
- helper registration is not a versioned descriptor catalog;
- line/Erl, writable Err fields and full dynamic Err.Raise metadata are incomplete;
- deep source recursion relies on the native stack and is not safely proven;
- persistent JIT package sessions and product cache do not exist;
- COM interfaces, external/native calls and pointer operations cause decline;
- the implementation is concentrated in a very large single source module;
- differential evidence is often status/tag based rather than fully structural.

The destination is [`spec/OXVBA_JIT_ARCHITECTURE_V1.md`](spec/OXVBA_JIT_ARCHITECTURE_V1.md). The older `JIT_V2_*` planning documents are historical design inputs, not current authority.

## 9. Windows COM and native realization

VM3, HAL, COM, host, build and comhost contain substantial Windows work: typelib loading, late/early bridge scaffolding, Declare execution, wrapper packaging, type-library emission and bounded COM serving.

The JIT currently rejects images containing real external calls or COM interface requirements and does not lower ComCallEarly, Declare/native calls or pointer operations. Project `WithEvents`/`RaiseEvent` proves internal event semantics, not native connection points.

### Material gaps against the contract

- authoritative registry/file typelib selection is not yet one stable metadata service for all consumers;
- VM3/JIT do not consume one verified backend-neutral interop call plan;
- real late/early COM JIT calls are absent;
- synchronous COM-event ByRef writeback is absent;
- JIT-backed COM serving/vtable generation is absent;
- exact nominal interface arrays and broad VT_RECORD shapes are incomplete;
- JIT Declare, pointer helpers and AddressOf callbacks are absent;
- x64 artifact and 64-bit Office certification is incomplete;
- JIT wrapper sessions and genuine native DLL/EXE exports do not exist.

The destination is [`spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`](spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md).

## 10. Host sessions, builds and outputs

`oxvba-host` compiles source/project closures for VM3 or one-shot JIT execution. `ProjectRuntimeSession` and `.oxi` session loading are VM3-backed. `oxvba-build` emits `.oxi` plus wrapper/COM metadata; `oxvba-comhost` loads the packaged image through VM3.

Current wrapped output infrastructure is valuable, but output labels must remain exact:

- `.oxi` is the current typed serialized artifact;
- WrapperExe/WrapperLibrary are runtime-backed wrappers where implemented;
- WrappedComServer is a reusable runtime-backed COM host;
- native DLL/EXE program outputs do not yet exist.

The target adds a backend-neutral verified session, JIT cache, JIT-backed wrappers and distinct genuine native outputs without loader-lock work.

## 11. Language-service realization

There is no active clean-stack language-service or LSP implementation. The former crates were removed from the workspace and later deleted. The VS Code extension and several older documents still describe the deleted surface and are deprecated by this architecture sweep.

Reusable foundations exist: lossless CST, symbols/scopes/signatures/declaration spans, providers, project closure loading, diagnostics, Core IR facts and historical tests/designs. Missing are the compiler AnalysisResult fact stream, semantic snapshots, overlays, indices, invalidation, direct query API, LSP transport and editor integration.

The destination is [`spec/OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md`](spec/OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md).

## 12. Debugging, forms, portability and security

No active debugger or forms-runtime crate exists. Older debugger/direct-host documents are design history rather than current capability. The system contract retains semantic debugger, forms runtime/designer and security as explicit extended profiles so their absence cannot be confused with either completion or permanent exclusion.

VM3 and much of the compiler/runtime are designed for portable hosts. Browser/WASM and desktop-shell documents describe earlier integration programs, but those targets were not certified in the 2026-07-10 review. Portable COM-shaped objects are not native Windows COM evidence.

Security currently appears mainly as host policy, artifact checks and unsafe-boundary discipline. A broader runtime security profile remains future work and must build on the same verified image and host capability model.

## 13. Capability status and accepted delivery

| profile | current status | accepted workset |
|---|---|---|
| Core VBA toolchain | broad, in-progress | [`worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md`](worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md) |
| Windows VBA compatibility | VM3 substrate plus missing JIT/general parity, in-progress | [`worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md`](worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md) |
| IDE foundation | not implemented on clean stack | [`worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md`](worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md) |
| Standalone tooling | `.oxi` and bounded wrappers only, in-progress | core and Windows worksets |
| Extended profiles | not assessed or not implemented as a unified profile | system contract plus future accepted worksets |

The dated review and these three accepted worksets are the current umbrella-program entry under `bd-59co`. Older ladders, worksets and handoffs remain provenance unless PROGRAM-0 explicitly consumes their residuals.

## 14. Documentation authority

The current hierarchy is:

1. `CHARTER.md` — mission and scope;
2. `OPERATIONS.md` — execution and evidence doctrine;
3. `docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md` — destination architecture and capability clauses;
4. this document — current realization and gaps;
5. current subsystem specifications;
6. accepted active worksets and canonical validation/evidence artifacts.

Superseded designs and guidance are classified in [`spec/DEPRECATION_LEDGER_2026-07-10.md`](spec/DEPRECATION_LEDGER_2026-07-10.md). A historical document remains useful for provenance but cannot override this hierarchy.
