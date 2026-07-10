# OxVBA Front-End & Core IR Contract — V1

> [!CAUTION]
> **Superseded architecture.** Retained for historical design provenance. Current authority is [`OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md`](OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md); Core IR is not the product bytecode/package.

- **Date:** 2026-06-04
- **Status:** superseded historical design contract. It formerly superseded and absorbed `HIR_RESOLUTION_ENVIRONMENT_V1.md`; current authority is the compiler contract V2 and OxIR/Image contract.
- **Builds on:** `docs/ARCHITECTURE.md` §End-State Destination, `docs/HIR_COVERAGE_GAPS_AND_WIDENING_PLAN_2026-06-03.md`, `docs/spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md`, `docs/spec/VBA_TYPE_SYSTEM_V1.md`. Authority for VBA semantics: **MS-VBAL** (the VBA language spec) + the real `VBA` type library.
- **Purpose:** define, from compiler-construction first principles, the front-end pipeline *and* the shape of the Core IR (the bytecode + metadata package) so that the implementation is a matter of satisfying a contract rather than inventing direction per step.

## 0. The defect this contract fixes (motivating evidence)

Measured 2026-06-04: `Instruction` has **235 variants; 136 (58%) are `Intrinsic<LibraryFunction>` opcodes** (`IntrinsicLenDigits`, `IntrinsicMsgBoxHost`, `IntrinsicFvI32`, `IntrinsicCollectionAdd`, `IntrinsicFileOpenHost`, …). The VBA library is baked into the instruction set. Combined with the front-end's three drifted intrinsic name-tables and `project.rs` source-rewriting, the system has **no clean separation between the primitive language core and the library** — at *either* the front-end or the bytecode layer. This contract removes that conflation top to bottom.

## 1. Principles (with the proven systems that embody them)

1. **Resolve once, against a uniform symbol table.** One `Symbol` notion; scopes populated by providers (this module, sibling modules, referenced projects, VBA library, COM typelibs, host). The resolver walks scopes and is **source-agnostic** — it never branches on "library vs project vs COM." *(Roslyn `ISymbol`; rust-analyzer `Definition`; CLR metadata: local, imported, and library symbols are one kind.)*
2. **Desugar surface → a small core. "Make implicit explicit."** Operators, conversions, default-member chains, property Get/Let/Set, `For Each`, `With`, statement-form built-ins (`Print #`, `Debug.Print`) all reduce to **resolved calls + place loads/stores + branches + error-state ops**. *(GHC → Core; Swift AST → SIL; Roslyn bound → lowered-bound; rustc HIR → MIR.)*
3. **Lower once into an explicit, total Core IR; many back-ends consume it.** No back-end re-derives semantics. The Core IR is exactly the in-memory form of the **bytecode + metadata package**. *(LLVM IR, .NET CIL, JVM bytecode, Wasm.)*
4. **The center of gravity is the binder + Core IR, not the library.** VBA's difficulty is property triples, default members, Let/Set, the Variant coercion lattice, early/late dispatch, ByRef places, and error-state — resolved *once*, explicit in the IR. The library is downstream surface; get the binder + IR right and library/operators/references slot in as "symbol + desugar rule."

## 2. Pipeline

```
source
  → oxvba-syntax  : lossless CST (green/red)                         [fidelity, IDE]
  → binder        : uniform symbol table + resolution                [§3, §4]
                    (names→Symbols, overloads, coercions, dispatch,
                     property accessors, default members, places)
  → lowering      : desugar to Core IR — make implicit explicit      [§5]
  → Core IR  ==  bytecode + metadata package (serialized)            [§6]
        ├── interpreting VM   (reference oracle; portable/WASM)
        └── Cranelift JIT     (optimizing; lowers the same package)
```

## 3. The symbol model (principle 1)

- One `Symbol`: identity, containing scope, kind (Variable | Procedure | Property | Type | Const | Event | Module | Namespace), a `Signature` where applicable, **provenance** (which provider/source), and an **impl** (how its body is reached — see §6.3). Properties are modeled as a **property group** with up to three accessors (Get/Let/Set), resolved as one logical member.
- **Scopes** form the resolution chain: local → procedure → module → sibling project modules → referenced projects → VBA library → host library → COM typelibs. Each non-source scope is populated by a *provider* (descriptor for the library, `TypeLibMetadataBlob` for COM, manifest for projects, catalog for host).
- **`resolve(name, context)` is source-agnostic**: it walks scopes, returns a `Symbol`. It must never contain a `match` on source kind. Qualified names (`VBA.Len`, `Module1.Foo`, `Scripting.FileSystemObject`) select a scope directly. Shadowing follows MS-VBAL (user declarations shadow library names within scope).

