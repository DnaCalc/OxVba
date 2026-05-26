# JIT v2 Cranelift Research Review

Status: `planning-input`
Date: 2026-05-26
Scope: research review for the future Cranelift-based JIT v2 planning stage.
This document records guidance only; it does not claim any JIT implementation
work.

## Repo Anchor

Current OxVba truth before JIT v2:

- `oxvba-jit` is a disabled public boundary. The previous Cranelift prototype
  was removed because it silently fell back to VM execution and treated the
  interpreter `Variant` slot file as the core execution model.
- Bytecode plus VM execution remains the executable semantic authority.
- Retained `Variant`, `BStr`, `ObjectRef`, and `SafeArray` are the runtime value
  truth. `RuntimeValue` is not an approved future-facing carrier.
- A future procedure-lowering contract, now named `ProcLoweringIr`, must be real:
  blocks, terminators, slot/value effects, helper/runtime calls, cleanup edges,
  error-state semantics, source/bytecode mapping, and COM/host boundary effects.

## Primary References

- Cranelift overview and project position:
  <https://cranelift.dev/>
- Cranelift documentation index:
  <https://raw.githubusercontent.com/bytecodealliance/wasmtime/main/cranelift/docs/index.md>
- Cranelift IR reference:
  <https://raw.githubusercontent.com/bytecodealliance/wasmtime/main/cranelift/docs/ir.md>
- Cranelift compared to LLVM:
  <https://raw.githubusercontent.com/bytecodealliance/wasmtime/main/cranelift/docs/compare-llvm.md>
- Cranelift testing:
  <https://raw.githubusercontent.com/bytecodealliance/wasmtime/main/cranelift/docs/testing.md>
- `cranelift_frontend` API:
  <https://docs.wasmtime.dev/api/cranelift_frontend/index.html>
- `cranelift_jit::JITBuilder` API:
  <https://docs.wasmtime.dev/api/cranelift_jit/struct.JITBuilder.html>
- `cranelift_jit::JITModule` API:
  <https://docs.wasmtime.dev/api/cranelift_jit/struct.JITModule.html>
- `cranelift_module::Module` API:
  <https://docs.wasmtime.dev/api/cranelift_module/trait.Module.html>
- Cranelift verifier API:
  <https://docs.wasmtime.dev/api/cranelift_codegen/verifier/index.html>
- Cranelift IR `Signature` and `AbiParam`:
  <https://docs.wasmtime.dev/api/cranelift_codegen/ir/struct.Signature.html>
  and
  <https://docs.wasmtime.dev/api/cranelift_codegen/ir/struct.AbiParam.html>
- Cranelift `MemFlags`:
  <https://docs.rs/cranelift/latest/cranelift/prelude/struct.MemFlags.html>
- Wasmtime platform and support tiers:
  <https://docs.wasmtime.dev/stability-platform-support.html> and
  <https://docs.wasmtime.dev/stability-tiers.html>
- Wasmtime fast-execution guidance:
  <https://docs.wasmtime.dev/examples-fast-execution.html>
- New stack maps for Wasmtime and Cranelift:
  <https://bytecodealliance.org/articles/new-stack-maps-for-wasmtime>
- Security and correctness in Wasmtime:
  <https://bytecodealliance.org/articles/security-and-correctness-in-wasmtime>
- Wasmtime and Cranelift in 2023:
  <https://bytecodealliance.org/articles/wasmtime-and-cranelift-in-2023>
- Wasmtime 28.0 release note:
  <https://bytecodealliance.org/articles/wasmtime-28.0>
- Wasmer runtime backend matrix:
  <https://docs.wasmer.io/runtime/features/>
- `rustc_codegen_cranelift`:
  <https://github.com/rust-lang/rustc_codegen_cranelift>
- V8 Sparkplug baseline compiler:
  <https://v8.dev/blog/sparkplug>
- V8 Maglev optimizing JIT:
  <https://v8.dev/blog/maglev>
- V8 WebAssembly speculative optimization and deoptimization:
  <https://v8.dev/blog/wasm-speculative-optimizations>
- V8 WebAssembly compilation pipeline:
  <https://v8.dev/docs/wasm-compilation-pipeline>
- V8 Turboshaft migration:
  <https://v8.dev/blog/leaving-the-sea-of-nodes>
