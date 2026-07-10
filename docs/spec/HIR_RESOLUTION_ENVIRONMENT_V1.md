# HIR Resolution Environment — Design V1

> [!CAUTION]
> **Superseded HIR-era design.** Current compiler authority is [`OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md`](OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md).

- **Date:** 2026-06-04
- **Status:** historical. It was first superseded by `OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md`; both are now superseded by the compiler contract V2. Kept only for the detailed resolution-environment derivation.
- **Owner:** DNA Kode
- **Supersedes framing of:** the staged "Option A" single-module-binder + flattening approach, and the interim in-compiler table ("Stage B") of `WORKSET_2026-05-30_DEFAULT_HOST_PROJECT_VBA_LIBRARY.md`.
- **Builds on:** [`docs/HIR_COVERAGE_GAPS_AND_WIDENING_PLAN_2026-06-03.md`](../HIR_COVERAGE_GAPS_AND_WIDENING_PLAN_2026-06-03.md), [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) §End-State Destination, the `DEFAULT_HOST_PROJECT_VBA_LIBRARY` workset, [`docs/spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_V1.md), [`docs/spec/VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md).

## 1. Problem this design closes

The front-end HIR binder is **single-module** (`build_hir_from_source(module_name, source)`, `collect_type_hooks_from_source("Main", source)`) and has **no type/member model**. Two consequences, proven by the 2026-06-03 gap audit:

- Multi-module projects only "work" because `project.rs` *flattens all modules into one source string* and feeds it to the single-module binder; `resolve_name` (`frontend_hir.rs:2058`) errors on any non-local name, and calls lower to `BoundExpr::ProcCall { name }` *by string* — never bound.
- `x.Member` lowers blindly to `BoundExpr::Member { receiver, member }` by name; the binder cannot resolve a member to a method, a COM dispatch token, or a library impl. That resolution lives only in legacy `project.rs` source rewrites.
- The VBA standard library is hardcoded at every layer (3 parallel intrinsic name tables, ~11 `is_*_stmt` text-prefix recognizers + ~12 dedicated `HirStmtKind` built-in variants, `Debug`/`Err`/`Collection` name-rewrites, ~80 `Instruction::Intrinsic*` opcodes). There is no library abstraction.

Every "references HIR can't bind / classes force legacy / default-members need carriers" symptom is downstream of these. This design replaces them with **one resolution environment**, and makes the VBA base library the first proving ground for it.

## 2. End-state architecture

The binder resolves every name and member against a single **`ResolutionEnvironment`**: an ordered stack of **symbol sources**, each a uniform provider that can answer *"bind name N (in context C)"* and *"describe type/members of T"*. User modules, referenced projects, the VBA base library, host globals, and COM typelibs are all symbol sources — the same kind of thing, different providers.

```
ResolutionEnvironment (per compilation)
  layer 0  local / procedure scope        (params, locals)
  layer 1  current module                  (module-level decls)
  layer 2  sibling project modules         (other modules, this project)
  layer 3  referenced projects             (public symbols, project-qualified)
  layer 4  VBA base library (implicit)     (Constants/Math/Strings/…, Debug/Err/Collection)
  layer 5  host library (host-injected)    (Application, ThisWorkbook, … — may be empty)
  layer 6  COM typelib references          (explicit COMReferences → TypeLibMetadataBlob)
```

Name resolution walks the stack in order; first match wins. A **qualified** name (`VBA.Len`, `Module1.Foo`, `OtherProject.X`, `Scripting.FileSystemObject`) selects a specific layer/namespace directly. The binder produces a **typed `Binding`** (below) for every resolved name; lowering emits from the binding kind. Source-flattening is retired — each module is bound against the environment, which includes the other modules as layer 2.

This is the same architecture `ARCHITECTURE.md` already mandates for the runtime ("every fact lives in the package"): here, every *name/member fact* lives in one resolution environment rather than in scattered parser arms and legacy rewrites.

