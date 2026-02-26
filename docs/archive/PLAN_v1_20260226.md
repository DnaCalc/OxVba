# OxVBA Project Plan

## Table of Contents

1. [Project Charter](#1-project-charter)
2. [Architecture](#2-architecture)
3. [Formal Approach](#3-formal-approach)
4. [Testing Strategy](#4-testing-strategy)
5. [Research Notes](#5-research-notes)
6. [Brainstorming Notes](#6-brainstorming-notes)
7. [Proposed Project Structure](#7-proposed-project-structure)
8. [Implementation Sequencing](#8-implementation-sequencing)

---

## 1. Project Charter

### 1.1 Mission

OxVBA is a full-fidelity implementation of the VBA 7 runtime engine written in Rust. It targets parsing, compilation, and runtime execution of VBA source code with correctness, performance, and cross-platform reach that exceed what the Office-bundled VBA engine provides.

OxVBA is developed by **DNA Kode** as part of the **DNA Calc** ecosystem. It is intended to be consumed by the DNA Calc spreadsheet system (developed in `../Foundation`) but operates as a standalone project with its own charter, sharing values, methodology, and operational guidance with the broader DNA Calc program.

- **License**: MIT
- **Organization**: DNA Kode
- **Repository**: `github.com/DnaCalc/OxVba`

### 1.2 Values Ordering

Values are listed from most important to least important. When values conflict, higher-ranked values prevail.

1. **Robustness** — No surprises, no crashes, no undefined behavior. The engine must have a rock-solid feel. Every state is well-defined; every error path is handled.
2. **Compatibility** — Any unintended or undocumented incompatibility versus VBA in Office is a high-priority bug. The reference behavior is VBA 7.0/7.1 as shipped in Office.
3. **Performance** — Exceptional runtime performance through state-of-the-art algorithms, techniques, and implementation approaches. We aim to exceed Office VBA's execution speed.
4. **Small runtime size** — Distribution should never be an issue. Small likely means faster, but in the trade-off we pick faster over smaller by a clear margin.
5. **Well-managed development environment** — The full development stack must be open-source. Setting up a development environment and rebuilding all artifacts must be well-documented and unproblematic. We prefer tooling that makes this possible, but not at the cost of higher values.

### 1.3 Scope

**In scope (initial focus):**
- VBA 7 language parser (full grammar, lossless concrete syntax tree)
- Compilation to bytecode and/or native code
- Runtime execution engine (stack machine VM, optional JIT)
- Full VBA/COM reference counting semantics
- Opt-in cycle-detecting garbage collector (one of few beyond-VBA features)
- Compilation to executable format (native or IL) without excessive dependencies (no shipping LLVM)
- Cross-platform core: language and basic libraries work on Windows, Linux, macOS
- Full COM compatibility on Windows
- Hosting interfaces: in-process hosting with host COM hookups and non-COM method exposure
- Event and object association (e.g., sheet code-behind in Excel-like hosts)
- Forms runtime including support for custom controls

**In scope (listed, not currently active):**
- Runtime security model
- Debugging protocol and interfaces
- IDE features (IntelliSense, go-to-definition, etc.)
- Forms Designer
- COM library interop on non-Windows platforms (abstraction layer exists but full story deferred)

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
- **[MS-COM]** — Component Object Model Plus (COM+) Protocol. Underlying object model.
- **VBA 7.0** (Office 2010) and **VBA 7.1** (Office 2013+) as the target runtime versions.
- DNA Calc Foundation Charter, Operations, and Architecture documents for methodology and doctrine.

### 1.6 Why Rust

| Concern | Rust's answer |
|---|---|
| Robustness (#1 value) | Memory safety without GC; no undefined behavior in safe code; algebraic types make illegal states unrepresentable |
| Performance (#3 value) | Zero-cost abstractions; no runtime overhead; competitive with C/C++ |
| Small runtime (#4 value) | No managed runtime to ship; static linking; minimal binary sizes achievable |
| COM interop | Excellent `windows` crate ecosystem; `repr(C)` for ABI-compatible types; raw pointer support where needed |
| Cross-platform | First-class support for Windows, Linux, macOS; conditional compilation for platform-specific code |
| Ecosystem alignment | Sibling project DnaVisiCalc is Rust; shared tooling, conventions, and developer knowledge |
| Cranelift availability | Cranelift JIT backend is a Rust-native project; tight integration without FFI overhead |

---

## 2. Architecture

### 2.1 Crate Decomposition

OxVBA is organized as a Cargo workspace with eight crates, each with a clear responsibility boundary.

```
oxvba (workspace root)
├── crates/
│   ├── oxvba-syntax        # Lexer, parser, concrete syntax tree
│   ├── oxvba-compiler       # Semantic analysis, type checking, bytecode emission
│   ├── oxvba-runtime        # Variant type, type coercion, built-in functions
│   ├── oxvba-vm             # Stack-based bytecode virtual machine
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
oxvba-compiler        ← oxvba-syntax, oxvba-runtime
    │
    ▼
oxvba-vm              ← oxvba-compiler, oxvba-runtime, oxvba-com
oxvba-jit             ← oxvba-compiler, oxvba-runtime, oxvba-com, cranelift-*
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
- **`oxvba-runtime` is dependency-free** — the Variant type and coercion logic are foundational; everything else builds on them.
- **`oxvba-vm` and `oxvba-jit` are peers** — either can execute compiled bytecode; the host selects which backend to use. The VM is always available; the JIT is opt-in.
- **`oxvba-host` is the integration facade** — external consumers (DNA Calc, standalone CLI) interact through host, never directly with VM or JIT.

### 2.2 Compilation Pipeline

The pipeline mirrors the conceptual model of Office VBA (Source → P-code → ExCode) but with modern compiler engineering.

```
┌──────────┐    ┌──────────┐    ┌───────────────┐    ┌──────────────┐    ┌───────────┐
│  Source   │───▶│  Lexer   │───▶│    Parser      │───▶│   Semantic    │───▶│  Bytecode │
│  (.bas,   │    │ (tokens) │    │ (lossless CST) │    │   Analysis    │    │  Emission │
│   .cls,   │    └──────────┘    └───────────────┘    │ (typed AST)   │    │ (OxVBA BC)│
│   .frm)   │                                         └──────────────┘    └─────┬─────┘
└──────────┘                                                                    │
                                                                    ┌───────────┴───────────┐
                                                                    │                       │
                                                              ┌─────▼─────┐          ┌─────▼─────┐
                                                              │  VM exec  │          │  JIT exec │
                                                              │ (default) │          │ (opt-in)  │
                                                              │ stack VM  │          │ Cranelift │
                                                              └───────────┘          └───────────┘
```

**Stage 1: Lexing** (`oxvba-syntax`)
- Tokenizes VBA source into a token stream.
- Handles VBA's line-continuation (`_`), line-oriented statements, and context-sensitive keywords.
- Preserves trivia (whitespace, comments) for lossless round-tripping.

**Stage 2: Parsing** (`oxvba-syntax`)
- Hand-written recursive descent parser producing a lossless concrete syntax tree (CST).
- Adopts the Roslyn green/red tree pattern: immutable green nodes (syntax data) with on-demand red wrappers (parent pointers, absolute positions).
- Full error recovery: always produces a tree, even for malformed input. Errors are attached to nodes, not thrown.
- Rationale: Hand-written over parser generators for full control over error recovery and error messages (serves Robustness value). Lossless CST enables future IDE tooling without reparsing.

**Stage 3: Semantic Analysis** (`oxvba-compiler`)
- Name resolution (modules, procedures, variables, types, COM references).
- Type checking with VBA's implicit coercion rules.
- Binding of late-bound (IDispatch) vs. early-bound (vtable) calls.
- Resolution of `ByRef` (default) vs. `ByVal` parameter passing.
- Produces a typed AST / HIR suitable for code generation.

**Stage 4: Bytecode Emission** (`oxvba-compiler`)
- Emits a custom stack-based bytecode format (OxVBA bytecode).
- Stack-based design chosen for faithfulness to VBA's evaluation model and simplicity of implementation.
- Bytecode is serializable: supports ahead-of-time compilation and caching.
- Instruction set includes: value operations, arithmetic/comparison with Variant semantics, control flow (GoSub/Return, On Error, For/ForEach/Do/While), procedure calls (ByRef/ByVal), COM dispatch, array operations.

**Stage 5: Execution** (`oxvba-vm` or `oxvba-jit`)
- **VM (default):** Interprets OxVBA bytecode directly. Always available, no platform-specific dependencies. Primary execution mode.
- **JIT (opt-in):** Translates OxVBA bytecode to native code via Cranelift. Suitable for hot paths and performance-critical workloads. Cranelift is a Rust-native code generator — no LLVM dependency.

### 2.3 Key Types: Variant

The `Variant` type is the most performance-critical data structure in the entire engine. Every VBA value passes through Variant. Its representation must balance memory efficiency, type-dispatch speed, and COM ABI compatibility.

**Design: 24-byte `repr(C)` tagged union**

```rust
#[repr(C)]
pub struct Variant {
    vtype: VarType,     // u16 discriminant (COM VARENUM compatible)
    _reserved: [u8; 6], // padding / flags (ByRef, Array indicators)
    payload: [u8; 16],  // type-specific data
}
```

**Supported variant types:**

| VarType | Rust payload | VBA type | Size in payload |
|---|---|---|---|
| `Empty` | (none) | uninitialized | 0 |
| `Null` | (none) | Null | 0 |
| `Integer` | `i16` | Integer | 2 |
| `Long` | `i32` | Long | 4 |
| `Single` | `f32` | Single | 4 |
| `Double` | `f64` | Double | 8 |
| `Currency` | `i64` | Currency (scaled) | 8 |
| `Date` | `f64` | Date (OLE) | 8 |
| `String` | `*mut BStr` | String (BSTR) | ptr |
| `Object` | `*mut ComObject` | Object | ptr |
| `Error` | `i32` | Error code | 4 |
| `Boolean` | `i16` | Boolean (0/-1) | 2 |
| `Decimal` | `[u8; 16]` | Decimal (96-bit) | 16 |
| `Byte` | `u8` | Byte | 1 |
| `LongLong` | `i64` | LongLong (64-bit) | 8 |
| `LongPtr` | `isize` | LongPtr (ptr-sized) | ptr |
| `Array` | `*mut SafeArray` | Array (SAFEARRAY) | ptr |
| `ByRef` | `*mut Variant` | ByRef wrapper | ptr |

Design rationale:
- 24 bytes fits in 3 cache lines on most architectures and allows `Decimal` (the largest inline value at 16 bytes) to be stored without heap allocation.
- `repr(C)` ensures ABI compatibility with COM VARIANT on Windows.
- The `vtype` field uses COM-compatible `VARENUM` discriminant values for zero-cost interop.

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
- The cycle detector runs periodically or on-demand, not continuously — avoids interfering with deterministic `Class_Terminate` ordering when cycles are absent.
- This is one of the few intentional beyond-VBA features: Office VBA leaks cycles silently; OxVBA can optionally detect and collect them.

**Invariants:**
- Reference counts are always non-negative.
- An object with refcount 0 is immediately destroyed (deterministic).
- The cycle detector only collects objects that are unreachable from any root — it never destroys objects that are still reachable.

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

---

## 3. Formal Approach

OxVBA uses a two-pronged formal specification strategy: exhaustive decision tables for finite combinatorial properties, and Lean 4 machine-checkable specifications for structural and inductive properties of the type system.

### 3.1 Decision Tables

Decision tables specify the observable behavior of VBA's type system and arithmetic operations as exhaustive, machine-readable matrices.

**Type coercion table (~20 x 20):**
- Rows: source VarType
- Columns: target VarType
- Cells: coercion result (success with target type, or specific error code)
- Validated against Office VBA observation harness

**Arithmetic result type table (~20 x 20 x 15):**
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

### 3.3 Error Handling State Machine

The VBA error handling model is specified as a finite state machine:

```
States: { Default, HandlerActive, ResumeNext, InHandler, Exiting }
Inputs: { OnErrorGoTo, OnErrorResumeNext, OnErrorGoTo0,
          RuntimeError, Resume, ResumeNext, ExitProcedure }
```

Transitions and observable effects are specified as a state transition table, validated against Office VBA behavior.

---

## 4. Testing Strategy

### 4.1 Three-Tier Testing

**Tier 1: Unit tests** (per-crate, `cargo test`)
- Standard Rust unit tests for each crate's internal logic.
- Parser tests: token streams, CST shapes, error recovery.
- Variant tests: type coercion, arithmetic, comparison.
- VM tests: instruction execution, stack behavior, control flow.
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

### 4.3 Miri

`cargo miri test` is run in CI to detect undefined behavior in unsafe code:
- Reference counting operations (pointer manipulation, aliasing).
- Variant payload access (union-like access patterns).
- COM vtable dispatch (raw pointer casts, function pointer calls).
- FFI boundary operations (BStr, SafeArray).

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

**Verdict:** Cranelift aligns with values #4 (small runtime) and #5 (well-managed dev env) while providing sufficient code quality for value #3 (performance). LLVM would provide better peak optimization but at unacceptable cost to binary size and build complexity.

**Cranelift integration approach:**
- OxVBA bytecode → Cranelift IR (CLIF) translation.
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

Key complexity areas identified:
- The coercion rules (Section 2.1.3) are extensive and have many special cases.
- `ByRef` semantics interact with default properties and type coercion in subtle ways.
- `On Error Resume Next` semantics must be implemented at the statement level, not the expression level.
- Late-bound member access (IDispatch) has complex overload resolution rules.

---

## 6. Brainstorming Notes

### 6.1 Parser Choice

**Decision: Hand-written recursive descent parser with lossless CST.**

Alternatives considered:
- **Parser generators (LALR, PEG, etc.):** Rejected. VBA's grammar is context-sensitive (keywords as identifiers, line-oriented rules, preprocessor directives). Parser generators struggle with VBA's grammar and produce poor error messages. Error recovery in generated parsers is limited.
- **Tree-sitter:** Considered for IDE use cases but rejected as primary parser. Tree-sitter's C-based runtime doesn't align with pure-Rust goals. Could potentially use tree-sitter grammar as a secondary parser for editor integration in the future.
- **Nom/winnow (parser combinators):** Viable but less control over error messages and recovery than hand-written. For a language with VBA's complexity, explicit control is worth the implementation cost.

**Roslyn green/red tree pattern:**
- **Green tree:** Immutable, parent-free, structurally shared. Contains syntax kind, width, children. Cheaply cloneable. This is the persistent representation.
- **Red tree:** On-demand wrapper over green nodes. Provides parent pointers, absolute text positions, navigability. Created lazily, not persisted.
- Enables: incremental re-parsing (future), lossless formatting, IDE features, macro expansion tracking.

### 6.2 Bytecode Design

**Decision: Custom stack-based bytecode.**

Rationale:
- VBA's evaluation model is naturally stack-based (expression evaluation, parameter passing).
- A register-based VM would add complexity without clear benefit for VBA's instruction patterns.
- Custom bytecode (vs. targeting an existing VM like WebAssembly) gives full control over VBA-specific operations: Variant dispatch, COM calls, ByRef semantics, error handling state transitions.

**Instruction categories (preliminary):**

| Category | Examples |
|---|---|
| Stack manipulation | `Push`, `Pop`, `Dup`, `Swap` |
| Constants | `LoadConst`, `LoadEmpty`, `LoadNull`, `LoadNothing` |
| Variables | `LoadLocal`, `StoreLocal`, `LoadByRef`, `StoreByRef` |
| Arithmetic | `Add`, `Sub`, `Mul`, `Div`, `IntDiv`, `Mod`, `Pow`, `Neg` |
| Comparison | `Eq`, `Ne`, `Lt`, `Gt`, `Le`, `Ge`, `Like`, `Is` |
| Logic | `And`, `Or`, `Not`, `Xor`, `Eqv`, `Imp` |
| String | `Concat`, `Mid`, `Len` (may also be built-in function calls) |
| Control flow | `Jump`, `JumpIf`, `JumpIfNot`, `GoSub`, `Return` |
| Calls | `CallSub`, `CallFunction`, `CallByName` (IDispatch) |
| Objects | `CreateObject`, `Set`, `Release`, `GetProp`, `PutProp`, `CallMethod` |
| Arrays | `NewArray`, `ReDim`, `Erase`, `ArrayAccess`, `ArrayAssign` |
| Error handling | `OnErrorGoTo`, `OnErrorResumeNext`, `OnErrorGoTo0`, `Resume`, `Raise` |
| Conversion | `CoerceToType`, `CInt`, `CLng`, `CDbl`, etc. |

### 6.3 Cross-Platform Story

**Core principle:** The VBA language runtime and all built-in types work identically on all platforms. Platform differences are isolated to COM interaction and hosting.

| Feature | Windows | Linux / macOS |
|---|---|---|
| VBA language core | Full | Full |
| Built-in functions (VBA.*) | Full | Full |
| Built-in objects (Collection, Dictionary, etc.) | Full | Full |
| COM object creation (CreateObject) | Real COM | Error or mock layer |
| Type library binding (early binding) | Full (via type libraries) | Stub / portable type info |
| Declare (DLL calls) | Full (LoadLibrary) | dlopen equivalent (best-effort) |
| Host-provided objects | Via COM hosting API | Via Rust hosting trait |
| Forms runtime (UserForm) | Native (via COM controls) | Portable rendering (future) |

### 6.4 Integration with DNA Calc

OxVBA integrates with DNA Calc through the hosting API (`oxvba-host`):

- **DNA Calc provides:** Application, Workbook, Worksheet, Range, and other spreadsheet objects as COM objects (on Windows) or Rust trait implementations (cross-platform).
- **OxVBA provides:** VBA execution engine, module management, event dispatch.
- **Interaction pattern:** DNA Calc creates an OxVBA engine instance, registers host objects, loads VBA project source, and triggers execution (event handlers, macro calls).
- **Object association:** Document modules (e.g., `Sheet1` code-behind) are associated with host objects through the hosting API. Events on host objects (e.g., `Worksheet_Change`) are dispatched to the corresponding VBA event handlers.
- **Mutation model:** VBA macro execution runs in exclusive mutation mode (per Foundation doctrine — no hidden mutation pathways). The host provides a structured operation interface; VBA code modifies the spreadsheet through host-mediated operations, not direct memory access.

### 6.5 Development Innovation

Following the DNA Calc Foundation doctrine, OxVBA aims to innovate in the development process:

- **Observation-driven development:** Systematically observe Office VBA behavior, capture as evidence artifacts, implement against observations, verify conformance.
- **Decision-table-driven implementation:** Type coercion and arithmetic implemented directly from exhaustive decision tables, not from narrative specification text.
- **Formally grounded:** Lean 4 specifications for core properties provide machine-checkable assurance that the type system is coherent.
- **Regression-as-asset:** Every bug discovered becomes a minimized test case in the conformance corpus (per Foundation Hygiene Doctrine).
- **Documentation as we go:** The development path, decisions, trade-offs, and discoveries are documented contemporaneously, not after the fact.

---

## 7. Proposed Project Structure

### 7.1 Directory Layout

```
OxVba/
├── PLAN.md                         # This document
├── README.md                       # Project overview, build instructions
├── LICENSE                         # MIT license
├── CLAUDE.md                       # AI assistant instructions
├── AGENTS.md                       # Execution doctrine for AI agents
├── Cargo.toml                      # Workspace root
├── .gitignore
│
├── crates/
│   ├── oxvba-syntax/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Public API: parse, SyntaxTree, SyntaxKind
│   │       ├── lexer.rs            # Tokenizer
│   │       ├── parser.rs           # Recursive descent parser
│   │       ├── syntax_kind.rs      # Token and node kinds enum
│   │       └── green.rs            # Green tree (immutable CST nodes)
│   │
│   ├── oxvba-runtime/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Public API: Variant, VarType, coerce, builtins
│   │       ├── variant.rs          # Variant type definition and operations
│   │       ├── coerce.rs           # Type coercion logic (driven by decision tables)
│   │       ├── arithmetic.rs       # Variant arithmetic (driven by decision tables)
│   │       ├── bstr.rs             # VBA string type (BSTR-compatible)
│   │       ├── safe_array.rs       # SAFEARRAY-compatible array type
│   │       ├── decimal.rs          # 96-bit Decimal type
│   │       └── builtins.rs         # Built-in VBA functions (VBA.Strings, VBA.Math, etc.)
│   │
│   ├── oxvba-compiler/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Public API: compile, Module, Bytecode
│   │       ├── resolve.rs          # Name resolution
│   │       ├── typecheck.rs        # Type checking and coercion insertion
│   │       ├── lower.rs            # AST → bytecode lowering
│   │       └── bytecode.rs         # Bytecode format definition
│   │
│   ├── oxvba-vm/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Public API: Vm, execute
│   │       ├── interpreter.rs      # Bytecode interpreter loop
│   │       ├── stack.rs            # Operand stack and call stack
│   │       └── error_state.rs      # On Error state machine
│   │
│   ├── oxvba-jit/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Public API: JitEngine, compile_function
│   │       └── cranelift.rs        # Bytecode → CLIF translation
│   │
│   ├── oxvba-com/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Public API: ComObject, Dispatch, IUnknown traits
│   │       ├── refcount.rs         # Reference counting (AddRef/Release)
│   │       ├── dispatch.rs         # IDispatch abstraction
│   │       ├── cycle_gc.rs         # Bacon-Rajan cycle detector
│   │       └── platform/
│   │           ├── mod.rs
│   │           ├── windows.rs      # Real COM via `windows` crate
│   │           └── portable.rs     # Trait-based COM on non-Windows
│   │
│   ├── oxvba-host/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Public API: Engine, HostConfig, Project
│   │       ├── engine.rs           # Engine lifecycle and orchestration
│   │       ├── project.rs          # VBA project (modules, references, metadata)
│   │       └── events.rs           # Event dispatch (host events → VBA handlers)
│   │
│   └── oxvba-cli/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs             # CLI entry point: run .bas files, REPL
│
├── formal/
│   └── lean/
│       ├── lakefile.lean           # Lean 4 build file
│       ├── lean-toolchain          # Lean 4 toolchain version
│       └── OxVba/
│           ├── VarType.lean        # VarType inductive definition
│           ├── Coerce.lean         # Coercion relation and proofs
│           ├── Arithmetic.lean     # Operator result type proofs
│           └── RefCount.lean       # Refcount reachability invariant
│
├── tables/
│   ├── coercion.csv                # Type coercion decision table
│   ├── arithmetic.csv              # Arithmetic result type decision table
│   └── comparison.csv              # Comparison semantics table
│
├── conformance/
│   ├── harness/                    # Office VBA observation harness
│   │   └── ...                     # VBA project files for running in Office
│   ├── golden/                     # Golden output files from Office VBA
│   │   └── ...                     # Structured output (JSON/CSV)
│   └── tests/                      # VBA source files for conformance testing
│       └── ...                     # .bas / .cls files
│
├── docs/
│   ├── ARCHITECTURE.md             # Detailed architecture document
│   ├── BUILDING.md                 # Build and development setup
│   ├── CONTRIBUTING.md             # Contribution guidelines
│   ├── VARIANT_DESIGN.md           # Variant type design notes
│   ├── COM_ABSTRACTION.md          # COM layer design
│   ├── BYTECODE_FORMAT.md          # Bytecode instruction set reference
│   └── evidence/                   # Clean-room evidence records
│       └── ...
│
└── scripts/
    └── ...                         # Build, CI, and development scripts
```

### 7.2 Workspace Cargo.toml (preliminary)

```toml
[workspace]
members = [
    "crates/oxvba-syntax",
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
oxvba-runtime = { path = "crates/oxvba-runtime" }
oxvba-compiler = { path = "crates/oxvba-compiler" }
oxvba-vm = { path = "crates/oxvba-vm" }
oxvba-jit = { path = "crates/oxvba-jit" }
oxvba-com = { path = "crates/oxvba-com" }
oxvba-host = { path = "crates/oxvba-host" }
thiserror = "2"
proptest = "1"
```

---

## 8. Implementation Sequencing

### Phase 0: Project Bootstrap
- Initialize repository, Cargo workspace, CI pipeline.
- Write CLAUDE.md, AGENTS.md, README.md, LICENSE.
- Set up `cargo fmt`, `cargo clippy`, `cargo miri` in CI.
- Create all 8 crate stubs (compiling, empty).
- Initial Lean 4 project skeleton.

### Phase 1: Lexer and Parser (`oxvba-syntax`)
- Implement lexer with full VBA 7 token set.
- Implement recursive descent parser with lossless CST.
- Handle context-sensitive keywords, line continuation, conditional compilation.
- Error recovery: parser always produces a tree.
- Property tests: roundtrip (parse → print = original), all MS-VBAL grammar productions covered.
- **Milestone: parse arbitrary Office VBA source files without crashing.**

### Phase 2: Core Runtime Types (`oxvba-runtime`)
- Implement `Variant` type with all VarType discriminants.
- Implement type coercion driven by decision tables.
- Implement Variant arithmetic driven by decision tables.
- Implement `BStr` (VBA string type).
- Implement `SafeArray` (VBA array type).
- Build observation harness; generate initial decision tables from Office VBA.
- Lean 4: formalize `VarType` and `Coerce`.
- **Milestone: Variant arithmetic matches Office VBA for all type combinations.**

### Phase 3: Compiler (`oxvba-compiler`)
- Semantic analysis: name resolution, type checking.
- Bytecode format definition and emission.
- Support for: modules, procedures, variables, expressions, control flow, error handling statements.
- **Milestone: compile simple VBA programs to bytecode.**

### Phase 4: Virtual Machine (`oxvba-vm`)
- Bytecode interpreter with operand stack and call stack.
- Error handling state machine.
- ByRef parameter passing.
- GoSub/Return.
- Built-in function dispatch.
- **Milestone: execute VBA programs that use basic control flow, arithmetic, and string operations.**

### Phase 5: COM and Object System (`oxvba-com`)
- Reference counting infrastructure.
- IUnknown / IDispatch trait abstractions.
- Class module support (VBA-defined classes with Class_Initialize/Terminate).
- Collection, Dictionary built-in objects.
- Platform-specific COM integration (Windows: real COM via `windows` crate).
- Cycle detector (Bacon-Rajan).
- **Milestone: VBA programs that create and use objects work correctly; deterministic destruction verified.**

### Phase 6: Hosting API (`oxvba-host`)
- Engine lifecycle (create, configure, load project, execute, destroy).
- Host object registration.
- Event dispatch.
- Project management (modules, references).
- **Milestone: OxVBA can be embedded in a host application and execute VBA event handlers.**

### Phase 7: JIT Compilation (`oxvba-jit`)
- Bytecode → Cranelift IR translation.
- Per-function JIT compilation.
- Runtime switching between VM and JIT execution.
- AOT compilation mode.
- **Milestone: JIT-compiled VBA programs run faster than VM interpretation and produce identical results.**

### Phase 8: CLI and Standalone Execution (`oxvba-cli`)
- Command-line interface for running VBA files.
- REPL for interactive VBA execution.
- AOT compilation to standalone executables.
- **Milestone: `oxvba run program.bas` executes a VBA program from the command line.**

### Phase 9: Conformance and Polish
- Expand conformance test corpus.
- Address compatibility gaps identified by conformance testing.
- Performance profiling and optimization.
- Documentation completion.
- **Milestone: OxVBA passes a comprehensive conformance suite against Office VBA.**

### Future (not sequenced):
- Forms runtime (UserForm support).
- Hosting integration with DNA Calc.
- Debugging protocol.
- IDE support (language server).
- Additional COM library compatibility on non-Windows.

---

*This document is the first artifact of the OxVBA project. It captures the project charter, architectural decisions, and implementation approach as discussed during project initiation. It is a living document that will be updated as the project evolves.*
