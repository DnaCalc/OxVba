# The OxVba Blueprint  
## Theoretical Architectures for a First-Principles Rust VBA Implementation

---

## Executive critique of the OxVba project plan

An analysis of the foundational OxVba project plan reveals a highly pragmatic, robust, but inherently conventional approach to implementing a Visual Basic for Applications (VBA) runtime engine in Rust. The project’s stated mission—to provide a high-performance, cross-platform VBA engine for parsing, compilation, and execution that exceeds the capabilities of the standard Office-bundled engine—is structurally sound. The project values, prioritized as robustness, compatibility, performance, small runtime size, and a well-managed development environment, are effectively addressed through the selection of Rust as the primary systems language. Rust provides memory safety, zero-cost abstractions, static linking for small distribution sizes, and exceptional cross-platform Component Object Model (COM) interoperability.

The proposed crate decomposition reflects a traditional, well-understood compiler pipeline architecture:

- `oxvba-syntax` (lexer and parser)  
- `oxvba-compiler` (semantic analysis and bytecode emission)  
- `oxvba-runtime` (foundational types like the `Variant` and coercion logic)  
- `oxvba-vm` (stack-based virtual machine)  
- `oxvba-jit` (Cranelift-based Just-In-Time compilation)  
- `oxvba-com` (COM abstraction layer)  
- `oxvba-host` (hosting API)  
- `oxvba-cli` (command-line runner)

This ensures modularity. The choice of a hand-written recursive descent parser leveraging Roslyn-style green/red trees ensures error recovery, while the integration of Cranelift for JIT compilation provides a realistic, industry-standard pathway to improved execution speeds.

However, when evaluated against an unconstrained, first-principles theoretical model executed by top-tier agentic coders utilizing maximum computational resources, the existing OxVba plan exhibits several architectural ceilings that preclude it from achieving the theoretical optimum of “Mach-1000” execution speeds and mathematically proven bedrock stability.

1. **Variant layout ceiling (cache + locality):**  
   The proposed `repr(C)` 24-byte tagged union for the core `Variant` type is inefficient for modern CPU cache lines. While it guarantees simple Microsoft COM ABI compatibility, it sacrifices instruction-level parallelism, incurs false-sharing penalties, and degrades spatial locality during intensive mathematical loops.

2. **Memory management ceiling (latency spikes):**  
   The reliance on standard COM-compatible reference counting (RC) paired with an opt-in Bacon–Rajan cycle-detecting garbage collector is a compromise between native VBA behavior and modern memory management. While preventing circular-reference leaks inherent to native VBA, runtime cycle detection introduces non-deterministic latency spikes that are incompatible with hyper-optimized execution.

3. **IR ceiling (premature semantic loss):**  
   Directly lowering an AST to a stack VM or straight to Cranelift bypasses intermediate domain-specific optimizations. Standard JIT compilers treat code as generalized computation, losing semantic context of VBA’s COM dispatch rules, implicit coercion matrices, and error-handling paradigms.

To transcend these limitations, the architecture must be fundamentally reimagined using pioneering compiler architectures, advanced functional data structures, and formally verified code generation techniques—creating a cybernetic development loop optimized for autonomous agentic code generation.

---

## Lexical and syntactic architectures: the lossless syntax paradigm

The parsing and lexical analysis phases require an architecture capable of:

- **Lossless round-tripping** (preserve trivia: whitespace/comments/invalid tokens)
- **Infinite error tolerance** (operate on incomplete states)
- **Non-destructive mutation** (fast edits without reparsing everything)

The OxVba plan correctly identifies the Roslyn Red/Green tree pattern as the baseline. An unconstrained implementation pushes further by integrating purely immutable, high-performance data structures and combinator-based rewriting to support real-time, near-zero-overhead metaprogramming.

---

## Lossless syntax trees and the Red–Green separation

A modern, IDE-ready (or agent-ready) compiler frontend requires a **Lossless Syntax Tree (LST)**. Traditional ASTs discard syntactic trivia, making them unsuitable for tooling, language servers, or dynamic rewriting.

The Red/Green pattern (Roslyn; also SwiftSyntax; and in Rust, *rowan* via rust-analyzer) separates the tree into two mathematical representations:

### Green tree (storage form)
- Untyped (e.g., `SyntaxKind` enums)
- Immutable and position-independent (no absolute offsets)
- Nodes contain only **relative width** (bytes/chars)
- Structural sharing (dedup of identical subtrees)

This supports massive deduplication for legacy enterprise VBA modules.

### Red tree (typed facade)
- Strongly typed API
- Computes **absolute position** on-demand by summing widths
- Ephemeral wrappers provide:
  - offsets/spans
  - parent pointers
  - ergonomic typed traversal

#### Red/Green comparison table

