# OxVba JIT Architecture V1

Date: 2026-07-10
Status: current destination architecture
System clauses: `JIT-*`, `SYS-DUAL-001`, `RUNTIME-ABI-001`, `HOST-SESSION-001`, `CONF-DIFF-001`
Supersedes: the `JIT_V2_*` planning family and `docs/OXVBA_JIT_PLAN.md` for architectural authority

## 1. Target state

The JIT consumes a VerifiedOxImage and lowers each OxProgram procedure through an inspectable backend-owned plan that makes calling convention, physical storage, coercion boundaries, fault edges, ownership, cleanup and helper requirements explicit.

Statically known VBA procedures use typed primary entry points and direct typed calls. A universal Variant-frame ABI remains as an adapter for dynamic invocation, host entry, late binding, COM and signatures that are genuinely unknown until runtime.

Compiled code lives in persistent project sessions and a versioned bounded cache. The same lowering/helper contracts support JIT execution and later native packaging without VM fallback or source-level semantic reconstruction.

## 2. Input and admission

The public JIT boundary accepts a sealed VerifiedOxImage or verified program set with explicit target/profile facts. Raw deserialized OxPrograms cannot reach code generation through product APIs.

Admission validates:

- supported target triple, pointer width and CPU features;
- OxImage schema and capability profile;
- helper and carrier/layout ABI versions;
- every instruction/terminator disposition;
- external/COM/native capability availability;
- session, host and apartment requirements.

A rejection occurs before partial code generation and returns a stable target/capability diagnostic. No accepted image silently falls back to VM3.

## 3. Procedure lowering plan

The procedure lowering plan is a consumer-side physical/code-generation representation, whether or not its Rust type is named `ProcLoweringIr`. It records:

- basic blocks, branches, returns and fault continuations;
- typed logical values and physical register/stack/addressable slots;
- declared storage and Variant boxing/unboxing boundaries;
- ByRef aliases, temporaries and copyback;
- direct, dynamic, library, COM and native call plans;
- error/Erl state transitions;
- AddRef/Release, cleanup and termination actions;
- helper IDs and ABI signatures;
- source/debug mappings;
- target-specific legalization requirements.

The plan does not contain unresolved names, parse syntax or an alternate type system. If a required fact is absent from OxIR/OxImage, the compiler/artifact contract is extended for both backends rather than reconstructed only in the JIT.

## 4. Calling model

### Typed primary entries

Known procedure signatures lower to target-native typed entries for scalars, floating-point values, stable references and addressable ByRef places when VBA semantics permit. Calls between compiled procedures use these entries directly and retain declared return/parameter information.

Typed entries include an explicit runtime/session context and a non-unwinding error/status convention. Error propagation, Optional/ParamArray preparation and coercion remain VBA semantics, not platform ABI accidents.

### Universal invocation thunk

Every exported/dynamically reachable procedure may expose a universal thunk logically equivalent to:

```text
(session/runtime context, dynamic call frame) -> status/result
```

The thunk validates argument shape, performs VBA coercion and ByRef/copyback rules, calls the typed entry where possible and boxes the result. It is used for reflection-like invocation, unknown calls, host APIs, late binding and interop boundaries rather than every static internal call.

### Dynamic and external calls

Dynamic project/object calls use verified dispatch descriptors. COM and native calls consume the shared Windows interop plan. No helper or thunk maintains a private name/signature allowlist.

## 5. Physical values and frames

Declared typed locals and temporaries remain unboxed in registers or typed stack slots where their addressability and VBA semantics allow. Variant-backed storage is retained for Variant declarations, dynamic values, array/record/object carriers and boundaries requiring canonical VARIANT behavior.

Address-taken and ByRef values have stable verified storage. Fixed strings, arrays, records and object references preserve their declared lifecycle and cleanup rules. The lowering plan, not incidental helper behavior, decides materialization.

Compiled calls use the native stack only within a bounded implementation policy. Deep VBA recursion reaches the VBA-compatible logical frame error before risking process-stack exhaustion, using guards, segmented/trampolined calls or another proven mechanism.

## 6. Error and lifecycle semantics

The JIT implements the full OxIR error model: active handlers, Resume targets, Err fields, dynamic `Err.Raise`, `Error`, line tracking and Erl. Faults retain exact source/procedure provenance and never degrade to line zero merely because execution is compiled.

Class initialize/terminate, object identity, AddRef/Release, termination drains, project events and host callbacks match VM3 ordering. Panics are contained at helper/generated-code boundaries, seat deterministic internal state and leave the session reusable or explicitly terminal.

## 7. Runtime helper ABI

`oxvba-rt-abi` owns one versioned helper descriptor catalog. The JIT consumes it and generates symbol registration from it; the JIT does not define a private catalog. Every helper descriptor contains:

- helper identity and ABI version;
- typed arguments and results;
- target availability;
- ownership and alias rules;
- allocation and cleanup behavior;
- VBA-error and internal-fault behavior;
- reentrancy and user-code callback effects;
- host/apartment requirements;
- source/debug observability.

VM3 and Windows adapters derive their registrations from the same catalog. Generated signatures and runtime entry points are checked for agreement. Incompatible catalog identity, digest or helper signatures reject a cached/image compilation before invocation.

## 8. Backend structure

The JIT implementation separates:

- target/admission and verified input;
- procedure planning and analysis;
- Cranelift IR emission;
- helper/ABI registration;
- runtime/session ownership;
- cache management;
- COM/native adapters;
- source/debug maps;
- differential and codegen tests.

The product backend boundary covers compile/load, session creation, invoke, reset/reload, dispose, diagnostics and capability reporting. It does not hide backend-specific optimization or code-generation decisions.

## 9. Sessions and cache

A compiled project owns executable memory, relocation/symbol bindings, lowering metadata, source maps and reusable class/interface descriptors. A runtime session owns mutable VBA state and references the compiled project safely.

The cache key includes:

- verified OxImage digest;
- target triple, pointer width and CPU features;
- helper-catalog identity, digest and ABI version;
- carrier/layout identity, digest and ABI version;
- host capability/profile and apartment facts;
- codegen settings that affect behavior or code shape.

Cache invalidation and eviction are deterministic and bounded. Stale or incompatible code is never invoked. Cold package load/compile/first call and warm repeated use are separate performance observables.

## 10. JIT/native continuity

Cranelift object/blob output reuses the same procedure plan, helper ABI and source maps. Native packaging adds relocations, imports, entries, export thunks and initialization policy without bypassing VerifiedOxImage or compiler descriptors.

Wrapped outputs may JIT or load code at runtime and are labelled accordingly. Genuine native outputs contain program-specific native entries and a defined external ABI; neither performs unsafe work under DLL loader lock.

## 11. Evidence and completion

The JIT is complete for a capability profile only when:

- every required verified OxIR operation is admitted and compiled;
- typed and dynamic calling matrices are green;
- source recursion, error/Erl and lifecycle behavior match VBA;
- structural VM3/JIT differential observables are green;
- Windows shared rows use the same interop plan as VM3;
- verified package sessions and cache lifecycle are stable;
- helper ABI mismatch and panic/fault tests are deterministic;
- cold/warm performance evidence includes compilation and session costs;
- no supported row depends on milestone-specific allowlists or VM fallback.
