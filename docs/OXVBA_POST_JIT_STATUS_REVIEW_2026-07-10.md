# OxVba Post-JIT Status Review

Date: 2026-07-10
Review state: complete
Scope: compiler/binder, Core IR, OxIR/OxImage, runtime/VM3, Cranelift JIT, validation posture, and language services
Authority: review evidence and work planning; this document does not itself close any capability lane

Architecture follow-up: the destination and current realization are now governed by [`spec/OXVBA_SYSTEM_CONTRACT_V1.md`](spec/OXVBA_SYSTEM_CONTRACT_V1.md) and [`ARCHITECTURE.md`](ARCHITECTURE.md). The three worksets linked below were subsequently rewritten around those contracts; this review remains their entry evidence rather than architecture authority.

## 1. Executive verdict

OxVba now has a credible clean compiler and dual-runtime architecture:

`project/source -> target-aware conditional preprocessing
               -> lossless CST of the normalized active-source view
               -> symbol providers -> binder/CoreProgram
               -> OxIR/OxImage -> VM3 or Cranelift JIT`

The compiler/binder is mature enough to be treated as a strong base. VM3 and the JIT execute a large language/runtime/library surface, and focused unit and differential coverage is unusually broad. The JIT is a real backend, not a stub: it compiles whole linked `OxProgram` sets without VM fallback, supports substantial control flow, calls, values, arrays, records, classes, cross-project classes, lifecycle behavior, and project events, and has promising execution-only performance evidence.

The requested “basics are in place” conclusion is nevertheless not supportable yet. The current repository cannot honestly claim a fully conforming VBA compiler, complete VM, complete JIT, or functioning language-service platform:

- ordinary workspace tests and Clippy are red;
- Windows exposes stale test expectations that the Linux implementation run did not see;
- the clean front end has concrete correctness gaps, including a UTF-8 panic and malformed conditional compilation that can fail open;
- the executable-image verifier/load boundary is too weak for an external artifact;
- VM3’s golden gate has a reproducible BSTR-balance failure;
- VM3/JIT parity evidence does not cover all advertised observables or all accepted OxIR;
- the JIT still lacks general error/Erl semantics, safe source recursion, persistent product sessions, and all real COM/native interop;
- the former language-service and LSP crates were removed and later deleted, while current docs still claim they exist.

The right status is therefore `in-progress`. There is no architectural barrier visible in this review, but there is a material delivery program before OxVba should become the trusted base for DNA Calc or be presented as complete standalone VB-universe tooling.

Three proposed worksets partition that program:

1. [Post-JIT core conformance and readiness](worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md)
2. [Windows JIT COM, native interop, and binary export](worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md)
3. [Clean-stack language services and IDE baseline](worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md)

## 2. Review method and evidence boundary

The review used:

- the governing charter, operations doctrine, current architecture and active status/workset files;
- current workspace code and crate tests;
- recent commit history, especially the clean-stack removal, JIT milestones, class tranche, and array fast-path tranche;
- current conformance, differential, package, host, and language-service evidence;
- targeted code review split across front end, package/VM, and JIT;
- Windows test execution available in this checkout;
- the captured Excel/VBA oracle replay already present in the repository.

No live Excel/VBE compile-oracle session, live Office COM matrix, registered TestEventServer matrix, 32-bit Office run, sanitizer run, Miri run, or native AOT/export run was performed during this review. Live Office/COM tests remain ignored and operator-run. Historical evidence is useful as target-behavior evidence, but evidence captured before the clean-stack replacement does not prove the current compiler/VM3/JIT path until replayed.

## 3. Current architecture

### 3.1 Actual production pipeline

The current production pipeline is:

1. `oxvba-project` loads a project/reference closure.
2. `oxvba-symbol` applies target-aware, length-preserving conditional blanking so active token offsets remain relative to the supplied module text.
3. `oxvba-syntax` builds a lossless green/red CST of the resulting normalized active-source view.
4. `oxvba-symbol` retains parsed modules, scans declarations, builds scopes/symbols/signatures, and composes project, referenced-project, VBA-library, host, COM-typelib, and Declare providers.
5. `oxvba-bind` lowers resolved CST directly to `oxvba_bundle::coreir::CoreProgram`.
6. `oxvba-oxir::elaborate` converts structured Core IR into typed CFG-based `OxProgram` values.
7. `OxImage` is the serialized project-closure artifact.
8. VM3 interprets `OxProgram`; `oxvba-jit` lowers the same linked `OxProgram` set to Cranelift.