**Invariant:** adding a new source (COM, host, another project) populates scopes via a provider and touches **zero** lookup-site branches.

## 4. The binder — VBA's hard semantics, resolved once

The binder turns the CST into a fully-resolved bound tree. It is where the real work is. It must resolve, explicitly:

- **Property access** → the specific accessor by context: read→`Get`, `Let x = …`→`Let`, `Set x = …`→`Set`. A property group is one member; the accessor is chosen here, not by string rewriting.
- **Default-member chains** → made explicit. `obj` in a value context where `obj` has a default member → an explicit `obj.<DefaultMember>` call, applied recursively per MS-VBAL.
- **Let vs Set** → an assignment carries an explicit intent; `Set` requires an object/Variant target; `Let` into an object is an error; coercion is computed.
- **Early vs late dispatch** → chosen by the static receiver type: typed receiver → early-bound (vtable slot / dispid from the provider); `Object`/`Variant` receiver → late-bound (`IDispatch` by name). The binder records the dispatch route; lowering emits it.
- **Coercion** → the **Variant coercion lattice** (MS-VBAL conversion rules). Every implicit conversion is computed and recorded as an explicit coercion node; the IR carries no implicit conversions.
- **Places vs values (ByRef)** → the binder classifies each operand as a *place* (l-value: local, field, array element, `ByRef` param) or a *value*. `ByRef` arguments pass places (aliases); `ByVal` pass values. This is a first-class IR concept (§5), not an ad-hoc alias carrier.
- **Error-state** → `On Error`/`Resume` resolved to explicit error-state transitions.

**Invariant:** no semantic decision is left to a downstream string match or to the VM; the bound tree is unambiguous.

## 5. The Core IR (desugared; principle 2)

A small, explicit, total IR. Grammar (conceptual):

```
Proc        = { params: [Place], locals: [Slot], body: [Stmt], error_model }
Stmt        = Assign(Place, Value, intent)        // intent ∈ {Let, Set}; coercion explicit
            | Eval(Value)                          // value-context call (statement form)
            | If(Value, [Stmt], [Stmt]) | Loop(..) | Branch(label) | Label(..)
            | ErrorState(SetHandler | Resume | Clear | Raise(Value))
            | Return(Value?)
Value       = Const(typed) | Load(Place)
            | Call(Callee, [Arg])                  // THE one call node
            | Coerce(Value, from, to)              // explicit; never implicit
Place       = Local(slot) | Field(Value, field) | Index(Value, [Value]) | ByRefParam(slot)
Arg         = ByVal(Value) | ByRef(Place)
Callee      = VbaProc(SymbolRef)                   // active or referenced project — by call target
            | Native(NativeImplId)                 // VBA library / Declare / host primitive
            | EarlyCom(typelib, vtable_or_dispid, spec)
            | LateDispatch(member_name)            // IDispatch by name
```

Everything implicit in VBA becomes an explicit node here: operators are `Call`s (pragmatically lowered to primitive opcodes — §6.2), conversions are `Coerce`, default members are explicit `Call`s, property access is a `Call` to the resolved accessor, `Print #1, x` is a `Call(Native(FilePrint), [#1, x])`, `For Each` is the explicit enumerator protocol.

**Invariant:** there is exactly **one** `Call` node. The difference between project/library/COM/host is the `Callee`, resolved by the binder — never a distinct opcode per library function.

## 6. The bytecode + metadata package = serialized Core IR (principle 3)

### 6.1 The partition (the §0 fix)
- **Primitive instruction set (~50–60 opcodes):** place load/store, arithmetic/comparison/logical/concat, `Coerce*`, branches, `Call`/`Return`, error-state (`RaiseError`/`SetOnError*`/`Resume*`/`LoadErr*`), object-ref load + `Is`, property accessor call, array alloc/bounds/element, `For Each` step. This is the machine.
- **One native-call opcode: `CallNative(impl_id)`** (or unify into `Call`). The **136 `Intrinsic*` opcodes are removed from the instruction set** and become **package metadata**: the library is a table of native-bodied symbols, each with a stable `NativeImplId`.