## 3. Typed bindings and member dispatch (the decisive layer)

### 3.1 `Binding` — the uniform result of resolving a name
Every name resolves to the **same shape** regardless of source (active project, referenced project, base library, COM typelib, host). Sources differ only in the **dispatch route**, not the resolution structure — there is no per-source binding kind and no library-specific id:

```
Binding =
  | Value(ConstValue)            // a constant: a base-library vb* value or a Const decl
  | Variable(SymbolRef)          // a local / parameter / field / module variable
  | Callable(CallableBinding)    // a Sub / Function / Property / method
  | Type(TypeRef)                // a type name in a type position (class, UDT, coclass, library object)

CallableBinding = {
  source:    SymbolSourceId,     // which layer resolved it: this project | ref-project N | VBA library | typelib M | host
  signature: SignatureFacts,     // arity, param/return facts, host-sensitivity
  dispatch:  DispatchRoute,      // HOW to invoke — the ONLY source-kind-specific part
}

DispatchRoute =
  | VbaProc(ProcRef)                       // compiled VBA: active OR referenced project (identical kind, different source)
  | Native(NativeImplId)                   // natively-implemented: VBA base library, Declare Lib, host primitives
  | Com { dispatch_token, member_spec }    // early-bound COM/typelib member
  | HostDynamic(HostMemberRef)             // host-provided global, dynamic dispatch
```

`NativeImplId` is **not** "the library's id"; it is the dispatch route for *natively-implemented* members, shared by the base library, `Declare Lib`, and host primitives — at the same level as `VbaProc` and `Com`. The active project is not special: its procedures resolve to `Callable { source = this project, dispatch = VbaProc(..) }`, exactly like a referenced project's (only `source` differs). It does not get a native id.

### 3.2 Member dispatch — one mechanism for all receivers
`x.Member(args)` resolves uniformly:
1. Resolve `x` to a `Binding` carrying a **type** (`As Class1`, `As Scripting.FileSystemObject`, predeclared `Debug`, a referenced-project class, …).
2. Look up `Member` in that type's member set, from the type's provider: project class descriptor (`ProjectSymbolIndex`), COM `TypeLibMetadataBlob` (`member_name → token`/vtable slot/spec), base-library object descriptor, or host catalog.
3. Produce the same `CallableBinding`; only `dispatch` reflects the receiver's source — `VbaProc` (project class method), `Com` (typelib member), `Native` (base-library object member such as `Debug.Print`), or `HostDynamic` (host global member). A member call and a free-function call have identical binding shape.
4. Lower to the matching instruction.

This single path replaces: `rewrite_early_bound_member_dispatch` (COM, `project.rs:5078`), the `Debug.Print`/`Err.*` special-cases, `MemberDispatchClass` routing, project-class member/default-member rewrites, and the field-array/PMR carriers. **Default members** are just a member lookup with the type's `is_default_member` entry.

## 4. The VBA base library — a VBA-shaped interface with native bodies (iii)

The base library is a **built-in reference injected by default into every compilation**, resolved through the *same* path as project and COM references. Guiding principle:

> At the **interface** level the base library should look like a rich VBA library one could have written and referenced — native imports/exports, module functions, classes shaped like COM types (some with events). Only the **bodies** are native. The core language stays primitive and *references* this library; it does not bake the library's surface into the parser/compiler.

### 4.1 Interface shape (VBA-implementable in principle)
The descriptor mirrors what a referenced VBA/COM library's metadata would expose, so the base library and a real referenced library are the *same kind* of `SymbolSource`:
- **Modules** (`Constants`, `Math`, `Strings`, `Conversion`, `Information`, `Interaction`, `FileSystem`, …) under the global `VBA` namespace, each holding constants and module functions.
- **Constants**: name → typed value → `Binding::Value` (slice 1, done).
- **Module functions**: name + full signature (params/optionality/return) + host-sensitivity → `Callable`.
- **Classes shaped like COM types**: `Collection`, `Err` (and conceptually `Debug`) as declared `Type`s with methods/properties — and, where VBA has them, **events** — resolved through the one member-dispatch path (§3.2).
- **Native imports/exports**: the library's "DLL side" — see §4.2.