| Architectural component | Mutability state     | Type safety                     | Positional awareness | Memory characteristics                            | Primary computational function |
|---|---|---|---|---|---|
| Green tree | Strictly immutable | Untyped (`SyntaxKind` enums) | Relative width only | Highly deduplicated; shared subtrees | Bottom-up structural storage; maximum memory efficiency; caching |
| Red tree | Immutable facade | Strongly typed API | Absolute offsets | Ephemeral; computed on-demand | Top-down traversal; semantic analysis; error recovery; IDE tooling |

Because the underlying structure is immutable, replacing a single node yields a new root that reuses the vast majority of the existing tree—near **O(1)** allocation for small edits and limited invalidation.

---

## Finger trees and combinator rewriting engines

To manipulate LSTs efficiently, use advanced functional data structures (as popularized in Eric Lippert’s “Fabulous Adventures in Data Structures and Algorithms” discussions).

### Finger trees for child storage

Store child sequences using **Finger Trees** (catenable deques), not plain `Vec<T>`:

- Amortized **O(1)** access at both ends  
- **O(log n)** concatenation and splitting  
- Efficient mid-stream edits (split → insert → concat) with structural sharing

This is especially useful for live debugging edits, macro rewriting, and agent-driven patching without full reparses.

Lippert-style constant-time reversals can be modeled as logical inversions via structure, rather than copying and reversing memory allocations.

### Combinator-based AST rewriting

Instead of heavy visitor boilerplate, treat transforms as pure functions:

\[
\text{AST} \rightarrow \text{AST}
\]

Then build *composable* combinators:

- run N rewrite permutations in parallel
- score candidates using an execution cost function
- select the best branch
- iterate until a fixpoint

\[
T_{k+1} = f(T_k), \quad \text{stop when } T_{k+1} = T_k
\]

Immutability makes speculative, parallel rewrites safe and deterministic in structure (even if evaluation strategies vary).

---

## Mathematical foundations of intermediate representation: the MLIR architecture

The baseline OxVba plan lowers:

- AST → stack VM bytecode, or
- AST → Cranelift IR (JIT)

This creates a semantic gap: VBA-specific semantics (COM, coercion, error handling) are discarded too early.

A first-principles architecture uses **MLIR** (Multi-Level Intermediate Representation) with dialects and progressive lowering.

---

## Multi-level dialects and progressive lowering

Define a lowering ladder with targeted optimizations at each tier:

1. **VBA High-Level Dialect (`vba.hl`)**  
   Closest to the typed syntax, but in data-flow form. Retains:
   - `For Each` over COM collections
   - implicit `Variant` coercions
   - `On Error GoTo` / `Resume Next` as first-class ops

2. **VBA Middle-Level Dialect (`vba.ml`)**  
   De-sugars:
   - expands `For Each` into explicit `IEnumVARIANT` flow
   - injects RC boundaries (`AddRef` / `Release`)
   - expands late-bound dispatch into:
     - `IDispatch::GetIDsOfNames`
     - `IDispatch::Invoke`

3. **Structured Control Flow (`scf`) and CFG (`cfg`) dialects**  
   Enables classic loop and control-flow optimizations:
   - constant folding
   - loop transforms
   - vectorization opportunities

4. **Lowerings to machine/portable targets**  
   - `llvm` dialect for native codegen
   - `emitc` dialect for optimized C emission (portability)

This preserves domain semantics long enough to perform VBA-aware optimizations before committing to low-level forms.

---

## Modelling unstructured control flow: `On Error Resume Next`

VBA’s unstructured error handling makes CFGs irreducible if naively lowered (“branch after every operation”).

Model it in `vba.hl` as a region with implicit guarded execution, then delay explicit lowering until late in the pipeline.

For a basic block \(B\) with operations \(O_1, O_2, \dots, O_n\), lower each \(O_i\) into a guarded form with two edges:

- **success edge** → \(O_{i+1}\)  
- **exception edge** → unified exception block

Exception block responsibilities:
- update global `Err` object (`Err.Number`, `Err.Description`)
- clear relevant exception state
- continue to \(O_{i+1}\)

By delaying this transformation until `vba.ml → cfg`, earlier passes can still reorder and optimize without becoming dominated by error edges.

---

## Alien-artifact algorithms: Knuth broadword + MMIX mechanisms

When JIT is constrained (security policy / memory), the interpreter path must be exceptionally fast.

Leverage “alien artifact” level techniques from Knuth (TAOCP, Fascicle 1) and MMIX-inspired designs.

---

## Broadword algorithms for instruction decoding (SWAR)

Instead of decoding opcodes byte-by-byte with branch-heavy dispatch, treat a word as packed bytes and use bitwise parallelism.

Let:

- \(x\) be a 64-bit word containing 8 opcode bytes
- \(c\) be the target opcode byte

Detect presence of \(c\) in any byte of \(x\):

\[
y = x \oplus \left(c \times \text{0x0101010101010101}\right)
\]

\[
z = (y - \text{0x0101010101010101}) \ \& \ (\sim y) \ \& \ \text{0x8080808080808080}
\]

If \(z \neq 0\), then \(c\) appears in \(x\).

