# Workset: Ideal Core Toolchain and Dual-Runtime Realization

Date: 2026-07-10
Owner: unassigned
Status: accepted; directed bead rollout in progress under `bd-59co`
Type: architecture, capability and conformance delivery
Profile: `PROFILE-CORE-001`
Source review: [`../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`](../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md)

## 1. Outcome

Realize the ideal OxVba core architecture: one compiler-owned semantic analysis pipeline, one sealed verified OxImage artifact, one exact runtime/value/host substrate, a complete VM3 reference interpreter and a complete platform-neutral Cranelift JIT with typed primary calls, dynamic adapters, persistent sessions and a product cache.

The result is suitable as the trusted foundation for DNA Calc and standalone VB-universe tooling. It matches VBA compile-time and run-time behavior for the declared core target and exposes compiler/runtime contracts that the Windows and language-service profiles can consume without parallel models or architectural rework.

This is not a patch collection. The workset closes only when the current implementation has been transformed into the architecture defined by:

- system clauses `SYS-*`, `SRC-*`, `SYN-*`, `PROJ-*`, `COMP-*`, `IR-*`, `IMAGE-*`, `RUNTIME-*`, `LIB-*`, `HOST-*`, `VM3-*`, `JIT-*`, `CONF-*`;
- [`../spec/OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md`](../spec/OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md);
- [`../spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](../spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md);
- [`../spec/OXVBA_JIT_ARCHITECTURE_V1.md`](../spec/OXVBA_JIT_ARCHITECTURE_V1.md).

## 2. Target and boundaries

### Required certification target

- Linux x64 compiler, VM3 and JIT for platform-neutral behavior;
- Windows x64 compiler, VM3 and JIT for the same core behavior;
- Windows x64 compile-time conditional and pointer-width semantics;
- VM3 and JIT x64 core rows where pointer width affects execution;
- actual 64-bit Excel/VBA evidence for width-sensitive VBA-observable rows;
- Windows x64 JIT session/codegen admission supplied by the Windows workset.

x86/32-bit Office, WOW64, ARM64 and other Windows architectures are outside this accepted target. They have no active successor workset and carry no implied support.

macOS, browser/WASM, forms, debugger and the broader security profile are explicit extended profiles, not implied green rows. Narrowing this target requires a user-approved scope split with a named open successor delivery owner.

### Owned here

- source decoding/provenance, preprocessing, syntax and compiler analysis;
- typed binding, calls, diagnostics and project/reference public surfaces;
- Core IR and Core IR-to-OxIR elaboration;
- verified OxProgram/OxImage and package compatibility;
- runtime carriers, semantic kernel, helper ABI and session ownership;
- complete VBA base library and portable host behavior;
- complete VM3 for verified platform-neutral OxIR;
- ideal platform-neutral JIT architecture and semantics;
- persistent backend-neutral sessions and JIT cache;
- differential, safety and Excel/VBA core conformance;
- architecture/spec/matrix truth.

### Owned elsewhere

The Windows workset owns native COM, Windows Declare execution, pointers/callbacks, Windows JIT interop, Windows-specific VM3 parity and native outputs. The language-service workset owns semantic snapshots, indexing, query APIs and LSP over compiler facts.

This workset owns production of the compiler AnalysisResult facts consumed by the language service.

## 3. Architectural transformation

| current state | required state | clauses |
|---|---|---|
| preprocessing/parser can panic or fail open | decoded, provenance-aware, fail-closed source pipeline | `SRC-ID-001`, `SRC-CC-001`, `SYN-CST-001` |
| compiler emits CoreProgram but not a complete fact result | one strict/tolerant AnalysisResult with use-site/type/diagnostic facts | `COMP-ANALYSIS-001`, `COMP-DIAG-001` |
| provider routes erase types/public data | one exact typed signature/public-surface model | `PROJ-REF-001`, `COMP-BIND-001` |
| raw OxProgram/OxImage reaches consumers | sealed VerifiedOxProgram/VerifiedOxImage boundary | `IMAGE-VERIFY-001` |
| `.oxi` has weak identity/ABI/provenance | versioned digest/target/helper/carrier/source contract | `IMAGE-ABI-001` |
| runtime semantics/metadata ownership is duplicated/leaked | shared semantic/helper/descriptor ownership with bounded sessions | `RUNTIME-*`, `HOST-SESSION-001` |
| broad library catalog without member proof | complete typed member-by-member VBA library | `LIB-VBA-001` |
| VM3 has accepted-operation and lifecycle gaps | complete reference interpreter for verified OxIR | `VM3-*` |
| JIT directly lowers through one dynamic ABI | verified lowering plan, typed primary calls, universal thunk | `JIT-CORE-001` |
| unversioned helpers, no JIT sessions/cache | versioned helper catalog, persistent sessions and bounded cache | `RUNTIME-ABI-001`, `JIT-CACHE-001` |
| status/tag differential evidence | full structural observable and current Excel oracle | `CONF-DIFF-001`, `CONF-ORACLE-001` |

## 4. Binding invariants

1. Real VBA behavior is the compatibility target.
2. One compiler analysis pipeline serves strict compilation and editor consumers.
3. Core IR owns resolved language meaning; OxIR owns executable meaning; backend lowering owns only physical/codegen decisions.
4. Every product backend consumes sealed verified artifacts.
5. Every verified operation has explicit VM3 and JIT dispositions.
6. VM3 is the permanent reference interpreter, validated against VBA rather than self-authorizing.
7. The JIT never silently falls back to VM3.
8. Exact runtime carriers and source provenance survive the full pipeline.
9. Host denial, unavailable capability and missing implementation remain distinct outcomes.
10. Support/docs/audit beads do not close capability epics.

## 5. Canonical truth artifacts

Rollout creates or designates:

1. `docs/validation/CORE_COMPILER_VM_JIT_READINESS_MATRIX_V1.csv`
2. `docs/validation/VBA_BASE_LIBRARY_PARITY_MATRIX_V1.csv`
3. `docs/validation/OXIR_BACKEND_SUPPORT_MATRIX_V1.csv`
4. `docs/validation/OXIMAGE_PACKAGE_CONTRACT_MATRIX_V1.csv`
5. `docs/validation/CURRENT_STACK_EXCEL_ORACLE_MATRIX_V1.csv`
6. a generated profile summary derived from those matrices.

Rows split whenever semantic subset, target, backend, evidence authority or residual owner differs. Broad “language,” “library,” “array” or “call” rows cannot hide partial subsets.

## 6. Execution epics

### CORE-0 — Authority, target ledger and rollout

Type: support
Clauses: `DOC-*`, `CONF-MATRIX-001`

Deliver:

- create workset/epic/rollout bead tree;
- seed all five matrices from the review and current tests;
- map every row to system/subsystem contract clauses;
- classify old worksets/ladders/blockers as imported residual, historical or superseded;
- define the exact Linux/Windows-x64/64-bit-Excel environment ledger;
- establish a generated architecture/profile summary.

First beads: rollout graph; matrix skeletons; residual migration; environment manifest.

Close: every required clause has a matrix owner and a delivery-ready path.

### CORE-1 — Deterministic repository and evidence baseline

Type: delivery/support
Clauses: `CONF-QUALITY-001`, `CONF-DONE-001`

Deliver:

- cross-platform line-ending policy and stable snapshots;
- fixture-addressable, process-isolated carrier/resource counters;
- fix the policy-error BSTR imbalance;
- repair stale host/JIT diagnostic expectations;
- restore strict Clippy and ordinary workspace tests;
- keep governance/meta checks green under the new authority model;
- add one versioned cross-platform gate runner with exact commands, environments, timeouts and evidence paths.

First beads: EOL/snapshot gate; balance isolation; policy-error leak; host expectations; Clippy; canonical runner.

Close: Linux and Windows default-parallel/single-thread baselines agree and all ordinary gates are green.

### CORE-2 — Source, preprocessing and syntax realization

Type: delivery
Clauses: `SRC-*`, `SYN-CST-001`, `DEBUG-MAP-001`

Deliver:

- explicit source encoding/code-page contract at the file boundary;
- UTF-8-safe lexer and no-panic valid-text behavior;
- total expression parsing where required;
- fail-closed conditional directives and expressions;
- case-insensitive project extension/attribute handling;
- original/virtual provenance through class preambles, startup/mainline generation and normalization;
- complete grammar fixture/route matrix;
- decoding, lexer, parser and edit fuzz/property lanes.

First beads: decoding/encoding; lexer Unicode; total expression API; conditional negative matrix; source provenance; grammar fixture tranches.

Close: supported source never panics, malformed compile-time syntax cannot select code, and every diagnostic has original or virtual provenance.

### CORE-3 — Compiler AnalysisResult, types, calls and references

Type: delivery
Clauses: `PROJ-REF-001`, `COMP-*`, `IR-CORE-001`, `LS-FACT-001`

Deliver:

- versioned AnalysisResult with declarations/use sites/types/calls/arguments/accessors/provenance/diagnostics and optional CoreProgram;
- strict/editor fact identity and poison/unknown isolation;
- one callable-signature model across project/library/host/COM/Declare providers;
- declared return/parameter/array/UDT/object/interface type preservation;
- complete ByVal/ByRef/Optional/named/omitted/ParamArray legality/coercion;
- cross-project public data and equivalent source/OxImage export surfaces;
- deterministic ambiguity/visibility/Option Private/diamond behavior;
- source-located stable diagnostics;
- cycle-aware default-member behavior;
- DefDec and a host-collation contract for Option Compare Database.

First beads: AnalysisResult types/fact sink; use-site facts; strict/tolerant parity; typed provider signatures; argument matrix split by scalar/array/UDT/object; public data exports; diagnostic spans; default-member cycles; DefDec/database collation.

Close: every compiler matrix row is decided once, typed, source-provenanced and consumable by Core IR/language services.

### CORE-LIB — Complete VBA library and portable host profile

Type: delivery
Clauses: `LIB-VBA-001`, `HOST-HAL-001`, `RUNTIME-EVAL-001`

Deliver:

- authoritative typed inventory of every public VBA library member/overload;
- exact Optional/named/ParamArray/ByRef compiler signatures;
- shared pure semantics and VM3/JIT routes;
- deterministic locale/calendar/code-page/time/random profiles;
- complete file/settings/environment/interaction host families;
- allowed/denied/error side-effect and Err matrices;
- CCT-033 stateful file I/O completion;
- current Excel/VBA or authoritative spec evidence per observable family.

First beads: inventory/signatures; pure scalar/string/math; date/locale/random; collection/object/array; file I/O; settings/environment/UI; terminal library sweep.

Close: every public member/overload has typed compiler, VM3/JIT, host-policy and oracle/spec evidence.

### CORE-4 — Verified OxIR and OxImage realization

Type: delivery
Clauses: `IR-*`, `IMAGE-*`, `SYS-ART-001`, `DEBUG-MAP-001`

Deliver:

- sealed VerifiedOxProgram/VerifiedOxImage product types;
- bounded decoder and full program/image verifier;
- explicit entry handling and unique link tables;
- complete types/ranks/arity/effect/descriptor/import/export verification;
- digest, schema, target/profile/capability, helper/carrier ABI and provenance;
- source/debug maps and compatible version migration/rejection;
- hostile artifact mutation/fuzz/resource gates;
- removal of legacy Bundle/.oxb product terminology and migration of needed VBA-library metadata.

First beads: schema/compat decision; bounded decoder; verified handles/API closure; ID/CFG/type verifier tranches; descriptor/link verifier tranches; entry/link tables; ABI/provenance maps; hostile artifact fuzz; Bundle metadata migration.

Close: no product path links, executes or compiles raw/unverified artifacts and every package clause has evidence.

### CORE-5 — Runtime, helper ABI and session ownership

Type: delivery
Clauses: `RUNTIME-*`, `HOST-SESSION-001`, `SEC-BOUNDARY-001`

Deliver:

- shared class/interface/record descriptor arenas;
- eliminate per-session Box::leak and leaked image/host ownership;
- explicit unsafe rt-abi contracts behind typed wrappers;
- versioned helper descriptor catalog;
- RAII drain/reentrancy/panic/fault state;
- deterministic internal fault seating;
- shared semantic ownership for error, lifecycle, array, object and call operations;
- backend-neutral verified project-session API;
- repeated compile/load/initialize/invoke/reset/drop stability.

First beads: descriptor arena; session-owned metadata; unsafe API audit; helper catalog; RAII/panic injection; semantic-kernel extraction tranches; shared session facade; lifecycle stress.

Close: runtime/helper/session ownership is sound, versioned and bounded with zero balance drift.

### CORE-6 — VM3 complete reference realization

Type: delivery
Clauses: `VM3-*`, `SYS-DUAL-001`

Deliver:

- explicit VM3 disposition for every verified instruction/terminator;
- implement lifecycle operations or reject them at verifier admission;
- honor image/program entry and initializer/link rules;
- complete error/Erl/source-map behavior;
- preserve heap-frame safe recursion/error 28;
- complete session state, lifecycle and portable host rows;
- resolve Collection selector/string-coercion oracle risks;
- replace VM3-minted-only evidence with VBA-backed current rows.

First beads: generated backend support table; lifecycle ops; entry/link behavior; error/source map; recursion; sessions; oracle-risk cases; golden migration.

Close: VM3 executes the complete verified core vocabulary and is VBA-validated reference evidence.

### CORE-7 — Ideal JIT lowering, calls and semantics

Type: delivery
Clauses: `JIT-CORE-001`, `JIT-PARITY-001`, `RUNTIME-ABI-001`

Deliver:

- sealed verified input/admission;
- inspectable procedure lowering plan for physical storage/calls/faults/cleanup/helpers;
- typed primary entry family and direct typed static calls;
- universal Variant invocation thunk for dynamic boundaries;
- generalized ByRef/copyback and return coercion;
- complete error/Erl/Err.Raise behavior;
- native-stack-safe source recursion;
- versioned generated helper registration;
- explicit target/layout policy;
- modularize admission/planning/codegen/helpers/sessions/interop/maps/tests.

First beads: verified admission; lowering-plan contract/prototype; typed scalar entry; typed ByRef/object/array entries; universal thunk; direct calls; error/Erl; recursion; helper generation; target policy; module extraction.

Close: every platform-neutral verified core row compiles through the ideal calling/helper architecture with VM3/VBA parity.

### CORE-8 — JIT sessions, cache and native continuity

Type: delivery
Clauses: `JIT-CACHE-001`, `JIT-AOT-001`, `HOST-SESSION-001`

Deliver:

- JIT-backed ProjectRuntimeSession equivalent;
- verified source-free `.oxi` load/compile/session path;
- persistent globals, objects, events and Err state;
- deterministic cache key over image/target/ABI/profile/settings;
- bounded code/metadata ownership, invalidation and eviction;
- thread/apartment declaration for portable sessions;
- object/blob/source-map continuity for the Windows native-output workset;
- cold and warm product performance evidence.

First beads: JIT session facade; `.oxi` load; state/reset parity; cache key; bounded cache; concurrency/lifetime; object/blob handoff; performance report.

Close: hosts select VM3 or JIT through equivalent persistent verified sessions and reusable code.

### CORE-9 — Structural parity, safety and Excel/VBA certification

Type: delivery/conformance
Clauses: `CONF-*`, `SEC-BOUNDARY-001`

Deliver:

- versioned full differential observable;
- structural arrays/records/objects/ProcRefs and identity comparison;
- full Err, side-effect, lifecycle/event and balance axes;
- complete fixture manifest tied to contract/matrix rows;
- property/fuzz, sanitizer, Miri-appropriate and repeated-session lanes;
- current-stack Excel compile/runtime oracle for every VBA-observable core/library row;
- Windows x64/64-bit Excel, non-default locale and source-encoding rows;
- captured environment/source/result/modal/cleanup evidence.

First beads: observable schema; structural carriers; side-effect/lifecycle axes; manifest generation; property/fuzz tranches; safety lanes; oracle harness; compiler diagnostics; runtime/library; x64/locale/source encoding.

Close: no required row relies on tag/status equality, historical capture alone or VM3-minted truth.

### CORE-10 — Terminal architecture and profile release

Type: support/conformance
Clauses: `CONF-DONE-001`, `DOC-*`

Deliver:

- reconcile system/subsystem contracts, architecture, code comments, matrices and workset/bead truth;
- remove or deprecate every residual competing architecture statement;
- generate the core profile report from matrices;
- run Linux/Windows/Excel/safety/performance gates;
- perform final code/docs/runnable-path fresh-eyes review;
- leave every uncovered required residual as an open delivery bead before parent closure.

Close: all delivery epics are closed and the `PROFILE-CORE-001` claim is demonstrably true.

## 7. Dependency graph

| epic | hard prerequisites | closure dependencies |
|---|---|---|
| CORE-0 | accepted workset | none |
| CORE-1 | CORE-0 | none |
| CORE-2 | CORE-1 | none |
| CORE-3 | CORE-2 | gates LS compiler facts |
| CORE-LIB | CORE-2 plus stable CORE-3 signatures | CORE-5/6/7/9 |
| CORE-4 | CORE-1 | final metadata consumes CORE-3 |
| CORE-5 | CORE-4 | none |
| CORE-6 | CORE-3, CORE-4, CORE-5 | CORE-LIB |
| CORE-7 | CORE-3, CORE-4, CORE-5 | CORE-LIB |
| CORE-8 | CORE-4, CORE-5, CORE-7 | none |
| CORE-9 | CORE-1 scaffolding | closes after CORE-2/3/LIB/4/5/6/7/8 |
| CORE-10 | every delivery epic | Windows x64 JIT prerequisite for final target summary |

Cross-workset producer edges:

- language-service compiler facts: CORE-2 provenance plus CORE-3 AnalysisResult/diagnostics;
- language-service source/OxImage/library/Declare references: CORE-3 public surfaces/Declare legality, CORE-LIB signatures, CORE-4 verified loading;
- Windows interop: CORE-4 verified package, CORE-5 helper/carrier/session substrate, CORE-7 JIT plan/calls, CORE-8 sessions/cache.

## 8. Checks and evidence

Per bead: touched-crate format/strict Clippy/tests; relevant compiler/VM3/JIT/oracle row or explicit N/A; matrix update; fresh-eyes review.

Merge gate after CORE-1 canonicalizes it:

- workspace format, strict Clippy and tests;
- default-parallel and single-thread differentials;
- governance/meta/truth reconciliation;
- matrix/schema validation.

Release gate additionally includes declared Linux and Windows x64 targets, current 64-bit Excel/VBA, safety/fuzz, repeated sessions, cold/warm performance and source/debug-map checks.

## 9. Terminal condition

This workset is complete only when:

1. every required epic and delivery bead is closed;
2. compiler analysis is complete, typed, source-provenanced and shared with editor consumers;
3. every product consumer accepts sealed verified artifacts;
4. the runtime/helper/session substrate is sound, versioned and bounded;
5. the complete VBA library is evidenced member by member;
6. VM3 implements and VBA-validates the complete verified core vocabulary;
7. the JIT realizes typed lowering/calls, dynamic thunk, full semantics, sessions and cache without fallback;
8. structural VM3/JIT and current Excel/VBA evidence are green;
9. all ordinary, safety and lifecycle gates are green;
10. contracts, architecture, code, matrices, worksets and generated summaries agree.

Any required `implemented-subset`, `planned` or `in-progress` row keeps the profile in progress unless it is genuinely outside this profile through an approved scope split.

## 10. Bead-preparation handoff

Create the workset root, CORE-0 through CORE-10 plus CORE-LIB epics, one rollout bead under each, and the first bead candidates above. Every executable bead names type, parent, contract clauses, matrix rows, direct dependencies, touched truth surfaces, exact acceptance command/evidence and residual behavior. Capability epics cannot close on support beads alone.