### 4.2 Native implementation binding (the body)
Every library member declares how it is implemented. In the uniform model that is `dispatch: Native(NativeImplId)` — the member's *body* is a hook into existing Rust (`Instruction::Intrinsic*` over `oxvba-runtime`, or an `oxvba-hal::HostServices` call). This is the analog of a typelib's interface + a DLL's implementation, and the same mechanism `Declare Lib` and host primitives use. We do **not** reimplement anything; the descriptor *declares the interface and points each body at its native hook*. A member could in principle instead carry a VBA body (`dispatch: VbaProc`) — the model allows it — but the base library's bodies are native. The emit dispatch becomes `(NativeImplId, args) → Instruction::…` (typed) instead of `(name: &str, args) → …`; the base-library descriptor is built once and cached (`OnceLock`).

### 4.3 Primitive core, library-supplied semantics (the target)
The end goal is a **primitive core language** — syntax, control flow, binding/resolution, and value/slot machinery — with as much *semantics* as possible supplied by the library and referenced uniformly:
- Conversions (`CStr`, `CLng`, `Val`, …) are library functions, not core built-ins.
- **Operators** (`+`, `&`, `Mod`, comparisons, …) are, at the limit, library operations (`op_Addition`-style members) the core resolves a syntactic operator to — not semantics baked into the typer/emitter.

This is the direction, not the first step: operators are currently wired deep into parser/typecheck/emit, so moving `op_Addition` to the library is a later, dedicated phase. Near-term the library owns constants, functions, and the predeclared objects; operators stay core until then. Stating the target keeps each step honest about whether it moves *toward* a primitive core or entrenches built-ins.

### 4.4 Library interface vs compiler-internal intrinsics (the partition)
A name is either a **library interface member** — it would appear in the library's public surface (`Len`, `CStr`, `MsgBox`, `Collection.Add`) and carries a native body — **or** a **compiler-internal structural intrinsic** (`__oxvba_array_field_*`, `__oxvba_withevents_*`, `dispatchinvoke`, `VarPtr`/`StrPtr`/`ObjPtr`, `__oxvba_project_instance`): plumbing that is *not* part of any library interface and stays internal (much of it disappears as the HIR matures). The three drifted legacy lists (`is_builtin_intrinsic_name`, `intrinsic_spec`, `is_intrinsic_call_name`) are reconciled along **this** line — not merged blindly; their meaningful membership differences (e.g. `cstr`/`val` recognized for HIR lowering but with no legacy `intrinsic_spec` arity) are resolved deliberately, with the full suite + conformance corpus as the parity gate.

### 4.5 Acknowledged syntactic integration points (the quirks)
A small, **explicit** set of library surfaces hook into syntax in ways a pure external reference could not — enumerated exceptions, not the norm:
- **`Err`** — members hook the VM error-state (`Err.Number`, `Err.Raise`, `Err.Clear`).
- **File I/O statements** — `Open … For … As #`, `Print #`, `Input #`, `Line Input #`, `Write #`, `Close`, `Get`/`Put`: bespoke statement grammar; the parser keeps the syntax, the semantics resolve to library callables (not a hardcoded opcode).
- **`Debug`** — `Debug.Print` / `Debug.Assert` statement-style member use.
- **`Mid` / `LSet` / `RSet` statement forms** — e.g. `Mid(s, i, n) = …` as an assignment target.

Everything else resolves as an ordinary library reference.

### 4.6 Host library
`Application`, `ThisWorkbook`, etc. are a *separate* host-injected layer (layer 5) with the same interface shape; it needs a host-supplied member catalog (new data — the one genuinely-missing metadata source). The base library never contains host globals.