- PyPy JIT backend notes:
  <https://rpython.readthedocs.io/en/latest/jit/backend.html>

## Research Findings

### Cranelift fit

Cranelift is a fast, embeddable compiler backend, not a complete language VM.
It is used in production by Wasmtime for JIT and AOT compilation and is also
used as an experimental Rust compiler backend. The project explicitly optimizes
for compile speed, security, and relative simplicity rather than LLVM-scale peak
optimization.

Planning consequence: OxVba should treat Cranelift as a code generator behind a
strong OxVba semantic contract. Cranelift should not become the place where VBA
semantics, COM behavior, cleanup policy, or error routing are discovered.

### Supported targets and risk

Current Cranelift support is concentrated on 64-bit targets: x86-64, aarch64,
s390x, and riscv64. Wasmtime support docs also state that non-Wasmtime
Cranelift usage, including non-Wasm calling conventions and some value types,
gets less testing and fuzzing than the paths Wasmtime exercises.

Planning consequence: JIT v2 should start with explicit supported-target gates.
The first accepted target should probably be Windows x64 because VBA/COM parity
depends on it. Unsupported targets should report deterministic
`jit-unavailable` diagnostics rather than falling back silently.

### IR shape

Cranelift IR is per-function, SSA-based, and uses block parameters rather than
phi instructions. Every basic block must end in an explicit terminator. The IR
has stack slots, global values, direct and indirect calls, source locations,
exception table data, trap codes, and user stack maps, but no aggregate types
and no high-level pointer/object model.

Planning consequence: `ProcLoweringIr` should be higher-level than CLIF and should
preserve OxVba semantics before lowering:

- slots and carrier tags;
- helper calls and helper side effects;
- cleanup obligations;
- error-state transitions;
- resumable error control flow;
- COM/native call descriptors;
- source and bytecode locations;
- deopt/snapshot points.

Lowering directly from bytecode to ad hoc CLIF will recreate the failure mode of
the old prototype.

### Frontend obligations

`cranelift_frontend` helps translate mutable variables to SSA. The newer
Cranelift stack-map design also intentionally shifts safepoint spills/reloads
and stack-map correctness toward the frontend/IR producer, with Cranelift
preserving annotations through codegen.

Planning consequence: OxVba must own liveness, cleanup, safepoints, and
snapshot/deopt metadata. For real `Variant`, `BStr`, `ObjectRef`, and
`SafeArray` carriers, the tracer bullets should require explicit live-carrier
maps at every helper call, COM/native boundary, allocation, and error edge.

### Helper ABI

Cranelift signatures describe ABI params, returns, and calling convention.
`JITBuilder` resolves external symbols through an internal symbol table, with
fallback to platform-specific symbol lookup if not found. `JITModule` requires
declarations/definitions to be finalized before function addresses are used, and
finalized function pointers stay valid until the module memory is freed.

Planning consequence: do not rely on ambient platform symbol lookup for runtime
helpers. JIT v2 should register a closed helper symbol table with stable
`extern "C"` helper shims, versioned ABI descriptors, and a startup check that
the helper catalog exactly matches compiled assumptions. Native Declare,
early-bound COM, late-bound COM, and exported callable trampolines should share
the same ABI-descriptor vocabulary rather than separate per-lane encodings.

### Memory flags and UB traps

Cranelift memory flags can loosen semantics for optimization. The docs warn
that adding flags can introduce undefined behavior assumptions. `notrap`,
`readonly`, `aligned`, and `can_move` are especially sensitive: they can allow
loads/stores to be removed or moved when the assumptions are true, and can be
wrong for host-owned or COM-visible memory.

Planning consequence: initial JIT lowering should use conservative memory flags
for runtime carrier slots, BSTR cells, SAFEARRAY data, object pointers, and
native/COM byref memory. Every future stronger flag must have a named proof:
alignment source, mutation boundary, trap behavior, and alias region.

### Verification and evidence

Cranelift has an IR verifier that checks block integrity, SSA dominance,
control-flow/dominator consistency, type checking, global values, and memory
type structure. Cranelift and Wasmtime also rely heavily on file tests,
execution tests, differential fuzzing, structured generators, custom oracles,
and formal or symbolic validation for critical compiler pieces.