The earlier VM2/`Bundle::Op` executable route, legacy compiler, frontend-v2/HIR fallback, source-rewrite compiler, and future-only JIT description are no longer current architecture.

### 3.2 Architectural strengths

- One CST, lossless relative to the normalized/preprocessed view, is shared by scanning and binding; conditional blanking preserves module offsets, while project-normalization provenance remains a gap described below.
- Symbol resolution is provider-based rather than source-specific.
- Core IR makes places, coercions, assignment intent, call arguments, properties/default members, project imports/exports, COM descriptors, events, arrays, records, and error statements explicit.
- OxIR is typed and CFG-structured, with explicit blocks, terminators, fault targets, local/global types, class metadata, record layouts, external descriptors, COM interfaces, and imports/exports.
- VM3 uses a heap frame stack and is suitable as the reference interpreter once its remaining trust gaps close.
- The JIT has a hard decline boundary and does not silently fall back to VM3.
- VM3 and JIT share `OxProgram`, `Variant`/runtime carriers, significant evaluation kernels, and `oxvba-rt-abi` state/helpers.

### 3.3 Architectural drift

The repository’s highest-level truth surfaces are behind the implementation:

- `docs/ARCHITECTURE.md` still describes VM2, `linearize`, a JIT stub, and Core IR as the sole active IR.
- executable-package specs still center the former Bundle shape and promise facts absent from `OxImage`;
- the JIT plan’s opening status stops around M4-2 while implementation has advanced into bounded M4-8;
- VM3 crate comments still describe an early bring-up stage;
- language-service docs describe crates and APIs that no longer exist;
- `CURRENT_BLOCKERS.md` and the front-end workset still refer to deleted fallback machinery;
- current validation matrices retain missing paths, “JIT planned” states, and pre-clean-stack evidence anchors.

This is not cosmetic. It makes status, scope, and closure claims unreliable.

## 4. Status by subsystem

| subsystem | current assessment | basis |
|---|---|---|
| Syntax/CST | strong foundation, in-progress | lossless parser and broad tests; UTF-8 panic, total-expression and malformed-directive gaps |
| Symbol/compiler/binder | mature foundation, in-progress | clean provider model and broad feature tests; typed return, call checking, project-field and diagnostic-span gaps |
| Core IR | substantial, in-progress | explicit semantic tree and rich descriptors; docs and package ownership are not reconciled |
| OxIR/OxImage | substantial, in-progress | typed CFG used by both backends; verifier/load contract and artifact metadata are incomplete |
| Runtime carriers/eval/rt-abi | substantial, in-progress | broad BSTR/Variant/SAFEARRAY/object/record support; duplicated semantics, unsafe API and lifetime issues |
| VM3 | broad reference candidate, in-progress | focused suites green and 645-row snapshot exists; golden balance failure, weak loader verification, unsupported accepted ops |
| JIT Linux-safe surface | broad implemented subset, in-progress | 164 crate tests and large focused differential set green; core error, recursion, session, evidence and architecture gaps |
| JIT Windows COM/native | planned/unimplemented | whole-image decline for external calls/COM interfaces; no real COM/native lowering or serving/export |
| VBA base library | broad implemented subset, in-progress | large corpus coverage; no complete current-stack Excel oracle matrix and bounded JIT allowlists remain |
| Project/reference support | broad implemented subset, in-progress | multi-project classes/calls work; public cross-project data fields and ambiguous-link validation remain |
| Language services | not implemented on clean stack | no active language-service/LSP crate or semantic query/session API; extension points at missing binary |
| Standalone wrapped/native outputs | bounded VM-backed output only, in-progress | `.oxi` and wrapped COM-server infrastructure exist; JIT sessions and general native DLL/EXE export do not |

## 5. Test and check results

### 5.1 Green focused lanes

| command/lane | result |
|---|---|
| `cargo fmt --all -- --check` | passed |
| syntax + symbol + bind + project tests | 843 passed, 0 failed |
| bundle + eval + OxIR + rt-abi + runtime + VM3 tests | 348 passed, 0 failed |
| `cargo test -p oxvba-jit` | 164 passed, 0 failed |
| selected Linux-safe JIT differential integration suites | 77 passed, 0 failed apart from shared snapshot-EOL gate |
| captured oracle conformance replay | 56 passed, 0 mismatches, 0 allowlisted cases; scope is mainly error/control-flow |
| VM3 native callback lane | 2 passed, 0 failed |