## 5. What this deletes (the payoff)

Once Phases 1–4 land, these are removed (not quarantined):
- The 3 intrinsic name tables: `is_builtin_intrinsic_name` (`frontend_hir.rs`), `intrinsic_spec` / `is_intrinsic_call_name` (`resolve.rs`).
- The `is_*_stmt` text-prefix recognizers and hardcoded built-in `HirStmtKind`→opcode mappings (`frontend_hir.rs`).
- `Debug`/`Err`/`Collection` name-rewrites (`resolve.rs`) → real declared library objects.
- `rewrite_early_bound_member_dispatch` and the `project.rs` class-construction / member / default-member / field-array source rewrites (incl. the `bd-aprs.9.13` carrier and the PMR carriers).
- Source-flattening (`full_hir_source` / `active_project_hir_source` concatenation).
- `project_compile_boundary` and its `ActiveHir`/`FullHir`/`FullLegacy` split (the binder handles all shapes) — and with it the `LegacyFallbackAfterHirUnsupported` route and the legacy `resolve.rs` compile path.

## 6. Build sequence (each step is a real piece of the end state — no throwaway scaffolding)

- **Phase 1 — Resolution environment skeleton + base library (proving ground).**
  Introduce `ResolutionEnvironment` + the `SymbolSource` layer interface and the uniform `CallableBinding`/`DispatchRoute`; refactor `resolve_name`/member resolution to consult it. Implement the base-library layer (iii): descriptor whose callables/objects resolve to `Native(NativeImplId)` over existing VM/HAL impls; route constants/functions/predeclared-objects through it. Delete the 3 name tables, the `Debug`/`Err`/`Collection` rewrites, and the `is_*_stmt` hardcoding. Self-contained (no project loading); proves the environment + member dispatch on `Debug`/`Err`/`Collection`; net-deletes the most hardcoding.
- **Phase 2 — Multi-module project binding.** Add the sibling-module source (layer 2). Bind each module against the environment; retire source-flattening for project compiles. `resolve_name` binds cross-module procs/types/classes to real symbols.
- **Phase 3 — Project references + project-class member dispatch.** Referenced-project layer (layer 3, qualified). Declared-type member dispatch for project classes (§3.2) → retires the project.rs class/member/default-member/field-array rewrites (closes `bd-aprs.9.13`, `8.3/8.4/8.7`, `9.12` by deletion, not quarantine).
- **Phase 4 — COM typelib + host globals.** COM layer consuming `TypeLibMetadataBlob` (member dispatch by token) → retires `rewrite_early_bound_member_dispatch` (`8.6/8.8`). Host-global layer + host member catalog.
- **Phase 5 — Boundary collapse + legacy deletion.** Remove `project_compile_boundary`, the legacy rewrite/compile paths, and the per-construct fallbacks. The new path is the only path (`bd-aprs.10.2`, `10.8`).

Each phase is independently shippable, net-deletes code, and is validated by the differential harness (HIR vs legacy) until the legacy path is removed in Phase 5.

## 7. Design decisions (alternatives + trade-offs)

1. **Base-library representation — CHOSEN: (iii) declarations + native-impl binding.**
   - (i) VBA-source library: maximal dogfood, but needs a "body is native" marker and can't express operator/statement surface; (ii) pure descriptor blob: precise but a second representation; **(iii)** declarations resolved like any reference, dispatching `Native(NativeImplId)`→native impl: matches real VBA (`VBA` typelib + native impl), reuses the reference/dispatch path, satisfies "partially VBA-level binding to Rust." We are **not** writing `.bas` for built-ins; we bind to them at the reference level.

