# OxVBA MACH-1000 Project Plan

> Historical status (2026-04-30): this is a synthesis and vision document, not
> the current execution authority. Current implementation truth is maintained in
> [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), and the current native-ready
> rebase program is
> [`docs/worksets/WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`](docs/worksets/WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md).
> Do not treat multi-level HIR/MIR/CFG or direct native AOT sections here as
> current implementation truth unless the current architecture and workset docs
> explicitly say so.

## Synthesis Provenance

This document is the output of synthesis run `20260226-mach1000-synthesis`. It integrates the baseline OxVBA project plan ([`docs/archive/PLAN_v1_20260226.md`](docs/archive/PLAN_v1_20260226.md)) with the MACH-1000 theoretical architectures ([`docs/archive/BRAINSTORM_MACH1000_20260226.md`](docs/archive/BRAINSTORM_MACH1000_20260226.md)) through a formal decision process documented in [`synthesis/runs/20260226-mach1000-synthesis/`](synthesis/runs/20260226-mach1000-synthesis/README.md).

This document is further refined by synthesis run `20260226-mach1000-refinement-synthesis`, integrating implementation-alignment suggestions from [`docs/MACH1000_PLAN_REFINEMENT_20260226.md`](docs/MACH1000_PLAN_REFINEMENT_20260226.md), documented in [`synthesis/runs/20260226-mach1000-refinement-synthesis/`](synthesis/runs/20260226-mach1000-refinement-synthesis/README.md).

**Refinement synthesis: 10 suggestions extracted; 8 accepted, 2 adapted, 0 deferred, 0 rejected.**

This document superseded the original `PLAN.md` as a synthesized project vision.
It now remains historical architecture and intent context; current execution
authority lives in the active architecture, workset, status, and evidence
documents.

---

## Table of Contents