The committed VM3 golden contains 645 source rows: 451 successful outcomes and 194 error outcomes, with no unsupported or timeout rows. It is primarily VM3-minted regression evidence; only a subset is tied to current Excel/VBA observations.

### 5.2 Red repository lanes

| command/lane | result and interpretation |
|---|---|
| `cargo test --workspace` | stopped in `oxvba-differential` at 1,134 passed / 6 failed |
| differential library, single-threaded | 1,138 passed / 2 failed |
| `jit_scope_snapshot` | deterministic LF-generated versus CRLF-checkout mismatch; no `.gitattributes` policy |
| `vm3_golden_snapshot` | deterministic `bstrs: +1` balance on raised error 5 “operation blocked by host policy”; harness omits fixture name |
| parallel differential run | four additional carrier/array rows failed but passed individually and in the single-thread run; process-global handle counters are not fully isolated |
| host unit lane | stale test expects `New Collection` to be unsupported, although the JIT now executes it |
| Windows native Declare host tests | one lane plus 19 string/marshalling rows execute the VM leg but fail on a stale JIT diagnostic-string assertion |
| `cargo clippy --workspace --all-targets -- -D warnings` | failed on six unsafe-comment/unsafe-doc violations added by the July 8 array fast path; a dead-code warning is also present |
| `./scripts/docs-check.ps1` | passed |
| `./scripts/check-governance.ps1` | failed because `docs/AUTORUN_STATE.md` no longer contains the machine-readable `Terminal gate: v...` field required by `validate-gate-sync.ps1` |
| `./scripts/meta-check.ps1 -Fast -NoArtifacts` | stopped at the same pre-existing gate-sync failure |

These failures are mostly test-contract and hygiene defects rather than newly demonstrated semantic divergences, but they invalidate a green-baseline claim and show that the Linux run did not close Windows truth.

### 5.3 Not run or not proven

- live Excel compile/runtime oracle against the current stack;
- live early/late COM client matrices under the JIT;
- COM connection points and synchronous ByRef event writeback under the JIT;
- COM server activation through the JIT;
- Windows native Declare through the JIT;
- 32-bit Windows/Office ABI;
- wrapped/native JIT DLL or EXE output;
- ASAN/Miri/fault-injection lifecycle lanes;
- repeated long-lived JIT session/cache behavior;
- language-service or LSP tests, because no active implementation exists.

## 6. Compiler and binder findings

### 6.1 Strengths

The current compiler is materially cleaner than the active docs imply. Parsed modules are retained in one `ResolutionEnvironment`; declaration scans publish stable symbol IDs, scopes, signatures, source declaration spans and project export surfaces; provider order models active project, referenced projects, VBA library, host and COM sources; the binder emits explicit Core IR without a legacy string-rewrite fallback.

Focused coverage is broad across calls, classes, properties/default members, Optional/ParamArray, arrays, records, error handling, events, host roots, COM metadata and cross-project calls.

### 6.2 High-priority gaps

1. **UTF-8 source can panic.** Identifier scanning is ASCII-only and the unknown-byte path advances one byte before slicing a UTF-8 string (`lexer.rs:162-165, 223-226`). `Sub Café()` reproduced the panic. Invalid input must diagnose rather than abort, and valid VBA source-encoding/identifier policy needs an explicit contract.

2. **Malformed conditional compilation can fail open.** Expression parsing does not require EOF, conditional evaluation ignores parse errors, malformed `#Const` can be discarded, and missing `Then` is accepted. Invalid directives can select a branch instead of producing a compile error.

3. **Referenced-project public data is incomplete.** The surface provider explicitly excludes public fields/module variables because there is no cross-bundle field representation (`surface_provider.rs:103-110`).

4. **Static return types are erased on important call routes.** Referenced-project/library/native/Declare calls commonly bind as `Variant` even where the provider knows the declared return type. The erasure is verified; its possible effects on checked arithmetic, coercion, accessor selection and downstream dispatch are high-confidence inferences that still need minimized compiler/runtime/oracle cases.

5. **Argument compatibility is incomplete.** General ByVal checking largely covers scalar-to-object rejection, while array/UDT and element-type combinations are not comprehensively validated. Declare binding preserves descriptor types but does not perform a full arity/type legality pass.

6. **Most bind/symbol diagnostics lack source locations.** The shared diagnostic model can represent source and labels, but many compiler errors construct message-only diagnostics. This blocks reliable squiggles and weakens CLI/oracle comparison.