Planning consequence: JIT v2 evidence should include:

- Cranelift verifier on every compiled function in debug/test lanes;
- textual CLIF artifacts for debugging, not semantic proof;
- VM/JIT differential tests as the semantic oracle;
- structured generators for bytecode/`ProcLoweringIr` subsets once tracer bullets
  stabilize;
- focused fuzz/differential lanes for arithmetic/coercion, error routing,
  cleanup edges, and COM/native descriptors.

### Tiering and compile budget

Wasmtime treats Cranelift as an optimizing compiler and Winch as a baseline
compiler; no automatic Winch-to-Cranelift tiering exists there. V8 uses tiered
execution: interpreter/baseline execution collects feedback, faster compilers
compile earlier with fewer optimizations, and optimizing tiers rely on guards,
runtime feedback, and deoptimization metadata.

Planning consequence: OxVba should not start with complex adaptive tiering. The
VM already supplies a correct baseline and observation point. JIT v2 can start
as opt-in/lazy per-procedure compilation with:

- hotness counters only after parity is proven;
- compile budget controls;
- a cache key including bytecode digest, host policy, target triple, helper ABI
  version, and COM/native descriptor digest;
- no silent fallback inside a compiled procedure unless the fallback is an
  explicit deopt/slow-helper contract with VM/JIT snapshot equality tests.

### Guards, deopt, and speculative specialization

Modern JITs make assumptions from observed behavior and guard them. V8's Maglev
records frame-state metadata on deoptimizing nodes, mapping interpreter
registers to SSA values. V8's Wasm deopt design shows why deopt exits can be
better than embedding full generic slow paths: optimized code can terminate a
bad assumption and resume baseline execution with reconstructed state.

Planning consequence: JIT v2 should define deopt before aggressive
specialization. For OxVba, a deopt point must include:

- procedure and bytecode/source location;
- live slot map and carrier ownership state;
- pending cleanup stack;
- current error state and resume target;
- byref writeback state;
- COM/native call boundary state if inside or immediately after interop;
- host policy/profile identity.

Until that exists, specialization should be limited to operations whose helper
fallback can prove exact VM-equivalent slot and error snapshots.

### COM/native interop

Cranelift provides generic call lowering, but COM HRESULT/EXCEPINFO, `BSTR`,
`SAFEARRAY`, `IDispatch`, vtable calls, native Declare, and object identity are
OxVba boundary semantics. Wasmer and Wasmtime backend matrices are useful as a
reminder that compiler backends expose different feature and platform support;
they do not solve language interop contracts.

Planning consequence: COM must be in the first design slice, as the planning
outline says. The procedure-lowering IR and helper ABI should model both late-bound
and early-bound COM as descriptor-backed helper calls or typed call stubs, with
shared ABI descriptor machinery for native Declare. Do not let a "fallback to
COM helper" become an untracked semantic escape hatch; it should be the designed
semantic path until a specialized call can prove parity.

### Debugging/profiling

Wasmtime and V8 both distinguish optimized execution from debugging-friendly
execution. V8's Wasm docs explicitly tier down optimized code for debugging
because optimized code may reorder or remove variables. Cranelift has source
locations, debug tags, exception tables, stack maps, and profiling/debugging
integration paths, but these are not a substitute for an OxVba source/bytecode
mapping contract.

Planning consequence: JIT v2 acceptance gates should require source/bytecode
location preservation before broad implementation. Debug mode may deliberately
disable JIT or restrict it to a conservative lowering profile. Profiling should
emit per-procedure compile time, code size, helper-call counts, deopt counts,
and fallback reasons.

## Recommended Planning Additions

Add these items to the JIT v2 planning package before implementation starts:

1. **JIT support matrix.** Record target triple, OS, arch, Cranelift backend
   status, COM/native availability, executable-memory policy, and allowed
   fallback/deopt policy.
2. **Helper ABI manifest.** Version every helper signature and symbol. Include
   calling convention, param/return carrier, ownership transfer, error behavior,
   may-allocate, may-run-host-code, may-reenter, may-set-Err, and cleanup
   obligations.
3. **ProcLoweringIr verifier.** Before CLIF lowering, verify block termination,
   slot dominance, error/resume edges, cleanup stack balance, helper ABI
   references, and source/bytecode mappings.