1. [Project Charter](#1-project-charter)
2. [Architecture](#2-architecture)
3. [Formal Approach](#3-formal-approach)
4. [Testing Strategy](#4-testing-strategy)
5. [Research Notes](#5-research-notes)
6. [Design Notes](#6-design-notes)
7. [Proposed Project Structure](#7-proposed-project-structure)
8. [Implementation Sequencing](#8-implementation-sequencing)

---

## 1. Project Charter

Canonical charter document:
- `CHARTER.md` (top-level). This section is a synchronized in-plan restatement.

### 1.1 Mission

OxVBA is a full-fidelity implementation of the VBA 7 runtime engine written in Rust. It targets parsing, compilation, and runtime execution of VBA source code with correctness, performance, and cross-platform reach that exceed what the Office-bundled VBA engine provides.

OxVBA is developed by **DNA Kode** as part of the **DNA Calc** ecosystem. It is intended to be consumed by the DNA Calc spreadsheet system (developed in `../Foundation`) but operates as a standalone project with its own charter, sharing values, methodology, and operational guidance with the broader DNA Calc program.

The MACH-1000 designation reflects the project's commitment to first-principles performance engineering: cache-optimal data layouts, multi-level domain-aware optimization, register-window execution, and formally verified unsafe code — pushing toward the theoretical performance ceiling for a VBA runtime, not merely exceeding the Office baseline.

- **License**: MIT
- **Organization**: DNA Kode
- **Repository**: `github.com/DnaCalc/OxVba`

### 1.2 Values Ordering

Values are listed from most important to least important. When values conflict, higher-ranked values prevail.

1. **Robustness** — No surprises, no crashes, no undefined behavior. The engine must have a rock-solid feel. Every state is well-defined; every error path is handled. Formal verification of critical unsafe paths.
2. **Compatibility** — Any unintended or undocumented incompatibility versus VBA in Office is a high-priority bug. The reference behavior is VBA 7.0/7.1 as shipped in Office.
3. **Performance** — MACH-1000 class execution through cache-optimal data layouts, multi-level IR with domain-aware optimization, register-window VM, broadword-accelerated interpretation, and JIT compilation. We aim not merely to exceed Office VBA's speed but to approach the theoretical optimum.
4. **Small runtime size** — Distribution should never be an issue. Small likely means faster, but in the trade-off we pick faster over smaller by a clear margin.
5. **Well-managed development environment** — The full development stack must be open-source. Setting up a development environment and rebuilding all artifacts must be well-documented and unproblematic. We prefer tooling that makes this possible, but not at the cost of higher values.

### 1.3 Scope

**In scope (initial focus):**
- VBA 7 language parser (full grammar, lossless concrete syntax tree)
- Multi-level intermediate representation with progressive lowering
- Compilation to bytecode and/or native code
- Runtime execution engine (register-window VM, optional JIT via Cranelift)
- Full VBA/COM reference counting semantics
- Opt-in cycle-detecting garbage collector (one of few beyond-VBA features)
- Compilation to executable format (native or IL) without excessive dependencies (no shipping LLVM)
- Clear separation between semantic project kind and physical emitted build target (`OutputType` vs future `BuildTarget`)
- Cross-platform core: language and basic libraries work on Windows, Linux, macOS
- Full COM compatibility on Windows
- Hosting interfaces: in-process hosting with host COM hookups and non-COM method exposure
- Host-aware runtime loading: host can provide root objects (e.g., `Application`) at engine initialization
- Event and object association (e.g., sheet code-behind in Excel-like hosts)
- Forms runtime including support for custom controls (Rust implementation)

**In scope (listed, not currently active):**
- Runtime security model
- Debugging protocol and interfaces
- IDE features (IntelliSense, go-to-definition, etc.)
- Forms Designer
- COM library interop on non-Windows platforms (abstraction layer exists but full story deferred)
- Wrapper build targets for self-contained EXE/DLL outputs over compiled OxVBA artifacts
- Future native image targets for EXE/DLL outputs after wrapper convergence
- Windowed executable semantics (`WinExe`) as a future semantic output type distinct from console/program-style `Exe`

**Out of scope:**
- Spreadsheet engine (that is DNA Calc's domain)
- VBA IDE implementation
- Office application object model (provided by host, not by OxVBA)

### 1.4 Clean-room Rule

OxVBA adopts the DNA Calc Foundation's clean-room rule (Charter, Section 4) as non-negotiable:

> DNA Calc development relies only on:
> - public specifications and documentation,
> - published research,
> - reproducible observation of Excel behavior.
>
> Excluded:
> - proprietary code, restricted materials, decompilation/disassembly of Excel internals, or reverse engineering of internals.

For OxVBA specifically, "Excel behavior" extends to "VBA runtime behavior in Office." Compatibility claims require evidence records following the Foundation's clean-room evidence workflow: claim identifier, admissible source type, capture/reproduction steps, and reviewer decision.

### 1.5 Normative References

- **[MS-VBAL]** — VBA Language Specification (Microsoft Open Specifications). The primary north-star reference for language semantics.
- **[MS-OAUT]** — OLE Automation Protocol. Governs COM Automation, IDispatch, Variant, SAFEARRAY, and type library semantics.
- **[MS-OVBA]** — Office VBA file format specification. Governs project/module storage structure in Office documents.
- **[MS-DTYP]** — Windows data types specification used by Automation and VBA-adjacent ABI contracts.
- **[MS-COM]** — Component Object Model Plus (COM+) Protocol. Underlying object model.
- **VBA 7.0** (Office 2010) and **VBA 7.1** (Office 2013+) as the target runtime versions.
- DNA Calc Foundation Charter, Operations, and Architecture documents for methodology and doctrine.
- Foundation reference doctrine and mirror index:
  - `../Foundation/REFERENCE_SPEC_FORMAT_AND_CONFORMANCE.md`
  - `../Foundation/reference/spec_seeds.csv`
  - `../Foundation/reference/index.csv`
  - `../Foundation/reference/runs/*/outputs/conformance_items.jsonl`
- **Knuth, TAOCP Fascicle 1** — Broadword algorithms and MMIX architecture (public research).
- **MLIR: Multi-Level Intermediate Representation** (Lattner et al.) — Progressive lowering methodology (public research; we implement the concepts in Rust, not the C++ framework).

### 1.6 Why Rust

| Concern | Rust's answer |
|---|---|
| Robustness (#1 value) | Memory safety without GC; no undefined behavior in safe code; algebraic types make illegal states unrepresentable |
| Performance (#3 value) | Zero-cost abstractions; no runtime overhead; competitive with C/C++; ideal for cache-line-aware data layout |
| Small runtime (#4 value) | No managed runtime to ship; static linking; minimal binary sizes achievable |
| COM interop | Excellent `windows` crate ecosystem; `repr(C)` for ABI-compatible types; raw pointer support where needed |
| Cross-platform | First-class support for Windows, Linux, macOS; conditional compilation for platform-specific code |
| Ecosystem alignment | Sibling project DnaVisiCalc is Rust; shared tooling, conventions, and developer knowledge |
| Cranelift availability | Cranelift JIT backend is a Rust-native project; tight integration without FFI overhead |
| Formal verification | Kani (bounded model checking) integrates natively with Rust; Lean 4 for specification-level proofs |

---

## 2. Architecture

### 2.1 Crate Decomposition

OxVBA is organized as a Cargo workspace with nine crates, each with a clear responsibility boundary.

```
oxvba (workspace root)
├── crates/
│   ├── oxvba-syntax        # Lexer, parser, lossless concrete syntax tree
│   ├── oxvba-ir             # Multi-level intermediate representation (VbaHir → VbaMir → CfgIr)
│   ├── oxvba-compiler       # Semantic analysis, type checking, IR lowering, bytecode emission
│   ├── oxvba-runtime        # Variant type, type coercion, built-in functions, VBA-specific allocator
│   ├── oxvba-vm             # Register-window bytecode virtual machine
│   ├── oxvba-jit            # Cranelift-based JIT compilation
│   ├── oxvba-com            # COM abstraction layer (real COM on Windows, traits elsewhere)
│   ├── oxvba-host           # Hosting API, engine orchestration, embedding interface
│   └── oxvba-cli            # Command-line runner and REPL
```

**Dependency graph:**

```
oxvba-syntax          (no internal deps)
    │
    ▼
oxvba-ir              ← oxvba-syntax, oxvba-runtime
    │
    ▼
oxvba-compiler        ← oxvba-syntax, oxvba-ir, oxvba-runtime
    │
    ▼
oxvba-vm              ← oxvba-compiler, oxvba-runtime, oxvba-com
oxvba-jit             ← oxvba-compiler, oxvba-ir, oxvba-runtime, oxvba-com, cranelift-*
    │
    ▼
oxvba-host            ← oxvba-vm, oxvba-jit, oxvba-compiler, oxvba-runtime, oxvba-com
    │
    ▼
oxvba-cli             ← oxvba-host

oxvba-runtime         (no internal deps; defines core types)
oxvba-com             ← oxvba-runtime
```

Design rationale:
- **`oxvba-syntax` is dependency-free** — enables use by external tools (formatters, linters, IDE support) without pulling in the full runtime.
- **`oxvba-ir` is the new multi-level optimization core** — houses the three IR tiers (VbaHir, VbaMir, CfgIr) and all optimization passes. Depends on syntax (for source mapping) and runtime (for type information). This crate embodies the MACH-1000 insight that premature lowering from AST to bytecode/Cranelift loses VBA-specific optimization opportunities.
- **`oxvba-runtime` is dependency-free** — the Variant type, coercion logic, and VBA-specific allocator are foundational; everything else builds on them.
- **`oxvba-vm` and `oxvba-jit` are peers** — either can execute compiled bytecode; the host selects which backend to use. The VM is always available; the JIT is opt-in.
- **`oxvba-host` is the integration facade** — external consumers (DNA Calc, standalone CLI) interact through host, never directly with VM or JIT.

### 2.2 Compilation Pipeline

The pipeline implements progressive lowering through domain-specific intermediate representations, preserving VBA semantics long enough for targeted optimization before committing to low-level execution forms.

```
┌──────────┐    ┌──────────┐    ┌───────────────┐    ┌──────────────┐
│  Source   │───▶│  Lexer   │───▶│    Parser      │───▶│   Semantic    │
│  (.bas,   │    │ (tokens) │    │ (lossless CST) │    │   Analysis    │
│   .cls,   │    └──────────┘    └───────────────┘    │ (binding)     │
│   .frm)   │                                         └──────┬───────┘
└──────────┘                                                  │
                                                              ▼
                                               ┌──────────────────────┐
                                               │     VBA HIR          │
                                               │  (high-level IR)     │
                                               │  For Each, On Error, │
                                               │  implicit coercions, │
                                               │  guarded regions     │
                                               └──────────┬───────────┘
                                                          │ VBA-aware optimizations:
                                                          │ constant folding, dead code,
                                                          │ coercion elimination,
                                                          │ early/late binding resolution
                                                          ▼
                                               ┌──────────────────────┐
                                               │     VBA MIR          │
                                               │  (mid-level IR)      │
                                               │  Explicit IEnum,     │
                                               │  RC boundaries,      │
                                               │  IDispatch calls,    │
                                               │  guarded error edges │
                                               └──────────┬───────────┘
                                                          │ Classic optimizations:
                                                          │ inlining, loop transforms,
                                                          │ register allocation prep
                                                          ▼
                                               ┌──────────────────────┐
                                               │     CFG IR           │
                                               │  (control-flow graph)│
                                               │  SSA form, fully     │
                                               │  expanded control    │
                                               │  flow, explicit      │
                                               │  error edges         │
                                               └──────────┬───────────┘
                                                          │
                                            ┌─────────────┴─────────────┐
                                            │                           │
                                      ┌─────▼─────┐             ┌─────▼─────┐
                                      │  Register  │             │    JIT    │
                                      │  Bytecode  │             │ Cranelift │
                                      │  Emission  │             │ IR (CLIF) │
                                      └─────┬─────┘             └─────┬─────┘
                                            │                         │
                                      ┌─────▼─────┐             ┌─────▼─────┐
                                      │  VM exec   │             │  Native   │
                                      │ (default)  │             │  exec     │
                                      │ reg-window │             │  (opt-in) │
                                      └───────────┘             └───────────┘
```

**Stage 1: Lexing** (`oxvba-syntax`)
- Tokenizes VBA source into a token stream.
- Handles VBA's line-continuation (`_`), line-oriented statements, and context-sensitive keywords.
- Preserves trivia (whitespace, comments) for lossless round-tripping.

**Stage 2: Parsing** (`oxvba-syntax`)
- Hand-written recursive descent parser producing a lossless concrete syntax tree (CST).
- Adopts the Roslyn green/red tree pattern: immutable green nodes (syntax data, relative-width-only, position-independent) with on-demand ephemeral red wrappers (parent pointers, absolute positions computed by summing widths).
- Green tree supports structural sharing — massive deduplication for legacy enterprise VBA modules with repeated patterns.
- Full error recovery: always produces a tree, even for malformed input. Errors are attached to nodes, not thrown.
- Rationale: Hand-written over parser generators for full control over error recovery and error messages (serves Robustness value). Lossless CST enables future IDE tooling without reparsing.

**Stage 3: Semantic Analysis** (`oxvba-compiler`)
- Name resolution (modules, procedures, variables, types, COM references).
- Type checking with VBA's implicit coercion rules.
- Binding of late-bound (IDispatch) vs. early-bound (vtable) calls.
- Resolution of `ByRef` (default) vs. `ByVal` parameter passing.
- Produces a bound tree suitable for lowering to VBA HIR.

**Stage 4: VBA High-Level IR — VbaHir** (`oxvba-ir`)

The highest IR tier, closest to VBA source semantics but in data-flow form. Retains:
- `For Each` over COM collections (not yet expanded to `IEnumVARIANT`)
- Implicit `Variant` coercions (not yet expanded to explicit conversions)
- `On Error GoTo` / `Resume Next` as first-class guarded-region operations (not yet expanded to CFG edges)
- Default property access (not yet resolved to explicit member dispatch)
- Late-bound calls preserved as semantic operations

VBA-aware optimizations at this level:
- **Constant folding** with VBA-specific semantics (Variant-aware)
- **Dead code elimination** (unreachable branches after constant folding)
- **Coercion elimination** (remove redundant coercions when source and target types are known)
- **Early-binding promotion** (promote late-bound calls to early-bound when type information is available)

**Stage 5: VBA Mid-Level IR — VbaMir** (`oxvba-ir`)

De-sugars VBA-specific constructs into explicit operations:
- `For Each` → explicit `IEnumVARIANT::Next` / `IEnumVARIANT::Reset` flow
- Implicit coercions → explicit `CoerceToType` operations
- RC boundaries → explicit `AddRef` / `Release` insertion
- Late-bound dispatch → explicit `IDispatch::GetIDsOfNames` + `IDispatch::Invoke`
- `On Error Resume Next` → guarded operations with explicit success/exception edges (but still within structured regions, not yet fully in CFG form)

Classic optimizations at this level:
- **Inlining** of small procedures
- **Loop-invariant code motion**
- **Common subexpression elimination**
- **Register allocation preparation** (liveness analysis, interference graphs)

**Stage 6: Control-Flow Graph IR — CfgIr** (`oxvba-ir`)

Fully lowered to explicit control-flow graph in SSA form:
- All structured control flow expanded to basic blocks and edges
- On Error regions fully expanded to explicit guarded blocks with success/exception edges
- All operations are primitive (no VBA-specific composite operations remain)
- SSA form enables standard optimization passes

This is the last representation before target-specific lowering.

**Stage 7a: Register Bytecode Emission** (`oxvba-compiler`)
- Emits a custom register-based bytecode format (OxVBA bytecode).
- Register-based design inspired by MMIX — reduces memory traffic versus stack-based bytecodes.
- Bytecode is serializable via `rkyv` for zero-copy memory-mapped loading.

**Stage 7b: JIT Compilation** (`oxvba-jit`)
- CfgIr → Cranelift IR (CLIF) translation.
- Per-function compilation.
- Register-based CfgIr maps naturally to Cranelift's SSA-based register IR — no impedance mismatch.

**Stage 8: Execution** (`oxvba-vm` or native)
- **VM (default):** Register-window interpreter with broadword-accelerated instruction decoding. Always available, no platform-specific dependencies.
- **JIT (opt-in):** Native code execution via Cranelift-compiled functions. Suitable for hot paths and performance-critical workloads.

### 2.3 Key Types: Variant

The `Variant` type is the most performance-critical data structure in the engine. Every VBA value passes through it, and correctness depends on matching VBA/COM semantics exactly.

**Current design note: OxVBA semantic runtime values are canonical; COM layouts are boundary representations**

OxVBA does not currently require its internal execution representation to equal native VBA/COM wire layout everywhere. The authoritative runtime model is the OxVBA semantic value model, while `oxvba-com` and other boundary layers translate to and from COM-facing shapes such as `VARIANT`, `BSTR`, `SAFEARRAY`, and COM interface pointers.

Known current differences from native VBA/COM internal representation include:
- strings may remain Rust-owned UTF-8 semantic values internally even when the boundary shape is `BSTR`,
- object/interface identity may remain handle- or facade-based internally rather than raw COM interface pointers,
- and similar internal/boundary representation differences may exist for other supported types.

These are known differences, not hidden assumptions:
- they may leak at some boundaries from time to time,
- they should be monitored through interop and conformance evidence,
- and they may be revisited later if they become a real compatibility or performance problem.

Where COM-style layout alignment is honest and useful, OxVBA may still use it as an optimization or local implementation choice. That does not make COM wire layout the canonical semantic ownership model for the engine.

```rust
#[repr(C)]
pub struct Variant {
    vt: u16,           // VARENUM
    reserved1: u16,
    reserved2: u16,
    reserved3: u16,
    data: VariantData, // COM union payload
}
```

**Supported variant types (canonical COM semantics):**

| VarType | `VARENUM` | Payload model | Notes |
|---|---:|---|---|
| `Empty` | `0x0000` | none | Uninitialized |
| `Null` | `0x0001` | none | SQL Null semantics |
| `Integer` | `0x0002` | `i16` in union | |
| `Long` | `0x0003` | `i32` in union | |
| `Single` | `0x0004` | `f32` in union | |
| `Double` | `0x0005` | `f64` in union | |
| `Currency` | `0x0006` | `CY`/scaled `i64` | |
| `Date` | `0x0007` | `DATE`/`f64` | |
| `String` | `0x0008` | `BSTR` pointer | |
| `Object` | `0x0009` | COM interface pointer | |
| `Error` | `0x000A` | `SCODE`/`i32` | |
| `Boolean` | `0x000B` | `VARIANT_BOOL` (`0` / `-1`) | |
| `Decimal` | `0x000E` | COM decimal overlay rules | |
| `Byte` | `0x0011` | `u8` in union | |
| `LongLong` | `0x0014` | `i64` in union | |
| `LongPtr` | platform | pointer-sized integer | |
| `Array` | flag `0x2000` | SAFEARRAY pointer | ORed with element type |
| `ByRef` | flag `0x4000` | by-ref pointer | ORed with referent type |

**Optional future optimization / revisit path:**

We may still revise internal representation choices for hot paths or interop pressure (for example compact immediate forms, different string storage, short-string embedding, indirection for long contents, or tighter internal/boundary alignment) if and only if evidence justifies it.

If introduced:
- it remains an internal implementation decision, not an implicit semantic redefinition of the whole runtime around COM wire layout,
- boundary marshalling must be deterministic and lossless,
- formal/conformance evidence must prove semantic equivalence at representation boundaries.

### 2.4 Memory Management

**Primary: Reference counting (COM-compatible)**
- All COM objects use `AddRef`/`Release` reference counting, matching VBA's deterministic destruction semantics.
- `Class_Terminate` is called deterministically when the last reference is released — this is load-bearing VBA semantics that many programs depend on.
- `BStr` (VBA strings) are reference-counted with COM `SysAllocString`/`SysFreeString` on Windows; Rust-managed equivalent on other platforms.

**Weak references:**
- Used internally to break known cycles (e.g., parent ↔ child object relationships).
- Not exposed to VBA user code (VBA has no weak reference concept).

**Opt-in cycle-detecting GC:**
- Implements the Bacon-Rajan cycle detection algorithm as an opt-in safety net.
- VBA programs can create reference cycles (e.g., circular object references) that pure reference counting cannot collect.
- The cycle detector is **scheduled, not concurrent** — runs at configurable trigger points (after N allocations, at idle, or on explicit host request), never interrupting VBA execution mid-statement.
- **Epoch-based batching:** Suspect objects are grouped into epochs. Detection runs process one epoch at a time, amortizing the cost across multiple collection opportunities and bounding worst-case latency per invocation.
- This is one of the few intentional beyond-VBA features: Office VBA leaks cycles silently; OxVBA can optionally detect and collect them.

**Boundary-tag allocator for VBA heap objects:**

Dynamic VBA allocations (BStr strings, SafeArrays, UDT buffers) are served by a purpose-built boundary-tag allocator:
- Each block carries size/status tags at both its start and end.
- On free, adjacent tags are inspected in **O(1)** and blocks are coalesced immediately.
- Reduces fragmentation over long-running workloads typical of Excel automation (macros that run for hours, allocating and freeing thousands of strings).
- Falls back to the system allocator for oversized requests.
- Thread-local arenas per engine instance (no cross-engine contention given STA model).

**Invariants:**
- Reference counts are always non-negative.
- An object with refcount 0 is immediately destroyed (deterministic).
- The cycle detector only collects objects that are unreachable from any root — it never destroys objects that are still reachable.
- The boundary-tag allocator maintains: (a) no overlapping live blocks, (b) every freed block is coalesced with free neighbors, (c) total allocated + free = arena capacity.

### 2.5 COM Abstraction

COM is fundamental to VBA — every object, collection, and class instance is a COM object with `IUnknown` and usually `IDispatch` interfaces.

**Windows (real COM):**
- Use the `windows` crate for COM interop.
- OxVBA objects implement real COM interfaces (`IUnknown`, `IDispatch`, `IConnectionPointContainer`, etc.).
- Host-provided objects (e.g., Excel's `Application`, `Worksheet`) are consumed as real COM objects via their type libraries.
- OxVBA can be hosted as a COM server itself.

**Non-Windows (trait-based abstraction):**
- `ComObject` and `Dispatch` traits define the interface contract.
- OxVBA's own objects (classes defined in VBA code, built-in objects like `Collection`, `Dictionary`) work through pure-Rust trait implementations.
- External COM libraries are not available on non-Windows — the abstraction layer provides clear error surfaces for attempts to use them.
- The cross-platform goal is: all VBA language features and built-in types work everywhere; host-provided and external COM objects are Windows-only unless the host provides cross-platform implementations.

### 2.6 Threading Model

VBA uses the COM Single-Threaded Apartment (STA) model:

- **Single VBA execution thread per engine instance.** All VBA code within one engine runs on a single thread. This is non-negotiable for compatibility — VBA programs assume single-threaded execution.
- **DoEvents** pumps the message queue, yielding the thread to process pending events (UI repaints, timer callbacks, etc.) before returning control to VBA.
- **Multiple engine instances** can run on separate threads (separate apartments), enabling host applications to run multiple independent VBA projects concurrently.
- **Callbacks from host** (event handlers, COM callbacks) are marshaled to the VBA thread via the apartment's message queue.

Rationale: This model exactly matches Office VBA behavior. Attempting to add multithreading within a VBA project would break compatibility with essentially all existing VBA code.

### 2.7 Error Handling

VBA's error handling is fundamentally different from exception-based systems. It uses a per-frame state machine that does not unwind the call stack.

**Error handling modes (per procedure frame):**

| State | Behavior |
|---|---|
| `Default` | No error handler active. Runtime errors propagate to the caller. |
| `On Error GoTo <label>` | Transfers control to the labeled handler within the same procedure. |
| `On Error Resume Next` | Silently continues to the next statement after an error. `Err` object is populated. |
| `On Error GoTo 0` | Resets to Default, disabling any active handler. |
| `Resume` | Retries the statement that caused the error. |
| `Resume Next` | Continues with the statement after the one that caused the error. |

**Key implementation details:**
- Error state is per-procedure-frame, stored on the call stack alongside locals and the return address.
- `On Error Resume Next` does not unwind — it sets a flag and the VM checks it after each statement.
- The `Err` object is a per-engine singleton, populated on error, cleared on successful `Resume` or new procedure entry.
- `GoSub`/`Return` is implemented as intra-procedure control flow (not a procedure call), sharing the same error handling frame.

**IR-level modeling (MACH-1000 innovation):**

`On Error Resume Next` creates irreducible control-flow graphs if naively lowered — every operation would need explicit branch-to-next and branch-to-handler edges, destroying optimization opportunities.

The multi-level IR handles this through staged lowering:

1. **VbaHir:** `On Error Resume Next` is a first-class **guarded-region** operation. A guarded region wraps a sequence of operations; the semantics are "execute each operation; if any faults, populate `Err` and continue to the next." The region is opaque to optimization passes that don't understand error semantics, preserving them for reordering and analysis by passes that do.

2. **VbaMir:** The guarded region is preserved but each operation within it acquires explicit success/exception edge annotations. Error-handler state transitions (`On Error GoTo`, `Resume`, etc.) become explicit state-machine operations.

3. **CfgIr:** Full expansion. For a basic block with operations O₁, O₂, ..., Oₙ under Resume Next, each Oᵢ is lowered to a guarded form with:
   - **success edge** → Oᵢ₊₁
   - **exception edge** → unified exception block (updates `Err.Number`, `Err.Description`, clears exception, continues to Oᵢ₊₁)

By delaying this expansion until CfgIr, the VbaHir and VbaMir passes can freely reorder, fold, and eliminate operations within guarded regions without being dominated by error-handling edges.

---

## 3. Formal Approach

OxVBA uses a three-pronged formal strategy: exhaustive decision tables for finite combinatorial properties, Lean 4 machine-checkable specifications for structural and inductive properties, and Kani bounded model checking for unsafe Rust correctness.

### 3.1 Decision Tables

Decision tables specify the observable behavior of VBA's type system and arithmetic operations as exhaustive, machine-readable matrices.

**Type coercion table (~20 × 20):**
- Rows: source VarType
- Columns: target VarType
- Cells: coercion result (success with target type, or specific error code)
- Validated against Office VBA observation harness

**Arithmetic result type table (~20 × 20 × 15):**
- Dimensions: left VarType, right VarType, operator
- Operators: `+`, `-`, `*`, `/`, `\` (integer div), `Mod`, `^`, `&`, comparison operators, `Like`, `Is`
- Cells: result VarType (or error code)
- Validated against Office VBA observation harness

**Comparison semantics table:**
- Covers `Option Compare Binary` vs `Option Compare Text`
- String vs numeric comparison promotion rules
- `Nothing` comparison rules
- `Null` propagation rules

These tables are:
- **Checked into the repository** as data files (CSV or structured format)
- **Generated from observation harness** runs against Office VBA
- **Used as test oracles** — the implementation must agree with the table for every cell
- **Exhaustive** — every type combination is covered; there are no "don't care" entries

### 3.2 Lean 4 Specifications

Lean 4 provides machine-checkable proofs of structural properties that cannot be captured by finite tables alone.

**Formalization scope:**

| Lean module | What it specifies |
|---|---|
| `VarType.lean` | Inductive definition of the `VarType` universe. Enumeration of all variant types with their properties (numeric?, string?, object?, ordinal size). |
| `Coerce.lean` | Coercion relation as a decidable relation on `VarType` pairs. Proof of transitivity (or documentation of where VBA intentionally breaks transitivity). Proof that the coercion relation is consistent with the decision table. |
| `Arithmetic.lean` | Operator result type as a total function on `(VarType, VarType, Op)`. Proof of consistency with the arithmetic decision table. Proof that numeric promotion is monotone (wider types never narrow). |
| `RefCount.lean` | Reachability invariant: an object is destroyed if and only if it is unreachable from any root. Proof that reference counting maintains the invariant in the acyclic case. Statement of the cycle-detection guarantee. |

**Principles:**
- Lean specifications serve as **Green-team artifacts** (in DNA Calc terminology) — machine-checkable, authoritative, reviewed.
- Lean does not generate Rust code. It is a separate verification artifact that must agree with the decision tables and with the implementation.
- File-based integration: Lean output (proofs, extracted tables) is checked against test oracles in CI.
- The Lean project is self-contained: `lakefile.lean` + `lean-toolchain` in `formal/lean/`.

### 3.3 Kani Bounded Model Checking

Kani provides bounded model checking for Rust code, particularly critical for proving correctness of `unsafe` blocks that the Rust type system cannot verify.

**Verification targets:**

| Target | What Kani proves |
|---|---|
| COM `VARIANT` layout invariants | `vt`/reserved/data fields remain ABI-compatible; union reads/writes preserve alignment/provenance and valid `VARENUM` handling. |
| Variant boundary marshalling (if alt internal repr enabled) | Internal compact representation roundtrips losslessly to canonical COM `VARIANT` at all boundaries. |
| Broadword decoder masks | The SWAR bitmasks cannot mis-detect an opcode byte under any 64-bit input word. No false positives, no false negatives. |
| Register-window bounds | The sliding register window never reads or writes beyond the allocated register file. Spill/fill operations preserve all values. Window shift on call/return is always within bounds. |
| Boundary-tag allocator | No overlapping live blocks. Coalescing never corrupts adjacent blocks. Free-list invariants hold after every operation sequence (up to bounded depth). |
| COM pointer casts | `IUnknown` → `IDispatch` → concrete interface casts preserve pointer provenance and alignment. |

**Integration:**
- Kani proofs run in CI alongside `cargo miri test`.
- Proof harnesses live next to the code they verify (in `#[cfg(kani)]` modules).
- Kani uses symbolic execution up to configurable loop/recursion bounds — not exhaustive, but covers all concrete paths within the bound.

### 3.4 Error Handling State Machine

The VBA error handling model is specified as a finite state machine:

```
States: { Default, HandlerActive, ResumeNext, InHandler, Exiting }
Inputs: { OnErrorGoTo, OnErrorResumeNext, OnErrorGoTo0,
          RuntimeError, Resume, ResumeNext, ExitProcedure }
```

Transitions and observable effects are specified as a state transition table, validated against Office VBA behavior.

### 3.5 Deferred: Verus and Creusot

**Verus** (deductive verification with SMT-backed invariants) and **Creusot** (separation-logic reasoning aligned with Rust ownership) are promising tools for proving deeper properties:
- Verus: coercion matrix correctness (no panics, correct overflow semantics, correct `Err` behavior)
- Creusot: semantic preservation across IR lowering passes

These are deferred until: (a) the Lean specifications are stable, (b) the multi-level IR is implemented, and (c) the tools have matured sufficiently for a project of this scope. The architecture is designed to accommodate them — critical `unsafe` code is isolated into small, well-bounded functions amenable to deductive proofs.

---

## 4. Testing Strategy

### 4.1 Four-Tier Testing

**Tier 1: Unit tests** (per-crate, `cargo test`)
- Standard Rust unit tests for each crate's internal logic.
- Parser tests: token streams, CST shapes, error recovery.
- Variant tests: type coercion, arithmetic, comparison (driven by decision tables).
- IR tests: lowering correctness at each tier (VbaHir → VbaMir → CfgIr).
- VM tests: instruction execution, register-window behavior, control flow.
- Fast, comprehensive, run on every commit.

**Tier 2: Conformance tests** (golden-file comparison against Office VBA)
- VBA source files paired with expected output.
- Executed by both OxVBA and Office VBA; outputs compared.
- Covers: expression evaluation, control flow, error handling, object lifecycle, string operations, array operations, COM interaction patterns.
- Output format: structured (not just stdout) — captures `Err` object state, variable types, reference counts at key points.
- Golden files generated by observation harness and checked into the repository.
- Any difference between OxVBA and Office VBA output is either:
  - A bug in OxVBA (fix it), or
  - A documented intentional divergence (rare; must be justified and tracked).

**Tier 3: Property-based tests** (`proptest`)
- Fuzz-style tests for Variant arithmetic, coercion, parser roundtripping.
- Parser roundtrip property: `parse(source).to_string() == source` for all well-formed inputs.
- Variant arithmetic property: result type matches decision table for all type combinations.
- Refcount property: after executing any sequence of object operations, all objects are either reachable or destroyed.
- IR lowering property: VbaHir → VbaMir → CfgIr → bytecode produces identical execution results for all test programs (semantic preservation).

**Tier 4: Formal verification** (Kani + Miri)
- `cargo miri test` for undefined behavior detection in unsafe code: reference counting, Variant payload access, COM vtable dispatch, FFI boundaries.
- Kani proof harnesses for bounded model checking of critical unsafe invariants (see Section 3.3).
- Run in CI on every commit (Miri) and on PR merge (Kani, which is slower).

### 4.2 Observation Harness

The observation harness is a key piece of infrastructure for the clean-room development approach.

**Purpose:** Systematically observe VBA runtime behavior in Office to produce golden files, decision table entries, and conformance test expectations.

**Design:**
- A VBA project running inside Office (Excel) that exercises specific behaviors and records results.
- Output is structured (e.g., JSON or CSV) for machine consumption.
- Captures: expression results, types, error codes, object lifecycle events.
- Results are checked into the repository as evidence artifacts.
- Harness source code is public and reproducible.

**Evidence workflow** (per Foundation Operations Section 9):
- Each observation is an evidence record: claim ID, source type (observation harness), capture steps, reviewer decision.
- Evidence records are gate inputs for stabilization claims involving compatibility.

---

## 5. Research Notes

### 5.1 VBA 7 Technical Details

**Language characteristics relevant to implementation:**
- **Line-oriented syntax** — statements are line-delimited with `_` line continuation. No semicolons.
- **Case-insensitive** — identifiers are case-insensitive; canonical casing preserved.
- **Context-sensitive keywords** — many keywords (e.g., `Error`, `Name`, `Type`) are valid identifiers in certain contexts.
- **Implicit variable declaration** — unless `Option Explicit` is set, undeclared variables are implicitly `Variant`.
- **ByRef default** — parameters are passed by reference unless explicitly `ByVal`. This is a major semantic difference from most languages.
- **Default properties** — objects can have a default property accessed by using the object reference without a member access. `Set` vs bare assignment distinguishes object assignment from default property assignment.
- **GoSub/Return** — intra-procedure goto with a return stack. Not a procedure call.
- **On Error Resume Next** — non-stack-unwinding error suppression. Per-frame, not global.
- **Deterministic destruction** — `Class_Terminate` is called immediately when the last reference is released, not deferred.
- **Array lower bounds** — arrays can have arbitrary lower bounds (`Dim a(5 To 10)`, `Option Base 1`).
- **Late binding (IDispatch)** — `Dim obj As Object` uses IDispatch for all member access; resolved at runtime.
- **Early binding (vtable)** — `Dim obj As Worksheet` uses vtable dispatch; resolved at compile time with type library.

**VBA 7 specific features (vs VBA 6):**
- `LongPtr` type — pointer-sized integer for 64-bit compatibility.
- `LongLong` type — explicit 64-bit integer.
- `PtrSafe` keyword for `Declare` statements.
- Conditional compilation: `#If VBA7 Then` / `#If Win64 Then`.

### 5.2 Existing Implementations and Prior Art

**twinBASIC** (Wayne Phillips)
- Commercial VBA-compatible language and IDE.
- C++/LLVM-based compiler.
- Targets full VBA compatibility plus language extensions.
- Demonstrates that full VBA compatibility is achievable outside Microsoft.
- We cannot use any twinBASIC code or non-public implementation details (clean-room rule).
- Public talks and documentation are admissible research material.

**ViperMonkey** (Philippe Lagadec)
- Open-source Python-based VBA emulator for malware analysis.
- Partial VBA implementation focused on macro execution.
- Demonstrates: VBA parsing approaches, common patterns in real-world VBA code.
- Limited fidelity — not aiming for full compatibility.

**LibreOffice Basic**
- Open-source Basic interpreter in LibreOffice.
- NOT VBA-compatible — different object model, different runtime behavior.
- Some VBA compatibility mode, but fundamentally different architecture.
- Useful as reference for what challenges arise in Basic runtime implementation.

**pcode2code** (Bonneaud, et al.)
- Open-source tool for decompiling VBA P-code.
- Provides insight into P-code instruction set structure (public research).

### 5.3 Cranelift Analysis

**What Cranelift is:**
- A code generator (compiler backend) written in Rust, developed by the Bytecode Alliance.
- Designed for JIT compilation: fast compile times, reasonable code quality.
- Used by Wasmtime (WebAssembly runtime) as its primary code generator.

**Why Cranelift over LLVM:**

| Factor | Cranelift | LLVM |
|---|---|---|
| Compile speed | Very fast (designed for JIT) | Slow (designed for AOT optimization) |
| Binary size | Small (pure Rust, static link) | Enormous (~100MB+ of libraries) |
| Rust integration | Native Rust crate, no FFI | Requires llvm-sys FFI bindings |
| Code quality | Good (not LLVM-tier optimization) | Excellent (mature optimizations) |
| Dependency footprint | Moderate (Rust crates only) | Heavy (C++ toolchain, linking) |
| Build simplicity | `cargo build` just works | Complex build system, platform issues |
| IR compatibility | Register-based SSA (natural fit for CfgIr) | Register-based SSA (also compatible) |

**Verdict:** Cranelift aligns with values #4 (small runtime) and #5 (well-managed dev env) while providing sufficient code quality for value #3 (performance). LLVM would provide better peak optimization but at unacceptable cost to binary size and build complexity. The MACH-1000 multi-level IR performs domain-specific optimizations that LLVM couldn't do anyway — the final lowering to native code is a thin translation, not where the interesting optimization happens.

**Cranelift integration approach:**
- CfgIr → Cranelift IR (CLIF) translation. Both are register-based SSA — natural mapping.
- Per-function compilation (no whole-program optimization — matches VBA's compilation model).
- JIT mode: compile on first call, cache native code.
- AOT mode: compile all functions ahead of time, serialize native code.

### 5.4 MS-VBAL Specification Notes

The [MS-VBAL] specification defines:
- Lexical grammar (Section 3.3): tokens, whitespace, line continuation, identifier rules.
- VBA module structure (Section 4): modules, procedures, declarations.
- Type system (Section 2.1): built-in types, user-defined types, classes, enums.
- Expression evaluation (Section 5.6): operator precedence, type coercion rules, evaluation order.
- Statement execution (Section 5): control flow, error handling, variable lifetime.
- Project structure: standard modules, class modules, document modules, form modules.
- Conditional compilation (Section 3.4): `#If`, `#Const`, predefined constants.

Implementation requirement clarification:
- OxVBA targets full MS-VBAL scope coverage, including project/module semantics (module naming, visibility, qualification, module/class/document/form categories, and project-level resolution rules).
- Forms/UI-host integration may be deferred by explicit phase policy, but these features remain required scope (not removed scope).
- Normative source material and extracted conformance candidates come from `../Foundation/reference` (see `docs/FOUNDATION_SPEC_REFERENCE.md`), not locally vendored spec snapshots.
- Formal PMR specification baseline:
  - `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`
  - `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`
  - `docs/spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md`
  - `docs/spec/PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md`

Key complexity areas identified:
- The coercion rules (Section 2.1.3) are extensive and have many special cases.
- `ByRef` semantics interact with default properties and type coercion in subtle ways.
- `On Error Resume Next` semantics must be implemented at the statement level, not the expression level.
- Late-bound member access (IDispatch) has complex overload resolution rules.

### 5.5 MLIR and Progressive Lowering

**Why multi-level IR matters for VBA:**

The traditional approach (AST → stack bytecode, or AST → Cranelift IR) creates a semantic gap. VBA has rich domain-specific semantics — COM dispatch conventions, implicit coercion matrices, and unstructured error handling — that are lost when lowered directly to a general-purpose representation. A general-purpose JIT compiler treats code as generic computation, unable to exploit knowledge of VBA's specific patterns.

Progressive lowering, as pioneered by the MLIR project (Lattner et al.), preserves domain semantics at each tier and performs targeted optimizations before committing to lower-level forms. OxVBA implements this concept as a Rust-native three-tier IR (VbaHir → VbaMir → CfgIr) rather than depending on the C++ MLIR framework itself.

**Key insight from the brainstorm:** By delaying the lowering of `On Error Resume Next` until CfgIr, and delaying the expansion of `For Each` and implicit coercions until VbaMir, the VbaHir tier can perform VBA-aware optimizations (coercion elimination, early-binding promotion, constant folding with Variant semantics) that would be impossible at lower tiers.

### 5.6 MMIX and Register-Window Architecture

**Why register-window over stack-based:**

Pure stack VMs incur heavy memory traffic. Every operation requires push/pop churn on the operand stack — even simple expressions like `a + b * c` require 5 stack operations. The MMIX architecture (Knuth, TAOCP) uses a sliding register window:

- Registers 0 to rL−1 are local to the current subroutine.
- Registers rG to 255 are global.
- Calls shift the register window; arguments live in an overlap region.
- Spills to memory happen only when call depth exceeds the physical register file capacity.

In OxVBA's Rust VM, this is emulated with:
- A contiguous register file (array of `Variant`).
- Window base/limit pointers (analogous to MMIX's rO/rS).
- Spill/fill logic for deep call trees.

Result: deep call trees execute with far fewer memory accesses. Most VBA procedure calls involve 0–10 local variables — these fit entirely within the register window, with arguments passed via the overlap region without any memory copies.

### 5.7 Broadword Algorithms (SWAR)

Instead of decoding opcodes byte-by-byte with branch-heavy dispatch tables, broadword (SWAR — SIMD Within A Register) techniques process 8 opcode bytes simultaneously in a single 64-bit word.

For detecting the presence of a target opcode byte `c` in a 64-bit word `x` containing 8 packed opcodes:

```
y = x XOR (c × 0x0101010101010101)
z = (y − 0x0101010101010101) AND (NOT y) AND 0x8080808080808080
```

If z ≠ 0, then `c` appears in `x`.

This enables scanning for Branch/Return/Error patterns in O(1) per 64-bit block, feeding the interpreter with better prefetch and fewer branch mispredictions. Critical for the interpreter fast path where branch prediction is the dominant performance bottleneck.

---

## 6. Design Notes

### 6.1 Parser Design

**Decision: Hand-written recursive descent parser with lossless CST.**

Alternatives considered:
- **Parser generators (LALR, PEG, etc.):** Rejected. VBA's grammar is context-sensitive (keywords as identifiers, line-oriented rules, preprocessor directives). Parser generators struggle with VBA's grammar and produce poor error messages. Error recovery in generated parsers is limited.
- **Tree-sitter:** Considered for IDE use cases but rejected as primary parser. Tree-sitter's C-based runtime doesn't align with pure-Rust goals. Could potentially use tree-sitter grammar as a secondary parser for editor integration in the future.
- **Nom/winnow (parser combinators):** Viable but less control over error messages and recovery than hand-written. For a language with VBA's complexity, explicit control is worth the implementation cost.

**Roslyn green/red tree pattern — enhanced specification:**

**Green tree (storage form):**
- Untyped: nodes carry `SyntaxKind` enum discriminants, not concrete Rust types.
- Strictly immutable and position-independent: no absolute offsets stored.
- Nodes contain only **relative width** (byte count of the subtree's text span).
- Structural sharing: identical subtrees are deduplicated. This is especially valuable for enterprise VBA modules where boilerplate patterns repeat extensively.
- Child sequences stored in tiered containers: `SmallVec<[GreenChild; 4]>` for typical small nodes, spilling to heap `Vec` for large nodes. (Finger-tree-inspired upgrade path reserved for when incremental reparsing demands O(log n) concat/split.)

**Red tree (typed facade):**
- Strongly typed API: each syntax node type has a concrete Rust struct with typed accessors.
- Computes **absolute position** on-demand by summing widths from root.
- Ephemeral wrappers: created on-demand, not persisted. Provides parent pointers, offset/span information, and ergonomic typed traversal.
- No memory overhead when not traversing: the green tree is the only persistent allocation.

Because the underlying structure is immutable, replacing a single node yields a new root that reuses the vast majority of the existing tree — near O(1) allocation for small edits and limited invalidation.

**Combinator-based rewriting:**

Transforms on the lossless syntax tree are expressed as pure functions `CST → CST`, composed using combinators:
- Sequential composition: apply transform A, then B
- Parallel alternatives: run N transforms, score results, select best
- Fixpoint iteration: apply transform until tree is unchanged (T_{k+1} = T_k)

Immutability of the green tree makes speculative, parallel rewrites safe and deterministic. This model supports macro expansion, code generation, and agent-driven patching without full reparses.

### 6.2 Bytecode and VM Design

**Decision: Custom register-based bytecode with MMIX-inspired register-window VM.**

This is the most significant architectural departure from the baseline plan, driven by the MACH-1000 analysis that pure stack VMs have an inherent memory-traffic ceiling.

**Register bytecode format:**

Instructions encode register operands explicitly:

```
ADD  r3, r1, r2       // r3 = r1 + r2 (Variant addition with coercion)
LOAD r1, const[5]     // r1 = constant pool entry 5
CALL r0, proc[3], 4   // call procedure 3 with 4 args starting at r0
```

Compared to stack-based:
```
PUSH r1
PUSH r2
ADD          // implicit: pop 2, push 1
```

The register format:
- Eliminates operand stack push/pop overhead.
- Makes liveness information explicit (registers, not implicit stack positions).
- Maps naturally to Cranelift's register-based IR for JIT compilation.
- Enables broadword scanning for opcode patterns (register operands are in fixed-width fields).

**Register-window VM architecture:**

```
┌────────────────────────────────────────────────────────────────┐
│                     Physical Register File                      │
│  ┌──────┬──────────────┬────────────┬──────────────┬────────┐  │
│  │ ...  │  Caller      │  Overlap   │  Callee      │  ...   │  │
│  │      │  locals      │  (args)    │  locals      │        │  │
│  │      │  r0..r7      │  r8..r11   │  r0..r5      │        │  │
│  └──────┴──────────────┴────────────┴──────────────┴────────┘  │
│          ▲ window_base              ▲ window_base (after call)  │
└────────────────────────────────────────────────────────────────┘
```

- **Window base pointer:** Each procedure frame has a window base. Register r0 in bytecode maps to `register_file[window_base + 0]`.
- **Overlap region:** When procedure A calls procedure B with 4 arguments, A places them in its highest registers. B's window base is set so that B's r0..r3 overlap with A's argument registers. Zero-copy argument passing.
- **Spill/fill:** When the call depth exceeds the physical register file size, the oldest frames are spilled to a spill stack (heap-allocated). On return, they are filled back. Typical VBA call depths (5–20 frames, 5–15 locals each) fit entirely in a 256-register file without spilling.
- **Global registers:** A configurable number of registers (e.g., r240–r255) are global — shared across all frames. Used for frequently accessed engine state (current `Err` object, `DoEvents` flag, etc.).

**Broadword-accelerated dispatch:**

The interpreter main loop uses SWAR techniques for hot-path optimization:
- Opcode + operand fields packed into fixed-width instruction words.
- Broadword scanning used to detect Branch/Return/Error sequences for prefetch hinting.
- Computed-goto dispatch (or match-based dispatch with profile-guided branch hints) for the main opcode switch.

**Zero-copy bytecode serialization:**

Bytecode modules are serialized using `rkyv` (zero-copy deserialization):
- On-disk layout matches in-memory layout exactly.
- Loading a bytecode module: `mmap` the file, validate bounds, cast to typed structures.
- No allocation-heavy decoding phase.
- Near-zero startup latency for large macro corpora.
- The serialized format includes: instruction stream, constant pool, string table, debug info, register allocation metadata.

**Instruction categories:**

| Category | Examples |
|---|---|
| Register operations | `Mov`, `LoadConst`, `LoadEmpty`, `LoadNull`, `LoadNothing` |
| Arithmetic | `Add`, `Sub`, `Mul`, `Div`, `IntDiv`, `Mod`, `Pow`, `Neg` |
| Comparison | `Eq`, `Ne`, `Lt`, `Gt`, `Le`, `Ge`, `Like`, `Is` |
| Logic | `And`, `Or`, `Not`, `Xor`, `Eqv`, `Imp` |
| String | `Concat`, `Mid`, `Len` (may also be built-in function calls) |
| Control flow | `Jump`, `JumpIf`, `JumpIfNot`, `GoSub`, `Return` |
| Calls | `Call`, `CallIndirect`, `CallLate` (IDispatch) |
| Objects | `NewObject`, `SetRef`, `Release`, `GetProp`, `PutProp`, `CallMethod` |
| Arrays | `NewArray`, `ReDim`, `Erase`, `ArrayGet`, `ArrayPut` |
| Error handling | `OnErrorGoTo`, `OnErrorResumeNext`, `OnErrorReset`, `Resume`, `Raise` |
| Conversion | `Coerce`, `CInt`, `CLng`, `CDbl`, etc. |
| Window | `WindowShift` (call), `WindowRestore` (return), `Spill`, `Fill` |

### 6.3 Cross-Platform Story

**Core principle:** The VBA language runtime and all built-in types work identically on all platforms. Platform differences are isolated to COM interaction and hosting.

HAL design note (current stage):
- Platform-sensitive behavior is being consolidated into a dedicated Host Abstraction Layer design track.
- The current HAL draft/spec set is in:
  - `docs/spec/HAL_DESIGN_DRAFT.md`
  - `docs/spec/HAL_INTERFACE_DRAFT.md`
  - `docs/spec/HAL_CONFORMANCE_DRAFT.md`
  - `docs/spec/HAL_PROFILE_MATRIX_DRAFT.md`
  - `docs/spec/HAL_SPEC_WORKING_DRAFT.md`
  - `docs/spec/HAL_SPEC_CROSSWALK.md`
  - `docs/spec/HAL_CONFORMANCE_SUITE.md`
- The model uses five explicit profiles (`windows`, `linux`, `macos`, `wasm`, `null`) and tracks both capability support and capability maturity.
- Current implementation decision: COM activation/dispatch is supported on Windows and explicitly unsupported on non-Windows profiles.

| Feature | Windows | Linux / macOS |
|---|---|---|
| VBA language core | Full | Full |
| Built-in functions (VBA.*) | Full | Full |
| Built-in objects (Collection, Dictionary, etc.) | Full | Full |
| COM object creation (CreateObject) | HAL capability supported | Explicitly unsupported (deterministic error) |
| Type library binding (early binding) | Full (via type libraries) | Deferred |
| Declare (DLL calls) | Full (LoadLibrary) | dlopen equivalent (best-effort) |
| Host-provided objects | Via COM hosting API | Via Rust hosting trait |
| Forms runtime (UserForm) | Native (via COM controls) | Portable rendering (future) |

### 6.4 Integration with DNA Calc

OxVBA is developed in the DNA Calc context, but repository responsibility is bounded: OxVBA provides a host-aware runtime API, while DNA Calc implements application-specific host integration on its side.

- **OxVBA provides:** VBA execution engine, module management, event dispatch, and host-aware registration surfaces.
- **Host provides:** root object graph (for example `Application`) plus additional objects (`Workbook`, `Worksheet`, `Range`, etc.) as COM objects (Windows) or trait implementations (cross-platform).
- **Interaction pattern:** host creates an OxVBA engine instance, registers root host objects, loads VBA project source, and triggers execution (event handlers, macro calls).
- **Object association:** Document modules (e.g., `Sheet1` code-behind) are associated with host objects through the hosting API. Events on host objects (e.g., `Worksheet_Change`) are dispatched to the corresponding VBA event handlers.
- **Mutation model:** VBA macro execution runs in exclusive mutation mode (per Foundation doctrine — no hidden mutation pathways). The host provides a structured operation interface; VBA code modifies the spreadsheet through host-mediated operations, not direct memory access.

Priority note:
- Host-awareness is part of initial OxVBA focus.
- Full DNA Calc application wiring is implemented as DNA Calc-side work, not as a hard dependency for OxVBA phase completion.

### 6.5 Development Innovation

Following the DNA Calc Foundation doctrine, OxVBA aims to innovate in the development process:

- **Observation-driven development:** Systematically observe Office VBA behavior, capture as evidence artifacts, implement against observations, verify conformance.
- **Decision-table-driven implementation:** Type coercion and arithmetic implemented directly from exhaustive decision tables, not from narrative specification text.
- **Formally grounded:** Lean 4 specifications for core properties provide machine-checkable assurance that the type system is coherent. Kani proofs for unsafe code provide bounded correctness guarantees.
- **Regression-as-asset:** Every bug discovered becomes a minimized test case in the conformance corpus (per Foundation Hygiene Doctrine).
- **Documentation as we go:** The development path, decisions, trade-offs, and discoveries are documented contemporaneously, not after the fact.

---

## 7. Proposed Project Structure

### 7.1 Directory Layout

```
OxVba/
├── MACH1000_PLAN.md                    # Historical synthesis and vision context
├── CHARTER.md                          # Project mission, scope, and values (authoritative charter)
├── OPERATIONS.md                       # Lightweight execution and development doctrine
├── README.md                           # Project overview, build instructions
├── LICENSE.md                          # MIT license
├── AGENTS.md                           # Execution doctrine for AI agents
├── Cargo.toml                          # Workspace root
├── .gitignore
│
├── synthesis/                          # Synthesis run artifacts
│   ├── README.md                       # Synthesis process documentation
│   └── runs/
│       ├── 20260226-mach1000-synthesis/
│           ├── README.md
│           ├── inputs/
│           ├── analysis/
│           ├── decisions/
│           ├── outputs/
│           └── logs/
│       └── 20260226-mach1000-refinement-synthesis/
│           ├── README.md
│           ├── inputs/
│           ├── analysis/
│           ├── decisions/
│           ├── outputs/
│           └── logs/
│
├── crates/
│   ├── oxvba-syntax/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public API: parse, SyntaxTree, SyntaxKind
│   │       ├── lexer.rs                # Tokenizer
│   │       ├── parser.rs               # Recursive descent parser
│   │       ├── syntax_kind.rs          # Token and node kinds enum
│   │       ├── green.rs                # Green tree (immutable CST nodes)
│   │       └── red.rs                  # Red tree (typed facade wrappers)
│   │
│   ├── oxvba-ir/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public API: VbaHir, VbaMir, CfgIr
│   │       ├── hir.rs                  # VBA High-Level IR definitions
│   │       ├── mir.rs                  # VBA Mid-Level IR definitions
│   │       ├── cfg.rs                  # Control-Flow Graph IR (SSA form)
│   │       ├── lower_hir_to_mir.rs     # VbaHir → VbaMir lowering
│   │       ├── lower_mir_to_cfg.rs     # VbaMir → CfgIr lowering
│   │       ├── opt_hir.rs              # VbaHir optimization passes
│   │       ├── opt_mir.rs              # VbaMir optimization passes
│   │       └── opt_cfg.rs              # CfgIr optimization passes
│   │
│   ├── oxvba-runtime/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public API: Variant, VarType, coerce, builtins
│   │       ├── variant.rs              # COM-compatible Variant (`VARIANT`) representation
│   │       ├── coerce.rs               # Type coercion logic (driven by decision tables)
│   │       ├── arithmetic.rs           # Variant arithmetic (driven by decision tables)
│   │       ├── bstr.rs                 # VBA string type (BSTR-compatible)
│   │       ├── safe_array.rs           # SAFEARRAY-compatible array type
│   │       ├── decimal.rs              # 96-bit Decimal type
│   │       ├── builtins.rs             # Built-in VBA functions (VBA.Strings, VBA.Math, etc.)
│   │       └── alloc.rs                # Boundary-tag allocator for VBA heap objects
│   │
│   ├── oxvba-compiler/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public API: compile, Module, Bytecode
│   │       ├── resolve.rs              # Name resolution
│   │       ├── typecheck.rs            # Type checking and coercion insertion
│   │       ├── lower_to_hir.rs         # Bound CST → VbaHir lowering
│   │       ├── emit.rs                 # CfgIr → register bytecode emission
│   │       └── bytecode.rs             # Bytecode format definition (rkyv-serializable)
│   │
│   ├── oxvba-vm/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public API: Vm, execute
│   │       ├── interpreter.rs          # Register-window interpreter loop
│   │       ├── register_file.rs        # Register file and window management
│   │       ├── broadword.rs            # SWAR instruction decoding utilities
│   │       └── error_state.rs          # On Error state machine
│   │
│   ├── oxvba-jit/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public API: JitEngine, compile_function
│   │       └── cranelift.rs            # CfgIr → CLIF translation
│   │
│   ├── oxvba-com/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public API: ComObject, Dispatch, IUnknown traits
│   │       ├── refcount.rs             # Reference counting (AddRef/Release)
│   │       ├── dispatch.rs             # IDispatch abstraction
│   │       ├── cycle_gc.rs             # Bacon-Rajan cycle detector (epoch-batched)
│   │       └── platform/
│   │           ├── mod.rs
│   │           ├── windows.rs          # Real COM via `windows` crate
│   │           └── portable.rs         # Trait-based COM on non-Windows
│   │
│   ├── oxvba-host/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Public API: Engine, HostConfig, Project
│   │       ├── engine.rs               # Engine lifecycle and orchestration
│   │       ├── project.rs              # VBA project (modules, references, metadata)
│   │       └── events.rs               # Event dispatch (host events → VBA handlers)
│   │
│   └── oxvba-cli/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs                 # CLI entry point: run .bas files, REPL
│
├── formal/
│   ├── lean/
│   │   ├── lakefile.lean               # Lean 4 build file
│   │   ├── lean-toolchain              # Lean 4 toolchain version
│   │   └── OxVba/
│   │       ├── VarType.lean            # VarType inductive definition
│   │       ├── Coerce.lean             # Coercion relation and proofs
│   │       ├── Arithmetic.lean         # Operator result type proofs
│   │       └── RefCount.lean           # Refcount reachability invariant
│   └── kani/
│       └── README.md                   # Kani harness inventory and instructions
│
├── tables/
│   ├── coercion.csv                    # Type coercion decision table
│   ├── arithmetic.csv                  # Arithmetic result type decision table
│   └── comparison.csv                  # Comparison semantics table
│
├── conformance/
│   ├── harness/                        # Office VBA observation harness
│   │   └── ...                         # VBA project files for running in Office
│   ├── golden/                         # Golden output files from Office VBA
│   │   └── ...                         # Structured output (JSON/CSV)
│   └── tests/                          # VBA source files for conformance testing
│       └── ...                         # .bas / .cls files
│
├── docs/
│   ├── README.md                       # Documentation index
│   ├── archive/
│   │   ├── README.md                   # Archive index (superseded documents)
│   │   ├── PLAN_v1_20260226.md         # Original baseline plan (superseded)
│   │   └── BRAINSTORM_MACH1000_20260226.md  # MACH-1000 brainstorm (consumed by synthesis)
│   ├── ARCHITECTURE.md                 # Detailed architecture document
│   ├── BUILDING.md                     # Build and development setup
│   ├── CONTRIBUTING.md                 # Contribution guidelines
│   ├── MACH1000_PLAN_REFINEMENT_20260226.md  # Refinement proposal input for synthesis
│   ├── spec/                           # Early-stage + normative spec docs
│   │   ├── README.md                   # Spec-draft index and maturity states
│   │   ├── HAL_DESIGN_DRAFT.md         # HAL scope/principles/profile plan
│   │   ├── HAL_INTERFACE_DRAFT.md      # HAL contracts + capability/maturity model
│   │   ├── HAL_CONFORMANCE_DRAFT.md    # HAL conformance model and gates
│   │   ├── HAL_PROFILE_MATRIX_DRAFT.md # Per-profile capability planning matrix
│   │   ├── HAL_SPEC_WORKING_DRAFT.md   # Implementation-linked HAL contract and policy semantics
│   │   ├── HAL_SPEC_CROSSWALK.md       # HAL-to-Foundation spec anchor mapping
│   │   ├── HAL_CONFORMANCE_SUITE.md    # Runnable HAL conformance lanes and artifact model
│   │   ├── PROJECT_MODULE_REFERENCE_SPEC_V1.md
│   │   ├── PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md
│   │   ├── PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md
│   │   └── PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md
│   ├── VARIANT_DESIGN.md               # VARIANT layout and optional internal-repr optimization notes
│   ├── COM_ABSTRACTION.md              # COM layer design
│   ├── BYTECODE_FORMAT.md              # Register bytecode instruction set reference
│   ├── IR_DESIGN.md                    # Multi-level IR design (VbaHir/VbaMir/CfgIr)
│   ├── VM_ARCHITECTURE.md              # Register-window VM and broadword dispatch
│   └── evidence/                       # Clean-room evidence records
│       └── ...
│
└── scripts/
    └── ...                            # Build, CI, and development scripts
```

### 7.2 Workspace Cargo.toml (preliminary)

```toml
[workspace]
members = [
    "crates/oxvba-syntax",
    "crates/oxvba-ir",
    "crates/oxvba-runtime",
    "crates/oxvba-compiler",
    "crates/oxvba-vm",
    "crates/oxvba-jit",
    "crates/oxvba-com",
    "crates/oxvba-host",
    "crates/oxvba-cli",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
authors = ["DNA Kode"]
repository = "https://github.com/DnaCalc/OxVba"

[workspace.dependencies]
oxvba-syntax = { path = "crates/oxvba-syntax" }
oxvba-ir = { path = "crates/oxvba-ir" }
oxvba-runtime = { path = "crates/oxvba-runtime" }
oxvba-compiler = { path = "crates/oxvba-compiler" }
oxvba-vm = { path = "crates/oxvba-vm" }
oxvba-jit = { path = "crates/oxvba-jit" }
oxvba-com = { path = "crates/oxvba-com" }
oxvba-host = { path = "crates/oxvba-host" }
rkyv = { version = "0.8", features = ["validation"] }
thiserror = "2"
proptest = "1"
```

---

## 8. Implementation Sequencing

This sequence follows Foundation operations discipline: dependency closure first, measurable obligations, and evidence-backed stabilization claims.

### 8.1 Phase Metadata Model

Each phase records:
- **Primary owner track** (`Red`, `Green`, `Logistics`)
- **Estimated duration**
- **Dependencies**
- **Parallelizable tracks**
- **Quantitative gate (Definition of Done)**

### 8.2 Execution Rules

- **MVP-first:** establish a thin end-to-end slice early, before full optimization architecture is enabled by default.
- **Feature-flagged risk:** high-risk performance paths ship behind explicit flags until correctness gates are green.
- **Quantitative milestones:** each phase has measurable gates (coverage/pass-rate/divergence/perf).
- **Evidence discipline:** compatibility claims require reproducible harness outputs and recorded evidence.
- **Recalc mindset:** plan updates treat edits as dirty-marking events that trigger dependency closure and gate updates.

### 8.3 Compatibility Matrix Gates (iterative)

Initial gate dimensions (to be expanded continuously):
- Reference runtime: Office VBA 7.0 and 7.1+
- Architecture: 32-bit and 64-bit behavior-sensitive cases
- Execution backend: VM / JIT / AOT backend
- Platform class: Windows (full COM) and Linux/macOS (core + hosted abstractions)

Initial policy:
- Each phase that changes semantics must add at least one matrix-backed conformance case.
- Matrix breadth grows over time; this is a progressive gate, not a one-shot end gate.

### 8.4 Risk Register (living)

| ID | Risk | Trigger signal | Mitigation | Owner |
|---|---|---|---|---|
| R-001 | Internal Variant layout diverges from COM boundary behavior | Differential failures at COM boundary tests | Keep explicit conversion layer and boundary-specific conformance pack | Red + Green |
| R-002 | `On Error Resume Next` lowering regresses semantics | Err-state divergence in guarded-region corpus | Preserve staged lowering with dedicated semantic-preservation tests | Red |
| R-003 | ByRef/default-property edge cases miscompile | Mismatch in argument mutation or property dispatch tests | Build focused decision-table and conformance corpus for these edges | Red + Green |
| R-004 | Cycle detection or RC lifecycle breaks deterministic destruction | Non-deterministic `Class_Terminate` behavior | Keep cycle GC opt-in and epoch-batched; test deterministic RC path as default | Red |
| R-005 | Broadword/register-window optimizations cause correctness regressions | Flag-on vs flag-off output mismatch | Keep optimizations behind flags until parity gates pass | Red |
| R-006 | Plan drift from Foundation evidence discipline | Claims without linked artifacts | Require synthesis/decision logs for doctrine-impacting plan changes | Logistics + Green |

### 8.5 Phase Plan

### Phase 0: Project Bootstrap and Gate Infrastructure
- Primary owner track: Logistics + Red + Green
- Estimated duration: 1-2 weeks
- Dependencies: none
- Parallelizable tracks: CI wiring, workspace scaffolding, initial formal skeleton
- Work:
- Initialize repository, Cargo workspace, CI pipeline.
- Write CLAUDE.md, AGENTS.md, README.md, LICENSE.
- Set up `cargo fmt`, `cargo clippy`, `cargo miri`, Kani in CI.
- Create all 9 crate stubs (compiling, empty).
- Initial Lean 4 project skeleton and `formal/kani/` README.
- Quantitative gate:
- `cargo check` green for all crates.
- CI runs fmt + clippy + unit tests + miri on at least one target.

### Phase 1: Lexer and Parser (`oxvba-syntax`)
- Primary owner track: Red
- Estimated duration: 3-5 weeks
- Dependencies: Phase 0
- Parallelizable tracks: grammar corpus collection, error-recovery fixtures
- Work:
- Implement lexer with full VBA 7 token set.
- Implement recursive descent parser with lossless CST.
- Green tree with `SmallVec`-based child storage and structural sharing.
- Red tree with ephemeral typed wrappers and on-demand absolute positioning.
- Handle context-sensitive keywords, line continuation, conditional compilation.
- Error recovery: parser always produces a tree.
- Quantitative gate:
- Parse corpus of at least 1,000 real-world modules with zero crashes/panics.
- Roundtrip property tests pass for well-formed corpus slice.

### Phase 2: Core Runtime Types (`oxvba-runtime`)
- Primary owner track: Red + Green
- Estimated duration: 4-6 weeks
- Dependencies: Phase 0
- Parallelizable tracks: observation harness, decision table generation, Lean specs
- Work:
- Implement COM `VARIANT`-compatible `Variant`, coercion/arithmetic tables, `BStr`, `SafeArray`, `Decimal`.
- Implement boundary-tag allocator for VBA heap objects.
- Build observation harness and generate initial decision tables from Office VBA.
- Lean 4: formalize `VarType` and `Coerce`.
- Kani: COM `VARIANT` field/union invariants and boundary marshalling harnesses.
- Quantitative gate:
- 100% filled cells for initial coercion/arithmetic decision tables in scope.
- Kani harnesses for Variant layout/marshalling pass in CI.

### Phase 3: End-to-End MVP Vertical Slice
- Primary owner track: Red
- Estimated duration: 2-4 weeks
- Dependencies: Phases 1-2
- Parallelizable tracks: minimal conformance corpus authoring
- Work:
- Build a thin compile-and-run path (parser -> binding -> minimal bytecode -> execution).
- Support essential statements, arithmetic, and control flow for a small executable subset.
- Quantitative gate:
- At least 50 MVP conformance programs execute end-to-end.
- At least 85% pass rate on MVP corpus with all divergences documented.

### Phase 4: Multi-Level IR Core (`oxvba-ir`)
- Primary owner track: Red
- Estimated duration: 4-6 weeks
- Dependencies: Phases 2-3
- Parallelizable tracks: lowering property tests, IR debug tooling
- Work:
- Define VbaHir, VbaMir, and CfgIr (SSA).
- Implement staged lowering and guarded-region modeling for `On Error Resume Next`.
- Add initial VbaHir optimization passes.
- Quantitative gate:
- Semantic-preservation suite for lowering passes shows zero unexpected divergences on targeted corpus.

### Phase 5: Compiler Core (`oxvba-compiler`)
- Primary owner track: Red
- Estimated duration: 3-5 weeks
- Dependencies: Phase 4
- Parallelizable tracks: bytecode format validation tools
- Work:
- Semantic analysis: resolution, type checking, lowering to VbaHir.
- CfgIr -> register bytecode emission.
- `rkyv`-serializable bytecode format.
- Quantitative gate:
- Compile success for at least 90% of MVP corpus modules in scope.
- Bytecode roundtrip serialization tests pass with validation enabled.

### Phase 6: VM Correctness Baseline (`oxvba-vm`)
- Primary owner track: Red
- Estimated duration: 4-6 weeks
- Dependencies: Phase 5
- Parallelizable tracks: error-state corpus, register-window safety harnesses
- Work:
- Implement register-window interpreter and error handling state machine.
- Implement ByRef register overlap, GoSub/Return, built-in dispatch.
- Quantitative gate:
- Core VM conformance suite (minimum 200 tests) reaches at least 95% pass rate.
- Kani register-window bounds harnesses pass.

### Phase 7: COM/Object System + Host-Aware API (`oxvba-com`, `oxvba-host`)
- Primary owner track: Red
- Estimated duration: 4-7 weeks
- Dependencies: Phase 6
- Parallelizable tracks: Windows COM adapters and portable trait adapters
- Work:
- Reference counting, `IUnknown`/`IDispatch` abstractions, class module lifecycle.
- Collection/Dictionary built-ins and Windows COM integration.
- Engine lifecycle, host object registration, event dispatch, project management.
- Host-aware runtime initialization with root-object injection (`Application`, etc.).
- Quantitative gate:
- Object-lifecycle corpus verifies deterministic destruction behavior.
- Host integration tests cover root-object injection and event dispatch scenarios.

### Phase 8: Forms Runtime Core
- Primary owner track: Red
- Estimated duration: 3-6 weeks
- Dependencies: Phase 7
- Parallelizable tracks: control/event conformance fixtures
- Work:
- Implement UserForm runtime behaviors and control/event wiring in Rust runtime layer.
- Integrate with host abstraction boundaries.
- Quantitative gate:
- Forms runtime suite covers creation, events, and control interaction paths in scope.

### Phase 9: JIT + AOT Backend Capability (`oxvba-jit`)
- Primary owner track: Red
- Estimated duration: 3-5 weeks
- Dependencies: Phases 5-6
- Parallelizable tracks: parity benchmarking and IR translation instrumentation
- Work:
- CfgIr -> CLIF translation and per-function JIT.
- Runtime switching between VM and JIT.
- **AOT backend capability:** compiler/runtime-level native artifact emission.
- Quantitative gate:
- JIT and AOT backend outputs are semantically identical to VM for targeted corpus.

### Phase 10: CLI and Standalone Packaging (`oxvba-cli`)
- Primary owner track: Red + Logistics
- Estimated duration: 2-4 weeks
- Dependencies: Phases 6 and 9
- Parallelizable tracks: packaging scripts and smoke-test automation
- Work:
- CLI execution (`run`) and REPL surfaces.
- **AOT packaging:** produce standalone deliverables from AOT backend artifacts.
- Quantitative gate:
- `oxvba run program.bas` passes smoke suite.
- CLI packaging flow validated on supported target environments.

### Phase 11: Optimization Push and Feature-Flag Graduation
- Primary owner track: Red
- Estimated duration: 4-8 weeks
- Dependencies: Phases 6-10
- Parallelizable tracks: benchmark design and optimization pass tuning
- Work:
- VbaHir/VbaMir/CfgIr optimization expansion.
- Feature-flagged performance paths:
- `mach_broadword_dispatch`
- `mach_zero_copy_bytecode`
- advanced register-window heuristics
- Promotion criteria from experimental to default:
- semantic parity against baseline backend,
- no new UB findings in Miri/Kani lanes,
- measurable benchmark gain.
- Quantitative gate:
- Demonstrate measurable speedup over baseline VM on representative benchmark corpus.

### Phase 12: Conformance and Stabilization
- Primary owner track: Green + Red + Logistics
- Estimated duration: 4-10 weeks (iterative)
- Status: complete for profile scope `mvp-perf-shape-v26` (gate passed on 2026-02-27)
- Gate evidence:
  - `docs/evidence/profiles/v26/matrix_latest.csv`
  - `docs/evidence/profiles/v26/gate_report.md`
  - `docs/evidence/formal/latest_run.md`
  - `docs/evidence/profiles/v26/benchmark_latest.md`
  - `docs/evidence/divergences/README.md`
- Dependencies: all prior phases
- Parallelizable tracks: matrix expansion, divergence triage, documentation finalization
- Work:
- Expand conformance corpus and compatibility matrix breadth.
- Close or document divergences.
- Finalize operational and architecture documentation.
- Quantitative gate:
- Required matrix cells for declared profile scope are green.
- Remaining divergences are explicitly documented with evidence records.

### Phase 13: Full Typing Semantics Closure (Post-v66 Ladder)
- Primary owner track: Red + Green
- Estimated duration: 6-12 weeks (iterative)
- Dependencies: Phase 12 stabilization baseline (`v66`) and existing formal async infrastructure
- Planned profile ladder: `v67..v86`
  - Canonical ladder doc: `docs/worksets/PROFILE_LADDER_2026-02-28_MACH1000_V67_V86_TYPING.md`
- Work:
- Complete full internal VBA typing semantics in scope:
  - declared types, default type rules (`Def*`), type characters,
  - full `Option Explicit` diagnostics and declaration checks,
  - assignment/argument/operator coercion and conversion conformance,
  - full string semantics in declared scope,
  - typed and `Variant` array semantics including non-zero lower bounds and multi-dimensions,
  - early/late binding interaction under typed call sites.
- Formal approach:
- Maintain `F3` profile obligations and run strict Kani as async long-running jobs.
- Use deferred formal gates for non-blocking profile progression while async runs are active.
- Track and reconcile deferred formal runs via:
  - `docs/evidence/formal/DEFERRED_GATES.md`
  - `docs/evidence/formal/latest_run.md`
- Quantitative gate:
- Required type/coercion/string/array matrix cells for `v86` scope are green.
- Deferred formal gates are reconciled (`dg-folded`) or explicitly documented with unblock steps.

### Future (not sequenced):
- Forms Designer.
- Debugging protocol.
- IDE support (language server).
- Additional COM library compatibility on non-Windows.
- Finger-tree child storage for incremental reparsing (when demand materializes).
- Verus/Creusot integration for deductive verification of IR lowering.

---

*This document is the historical MACH-1000 synthesis for OxVBA, produced by formal synthesis of the baseline plan and theoretical architecture brainstorm, then refined by a follow-up synthesis pass. It captures project vision, architectural options, advanced performance engineering goals, formal verification strategy, and implementation sequencing context. Current implementation truth lives in `docs/ARCHITECTURE.md`, active worksets, status files, and evidence artifacts.*