2. **Uniform `CallableBinding`/`DispatchRoute`; `NativeImplId` a typed enum, not a name string, and shared across sources.** Every source resolves to one `CallableBinding`; only `dispatch` differs (`VbaProc`/`Native`/`Com`/`HostDynamic`). `NativeImplId` is the route for natively-backed members (base library, `Declare`, host primitives) — **not** a library-only id — typed as an enum for compile-time exhaustiveness over the ~80 impls (the lesson from the `syntax_kind` typing). Active and referenced projects use `VbaProc` and get no native id. Trade-off: the existing emit dispatch is string-keyed; we re-key it to the enum (mechanical, removes the silent `_ => {}` fall-through). **Rejected:** a per-source binding soup (`LibraryCallable`/`ComCoclass`/`ProjectRefItem`) — it rebakes the special-casing this design removes.

3. **Symbol-source acquisition — recommend a lazy `SymbolSource` trait stack, not an eager merged `SymbolModel`.** Lazy querying scales to large reference sets and supports COM-on-demand resolution; eager merge is simpler but rebuilds everything per compile and bloats the hot path. Base-library descriptor cached via `OnceLock`.

4. **Statement-syntax built-ins — keep dedicated grammar, drop hardcoded lowering.** VBA requires the statement syntax; but the node resolves to a `Callable` (`dispatch: Native`) rather than a builder text-match → fixed opcode. Alternative (generic "library statement call") rejected: loses the syntactic fidelity the CST needs.

5. **Typed-symbol model — recommend resolved bindings as attached facts, keep HIR node kinds.** Extend the existing side-table pattern (`HirTypeHooks`) so each name/member expr carries its resolved `Binding`; lowering reads the binding. Avoids a disruptive rewrite of `HirExprKind` while giving lowering everything it needs.

6. **Resolution/shadowing order — explicit, tested.** Layer order as in §2; user-defined names may shadow library names within scope (VBA semantics — confirm and regression-test); qualified prefix selects the layer. This is a behavior contract, not an implementation detail.

7. **Descriptor home — base library compiler-owned initially; host library host-injected.** The base-library descriptor lives in `oxvba-compiler` (compile-time, cached). The host library arrives through a host-injection seam so a host (Excel/CLI/headless) extends or swaps it with no compiler edit. Revisit a shared-crate home if the host must read the base descriptor.

## 8. Risks
- **Hot-path cost** — base-library descriptor must be built once (`OnceLock`); per-compile rebuild would regress compile time.
- **Resolution-order regressions** — introducing default layers changes lookup; mitigate with an explicit tested order + shadowing regressions.
- **Schema scope for predeclared objects** — `Debug`/`Err`/`Collection` are more than constants; Phase 1 pins the member-descriptor schema against exactly today's consumers before expanding coverage.
- **Differential safety** — keep the legacy path runnable behind the differential harness through Phases 1–4; delete only in Phase 5 once parity is proven.
- **Host-global metadata is genuinely new data** — host catalog has no existing source; scope it conservatively in Phase 4.

## 9. Out of scope / unchanged
- Conditional-compilation constants (`#If`, `VBA7`, `Win64`) stay a preprocessor concern (`builtin_pp_constants`), not a library layer.
- The bytecode/metadata package contract and the VM/JIT consumers are unchanged — this is a front-end (source → package) design; impls remain the existing VM ops + HAL.
- No Office/host object-model parity is claimed; only the injection seam.

## 10. Bead mapping
- Resolution environment + base library (Phase 1): the unstarted `DEFAULT_HOST_PROJECT_VBA_LIBRARY` workset (assign a bead), built directly to its end-state ("C") on the new environment.
- Multi-module + project-class dispatch (Phases 2–3): FE-7.3/7.4 (`bd-aprs.8.3`, `8.4`, `8.7`), FE-8.5.c (`9.12`), and the field-array carrier `bd-aprs.9.13` (now a *deletion*, not a quarantine).
- COM/host references (Phase 4): FE-7.6/7.6.a (`bd-aprs.8.6`, `8.8`).
- Boundary collapse + legacy deletion (Phase 5): FE-9.2/9.8 (`bd-aprs.10.2`, `10.8`).
