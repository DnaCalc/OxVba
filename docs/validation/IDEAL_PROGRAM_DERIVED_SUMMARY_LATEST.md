# Ideal Program Derived Validation Summary

Program: `ideal-2026-07` / `bd-59co`
Manifest: `docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json`
Ownership: `docs/validation/IDEAL_MATRIX_OWNERSHIP_V1.csv`

This file is generated from the manifest-owned canonical matrices. It is a projection, not an independent capability claim.

## Profile totals

| Profile | Matrices | Rows | Planned | In progress | Implemented subset | Implemented full | Verified | Archived |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| core | 5 | 59 | 58 | 0 | 0 | 0 | 1 | 0 |
| windows-x64 | 6 | 57 | 57 | 0 | 0 | 0 | 0 | 0 |
| ide | 4 | 77 | 77 | 0 | 0 | 0 | 0 | 0 |

## Matrix totals

| Matrix | Profile | Role | Owner epic | Rows | Verified | Open | Trace relationships |
|---|---|---|---|---:|---:|---:|---:|
| CORE-READINESS | core | primary | bd-59co.2.1 | 24 | 1 | 23 | 78 |
| VBA-LIBRARY | core | primary | bd-59co.2.5 | 7 | 0 | 7 | 7 |
| OXIR-BACKENDS | core | primary | bd-59co.2.6 | 12 | 0 | 12 | 12 |
| OXIMAGE-CONTRACT | core | primary | bd-59co.2.6 | 8 | 0 | 8 | 8 |
| EXCEL-ORACLE | core | evidence | bd-59co.2.11 | 8 | 0 | 8 | 10 |
| WIN-COM-CLIENT | windows-x64 | primary | bd-59co.3.4 | 9 | 0 | 9 | 27 |
| WIN-COM-EVENTS | windows-x64 | primary | bd-59co.3.6 | 7 | 0 | 7 | 25 |
| WIN-COM-SERVER | windows-x64 | primary | bd-59co.3.7 | 7 | 0 | 7 | 20 |
| WIN-NATIVE-IMPORT | windows-x64 | primary | bd-59co.3.10 | 8 | 0 | 8 | 27 |
| WIN-NATIVE-EXPORT | windows-x64 | primary | bd-59co.3.13 | 8 | 0 | 8 | 21 |
| WIN-ABI-CARRIER | windows-x64 | quality | bd-59co.3.2 | 18 | 0 | 18 | 73 |
| LS-BASELINE | ide | primary | bd-59co.4.1 | 29 | 0 | 29 | 31 |
| LS-REFERENCES | ide | primary | bd-59co.4.7 | 10 | 0 | 10 | 12 |
| LSP-METHODS | ide | projection | bd-59co.4.11 | 30 | 0 | 30 | 33 |
| LS-PERFORMANCE | ide | quality | bd-59co.4.10 | 8 | 0 | 8 | 9 |

## Remaining accepted scope