### 6.3 Additional explicit gaps and risks

- `DefDec` is rejected.
- `Option Compare Database` is rejected; Access-specific scope needs a decision and oracle.
- default-member expansion uses an arbitrary depth limit of 16 instead of a proven cycle rule;
- project loading rewrites startup/top-level-mainline sources, strips class preambles, normalizes line endings and reads module files strictly as UTF-8;
- auto-discovery is case-sensitive for lowercase extensions;
- attribute parsing has mixed case-insensitive detection and case-sensitive extraction behavior;
- the grammar matrix has 44 of 110 rows fixture-anchored, leaving 66 without fixtures; all 110 still carry stale `none_yet` route evidence. The language matrix contains only four rows and also lacks current-route anchors.

## 7. Core IR, OxIR, runtime and VM3 findings

### 7.1 Package trust boundary is not ready

`OxImage::validate` checks only format, version, non-empty programs and entry range. It does not verify each `OxProgram`. VM3 link selects the last program rather than honoring `image.entry`. Public load/JIT paths can therefore accept structurally invalid or semantically inconsistent serialized IR, and a valid non-last entry can execute the wrong program.

The existing verifier is useful but not a complete package verifier. It does not comprehensively cover entry/global-initializer IDs, exports, event handlers, descriptor IDs, record-layout references, duplicate units/exports, full operand/result typing, ranks, arity/signatures, or ownership/effect invariants.

### 7.2 Backend vocabulary mismatch

`AddRef`, `Release` and `DrainTerminations` are accepted OxIR instructions. The JIT lowers them, while VM3’s catch-all returns `Unimplemented`. The current elaborator appears not to emit them from ordinary source, but an accepted hand-built or serialized OxProgram is not backend-neutral.

### 7.3 Ownership and ABI risks

- VM3 and rt-abi independently build runtime class/interface descriptors, creating drift risk.
- Both use `Box::leak` for descriptor/name/parameter storage.
- host package-session preparation leaks the image and host Arc for `'static` lifetime.
- repeated compile/activate/drop can grow process memory even if carrier counters balance.
- rt-abi publishes safe `extern "C"` functions that dereference raw pointers; safe Rust callers can violate their hidden contracts.
- panic-to-status wrappers may return `ST_FAULT` without seating a deterministic internal error.
- manual drain-state toggles can remain stuck after panic.
- linker and export lookup choose first case-insensitive matches instead of rejecting ambiguous duplicate units/exports.

### 7.4 Artifact contract gaps

The real `OxImage` artifact does not yet carry several facts promised by package specs:

- content/integrity digest;
- helper ABI and carrier-layout versions;
- target/profile/capability requirements;
- source/debug maps;
- build and reference provenance.

`ArrayElementType` also cannot express nominal COM interface/object array elements, and record layouts lack broad nominal `VT_RECORD` identity. These become blocking at Windows COM boundaries.

### 7.5 Semantic-kernel and oracle risks

The shared-evaluation claim is only partially realized; VM3 still owns large areas of array, object, call, lifecycle and error semantics. Two suspicious but unconfirmed edges need Excel/VBA clarification:

- Collection selector conversion for fractional, huge, Empty/Null/Boolean/Date/Currency inputs;
- VM3 paths that convert failed string coercions to an empty string, including some `Err.Raise` operands.

## 8. JIT findings

### 8.1 Honest implementation status

The JIT directly lowers OxIR blocks to Cranelift. It currently uses one dynamic ABI:

`unsafe extern "C" fn(*mut JitRun, *mut RawExecState) -> i32`

Static calls can invoke local compiled functions, but Variant-backed frames and helpers still materialize the call state. There is no separate `ProcLoweringIr`, typed-primary-entry family, versioned helper descriptor catalog, general backend abstraction, or product cache.

M4-3 is complete. M4-4 through M4-7 contain large implemented subsets. M4-8 is complete only for its documented project-class subset. M4-9 and later Windows/native/serving/export lanes remain open.

### 8.2 Core Linux-safe gaps

1. **The public compiler does not call `verify_program`.** Direct or bundle-only OxPrograms can reach codegen/helpers without the available structural checks.

2. **General error semantics are incomplete.**
   - `SetLineNumber` is a no-op.
   - fault dispatch uses line zero.
   - `ErlGet` and `ErrFieldSet` are unsupported.
   - `Err.Raise`/`Error` accepts only bounded constant/basic forms and lacks full source/description/help metadata behavior.