4. **CLIF verifier gate.** Run Cranelift verifier for every compiled function
   in tests and debug builds.
5. **No ambient-symbol rule.** JIT modules may call only registered helper
   symbols or explicitly declared native/COM thunk symbols from an audited
   descriptor.
6. **Conservative memory rule.** Do not use `trusted`, `readonly`, `aligned`,
   `notrap`, `can_move`, or alias-region narrowing unless a design note proves
   the exact runtime carrier and host-boundary assumptions.
7. **Deopt/snapshot contract.** Define deopt metadata before specializing
   carrier tags or inlining helper behavior. At minimum, every deopt must
   reconstruct the VM slot file, error state, cleanup state, and byref
   writebacks exactly.
8. **Safepoint/live-carrier map.** Treat helper calls, allocation, COM/native
   calls, and deopt exits as safepoints requiring an explicit live-carrier and
   cleanup map.
9. **Tracer-bullet CLIF artifacts.** Each tracer bullet should save textual CLIF
   and source/bytecode mapping artifacts for diagnosis while using VM/JIT
   differential snapshots as the proof.
10. **Differential fuzz seed path.** After tracer bullets 1-4, add generators
    for small bytecode/`ProcLoweringIr` programs over arithmetic, string, array,
    and error semantics. Extend to COM/native descriptors only after deterministic
    fixture descriptors exist.

## Tracer-Bullet Adjustments

The existing tracer bullets are directionally right. The research pass suggests
these acceptance additions:

- **Primitive typed scalar loop:** require CLIF verifier pass, helper-call
  catalog checks, proof that declared `Long`/`Double`/`Boolean` carriers are
  represented directly in `ProcLoweringIr`, and proof that no unsafe memory flags
  are used for frame loads/stores.
- **UDT struct field/copy path:** require descriptor-backed field offsets,
  field carrier kinds, whole-copy independence, cleanup/deopt materialization,
  and proof that UDT fields are not boxed as VARIANT unless declared that way.
- **Error-routing path:** include deopt/slow-helper behavior for a failing
  helper and prove `Err` state plus resume target equality.
- **BSTR lifetime path:** require explicit cleanup edge map for every branch,
  early return, helper failure, and deopt exit.
- **SAFEARRAY path:** require bounds-error equivalence and element live-map
  behavior around helper calls.
- **Late-bound COM path:** require HRESULT, EXCEPINFO, ArgErr, named/default
  member metadata, `Resume Next`, and object identity evidence. Treat the COM
  helper as the semantic path until specialization proves parity.
- **Early-bound COM path:** require descriptor identity and dispatch/vtable
  parity, not only call success.
- **Native Declare path:** prove ABI descriptor reuse with COM/native shared
  machinery, including byref writeback and cleanup when the callee fails.
- **Exported callable path:** require inbound frame projection, cleanup, error
  return policy, and a no-silent-fallback rule for unsupported inbound shapes.

## Open Design Questions

- What is the first supported JIT target: Windows x64 only, or Windows x64 plus
  Linux/macOS for non-COM tracer bullets?
- Is the first compiled function signature `fn(vmctx, frame) -> status`, or a
  more specialized procedure-specific signature?
- What is the exact typed frame representation for primitive scalars, UDT
  structs, declared `Variant` cells, and VM-compatible snapshot
  materialization?
- What exact helper ABI is used for COM/native calls that can reenter the host?
- Do we need user stack maps immediately for retained carriers, or is an
  explicit frame cleanup/live map sufficient until a moving/GC-like carrier
  exists?
- What is the policy for optimized code during debugging: disabled, tiered down,
  or conservative-only?

## Bottom Line

The best modern direction is not "compile bytecode to native as soon as
possible." It is:

1. keep the VM as the oracle;
2. build a real OxVba-native semantic IR;
3. make helpers, cleanup, error routing, COM/native descriptors, and deopt state
   explicit;
4. lower only verified slices to Cranelift;
5. prove every slice with VM/JIT differential evidence before adding
   specialization.

Cranelift is a strong backend choice for that plan, but only if OxVba owns the
semantic and frontend obligations that Cranelift deliberately leaves to the
embedder.