| Row | Matrix | Capability | Subset | Truth state | Residual disposition | Residual owner |
|---|---|---|---|---|---|---|
| CORE-SOURCE-IDENTITY-PROVENANCE | CORE-READINESS | source identity and provenance | original virtual generated and normalized spans retain stable identity | planned | remaining-accepted-scope | bd-59co.2.3.1 |
| CORE-SOURCE-UTF8-SPANS | CORE-READINESS | UTF-8 compiler span contract | Unicode and CRLF byte spans remain exact and convertible | planned | remaining-accepted-scope | bd-59co.2.3.1 |
| CORE-SYNTAX-CST | CORE-READINESS | lossless total CST | valid and incomplete VBA text parses without panic | planned | remaining-accepted-scope | bd-59co.2.3.1 |
| CORE-ANALYSIS-FACTS | CORE-READINESS | versioned AnalysisResultV1 identities syntax and scopes | immutable syntax declarations uses types calls arguments diagnostics provenance and optional program | planned | remaining-accepted-scope | bd-59co.2.4.1 |
| CORE-ANALYSIS-STRICT-EDITOR | CORE-READINESS | strict and editor analysis equivalence | valid source produces identical identities types calls and diagnostics in both modes | planned | remaining-accepted-scope | bd-59co.2.4.1 |
| CORE-TYPED-BINDING | CORE-READINESS | typed calls arguments references and providers | ByVal ByRef Optional named omitted ParamArray returns arrays UDTs and objects | planned | remaining-accepted-scope | bd-59co.2.4.1 |
| CORE-COMP-NUMERIC-MODE | CORE-READINESS | VBA-compatible NumericMode selection | provable fixed numeric lanes and overflow coercion | planned | remaining-accepted-scope | bd-59co.2.4.3 |
| CORE-BASELINE-UNSAFE-CLIPPY | CORE-READINESS | strict clean-build unsafe audit | SafeArray VBA record and current HAL strict-warning sites | planned | remaining-accepted-scope | bd-59co.2.2.12 |
| CORE-BASELINE-EOL-SNAPSHOT | CORE-READINESS | cross-platform line ending and snapshot determinism | source and golden artifacts remain semantically and byte stable across LF and CRLF checkouts | planned | remaining-accepted-scope | bd-59co.2.2.12 |
| CORE-BASELINE-BALANCE-LIFECYCLE | CORE-READINESS | fixture-isolated carrier balance and policy-error cleanup | named subprocess fixtures report exact BSTR object SAFEARRAY record and related carrier deltas | planned | remaining-accepted-scope | bd-59co.2.2.12 |
| CORE-BASELINE-HOST-JIT-DIAGNOSTICS | CORE-READINESS | current host behavior and stable JIT diagnostics | supported Collection execution and declined native shapes use structured contract fields | planned | remaining-accepted-scope | bd-59co.2.2.12 |
| CORE-BASELINE-CROSS-PLATFORM-GATES | CORE-READINESS | versioned Windows and Linux ordinary gate baseline | pinned environments canonical runner strict checks parallel and serial differentials | planned | remaining-accepted-scope | bd-59co.2.2.12 |
| CORE-RUNTIME-HELPER-SESSION | CORE-READINESS | owned runtime helper ABI and project sessions | versioned descriptors typed wrappers lifecycle error and reset | planned | remaining-accepted-scope | bd-59co.2.7.1 |
| CORE-VM3-REFERENCE | CORE-READINESS | complete VM3 reference interpreter | all admitted verified Core and OxIR vocabulary plus entries links errors recursion and sessions | planned | remaining-accepted-scope | bd-59co.2.8.1 |
| CORE-JIT-LOWERING | CORE-READINESS | ideal JIT lowering and call architecture | inspectable lowering plan typed entries direct calls universal thunk ByRef errors and recursion | planned | remaining-accepted-scope | bd-59co.2.9.1 |
| CORE-JIT-CACHE-OBJECT | CORE-READINESS | JIT session cache and native continuity | source-free verified image sessions deterministic key bounded cache persistent objects and object blobs | planned | remaining-accepted-scope | bd-59co.2.10.1 |
| CORE-DIFF-SEMANTICS-FUZZ | CORE-READINESS | structural VM3 JIT differential fuzzing | scalar Variant control-flow call and error hazards | planned | remaining-accepted-scope | bd-59co.2.11.3 |
| CORE-DIFF-STRUCTURAL | CORE-READINESS | full differential observable | arrays records objects ProcRefs identities errors effects lifecycle and balance | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| CORE-PORTABLE-CERT | CORE-READINESS | portable Core implementation and oracle certification | compiler library OxIR OxImage VM3 JIT sessions cache differential locale lifecycle and current VBA authority | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| CORE-SAFETY-LIFECYCLE | CORE-READINESS | safety lifecycle and repeated-session proof | hostile artifacts panic and fault injection sanitizer Miri-appropriate and repeated sessions | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| CORE-PERFORMANCE-COLD-WARM | CORE-READINESS | cold and warm product performance | compile load invoke cache hit reset and repeated session budgets | planned | remaining-accepted-scope | bd-59co.2.10.1 |
| CORE-PORTABLE-PROFILE | CORE-READINESS | portable host profile | compiler library VM3 JIT and host services operate without Windows-only dependencies | planned | remaining-accepted-scope | bd-59co.2.12.1 |
| CORE-TERMINAL-PROFILE | CORE-READINESS | Core profile terminal certification | all compiler library OxIR OxImage VM3 JIT session cache differential oracle safety and performance rows | planned | remaining-accepted-scope | bd-59co.2.12.1 |
| LIB-INVENTORY-PUBLIC | VBA-LIBRARY | complete public VBA library inventory | every module member overload constant enum and object surface | planned | remaining-accepted-scope | bd-59co.2.5.1 |
| LIB-COMPILER-BINDING | VBA-LIBRARY | typed compiler binding for library calls | Optional named ParamArray ByRef overload legality and coercion | planned | remaining-accepted-scope | bd-59co.2.5.1 |
| LIB-PURE-SEMANTICS | VBA-LIBRARY | pure scalar string math date and conversion semantics | pure deterministic members across Variant and typed carriers | planned | remaining-accepted-scope | bd-59co.2.5.1 |
| LIB-HOST-SERVICES | VBA-LIBRARY | host-sensitive library services | file settings environment interaction and time randomness | planned | remaining-accepted-scope | bd-59co.2.5.1 |
| LIB-LOCALE-CALENDAR-CODEPAGE | VBA-LIBRARY | locale calendar and code-page profiles | date string formatting comparison and byte conversion | planned | remaining-accepted-scope | bd-59co.2.5.1 |
| LIB-ERROR-SIDE-EFFECTS | VBA-LIBRARY | library Err and side-effect parity | success failure Resume Next and cleanup behavior across member families | planned | remaining-accepted-scope | bd-59co.2.5.1 |
| LIB-DUAL-RUNTIME | VBA-LIBRARY | VM3 and JIT library route parity | every public overload uses shared semantics or an explicitly identical host route | planned | remaining-accepted-scope | bd-59co.2.5.1 |
| OXIR-SCALAR | OXIR-BACKENDS | scalar and Variant operations | constants assignments coercions comparisons and arithmetic | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXIR-CONTROL | OXIR-BACKENDS | control flow and procedure boundaries | branches loops gosub exits statement boundaries and returns | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXIR-CALL-BYREF | OXIR-BACKENDS | calls arguments and ByRef writeback | typed static calls dynamic thunk Optional named ParamArray copyback and returns | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXIR-ERROR | OXIR-BACKENDS | error handling and source seating | On Error Resume Resume Next Erl Err.Raise and internal faults | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXIR-STRING-OBJECT | OXIR-BACKENDS | string and object operations | BSTR concat compare default member identity dispatch and lifecycle | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXIR-ARRAY | OXIR-BACKENDS | array operations | fixed dynamic Preserve Erase bounds indexing assignment and ByRef arrays | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXIR-RECORD | OXIR-BACKENDS | record and UDT operations | nominal fields fixed strings nested arrays assignment and ABI descriptors | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXIR-HOST-LIBRARY | OXIR-BACKENDS | host and VBA library descriptors | versioned imports capabilities signatures and backend-neutral invocation | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXIR-RUNTIME-HELPERS | OXIR-BACKENDS | shared runtime semantics helper catalog and session ownership | every admitted operation names versioned helpers ownership effects fault seating and backend-neutral session behavior | planned | remaining-accepted-scope | bd-59co.2.7.1 |
| OXIR-VM3-DISPOSITION | OXIR-BACKENDS | complete VM3 disposition and execution | every verified portable instruction terminator entry link initializer fault lifecycle and session operation | planned | remaining-accepted-scope | bd-59co.2.8.1 |
| OXIR-JIT-DISPOSITION | OXIR-BACKENDS | complete ideal JIT disposition and lowering | every verified portable instruction terminator call ABI ByRef fault cleanup recursion and helper route | planned | remaining-accepted-scope | bd-59co.2.9.1 |
| OXIR-VERIFIER-REJECT | OXIR-BACKENDS | fail-closed verifier rejection | unknown malformed out-of-range type effect descriptor and capability cases | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXI-BOUNDED-DECODE | OXIMAGE-CONTRACT | bounded OxImage decoding | length count nesting allocation and version limits | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXI-SEALED-VERIFY | OXIMAGE-CONTRACT | sealed verified handles | VerifiedOxProgram and VerifiedOxImage are owning unforgeable product inputs | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXI-IDENTITY-ABI | OXIMAGE-CONTRACT | digest schema target profile capability and ABI identity | deterministic artifact identity includes helper carrier and target contracts | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXI-PROVENANCE-DEBUG | OXIMAGE-CONTRACT | source provenance and debug maps | original virtual generated spans statement maps and procedure identities | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXI-EXPORT-REFERENCES | OXIMAGE-CONTRACT | exports imports and reference surfaces | procedures properties public data types constants events and provider identities | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXI-VERIFIED-CONSUMERS | OXIMAGE-CONTRACT | verified-only product consumers | VM3 JIT host build cache and language service accept verified handles only | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXI-DISTRIBUTABLE-PACKAGE | OXIMAGE-CONTRACT | verified distributable OxImage package | sealed image identity provenance exports target and ABI manifest with compiler-free loading | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| OXI-CORRUPTION-RESOURCE | OXIMAGE-CONTRACT | corruption mutation fuzz and resource safety | truncation bit flips duplicated IDs bad CFG types ranks descriptors links and zip-bomb shapes | planned | remaining-accepted-scope | bd-59co.2.6.1 |
| XOR-COMPILER-FACTS | EXCEL-ORACLE | compiler syntax binding and diagnostics oracle | valid invalid and incomplete-adjacent compile cases | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| XOR-LIBRARY | EXCEL-ORACLE | VBA library observable oracle | pure host locale and error member families | planned | remaining-accepted-scope | bd-59co.2.11.5 |
| XOR-VM3 | EXCEL-ORACLE | VM3 current-stack oracle | complete admitted core vocabulary and sessions | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| XOR-JIT | EXCEL-ORACLE | JIT current-stack oracle | typed direct dynamic ByRef error recursion and session cases | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| XOR-ERRORS | EXCEL-ORACLE | full VBA error oracle | compile and runtime errors Resume Next handlers Err.Raise and Erl | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| XOR-STATE-LIFECYCLE | EXCEL-ORACLE | state and lifecycle oracle | globals static objects events Err reset reload termination and repeated sessions | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| XOR-NONDEFAULT-LOCALE | EXCEL-ORACLE | non-default locale and source-encoding oracle | LCID calendar decimal date case codepage and Unicode CRLF | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| XOR-PORTABLE-ORACLE | EXCEL-ORACLE | portable Core Excel/VBA certification | compiler library OxIR OxImage VM3 JIT sessions cache differential locale and lifecycle; excludes Windows COM native and packaging | planned | remaining-accepted-scope | bd-59co.2.11.4 |
| WCC-PLAN-LATE | WIN-COM-CLIENT | late-bound COM client | scalar activation invocation and property access | planned | remaining-accepted-scope | bd-59co.3.4 |
| WCC-LATE-ARGS | WIN-COM-CLIENT | late-bound COM client | named omitted Optional ParamArray put putref and ByRef arguments | planned | remaining-accepted-scope | bd-59co.3.4 |
| WCC-LATE-STRUCTURAL | WIN-COM-CLIENT | late-bound COM client | object identity arrays records chaining and enumeration | planned | remaining-accepted-scope | bd-59co.3.4 |
| WCC-LATE-OUTPROC-ERROR | WIN-COM-CLIENT | late-bound COM client | GetObject out-of-proc locale error and reentry behavior | planned | remaining-accepted-scope | bd-59co.3.4 |
| WCC-PLAN-EARLY | WIN-COM-CLIENT | early-bound COM client | scalar typed native-vtable invocation | planned | remaining-accepted-scope | bd-59co.3.5 |
| WCC-EARLY-COMPLEX | WIN-COM-CLIENT | early-bound COM client | ByRef arrays records interfaces inheritance and QI identity | planned | remaining-accepted-scope | bd-59co.3.5 |
| WCC-EARLY-CUSTOM | WIN-COM-CLIENT | early-bound COM client | custom interface signatures HRESULT out retval and inheritance | planned | remaining-accepted-scope | bd-59co.3.5 |
| WCC-EARLY-OUTPROC | WIN-COM-CLIENT | early-bound COM client | out-of-proc typed proxy marshalling and apartment behavior | planned | remaining-accepted-scope | bd-59co.3.5 |
| WCC-EXCEL-AUTHORITY | WIN-COM-CLIENT | 64-bit Excel/VBA COM client compatibility authority | controlled late-dispatch and early-vtable compile and runtime observations including results full Err effects transport lifecycle and balance | planned | remaining-accepted-scope | bd-59co.3.15.32 |
| WCE-PLAN-INCOMING | WIN-COM-EVENTS | incoming COM events | synchronous cancellable scalar ByRef event | planned | remaining-accepted-scope | bd-59co.3.6.4 |
| WCE-INCOMING-COMPLEX | WIN-COM-EVENTS | incoming COM events | object interface array and record event arguments | planned | remaining-accepted-scope | bd-59co.3.6.4 |
| WCE-INCOMING-CUSTOM | WIN-COM-EVENTS | incoming COM events | custom native-vtable source-interface sink | planned | remaining-accepted-scope | bd-59co.3.6.4 |
| WCE-INCOMING-APARTMENT | WIN-COM-EVENTS | incoming COM events | cross-apartment and out-of-proc synchronous reentry | planned | remaining-accepted-scope | bd-59co.3.6.4 |
| WCE-INCOMING-LIFECYCLE | WIN-COM-EVENTS | incoming COM events | fan-out replacement unsubscribe termination and handler errors | planned | remaining-accepted-scope | bd-59co.3.6.4 |
| WCE-PLAN-OUTGOING | WIN-COM-EVENTS | outgoing COM events | served source-interface scalar event fan-out | planned | remaining-accepted-scope | bd-59co.3.9 |
| WCE-OUTGOING-COMPLEX | WIN-COM-EVENTS | outgoing COM events | object array ByRef errors lifecycle and Excel consumption | planned | remaining-accepted-scope | bd-59co.3.9 |
| WCS-LATE-INPROC | WIN-COM-SERVER | late-bound COM serving | same verified serving plan with late dispatch and VM3/JIT plan-digest equality | planned | remaining-accepted-scope | bd-59co.3.7 |
| WCS-LATE-LOCALSERVER | WIN-COM-SERVER | late-bound COM serving | out-of-process LocalServer activation apartment and shutdown | planned | remaining-accepted-scope | bd-59co.3.7 |
| WCS-LATE-COMPLEX | WIN-COM-SERVER | late-bound COM serving | Optional named ParamArray ByRef arrays records interfaces errors and lifecycle | planned | remaining-accepted-scope | bd-59co.3.7 |
| WCS-DUAL-INPROC | WIN-COM-SERVER | early and dual COM serving | same verified serving plan with early/dual vtable VM3/JIT plan-digest equality and no dispatch fallback | planned | remaining-accepted-scope | bd-59co.3.8 |
| WCS-IMPLEMENTS-CUSTOM | WIN-COM-SERVER | early and dual COM serving | imported Implements and custom interface vtables | planned | remaining-accepted-scope | bd-59co.3.8 |
| WCS-EARLY-OUTPROC | WIN-COM-SERVER | early and dual COM serving | out-of-process early proxy marshalling arrays records interfaces and ByRef | planned | remaining-accepted-scope | bd-59co.3.8 |
| WCS-SERVER-SAFETY | WIN-COM-SERVER | COM serving lifecycle and deployment | class factories locks registration unload errors and hostile clients | planned | remaining-accepted-scope | bd-59co.3.7 |
| WNI-PLAN-DECLARE | WIN-NATIVE-IMPORT | VBA7 Declare import | scalar named-symbol x64 call | planned | remaining-accepted-scope | bd-59co.3.10 |
| WNI-DECLARE-STRINGS | WIN-NATIVE-IMPORT | VBA7 Declare import | ANSI Wide BSTR and mutable string-buffer calls | planned | remaining-accepted-scope | bd-59co.3.10 |
| WNI-DECLARE-STRUCTURAL | WIN-NATIVE-IMPORT | VBA7 Declare import | arrays UDT As Any aliases and structural ByRef calls | planned | remaining-accepted-scope | bd-59co.3.10 |
| WNI-DECLARE-LOADER-ERROR | WIN-NATIVE-IMPORT | VBA7 Declare import | ordinal resolution missing library export policy and LastDllError | planned | remaining-accepted-scope | bd-59co.3.10 |
| WNI-POINTER-HELPERS | WIN-NATIVE-IMPORT | pointer helpers and AddressOf | VarPtr StrPtr ObjPtr addressability and LongPtr results | planned | remaining-accepted-scope | bd-59co.3.11 |
| WNI-CALLBACK-SYNC | WIN-NATIVE-IMPORT | AddressOf callbacks | typed synchronous scalar and ByRef callback | planned | remaining-accepted-scope | bd-59co.3.11 |
| WNI-PLAN-CALLBACK | WIN-NATIVE-IMPORT | AddressOf callbacks | externally owned retained callback lifetime and release | planned | remaining-accepted-scope | bd-59co.3.11.4 |
| WNI-CALLBACK-NESTED | WIN-NATIVE-IMPORT | AddressOf callbacks | nested native COM VBA reentry errors threads and disposed-session safety | planned | remaining-accepted-scope | bd-59co.3.11.4 |
| WNE-WRAPPER-EXE | WIN-NATIVE-EXPORT | JIT-backed wrapped outputs | standalone WrapperExe with embedded or deployed verified OxImage | planned | remaining-accepted-scope | bd-59co.3.12 |
| WNE-WRAPPER-LIBRARY | WIN-NATIVE-EXPORT | JIT-backed wrapped outputs | WrapperLibrary external ABI and repeated session lifecycle | planned | remaining-accepted-scope | bd-59co.3.12 |
| WNE-PLAN-WRAPPED | WIN-NATIVE-EXPORT | JIT-backed wrapped outputs | JIT WrappedComServer with verified package and Excel clients | planned | remaining-accepted-scope | bd-59co.3.12 |
| WNE-PLAN-NATIVE | WIN-NATIVE-EXPORT | genuine native DLL output | scalar Cranelift object and x64 native DLL export | planned | remaining-accepted-scope | bd-59co.3.13 |
| WNE-NATIVE-EXE | WIN-NATIVE-EXPORT | genuine native EXE output | program-specific x64 native executable | planned | remaining-accepted-scope | bd-59co.3.13 |
| WNE-NATIVE-ABI-BREADTH | WIN-NATIVE-EXPORT | genuine native DLL output | external signature ownership errors concurrency names and ordinals | planned | remaining-accepted-scope | bd-59co.3.13 |
| WNE-NATIVE-REPRO-DEPLOY | WIN-NATIVE-EXPORT | genuine native DLL and EXE outputs | PE COFF relocations imports ASLR reproducibility debug maps and clean deployment | planned | remaining-accepted-scope | bd-59co.3.13 |
| WNE-PROFILE-TOOL-TERMINAL | WIN-NATIVE-EXPORT | VB-universe Windows tooling profile terminal | verified distributable OxImage wrapped outputs COM-server artifacts and genuine native outputs remain distinct claim classes | planned | remaining-accepted-scope | bd-59co.3.16.1 |
| WAC-BSTR-LAYOUT | WIN-ABI-CARRIER | exact x64 carriers | BSTR layout ownership copy move ByRef and cleanup | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-VARIANT-LAYOUT | WIN-ABI-CARRIER | exact x64 carriers | VARIANT subtype union ownership ByRef copy and clear | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-SAFEARRAY-LAYOUT | WIN-ABI-CARRIER | exact x64 carriers | SAFEARRAY ranks bounds elements ownership redim and ByRef | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-IUNKNOWN-IDENTITY | WIN-ABI-CARRIER | exact x64 carriers | IUnknown identity QI AddRef Release and object carrier mapping | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-NUMERIC-LONGPTR | WIN-ABI-CARRIER | exact x64 carriers | numeric primitives LongLong LongPtr Currency Date Decimal and Boolean layouts | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-INTERFACE-ARRAY | WIN-ABI-CARRIER | exact x64 carriers | nominal interface and object arrays with VT_DISPATCH VT_UNKNOWN | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-VT-RECORD | WIN-ABI-CARRIER | exact x64 carriers | nominal VT_RECORD descriptors scalar arrays copy and clear | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-CARRIER-EXCEL-ROUNDTRIP | WIN-ABI-CARRIER | exact x64 carriers | 64-bit Excel cross-boundary carrier roundtrip and writeback | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-SAFETY-MUTATION | WIN-ABI-CARRIER | native-boundary carrier safety | malformed descriptors fault injection repeated lifecycle and balance | planned | remaining-accepted-scope | bd-59co.3.14.2 |
| WAC-TARGET-DEV-ENV | WIN-ABI-CARRIER | x64 target and development oracle environment | explicit x64-only scope plus current development-oracle host role | planned | remaining-accepted-scope | bd-59co.3.1.2 |
| WAC-TYPELIB-METADATA | WIN-ABI-CARRIER | raw typelib resolution and stable metadata identities | registered/file typelib selection; GUID version LCID platform and reference order; stable library type member event inheritance default source coclass and broken-reference facts; resolver digest and compiler package language-service handoff | planned | remaining-accepted-scope | bd-59co.3.2 |
| WAC-VERIFIED-INTEROP-PLAN | WIN-ABI-CARRIER | shared verified x64 interop plan | one plan identity unchanged VM3/JIT adapters common Core session/cache and no private Windows helper catalog mutable state or cache | planned | remaining-accepted-scope | bd-59co.3.3.1 |
| WAC-WINDOWS-DESCRIPTORS | WIN-ABI-CARRIER | Windows x64 interop descriptor vocabulary | verified OxImage Windows descriptors only; raw metadata or unverified decoded artifacts cannot elaborate a plan | planned | remaining-accepted-scope | bd-59co.3.3.1 |
| WAC-CLEAN-CERT-ENV | WIN-ABI-CARRIER | clean pinned release certification environment | pinned Windows x64 actual Excel64 clean snapshot locale fixtures and hashes | planned | remaining-accepted-scope | bd-59co.3.15.3 |
| WAC-RELEASE-CERT | WIN-ABI-CARRIER | Windows native client and Excel certification | all mandatory COM native carrier lifecycle locale deployment and cleanup rows | planned | remaining-accepted-scope | bd-59co.3.15.2 |
| WAC-EXCEL-COM-CERT | WIN-ABI-CARRIER | Excel64 COM client server and event certification | late dispatch early vtable serving incoming and outgoing events cancellable ByRef reentry apartments and cleanup | planned | remaining-accepted-scope | bd-59co.3.15.32 |
| WAC-EXCEL-NATIVE-CERT | WIN-ABI-CARRIER | Excel64 Declare callback and native-output certification | PtrSafe imports pointers callbacks wrapped exports genuine DLL and EXE outputs deployment and cleanup | planned | remaining-accepted-scope | bd-59co.3.15.33 |
| WAC-PROFILE-TERMINAL | WIN-ABI-CARRIER | Windows x64 profile terminal | all required delivery epics matrices environments safety deployment and documentation | planned | remaining-accepted-scope | bd-59co.3.16.1 |
| LSB-AUTHORITY-ROLLOUT | LS-BASELINE | language-service authority and rollout | historical deleted stack separated from current clean-stack owners and matrices | planned | remaining-accepted-scope | bd-59co.4.1.1 |
| LSB-WORKSPACE-SNAPSHOT | LS-BASELINE | workspace project document and snapshot lifecycle | real projects overlays immutable snapshots and reloads | planned | remaining-accepted-scope | bd-59co.4.3.1 |
| LSB-DIAGNOSTICS | LS-BASELINE | diagnostics | syntax symbol bind project reference artifact and generated-source diagnostics | planned | remaining-accepted-scope | bd-59co.4.4.1 |
| LSB-DOCUMENT-SYMBOLS | LS-BASELINE | document symbols | module members procedures properties types and fields | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-WORKSPACE-SYMBOLS | LS-BASELINE | workspace symbols | cross-project declarations and virtual metadata | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-SEMANTIC-TOKENS | LS-BASELINE | semantic classification and tokens | compiler-bound token kinds modifiers and active regions | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-HOVER | LS-BASELINE | hover | typed symbol signature documentation provenance and value category | planned | remaining-accepted-scope | bd-59co.4.6.1 |
| LSB-COMPLETION | LS-BASELINE | completion and resolve | scope keywords typed members visibility references defaults With dot and bang | planned | remaining-accepted-scope | bd-59co.4.6.1 |
| LSB-SIGNATURE | LS-BASELINE | signature help | procedure property library Declare COM Optional named ParamArray active parameter | planned | remaining-accepted-scope | bd-59co.4.6.1 |
| LSB-DEFINITION | LS-BASELINE | definition | bound declaration and virtual metadata target | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-TYPE-DEFINITION | LS-BASELINE | type definition | declared nominal array object interface and provider type targets | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-IMPLEMENTATION | LS-BASELINE | implementation | Implements interface member and project implementation targets | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-REFERENCES | LS-BASELINE | references | classified source-owned read write call type and event uses | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-HIGHLIGHTS | LS-BASELINE | document highlights | read write and textual bound occurrences in one document | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-PREPARE-RENAME | LS-BASELINE | prepare rename | source-owned case-insensitive bound symbol eligibility | planned | remaining-accepted-scope | bd-59co.4.8.1 |
| LSB-RENAME-ACTIONS | LS-BASELINE | safe rename and bounded code actions | versioned multi-document edits collisions qualification and diagnostic actions | planned | remaining-accepted-scope | bd-59co.4.8.1 |
| LSB-FOLDING | LS-BASELINE | folding ranges | procedures blocks types regions continuations and multiline constructs | planned | remaining-accepted-scope | bd-59co.4.9.1 |
| LSB-SELECTION | LS-BASELINE | selection ranges | token expression statement block procedure and module hierarchy | planned | remaining-accepted-scope | bd-59co.4.9.1 |
| LSB-VIRTUAL-SOURCE-GENERATED | LS-BASELINE | read-only generated-source virtual documents | generated class preambles startup mainline normalization and source-provenance definitions | planned | remaining-accepted-scope | bd-59co.4.5.1 |
| LSB-VIRTUAL-OXIMAGE | LS-BASELINE | read-only OxImage metadata documents | sealed exports public types procedures properties events documentation and provenance | planned | remaining-accepted-scope | bd-59co.4.7.3 |
| LSB-VIRTUAL-LIBRARY | LS-BASELINE | read-only VBA library metadata documents | complete modules members overloads constants enums objects documentation and signatures | planned | remaining-accepted-scope | bd-59co.4.7.4 |
| LSB-VIRTUAL-HOST | LS-BASELINE | read-only host-provider metadata documents | versioned root objects types members signatures capabilities documentation and provenance | planned | remaining-accepted-scope | bd-59co.4.7.5 |
| LSB-VIRTUAL-COM | LS-BASELINE | read-only COM metadata documents | typelib libraries coclasses interfaces members enums records events signatures and provenance | planned | remaining-accepted-scope | bd-59co.4.7.6 |
| LSB-DIRECT-API | LS-BASELINE | stable direct compiler-fact query API | immutable versioned fact snapshots handles query requests results diagnostics and edits | planned | remaining-accepted-scope | bd-59co.4.2.1 |
| LSB-DIRECT-SESSION | LS-BASELINE | stable direct workspace and project session API | versioned workspace project document provider snapshot query edit reload reset and dispose DTOs | planned | remaining-accepted-scope | bd-59co.4.3.1 |
| LSB-CANCEL-STALE | LS-BASELINE | cancellation stale suppression and invalidation | obsolete analysis queries providers and publishes | planned | remaining-accepted-scope | bd-59co.4.10.2 |
| LSB-UNICODE-POSITIONS | LS-BASELINE | Unicode CRLF and position provenance | UTF-8 compiler bytes convert explicitly without splitting Unicode scalars | planned | remaining-accepted-scope | bd-59co.4.2.1 |
| LSB-HOST-EDITOR-SMOKE | LS-BASELINE | embedded host and editor user path | open diagnostics completion hover navigation references rename tokens reload restart and close | planned | remaining-accepted-scope | bd-59co.4.12.1 |
| LSB-PROFILE-TERMINAL | LS-BASELINE | IDE foundation profile terminal | all direct reference projection host editor robustness and performance rows | planned | remaining-accepted-scope | bd-59co.4.14.1 |
| LSR-SOURCE | LS-REFERENCES | active and referenced source projects | project declarations public data types procedures properties and use sites | planned | remaining-accepted-scope | bd-59co.4.7.2 |
| LSR-OXIMAGE | LS-REFERENCES | verified OxImage references | sealed exports public data types procedures properties events and provenance | planned | remaining-accepted-scope | bd-59co.4.7.3 |
| LSR-LIBRARY | LS-REFERENCES | complete typed VBA library references | all modules members overloads constants enums objects and documentation | planned | remaining-accepted-scope | bd-59co.4.7.4 |
| LSR-HOST | LS-REFERENCES | versioned host providers | root objects types members signatures capabilities and documentation | planned | remaining-accepted-scope | bd-59co.4.7.5 |
| LSR-COM | LS-REFERENCES | direct authoritative COM typelib reference queries | registered and file libraries coclasses interfaces members enums records events exposed through direct semantic queries | planned | remaining-accepted-scope | bd-59co.4.7.6 |
| LSR-COM-CERT | LS-REFERENCES | Windows x64 COM reference certification | registered and file typelibs revision order virtual definitions early binding known late projections x64 contexts broken references and Excel metadata | planned | remaining-accepted-scope | bd-59co.4.13.2 |
| LSR-DECLARE | LS-REFERENCES | source Declare declarations | PtrSafe alias exact signature pointer carriers and compile legality | planned | remaining-accepted-scope | bd-59co.4.7.7 |
| LSR-GENERATED | LS-REFERENCES | generated and normalized source provenance | class preambles startup mainline and virtual normalized maps | planned | remaining-accepted-scope | bd-59co.4.7.8 |
| LSR-COLLISION | LS-REFERENCES | cross-provider collision and precedence | source OxImage library host COM Declare and generated name collisions | planned | remaining-accepted-scope | bd-59co.4.7.9 |
| LSR-COM-VBA-AUTHORITY | LS-REFERENCES | Excel/VBA compatibility authority for COM metadata references | reproducible 64-bit Excel public metadata compile diagnostics reference precedence signatures events records enums and broken-reference observations | planned | remaining-accepted-scope | bd-59co.4.13.2 |
| LSP-INITIALIZE | LSP-METHODS | LSP initialize and capability negotiation | root precedence position encoding and green-only capabilities | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-TEXT-SYNC | LSP-METHODS | LSP document lifecycle | didOpen didChange didSave didClose and versioned overlays | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-DIAGNOSTIC | LSP-METHODS | LSP diagnostics | textDocument diagnostic plus declared pull or push policy | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-DOCUMENT-SYMBOL | LSP-METHODS | LSP document symbols | hierarchical document symbols | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-WORKSPACE-SYMBOL | LSP-METHODS | LSP workspace symbols | workspace query and resolve if advertised | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-SEMANTIC-FULL | LSP-METHODS | LSP semantic tokens full | legend result ID and full token encoding | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-SEMANTIC-DELTA | LSP-METHODS | LSP semantic tokens delta | delta result equals recomputed full result | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-HOVER | LSP-METHODS | LSP hover | contents range and provenance | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-COMPLETION | LSP-METHODS | LSP completion | candidates ranking text edits and data | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-COMPLETION-RESOLVE | LSP-METHODS | LSP completion resolve | lazy documentation details and edits bound to snapshot | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-SIGNATURE | LSP-METHODS | LSP signature help | overloads active signature and active parameter | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-DEFINITION | LSP-METHODS | LSP definition | source and virtual location links | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-TYPE-DEFINITION | LSP-METHODS | LSP type definition | source and virtual nominal type links | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-IMPLEMENTATION | LSP-METHODS | LSP implementation | interface and Implements target links | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-REFERENCES | LSP-METHODS | LSP references | classified include-declaration source-owned locations | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-HIGHLIGHT | LSP-METHODS | LSP document highlights | read write and text highlight ranges | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-PREPARE-RENAME | LSP-METHODS | LSP prepare rename | range placeholder and rejection reason | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-RENAME | LSP-METHODS | LSP rename | versioned WorkspaceEdit for validated source-owned references | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-CODE-ACTION | LSP-METHODS | LSP bounded code actions | diagnostic-driven declared action families and resolve policy | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-FOLDING | LSP-METHODS | LSP folding ranges | CST-derived line ranges | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-SELECTION | LSP-METHODS | LSP selection ranges | nested token-to-module selection hierarchy | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-VIRTUAL-SOURCE-GENERATED | LSP-METHODS | LSP generated-source virtual content | generated class preambles startup mainline normalization and provenance content | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-VIRTUAL-OXIMAGE | LSP-METHODS | LSP OxImage virtual metadata content | sealed OxImage export metadata content and read-only locations | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-VIRTUAL-LIBRARY | LSP-METHODS | LSP VBA library virtual metadata content | complete VBA library modules members signatures documentation and locations | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-VIRTUAL-HOST | LSP-METHODS | LSP host-provider virtual metadata content | versioned host roots types members signatures documentation and locations | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-VIRTUAL-COM | LSP-METHODS | LSP COM virtual metadata content | typelib libraries coclasses interfaces members records events signatures and locations | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-WATCHED-RELOAD | LSP-METHODS | LSP watched files and reference reload | provider file project reference and target invalidation | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-UNICODE-CONVERSION | LSP-METHODS | LSP Unicode and CRLF conversion | all position-bearing methods roundtrip UTF-8 compiler spans to negotiated positions | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-SHUTDOWN-EXIT | LSP-METHODS | LSP shutdown exit and clean framing | exact lifecycle and stdout discipline | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSP-NEGATIVE-CAPABILITIES | LSP-METHODS | negative capability and MethodNotFound policy | all deferred and unimplemented methods remain absent | planned | remaining-accepted-scope | bd-59co.4.11.2 |
| LSPF-COLD-WORKSPACE | LS-PERFORMANCE | cold workspace load and analysis | project parse bind fact index and first diagnostics | planned | remaining-accepted-scope | bd-59co.4.10.2 |
| LSPF-LOCAL-EDIT | LS-PERFORMANCE | local edit update | overlay change incremental analysis diagnostics and hot queries | planned | remaining-accepted-scope | bd-59co.4.10.2 |
| LSPF-INVALIDATION | LS-PERFORMANCE | project and provider invalidation | reference option target typelib OxImage and host revision rebuild | planned | remaining-accepted-scope | bd-59co.4.10.2 |
| LSPF-COMMON-QUERY | LS-PERFORMANCE | common document query latency | diagnostics completion hover navigation references tokens and structure | planned | remaining-accepted-scope | bd-59co.4.10.2 |
| LSPF-CANCEL-STALE | LS-PERFORMANCE | cancellation and stale suppression latency | rapid edits provider reloads long queries and response races | planned | remaining-accepted-scope | bd-59co.4.10.2 |
| LSPF-MEMORY-HANDLES | LS-PERFORMANCE | memory and handle lifecycle | repeated load edit reload query close and server restart | planned | remaining-accepted-scope | bd-59co.4.10.2 |
| LSPF-FUZZ-NO-PANIC | LS-PERFORMANCE | edit position and protocol fuzz robustness | Unicode CRLF malformed incomplete edits ranges URIs frames and cancellations | planned | remaining-accepted-scope | bd-59co.4.10.2 |
| LSPF-CONCURRENT-READS | LS-PERFORMANCE | deterministic concurrent reads | multiple direct and LSP queries across immutable snapshots | planned | remaining-accepted-scope | bd-59co.4.10.2 |