### 6.2 Why this is correct *and* cheap
A dedicated opcode and `CallNative(impl_id)` dispatched by the VM's match on `impl_id` are **runtime-equivalent** (a tag dispatch / jump table either way). So collapsing 136 opcodes into one carrier is a **contract cleanup with negligible runtime cost** — the VM's big `match` moves from "match opcode" to "match impl_id." Benefits: the IR is primitive and stable; the JIT gets **one** native-call lowering + an inline allowlist instead of 136 opcodes (this is what makes the dual-target goal *feasible*); adding a VBA function touches only the descriptor + a VM impl-fn — no new opcode, no IR change, no JIT change. **IR shape (clean) is separated from VM implementation (may fast-path).**
- *Pragmatic exception:* arithmetic/comparison/coercion stay primitive opcodes in v1 (hottest path; `Variant` boxing cost dominates). Making operators themselves library calls (`op_Addition`) is the noted limit (§ principle 2), deferred to a dedicated phase.

### 6.3 The VBA base library (absorbed from `HIR_RESOLUTION_ENVIRONMENT_V1` §4)
Shaped like the real `VBA` type library: modules (`Strings`/`Math`/`Information`/`Interaction`/`Conversion`/`FileSystem`/`Constants`), predeclared objects (`Err`/`Debug`/`Collection`, COM-type-shaped, some with events). Each member's **body is `Native(NativeImplId)`** — the typelib-interface + DLL-impl analog, reusing existing `oxvba-runtime`/`oxvba-hal` code unchanged. `Declare Lib` and host primitives use the same `Native` mechanism. Library-interface members are partitioned from compiler-internal structural intrinsics (`__oxvba_*`, `VarPtr`/`StrPtr`/`ObjPtr`, `dispatchinvoke`), which are *not* library surface. Enumerated syntactic quirks (`Err`, file I/O statements, `Debug`, `Mid`-statement) are explicit desugar rules.

## 7. The contract (invariants every change is held to)

1. Production compile path is `source → CST → binder → Core IR → package`; no string rewriting, ever.
2. The resolver has no per-source branch; sources are providers populating scopes.
3. The Core IR is total and explicit: no implicit coercion, no implicit default-member, no unresolved dispatch, no semantics deferred to the VM.
4. Exactly one `Call` node / one `CallNative` opcode; library functions are metadata + native bodies, never instruction-set opcodes.
5. The VM and the JIT consume the *same* package; any fact a back-end needs is in the package.
6. Correctness is judged by the **differential oracle** (observed real-VBA behavior), not by intuition.

## 8. Phasing (dependency-ordered — foundation before surface)

1. **Symbol model + source-agnostic binder.** The uniform `Symbol`/scope/provider model; `resolve` rewritten to walk scopes. (Constants slice already shipped fits here as a leaf provider.)
2. **Core IR + `CallNative` + codegen.** Introduce the explicit Core IR and `CallNative(impl_id)`; collapse the 136 `Intrinsic*` opcodes into metadata-described native bodies (staged in batches behind the differential harness; hot arithmetic stays opcodes). This makes the package primitive and JIT-ready.
3. **Hard semantics onto the binder/Core IR:** property triples, default members, coercion lattice, places/ByRef, early/late dispatch — *this deletes the `project.rs` rewrites and the field-array/PMR carriers.*
4. **Library / operators / references** as symbols + desugar rules (base library typelib shape; project refs; COM via `TypeLibMetadataBlob`; host catalog). Operators-as-library optional/deferred.
5. **Flip & delete:** remove `project_compile_boundary`, the legacy resolver/rewriter, the legacy `Bound*` lowering target, and the obsolete intrinsic opcodes.

Each phase net-deletes code and is gated by full suite + conformance corpus + the differential oracle.

## 9. Process (so direction is set by contract, not by correction)

- **Contract-first:** this document + the MS-VBAL references are the spec; steps satisfy §7's invariants. Design questions are answered against MS-VBAL and the proven-system precedents, not intuition.
- **Differential oracle as arbiter:** semantic edge cases (coercion, default-member chains, dispatch) are settled by observed real-VBA behavior via the differential harness.

## 10. Relationship to committed work
- The constants slice (`frontend_library`) is consistent — a leaf provider feeding §3; no change needed.
- The uniform `CallableBinding`/`DispatchRoute` and VBA-shaped-library refinements are absorbed here (§5 `Callee`, §6.3).
- **New and significant:** §6's bytecode reshape (collapse 136 intrinsic opcodes → `CallNative` + metadata). It touches the package contract being stabilized on `single-package-descriptor-vm`; it is staged behind the differential harness and is what makes the JIT tractable. It is compatible with "one VM runs the package correctly" — it makes the package *primitive*, the library *metadata*.