This enables scanning for Branch/Return/Error patterns in **O(1) per 64-bit block**, feeding the interpreter with better prefetch and fewer branch mispredicts.

---

## The MMIX sliding register window

Pure stack VMs incur heavy memory traffic (push/pop churn). MMIX uses a **local register stack** with a **sliding window**:

- Registers \(0\) to \(rL-1\): local to current subroutine
- Registers \(rG\) to \(255\): global
- Calls shift the register window; arguments live in an overlap region
- Spills happen only when depth exceeds physical capacity

In a Rust VM, emulate this via:

- a register file abstraction
- window pointers equivalent to `rO` / `rS`
- spill/fill logic

Result: deep call trees execute with far fewer RAM accesses, approaching native speed even under interpretation.

---

## Boundary-tag memory allocation

For dynamic allocations (BSTR strings, SAFEARRAYs), use boundary tags:

- store size/status at start *and* end of each block
- on free, check adjacent tags in **O(1)** and coalesce

This reduces fragmentation over long-running workloads typical of Excel automation.

---

## The Mach-1000 execution engine: memory, Variant representation, and zero-copy

Performance hinges on data layout and transitions from disk → memory → registers.

The baseline 24-byte `Variant` is COM-friendly but cache-hostile.

---

## Optimizing the Variant: NaN-boxing vs 128-bit tagged payloads

Goal: fit `Variant` into **16 bytes** so exactly **4 variants** pack into a 64-byte cache line.

Constraint: VBA **Decimal** requires (effectively) a 16-byte footprint (payload + tag), leaving no slack.

### 64-bit NaN-boxing limitation

NaN-boxing uses IEEE-754 NaN payload bits (≈ 51 usable bits) to store type tags/pointers. This cannot represent Decimal’s 96-bit integer + scale/sign requirements.

### 128-bit tagged scheme (16-byte exact fit)

Use:

- **Bytes 0–1:** `VARTYPE` tag (COM-compatible)
- **Bytes 2–15:** 14-byte payload

Example mapping:

| Variant type | Bytes 0–1 (`VARTYPE`) | Bytes 2–15 (payload) | Alignment behavior |
|---|---:|---|---|
| Decimal | `0x000E` (`VT_DECIMAL`) | 12-byte integer + 2-byte scale/sign | Exact fit, no padding |
| Double | `0x0005` (`VT_R8`) | 8-byte `f64` + 6 bytes padding | 8-byte aligned |
| Object | `0x0009` (`VT_DISPATCH`) | 8-byte pointer (+ capability metadata if applicable) | Maintain pointer provenance |
| String | `0x0008` (`VT_BSTR`) | 14-byte inline (SSO) | Avoid heap for small strings |

This guarantees tight packing and enables aggressive vectorization on arrays of `Variant`.

---

## Zero-copy bytecode loading with `rkyv`

To avoid startup latency on large macro corpora:

- memory-map the bytecode blob (`mmap`)
- use `rkyv` archived representations so on-disk layout matches in-memory layout
- validate bounds (optional) and cast to typed structures without allocation-heavy decoding

This yields near-zero overhead load paths and rapid iteration loops.

---

## Bedrock stability: formal verification in Rust

Rust prevents many classes of memory errors, but not logical correctness. High-performance paths will require `unsafe`:

- MMIX-style register windows
- raw COM pointers
- 128-bit tagged unions
- broadword decoding internals

Testing alone is insufficient for “bedrock” claims. Use formal methods:

- **Verus** (spec/proof/exec modes; SMT-backed invariants)
- **Creusot** (separation-logic style reasoning aligned with Rust ownership)
- **Kani** (bounded model checking for unsafe, bit-level correctness)

### Deductive verification (Verus, Creusot)

Use Verus to prove coercion matrices:
- no panics
- correct overflow semantics
- correct `Err` behavior under specified constraints

Use Creusot to prove transformation correctness across lowering and rewrite passes (semantic preservation).

### Bounded model checking (Kani)

Prove:
- broadword decoder bitmasks cannot mis-detect under any input word
- 16-byte union loads never misalign or violate invariants
- register window never exceeds allocated bounds (no UB)

This parallels “verified compiler” ambitions (CompCert-like rigor), applied to an interpreted/JIT language engine.

---

## Synthesis and the agentic execution model

Combine:

- **Lossless syntax + Red/Green trees** for robust, tooling-grade parsing
- **Finger trees + combinator rewrites** for fast, speculative, parallel AST transforms
- **MLIR dialect ladder** to preserve VBA semantics long enough for domain-specific optimization
- **Broadword + MMIX-style VM** for ultra-fast interpretation
- **16-byte Variant + zero-copy loading** for cache-perfect execution and instant startup
- **Formal verification** to turn unsafe high-performance code into provably correct components

The result is not merely a legacy interpreter, but a mathematically constrained execution matrix optimized for autonomous, high-throughput agentic development—where proof systems act as the ultimate arbiter of correctness during aggressive optimization.

---