3. **Source recursion is not safely proven.** Direct compiled calls use the native stack. Tests seed the logical frame vector near the ceiling but do not prove deeply recursive VBA reaches error 28 before a process stack overflow.

4. **Persistent JIT sessions are absent.** `prepare_image_session` rejects JIT. Source/manifest execution recompiles every invocation. There is no persistent globals/object state, `ProjectRuntimeSession`, `.oxi` JIT load session, comhost backend selection, or cache key.

5. **Helper panic handling is lossy.** A panic becomes bare `ST_FAULT` without a stable diagnostic or guaranteed Err state.

6. **Call/library coercion remains allowlisted.** Static return destinations and several dynamic/native library routes are narrower than general VBA rules.

### 8.3 Differential-evidence gaps

The harness advertises six axes, but `RunOutcome` currently carries values, final Err, raised/unsupported state and handle balance. It does not generally capture side-effect journals, lifecycle event order, COM transport counts or typed COM observations.

Arrays, records, objects and ProcRefs are often compared only by tag. The JIT scope snapshot records only compiled/raised/declined status and is not a value/error differential. Its zero-decline state therefore means “all included rows were accepted,” not “the full language is compiled and conforming.”

### 8.4 Maintainability and target policy

`oxvba-jit/src/lib.rs` is about 34,000 lines and combines codegen, symbol setup, runtime state, arrays, records, objects, members, events, helpers and extensive tests. Architecture docs disagree on whether direct OxIR-to-CLIF lowering is allowed. There is no helper ABI version, and target selection relies on `cranelift_native::builder()` without a declared supported-target/layout matrix.

Execution-only benchmarks are encouraging, with reported rows roughly 1.34x to 4.13x over VM3 and millisecond-scale compilation medians. They do not measure package load, compile, first call, repeated product invocations, or cache behavior.

## 9. Windows COM/native boundary

The known Windows boundary is confirmed in code:

- an image with `external_calls` or `com_interfaces` is declined;
- `ComCallEarly` is not JIT-lowered;
- imported COM/VBA coclass activation is unsupported;
- Declare/native calls and `OxInst::Ptr` are unsupported;
- project `WithEvents`/`RaiseEvent` is not evidence for COM connection points;
- there is no JIT COM server/vtable generation;
- there is no JIT comhost session, native DLL export, or AOT PE loader/output.

The VM and `oxvba-com` contain valuable Windows implementation and fixtures, but their existence does not confer JIT support. The Windows workset must cover:

- authoritative registry/file typelib discovery and reference resolution, including GUID/version/LCID/platform selection, aliases, inherited/default/source interfaces, coclass activation metadata and broken-reference diagnostics;
- one verified backend-neutral interop call plan for marshalling, cleanup, ByRef writeback and HRESULT/Err mapping, consumed by VM3 and JIT transports;
- late-bound `IDispatch` client behavior;
- early-bound vtable client behavior;
- connection points, event reentrancy and synchronous ByRef writeback;
- VBA classes served through `IDispatch`, type information and dual vtables;
- outgoing COM events;
- Declare, pointer helpers and `AddressOf` callbacks;
- exact object/interface/SAFEARRAY/record carriers;
- wrapped and native DLL/EXE output.

VBA7 Windows compatibility and native-output extensions are different claim classes. COM client/server/events and Declare parity form the compatibility gate; generic native DLL/EXE export forms a standalone-tooling extension gate. The overall workset closes only when both pass, but neither may be used as evidence that the other is complete.

## 10. Language-service status

### 10.1 Current fact

There is no active clean-stack language service.

The workspace has no `oxvba-languageservice` or `oxvba-lsp` member and no active `SemanticModel`, workspace overlay session, semantic-query API, or LSP server. The VS Code extension still tries to launch `oxvba-lsp` and instructs users to build a nonexistent crate.

Git history is explicit:

- commit `f69ec0b2` (2026-06-07) moved the language-service/LSP/tooling cluster out of the clean build and said it must be reimplemented over `oxvba-symbol` and `oxvba-syntax`;
- commit `b2773030` (2026-06-18) deleted the harvest copy.

The active docs and validation matrix still claim direct APIs, workspace sessions, LSP methods and tests that no longer exist. Those are historical designs/corpora, not current capability.

### 10.2 Reusable foundation

The rebuild does not start from zero:

- lossless CST with byte offsets and parser recovery;
- symbol IDs, scopes, declaration spans, visibility and signatures;
- project/reference closure loading;
- VBA-library, host, COM typelib, Declare and referenced-project providers;
- conditional preprocessing that preserves byte length for active-source offsets;
- a shared diagnostic DTO;
- rich Core IR call/type/dispatch facts;
- historical language-service/LSP code and tests recoverable from git history.

### 10.3 Missing foundation and product surface

- immutable, versioned semantic snapshots;
- document/workspace identity and overlay lifecycle;
- compiler-owned use-site binding/reference index;
- typed expression/member/call facts at source spans;
- source-mapped bind/symbol diagnostics;
- incremental invalidation and cancellation;
- definition/reference/rename safety across source, projects, verified OxImage exports and COM metadata;
- completion, signature, hover and semantic classification;
- virtual metadata locations for VBA library, verified OxImage and COM definitions;
- a thin LSP transport;
- a working editor smoke path.

The third workset must rebuild these against the clean compiler, not revive a second semantic model.

## 11. Explicitly unassessed or outside this basics gate

This review is not a charter-wide product audit. It did not assess the forms runtime, forms designer, debugger/debug protocol, broader runtime security model, Office application object-model implementation, or polished IDE product UX. It also did not certify macOS, browser/WASM, Tauri, or other deployment targets beyond the Linux and selected Windows-hosted checks described above.

Those surfaces are not implicitly complete. Any existing support claim for them needs its own current evidence and workset/status owner. The three worksets below establish the compiler, shared package, VM3/JIT, Windows VBA/COM/native boundary, and basic language-service foundation requested here; they do not authorize a charter-wide “all OxVba features complete” statement.

## 12. Workset partition and dependencies

### Workset 1: core conformance and readiness

Owns platform-neutral language/compiler/project semantics for the declared Linux/Windows certification targets, Core IR/OxIR/package verification, runtime/VM3 trust, Linux-safe JIT completion, product JIT sessions/cache, differential/oracle quality, test health, documentation truth and Windows/Excel validation of non-COM language behavior.

It also owns an explicit member-by-member VBA base-library and portable-host sweep; generic corpus coverage is not a substitute for that inventory.

It does not implement Windows COM/native JIT features or the language-service product.

### Workset 2: Windows COM/native and binary outputs

Owns all Windows-specific COM client/server/event repairs and parity in VM3 plus their JIT implementation, Windows Declare/native ABI, pointer/callback behavior, exact carrier shapes needed at those boundaries, wrapped JIT hosting, native DLL/EXE export and Windows/Excel certification. Workset 1 retains only platform-neutral VM3 trust.

It also owns authoritative Windows typelib/reference resolution and a shared verified interop-call contract used by VM3 and JIT.

It consumes the verified package/runtime/session baseline from workset 1.

### Workset 3: language services and IDE baseline

Consumes Core-owned compiler analysis facts into immutable semantic snapshots, indices and query APIs; owns project-aware editor workspaces, source/reference/COM/verified-OxImage coverage, direct APIs, LSP transport, VS Code smoke integration, performance/cancellation and truth repair.

It can begin in parallel, but final diagnostics and reference coverage depend on compiler/package fact deliverables from workset 1 and the authoritative raw typelib resolver/metadata handoff in WIN-1.6/WIN-1.7. It does not need to wait for COM runtime, serving or native-export completion to answer metadata queries.

## 13. Final readiness gate

OxVba may claim the requested standalone “basics” only when:

1. all three worksets satisfy their terminal gates;
2. no required child delivery bead remains open;
3. the canonical matrices show full support for the declared VBA7/Windows Office target, or a user-approved workset scope split with a named open successor owner;
4. VM3 and JIT pass the same accepted core corpus with structural values, error state, side effects, lifecycle and handle balance;
5. current-stack Excel/VBA compile and runtime oracle gates are green;
6. real Windows COM/native import, serving and event matrices are green for both VM3 and JIT; JIT-specific wrapped/native-output rows pass their separate artifact/client gate;
7. the direct language-service API and thin LSP use the compiler’s semantic facts across source, project, verified OxImage, VBA-library and COM references;
8. architecture, specs, validation matrices, worksets and executable tests tell the same truth.

Within that overall gate, the Windows workset reports two independently visible internal gates: VBA7 Windows compatibility and standalone native-output extension. Both must be green for the overall three-workset destination, while status reporting must keep their evidence distinct.

Until then, the repository should describe the system as a strong, broad, in-progress implementation rather than a fully conforming completed VBA toolchain.
