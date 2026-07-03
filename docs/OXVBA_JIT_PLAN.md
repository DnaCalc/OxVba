# OxVba — JIT Plan

**Status:** Plan (design-verified 2026-07-02; two design passes + a fresh-eyes convention sweep folded in). M4-0 baseline implementation is complete; evidence is recorded in `docs/evidence/perf/JIT_M4_BASELINE_20260703.md`. M4-1 IR-prep implementation is complete; evidence is recorded in `docs/evidence/jit/JIT_M4_IR_PREP_20260703.md`. M4-2 runtime ABI implementation is complete; evidence is recorded in `docs/evidence/jit/JIT_M4_RT_ABI_20260703.md`.
**Scope:** M4 — the Cranelift JIT backend: architecture, design, and implementation program, from IR-prep through full corpus parity, typed fast paths, JIT-generated COM vtables, and AOT PE export.
**Companion documents:** `OXIR_VM3_ERROR_MODEL.md` (the normative error semantics this plan compiles), `spec/JIT_V2_RUN_PROTOCOL_V1.md` (the shared vm3/JIT activation and entry sequencing contract), `AOT_CRANELIFT_PE_EXPORT_DESIGN_2026-06-20.md` (the AOT packaging substrate §11 builds on), `VM3_COMPLETION_AND_VM2_RETIREMENT_PLAN.md` (the predecessor plan whose workset idiom this document follows).

---

## Readiness assessment

**Verdict: READY.** The prerequisites that are missing are well-bounded IR/infrastructure work that forms this plan's first worksets (M4-0..M4-2), not blockers.

What is in place:

- **vm3 is a complete, oracle-validated executable specification.** M3 is complete; the `bd-4ktq` spec-gap epic is closed (all Critical/High SilentWrong items fixed; the ~70 remaining Tier-3/4/5 inventory items are open but non-blocking and covered by the lockstep rule in §9). Since the vm2-retirement worksets executed, vm3 is the **sole product runtime** — comhost cdylib, CLI, and host sessions all run `OxImage` on vm3; `oxvba-vm2` is deleted.
- **OxIR was designed for native lowering.** It is a fully typed basic-block CFG: 65 `OxInst` variants and 12 terminators (`crates/oxvba-oxir/src/inst.rs`), an 18-variant type lattice with documented machine-representation invariants (`ty.rs`, `box_unbox_is_identity`), explicit per-statement fault edges (`OxBlock.fault_target` + `FaultDispatch { resume, resume_next }`), `is_fallible()` contracts on every op, typed COM descriptors interned inline in the program, and an existing verifier (`verify.rs`). The instruction documentation itself says fallible ops "lower cleanly to a branch on a returned status" — the IR anticipated exactly the compilation model in §3.
- **The runtime is already a library.** ~73K lines of runtime services are cleanly separable and JIT-reusable: `oxvba-eval` (arith/compare/coerce as free functions), `oxvba-lib::invoke` (every builtin), `oxvba-runtime` (Variant/BSTR/SafeArray/VbaRecord/ObjectRef + the termination queue), `oxvba-com` and `oxvba-hal` behind `&dyn HostServices`. Only ~6K lines of `oxvba-vm3` are interpreter-specific.
- **The regression net exists.** ~300-program conformance corpus; `crates/oxvba-differential/vm3_golden.snap` (insta; re-bless via `OXVBA_BLESS_GOLDEN=1`); live-Excel oracle captures under `docs/evidence/conformance/`; live com_matrix suites in `oxvba-host`; six differential axes (result values, Err state, side-effect journal, Init/Terminate timing, COM transport counts, COM typed-arg fidelity).
- **The error model is normative and largely IR-encoded.** R1–R14 in `OXIR_VM3_ERROR_MODEL.md`; fault edges, landing pads, `ClearErr`-on-Exit, and GoSub terminators are already in the IR, so most of the model is shared between engines by construction.

What is missing (→ M4-0..M4-2):

1. **No escape analysis.** `OxLocal.escaped` exists but is always `false` (`elaborate/lower.rs`, param construction). Required before scalars can be register-promoted.
2. **Box/Unbox never emitted.** `OxInst::Box`/`Unbox` are defined but elaboration inserts no typed↔Variant boundary conversions. Additionally, **`OxFunc` has no temp type table** — `TempId` is an untyped index (vm3 stores temps in a `HashMap`).
3. **No performance baseline.** Zero criterion infrastructure; only wall-clock regression tests (`array_access_perf_vm3.rs`).
4. **No Cranelift integration.** `oxvba-jit` is a deliberate stub: the v1 prototype was removed because it silently fell back to VM execution and built on the interpreter's Variant-slot model. v2 — this plan — lowers the typed OxIR.

---

## §0 Fixed design principles

1. **vm3 is the executable spec.** The JIT must match vm3 on all six differential axes over the full corpus. Every design decision below is scored first on "can this diverge from vm3?", second on performance.
2. **Shared runtime, not shared execution model.** Runtime services are reused as libraries behind `extern "C"` shims. vm3's frame/slot machine is *not* reused — that is the documented cause of JIT-v1's deletion.
3. **The typed IR is the point.** Wherever the IR carries static types — operands, calling signatures, array element types, COM member descriptors — the generated code and its runtime boundaries are typed. Variant-shaped machinery exists only where the language is genuinely dynamic. (Three early-draft decisions failed this test and were reversed; the decision histories are recorded in §5.1, §5.5, and §6 so the mistake is not re-made.)
4. **No native exceptions.** Cranelift has no usable Win64 unwind/EH story (no SEH registration for JIT frames, no landing pads). All VBA faults are status-code returns. This matches OxIR's explicit fault edges by design — it is the model, not a stopgap.
5. **Phased inside one architecture.** Phase A (helper-heavy, correct) commits to the same typed value model and typed calling convention that Phase B (inline fast paths) needs, so B is additive per-op work, never a representation migration.
6. **No silent fallback, ever.** An unimplemented construct is a whole-program clean `Unsupported` decline, tracked by a monotone scope ratchet. Mixed vm3/JIT execution exists only as a loudly-logged `jit-debug` bisection facility, unreachable from product configuration.

---

## §1 Crate architecture

### 1.1 The dependency problem

JIT-compiled code cannot contain VBA semantics inline (Phase A is helper-heavy by design); it calls runtime services through a C-ABI shim surface. Those shims need `oxvba-eval`, `oxvba-lib`, `oxvba-runtime`, `oxvba-com`, and `oxvba-hal`. Meanwhile `oxvba-vm3` must never grow a cranelift dependency, and nothing the shims depend on may depend back on them.

| Option | Shape | Verdict |
|---|---|---|
| A. Extend `oxvba-runtime` with the shim surface | no new crate | **Cycle.** `oxvba-eval`/`oxvba-lib` depend on `oxvba-runtime`; the shims must call *into* eval/lib/com. Runtime is a leaf below them; hoisting shims into it inverts the DAG. Ruled out structurally. |
| B. Shims inside `oxvba-jit` | one JIT crate | No cycle, but the shim surface is pure runtime code with zero cranelift content: burying it under cranelift deps taxes every shim unit test with cranelift compile time, and kills the AOT seam — the stub-loader design needs the helper surface as an ordinary linkable rlib/staticlib. Also the shared-state extraction (§4) must live somewhere vm3 can depend on, and vm3 must not depend on `oxvba-jit`. |
| **C. New crate `oxvba-rt-abi`** | runtime ABI + shared execution state | **Recommended.** Clean DAG, shims testable without cranelift, AOT-ready (the same crate compiles as the AOT support library). Forced anyway the moment vm3 and the JIT share any runtime state object: that type must live below both, and `oxvba-runtime` cannot host it (it needs `LibContext` from `oxvba-lib` and `&dyn HostServices` from `oxvba-hal`). |

### 1.2 Crate diagram (target state)

```
                 oxvba-syntax → oxvba-symbol → oxvba-bind → oxvba-oxir
                                                              │  (OxProgram / OxImage,
                                                              │   + M4 prep passes & verifier exts)
        oxvba-hal   oxvba-runtime   oxvba-eval   oxvba-lib   oxvba-com
           ▲             ▲              ▲            ▲           ▲
           └─────────────┴──────┬───────┴────────────┴───────────┘
                                │
                        ┌───────┴────────┐
                        │  oxvba-rt-abi  │  NEW: ExecState, ErrEngine (R1–R14 cells + routing),
                        │  (no cranelift)│  ProcInvoker seam, extern "C" shim surface,
                        └───┬────────┬───┘  test-build live-handle counters
                            │        │
              ┌─────────────┘        └─────────────┐
        ┌─────┴─────┐                        ┌─────┴─────┐
        │ oxvba-vm3 │                        │ oxvba-jit │ ← cranelift-{codegen,frontend,
        │ (interp)  │                        │           │    module,jit,native}
        └─────┬─────┘                        └─────┬─────┘
              └───────────────┬────────────────────┘
                        ┌─────┴──────┐
                        │ oxvba-host │  ExecBackend selection → CLI / comhost / sessions
                        └─────┬──────┘
                    ┌─────────┴──────────┐
                    │ oxvba-differential │  Executor::{Vm3, Jit}, golden, balance axis,
                    └────────────────────┘  fuzz, criterion benches
```

DAG invariants (enforced by `cargo tree` checks in M4-2's verify gate): `oxvba-vm3` has no path to cranelift; `oxvba-jit` has no path to `oxvba-vm3`; cranelift appears in exactly one crate.

`oxvba-rt-abi` contains:

- **`ExecState`** — the VM-agnostic observable runtime state (§4).
- **`ErrEngine`** — the R1–R14 state cells + the single routing decision, extracted from vm3 (§3.1).
- **`ProcInvoker`** — `invoke(&mut ExecState, prog, func, args) -> Result<Variant, Fault>`. vm3 installs an adapter over `run_proc_with_values`; the JIT installs a lookup into its compiled-function table. This is the re-entrancy spine (Terminate drain, event delivery, RaiseEvent fan-out run user code through it) and, deliberately, the seam a future M6 tiering engine plugs into.
- The **`extern "C"` shim surface** (§5.5–§5.6), unit-testable from plain Rust.

### 1.3 Engine selection

No backward-compat constraints on internal APIs, so do it cleanly:

- `pub enum ExecBackend { Vm3, Jit }` in `oxvba-host`; `HostConfig { backend: ExecBackend }` **replaces** `enable_jit` (delete it). CLI: `--backend vm3|jit` canonical, `--jit` kept as alias.
- comhost cdylib: `OXVBA_BACKEND=jit` env override read at `Engine::new` (no API change for COM clients).
- `Executor::Jit` in `oxvba-differential`; every existing runner works unchanged once `HostConfig.backend` exists.
- Honest decline: a JIT compile hitting an unimplemented construct surfaces as the standard `Unsupported("jit: <op> not lowered")` outcome — the exact vm3-M2 bring-up pattern the harness already skip-tracks.

---

## §2 Compilation unit & module strategy

**Decision: full AOT compile of the whole OxImage at session load.**

| Alternative | Pros | Cons |
|---|---|---|
| **(a) Compile every `OxFunc` of every `OxProgram` at image load** | Deterministic (no warm-up divergence visible to the corpus); direct calls everywhere (all callees known at relocation time); one `finalize_definitions()`; matches vm3's "whole image present" model | Pays compile time for dead functions — irrelevant at VBA scale (Cranelift compiles on the order of MB/s of CLIF; see §10 compile budget) |
| (b) Lazy per-function tiering | faster start for huge projects | call patching/trampolines, non-deterministic compile timing interleaved with user side effects — a class of JIT-only bugs the oracle cannot see. **Pre-designed as a contained fallback** behind the function-table indirection if the compile budget breaks (§10), not built speculatively. |
| (c) One JITModule per function | isolation | pointless relocation complexity, no direct calls |

Flow (`JitEngine::compile(image: &OxImage, …) -> CompiledImage`):

1. `JITBuilder::with_isa(native_isa)`; every runtime shim registered by address via `JITBuilder::symbol("rt_…", fn as *const u8)` — explicit registration; Windows has no dlsym-style global namespace for Rust statics.
2. **Declare pass**: for each `(prog p, func f)` declare `Linkage::Local` function `ox$p{p}$f{f}$<sanitized name>` — uniqueness from the indices; the name suffix exists for `OXVBA_JIT_TRACE` diagnostics only.
3. **Define pass**: lower each `OxFunc` with `cranelift_frontend::FunctionBuilder` (per-function context cleared between functions).
4. One `module.finalize_definitions()` (flips W^X page permissions); harvest finalized code pointers into per-program **function tables** `Vec<*const u8>` on the `CompiledImage`. The tables back all indirect dispatch: `CallProcRef`, `CallByName`, class/property member dispatch, event routes, entry thunks — and they are the patchable seam M6 tier-up needs.
5. **Cross-program `CallExtern`**: the whole image compiles together, so `ImportId → (prog, func)` resolves at define time to a **direct call**. No PLT/GOT machinery; `is_pic = false`.
6. **Globals**: one Rust-owned `Box<[Variant]>` per program (16-byte slots), base pointers on the run context. Rust-side ownership means image teardown releases globals exactly like vm3 dropping its slot vectors. Compiled code addresses a global as `global_bases[p] + 16·g`.
7. **`global_initializer` / `entry`** are ordinary `OxFunc`s. A Rust **driver** replicates vm3's exact run protocol — per-program initializer order, entry invocation, Halt semantics, end-of-run drain — extracted as a documented shared contract in M4-2 so both engines follow one sequencing spec.
8. **`Backend` trait** over `cranelift-module` so that `JITModule` vs `ObjectModule` is one file's concern. This is the AOT seam (§11); code outside that file must not call `JITModule`-only APIs.

---

## §3 Error / fault model compilation

This section compiles `OXIR_VM3_ERROR_MODEL.md` (R1–R14). Key observation: most of the model already lives in the IR — explicit fault edges, per-statement landing pads ending in `FaultDispatch`, `ClearErr` on Exit paths (R10), `ErrorHandler::ClearActiveError` for `On Error GoTo -1` (R13), `GoSub`/`GoSubReturn` terminators (R12), `Raise { inherit }` (R11). Both engines consume the same blocks, so those rules are shared by construction. What remains engine-side is a small set of state cells and one routing decision.

### 3.1 ErrEngine: shared implementation, not replication

| Alternative | Analysis |
|---|---|
| Full extraction of vm3's dispatch loop into an exec-core crate | The 6,100-line vm3 entangles fault routing with frame management, `Loc` resolution, and instruction dispatch. Carving out the whole propagation loop is a multi-week refactor **of the oracle itself**, which would then need re-validation. Rejected. |
| Pure replication in the JIT, differential corpus as the guarantee | The routing decision has genuinely subtle cases (handler re-raise, `Resume` without active error → 20, Err inheritance fields, Erl copy timing) where silent drift between two hand-written copies is exactly the bug class that survives until a corpus program happens to hit it. Rejected. |
| **Extract the cells + decision function only — `ErrEngine` in `oxvba-rt-abi`** | The engine-side surface is small: cells (`error_mode`, `active_error`, `err: ErrState`, `erl_line`, `last_dll_error`) with per-activation save/restore, plus one function `route_fault(fault) -> FaultAction { Handle(pad) | ResumeNext | Propagate | Fatal }` encoding single-shot Goto demotion (R9) and the Resume-legality check. vm3 is re-pointed in the same workset; **the gate is a byte-identical vm3 golden snapshot** — the proof that extraction is behavior-preserving. **Recommended.** |

GoSub stacks stay engine-private: they are pure control state (per-frame LIFO of block references) with no semantic content to drift.

### 3.2 The dynamic-block-target problem

Three mechanisms jump to a **runtime-held** block: the handler in `error_mode = Goto(h)`; the `resume`/`resume_next` seeds captured in `active_error`; `GoSubReturn`'s popped return block. Native code cannot jump to a dynamic address without a table. All such targets are intra-function (the cells are per-activation, per the model doc) and statically enumerable:

- `H` = all `SetErrorHandler(GotoLabel(b))` targets in the function,
- `S` = all `{resume, resume_next}` seeds on the function's `FaultDispatch` pads,
- `G` = all `GoSub { ret }` blocks.

| Alternative | Analysis |
|---|---|
| `br_table` over **all** blocks of the function | trivially correct, but O(blocks) table per function and — worse — it *hides* invariant violations: a corrupt index jumps somewhere plausible. |
| **Compact table over `T = sort(H ∪ S ∪ G)`; runtime cells store the small index** | Table is tiny (typically < 10 entries). A bad index hits the `br_table` default → `trap` (compiler bug caught loudly). Storing function-relative indices makes the runtime state meaningless across frames — exactly the per-activation semantics R1 demands. **Recommended.** |

Each compiled function gets one **redispatch block**: a single `i32` block parameter → `br_table` over `T` → jump. `SetErrorHandler(GotoLabel(b))` stores `index_of(b)`; `FaultDispatch` seeds are passed as immediates.

### 3.3 Status-code return convention

Every shim and every compiled call returns `i32`:

```
ST_OK    = 0   // success
ST_FAULT = 1   // VBA fault pending; payload written to ExecState
ST_HALT  = 2   // End statement propagating (bypasses handlers, R14)
```

Every call site lowers to:

```
s = call <callee>(…)
brif s == 0        → ok
brif s == ST_HALT  → jump cleanup_epilogue(ST_HALT)
jump <this block's fault pad>          ; ST_FAULT — caller's pad = the call statement's pad,
                                        ; which is R2 (default propagation) for free
```

Fault details never travel in the return value: on `ST_FAULT` the shim writes the rich `Fault { number, description, source, help… }` into ExecState (preserving the M3 COM HRESULT→Err fidelity).

### 3.4 FaultDispatch lowering

```
; native pad block for FaultDispatch { resume: r, resume_next: n }
d = call rt_route_fault(state, idx(r), idx(n))
  ; ErrEngine: seats Err from the pending fault, then on error_mode:
  ;   None       → DISP_UNWIND
  ;   ResumeNext → active_error = Some{r,n,caught}; DISP_RESUME_NEXT
  ;   Goto(h)    → demote mode to None (R9 single-shot);
  ;                active_error = Some{r,n,caught=Goto(h)};
  ;                dispatch_target = idx(h); DISP_HANDLER
switch d:
  DISP_UNWIND      → jump cleanup_epilogue(ST_FAULT)   ; fault stays pending for the caller
  DISP_RESUME_NEXT → jump native(n)                    ; STATIC target — no table needed
  DISP_HANDLER     → t = load dispatch_target ; jump redispatch(t)
```

Note only the handler leg needs the table; `resume_next` is a static jump at each site.

### 3.5 Resume family and GoSub

`Resume` / `Resume Next` terminators (fallible — their block has a pad):

```
s = call rt_resume(state, KIND, &out_idx)
  ; no active_error → pending fault = err 20 "Resume without error"; ST_FAULT
  ; else: reset Err, clear the latch, RE-ARM the caught handler policy, out_idx = seed
brif s != 0 → this block's pad     ; err 20 routes like any fault
jump redispatch(out_idx)
```

`Resume <label>`: same helper for the check/reset/re-arm, then a **static** jump to the label block. `GoSub { target, ret }`: push `idx(ret)` onto the activation's gosub stack (unbounded LIFO, R12), static jump to `target`. `GoSubReturn`: pop (empty → err 3 via the pad) → `redispatch(out_idx)`.

### 3.6 Cross-frame unwinding

The callee runs its cleanup epilogue and activation-leave **before** returning `ST_FAULT`, so the caller's error cells are already restored when its pad dispatches — the native status return plays vm3's `propagate_fault` exactly, with the resume seeds static at each call statement's pad (so `Resume` in the caller re-runs the right statement). Driver-level `ST_FAULT` is the host terminate boundary (R1); `ST_HALT` truncates with no drain (R14).

---

## §4 Runtime state: ExecState and the JIT run context

**Principle:** every piece of vm3 state observable across a re-entrancy boundary (event delivery, Terminate drain, host session invoke) moves into a VM-agnostic `ExecState` in `oxvba-rt-abi`, and both engines operate on the same instance. Interpreter-implementation details stay engine-private. This is what makes re-entrancy work without a translation layer: a COM event arriving mid-run lands in `ExecState`, and whichever engine is active delivers it through the same subscription maps.

### 4.1 State-ownership table

| vm3 state today | Semantics | M4 home |
|---|---|---|
| `err: ErrState`, `erl_line`, `last_dll_error`; `error_mode`, `active_error` (+ per-frame saves) | Err object + handler policy, R1–R14 | `ExecState.err_engine` (`ErrEngine`, §3.1); save/restore via `enter_activation()/leave_activation()` in the frame shims |
| `pending_fault` | fault in flight to a pad | ExecState scratch cell (the flag is the return status) |
| pending-termination queue + `draining` guard | Class_Terminate fixpoint | queue stays global in `oxvba-runtime` (it already is); the `draining` guard moves to ExecState |
| `withevents`, `com_subscriptions` (+by-key), `pumping`, `withevents_iters`, `project_event_sink`, ordering counters | event fabric | ExecState (moved verbatim) |
| `for_each`, `param_array_aliases`, `as_new_slots` (Loc-keyed maps) | loop/alias/lazy-init bookkeeping | **engine-private.** These maps are interpreter bookkeeping for information the JIT has statically: For-Each state lives in a hidden per-lexical-loop-site stack slot (keyed at compile time by the iter place; per-native-frame ⇒ recursion-correct, matching vm3's frame-indexed Locs); AsNew guards are static per-place — the Nothing-ness of the slot *is* the state; ParamArray alias copy-out is caller-side static from `ArrayLiteral{aliases}`. No Loc-equivalent-key machinery is built; the differential corpus proves equivalence. vm3 keeps its maps unchanged. |
| `lib: LibContext` (Rnd seed) | builtin state | ExecState |
| `host: &dyn HostServices` | HAL | ExecState (all effectful shims reach it through the state pointer) |
| per-program globals, `class_descriptors`, predeclared singletons, `event_routes`, `next_instance_id` | loaded-image tables | ExecState.loaded[] |
| frames / temps / aliases / ip (interpreter); gosub stacks | activation records | engine-private |

`ExecState` is `!Send`; one per session/thread, exactly vm3's thread-local model.

### 4.2 The JIT run context

Every compiled function takes a hidden first parameter:

```rust
#[repr(C)] pub struct JitRun {
    pub exec: *mut ExecState,           // shared observable state (§4.1)
    pub fn_tables: …,                    // per-program FuncId → code ptr (typed + dynamic entries)
    pub image_meta: *const OxImageMeta,  // classes / COM descriptor arena / dispatch metadata
    pub activations: Vec<Activation>,    // shadow activation stack (parallel to the native stack)
    pub stack_limit: usize,              // native-stack guard watermark (§6)
    pub trace: TraceConfig,
}
#[repr(C)] pub struct Activation {
    pub prog: u32, pub func: u32,
    pub saved_err: ErrEngineSaved,       // R1/R2 save/restore
    pub erl_line: i32,
    pub gosub: SmallVec<[u32; 4]>,       // dispatch-table indices (§3.2)
}
```

`rt_frame_enter(ctx, prog, func) -> *mut Activation` pushes (saving the caller's error cells, zeroing the callee's per R1) and returns the record pointer, which the prologue caches so `SetLineNumber`/`ErlGet` are single inline stores/loads. `rt_frame_leave(ctx)` pops and restores — called from the unified cleanup epilogue on **all** exits, which is vm3's `propagate_fault` restore (R2) on the fault path.

---

## §5 Value representation & lowering

### 5.1 The typed model (central decision)

**Rejected: model (a), "every place is a 16-byte Variant slot".** Not merely slow — architecturally self-defeating:

- It reproduces vm2's type erasure, the exact thing OxIR exists to undo. Every op re-inspects tags at runtime; the JIT becomes a call-threaded interpreter with the dispatch loop unrolled (measured ceiling ~1.5–2× over vm3).
- It leaves no room for Phase B: inlining a Long add under (a) still needs tag checks and reboxing on both sides, so "add fast paths later" means changing the storage model — the rewrite, deferred.
- It makes `Box`/`Unbox` meaningless and wastes the IR's bit-identical payload invariant.
- Historically, JIT v1 died of it (recorded in `oxvba-jit/src/lib.rs`).

**Recommended: model (b), typed representation.**

**OxTy → CLIF type map** (Win64; matches the `ty.rs` machine-representation table exactly):

| OxTy | CLIF register type | Slot | Notes |
|---|---|---|---|
| Bool | `i8` (0 / −1) | 1 B | the ONE non-identity box: box = `sextend` to i16 (−1/0) in Variant, unbox = `ireduce`. **Never identity-fuse** (verifier rule). |
| Byte | `i8` | 1 B | unsigned ops |
| Integer | `i16` | 2 B | |
| Long | `i32` | 4 B | |
| LongLong / LongPtr | `i64` | 8 B | 64-bit target fixed for M4 |
| Single | `f32` | 4 B | |
| Double / Date | `f64` | 8 B | Date = OLE serial |
| Currency | `i64` | 8 B | scaled ×10⁴; mul/div helper-only (i128 kernel, §8) |
| Decimal | — | 16 B memory-only | by-pointer to helpers; no i128 in CLIF |
| Str / FixedStr(n) | `i64` (BSTR) | 8 B handle | owning |
| Object(_) | `i64` (IUnknown ptr) | 8 B handle | owning |
| Record(_) | `i64` (payload ptr) | 8 B handle | owning, value semantics |
| Array(_, _) | `i64` (SAFEARRAY ptr) | 8 B handle | owning |
| ProcRef | `i64` packed (prog:u32, func:u32) | 8 B | non-owning |
| Variant | — | 16 B slot, 8-aligned | always memory, always addressable |

**Two-tier place assignment:**

| Tier | Which places | Representation |
|---|---|---|
| **R (register)** | non-escaped, non-owning scalar locals + scalar temps | `cranelift_frontend::Variable` — full SSA promotion, phis for free |
| **M (memory)** | escaped locals; owning types; Variant; Decimal; ByRef params (incoming pointer); globals | explicit `StackSlot` / global cell |

**Bare-handle rule** *(decision history: an earlier draft proposed a "tagged-slot rule" — owning locals stored as full 16-byte Variants with statically-known tags — for zero-copy Variant-shim boundaries and one uniform cleanup helper. Rejected on the same grounds as the Variant calling convention in §6: it is the interpreter's slot shape leaking in, and that draft already admitted bare handles were the end state via a planned Phase-B "promotion" — i.e. a guaranteed migration. Do it once, correctly.)*

Owning-typed places (Str/Object/Record/Array) are stored as **bare 8-byte handles** in their machine representation:

- Cleanup is a **static type-specific release list** per function: `rt_bstr_release` / `rt_object_release` / `rt_array_release` / `rt_record_release`, all null-safe; the prologue zeroes handles. Variant-typed places keep 16-byte slots and the uniform variant release.
- **ByRef alignment bonus:** a ByRef String slot is exactly a `BSTR*`, ByRef Object exactly `IUnknown**` — the same shapes Declare and COM native conventions use. Typed ByRef, NativeThunk vtable slots (§6), and Declare writebacks all point at the slot directly, with no payload-offset arithmetic.
- Boxing into a Variant happens only at genuinely dynamic boundaries (late-bound COM args, `CallByName`, Variant-typed places) — a static, mechanical two-store site.

### 5.2 Ownership discipline (O1–O8) — the hard correctness area

vm3 gets lifetime correctness from Rust `Variant: Drop`. Native code must make every drop explicit. Invariants:

- **O1 — Slot ownership.** Every M-tier slot owns its contents at every instruction boundary. The prologue zeroes (null handles; `VT_EMPTY` for Variant slots), so the type-specific null-safe releases are always safe and idempotent-after-zero.
- **O2 — Helper out-params are moves.** Every helper writes its `dst` as a fresh owned value and never reads or frees prior contents. Therefore `dst` must be *dead* at call time — lowering always targets a per-op scratch, never a live place.
- **O3 — Store protocol.** "Place P ← result": (1) helper writes owned result into scratch S; (2) release P's old value (statically elided for non-owning types); (3) copy S→P; (4) S is dead (ownership moved). Compute-then-release makes `s = s & "x"` and `Set o = o.Child` safe; S≠P makes aliasing a non-issue. The release in (2) may **enqueue** a Terminate — it never runs user code inline (O6).
- **O4 — Operand reads are borrows.** Helper operands pass by pointer with no AddRef/clone; helpers that retain, clone internally (they already do — vm3 passes `&Variant` identically). Explicit `OxInst::AddRef` is lowered to a helper call, never elided.
- **O5 — Temps.** Single-assignment ⇒ each owning temp slot is written at most once between clears. `StmtBoundary { clear_temps_from }` lowers to a static release+zero sequence over owning temp slots ≥ the floor. Releasing already-empty slots is a free no-op. **This must replicate vm3's temp-clear timing exactly** — temp releases are Terminate-observable.
- **O6 — Terminate timing.** Releases only **enqueue** pending terminations (the shared `oxvba-runtime` queue). User `Class_Terminate` code runs only at `DrainTerminations` and at frame-pop points, via `rt_maybe_drain` — vm3's `maybe_drain` core extracted into rt-abi in M4-2, one implementation, re-entrancy guard in ExecState.
- **O7 — One cleanup epilogue per function.** Release all owning M-tier locals (excluding ByRef indirections — caller-owned) and owning temp slots, `rt_frame_leave`, `return status`. All exits — normal Return, fault unwind, Halt — jump here with `status` as a block parameter. This is what replaces Rust `Drop` on the unwind path.
- **O8 — ByVal owning args.** Caller materializes an owned copy in a scratch; callee clones into its own local in the prologue; the caller's scratch dies at its next temp clear. Two clones is vm3-equivalent; a move flag is a later optimization if profiling justifies it.

### 5.3 Escape analysis (IR-prep, precise rules)

A local L of function F is **escaped** iff any of:

1. `OxArg::ByRef(Local(L))` in any `CallProc` / `CallProcRef` / `CallExtern` / `RaiseEvent`;
2. `OxCallArg::ByRef(Local(L))` (writeback target) in any `CallNative` / `CallByName` / `ComCallEarly` / `ComCallLate`;
3. `Ptr { VarPtr | StrPtr | ObjPtr }` with L as source (address-taken);
4. Declare pointer-writeback target;
5. member of `ArrayLiteral { aliases }` (ParamArray element aliasing);
6. the `iter` of `ForEachInit`/`ForEachNext`;
7. ever the place of `AsNew`.

Globals are always M-tier and need no flag. Escaped **temps** are a computed per-function analysis set (temps have no IR metadata — see the temp type table in M4-1). The flag is only a tiering input for owning types (M-tier anyway); it is load-bearing for **scalars**, which otherwise become SSA Variables and lose their address. The pass lives in `oxvba-oxir` (vm3 and the verifier see the same flags) and is behavior-neutral for vm3 — gate: golden unchanged.

### 5.4 Box/Unbox insertion (scoped hybrid)

| | (i) IR-level insertion in elaboration | (ii) JIT-side materialize-on-demand |
|---|---|---|
| Oracle coverage | vm3 executes the converted IR first → the pass is validated against the golden **before the JIT exists** | conversions live only in the JIT; a bug there is only caught by the differential, conflated with every other JIT bug |
| Verifier | can enforce "Assign is representation-preserving" — a strong well-typedness invariant | Assign stays polymorphic, untypeable |
| IR churn | inflates instruction count; touches elaboration + snapshot review | zero |
| Phase B | Box/Unbox pairs are what fusion cancels (the identity invariant exists for this) | fast paths must re-derive boundary knowledge |

**Recommendation: scoped hybrid.** Do (i) for **Assign normalization only**: after the pass, every `Assign` is representation-preserving, with `Box`/`Unbox`/`Coerce` explicit — validated by an unchanged vm3 golden before any JIT consumes it; the verifier gains the "same-repr Assign" rule. Helper-argument boxing stays JIT-side (ii): with typed helper families (§5.5) it occurs only at genuinely dynamic boundaries, emitted mechanically by one `emit_operand_as_variant()`. Full insertion is revisited as a Phase-B fusion prerequisite. Unbox of helper results is `checked` unless the helper contract guarantees the tag.

### 5.5 Instruction lowering classification (Phase A)

**Typed-helper principle** *(decision history: an earlier draft routed ALL ops through Variant-pointer shims — "box operands into scratch Variants, call `oxjit_arith(*const Variant, …)`", letting the shim re-dispatch on tags. That is the same type-erasure-at-every-boundary disease as the rejected calling convention. Reversed.)*

Shims are **typed where the IR is typed**; Variant-shaped shims exist only where the IR is genuinely dynamic. The IR already separates the regimes: `NumericMode::Checked(ty)` ops have statically-typed operands and results — `oxvba-eval` computes them through exact typed kernels (`checked_binop` in i64, `currency_*` in i128) behind its Variant facade — while `NumericMode::Widening` is the Variant regime.

- **Checked lanes → typed shims**: `rt_add_i32(l: i32, r: i32, dst: *mut i32) -> i32` etc. — no boxing, no tag dispatch. Likewise Compare on statically same-typed scalars, Truthy on Bool, typed array element access.
- **Widening lanes / Variant operands → Variant shims.**
- **One semantic kernel, two facades** (M4-2): refactor `oxvba-eval` to expose the typed kernels as public typed entry points; the existing Variant-facade functions AND the new typed shims both delegate to them. No semantic is implemented twice — divergence between lanes is structurally impossible.
- Phase B then merely **inlines** typed-shim bodies (`iadd` + overflow branch, bounds-checked element address) — per-op local changes gated by the differential, never a representation change.

Classification:

**Inline CLIF (no call):** `Assign` (post-normalization moves; M↔M copies + O3 release), `Box`/`Unbox{unchecked}` (tag+payload stores/loads; Bool sext/ireduce), `LoadProcRef` (iconst), `SetLineNumber` (store to the cached activation), `StmtBoundary` (static release sequence + optional trace hook), terminators `Jump`/`Branch`/`Unreachable` (→ trap: compiler bug).

**Helper families (everything else):**

| Family | OxInsts | Backing |
|---|---|---|
| arith | Arith/Div/Pow/Neg/Concat/Compare/Logical/Not/Truthy/Coerce/Unbox{checked}/CompareObjectIs/TypeOfIs/VariantChanged/ValidateAssignment | `oxvba-eval` kernels (typed + Variant facades) |
| calls | CallNative → `oxvba_lib::invoke`; CallByName → member resolver | lib / image tables |
| COM | ComCallEarly / ComCallLate | HAL; per-site immutable descriptors in a compile-time arena, referenced by absolute-address iconst |
| objects | NewObject/NewExtern/Predeclared*/FieldGet/FieldSet/FieldArray*/AddRef/Release/DrainTerminations | `oxvba-runtime` object model |
| records | NewRecord/RecordGet/Set/LSet/ArrayGet/ArraySet | VbaRecord |
| arrays | ArrayLiteral/Append/Redim/Get/Set/Erase/Bound/ForEachInit/Next | SafeArray |
| events | WithEvents×5 / RaiseEvent | event fabric in ExecState |
| error | SetErrorHandler (helper — R5's Err auto-reset), ClearErr, ErrFieldGet/Set | ErrEngine |
| ptr | Ptr{VarPtr/StrPtr/ObjPtr} | address computation over M-tier slots |

AsNew-marked places: every operand read lowers through `rt_read_asnew` (lazy `Class_Initialize` ⇒ fallible — which is why `Assign` is fallible in the IR). **Nothing is hybrid in Phase A** — one mechanism per op keeps the first differential run debuggable.

### 5.6 Shim ABI and representative lowerings

All shims: `extern "C"`, registered by address, bodies wrapped in `catch_unwind` — a Rust panic must never unwind through Cranelift frames (UB on Win64: no unwind info is registered for JIT code). Panic ⇒ InternalError fault, or abort under `OXVBA_JIT_STRICT`. Fault payloads go to ExecState, never the return value. Representative signatures:

```rust
// typed lane (Checked): no boxing, no tags
extern "C" fn rt_add_i32(state: *mut ExecState, l: i32, r: i32, dst: *mut i32) -> i32;
extern "C" fn rt_currency_mul(state: *mut ExecState, l: i64, r: i64, dst: *mut i64) -> i32; // i128 kernel

// Variant lane (Widening / dynamic)
extern "C" fn rt_arith_v(state: *mut ExecState, op: u32, mode: u32,
                         l: *const Variant, r: *const Variant, dst: *mut Variant) -> i32;

extern "C" fn rt_array_get_i32(state, arr: i64 /*SAFEARRAY*/, idx: *const i32, n: usize,
                               dst: *mut i32) -> i32;                    // typed element lane
extern "C" fn rt_call_native(state, site: *const NativeSiteDesc,
                             args: *const Variant, n: usize,
                             byref_out: *const *mut Variant, dst: *mut Variant) -> i32;
extern "C" fn rt_com_call_late(state, site: *const LateSiteDesc, recv: *const Variant,
                               args: *mut Variant, n: usize, dst: *mut Variant) -> i32;
extern "C" fn rt_variant_release(state, v: *mut Variant);                // enqueue-only, infallible
extern "C" fn rt_bstr_release(state, h: i64);                            // null-safe, ditto per handle type
extern "C" fn rt_maybe_drain(state) -> i32;                              // runs Class_Terminate fixpoint
```

**`Arith { dst, Add, l, r, Checked(Long) }`** (block has fault pad F):

```
s = call rt_add_i32(state, v_l, v_r, &scratch_i32)     ; v_l, v_r are SSA i32 values — no boxing
brif s == 0 → ok ; brif s == ST_HALT → epilogue(HALT) ; jump F_native
ok: dst_var = load scratch_i32                          ; or direct Variable def
```

**`ComCallLate`**: box receiver + each `OxCallArg` into a contiguous scratch Variant array (Named/Omitted/Const shapes encoded in the static `LateSiteDesc`), call shim; on OK move `dst` per O3, then run the descriptor's ByRef copy-out list (the shim mutated the scratch array in place; copy-out uses the same `VariantChanged`-guarded shared function vm3 uses).

**`ForEachInit`/`ForEachNext`**: enumerator state lives in this loop site's hidden stack slot (§4.1); `rt_foreach_next(state, slot, item_out, has_out)` writes the owned item (O3 into the item place) and a Bool. SafeArray-vs-IEnumVARIANT dispatch is inside the shared helper — identical to vm3.

**`StmtBoundary { stmt, clear_temps_from }`**: static release+zero over owning temp slots ≥ floor (O5); under `OXVBA_JIT_TRACE=stmt`, additionally `call rt_trace_stmt(state, stmt)`. This is a real, labeled site — never compiled away (§14.5).

**`DrainTerminations`**: `call rt_maybe_drain(state)` — re-enters compiled code (Terminate bodies) through ProcInvoker; plain native recursion under the shared `draining` guard.

---

## §6 Calling convention — typed primary entry + dynamic-entry thunk

**Decision history.** An earlier draft selected a uniform Variant-pointer convention (`fn(ctx, ret: *mut Variant, argv: *const *mut Variant, argc) -> i32`) as the *only* convention, citing Optional/Missing and ParamArray marshaling complexity and the cost of per-signature thunks at indirect call sites. A design challenge prompted re-investigation, which **refuted all three premises**:

1. **The binder already normalizes every call to one-arg-per-param with caller-side defaults** (`oxvba-bind/src/call.rs`, `omitted_optional_arg`): an omitted optional binds its folded default expression coerced to the declared type, else the declared-type zero, else `Nothing` for objects. The `Missing` sentinel (`MISSING_ARG` = 0x80020004) is passed **only** for `Optional x As Variant` with no default — it can only ever land in a Variant-typed slot. Typed parameters never see it; no suppliedness channel is needed.
2. **ParamArray is boxed by the binder into one array argument** (call.rs: "keeps the call vector one-arg-per-param, so free procs and methods need no downstream variadic handling").
3. **The per-signature thunk machinery is required by the product regardless**: JIT-generated COM interface vtable implementations (typed native slots per `TypeLibMemberMetadata` — today comhost hand-implements only a bounded Automation-safe dual tier), AddressOf→Declare callbacks, native `.dll` exports with arbitrary signatures, and (Phase B) direct typed calls *into* native COM vtables and Declare targets replacing libffi. An untyped internal convention avoids none of that machinery — it only adds boxing at every VBA-internal call and pushes typed codegen to the edges where it must exist anyway.

Conclusion: the Variant-argv shape was vm3's frame representation leaking downstream. The typed OxIR exists precisely to enable typed compilation, and the call vector is already shaped for it.

### 6.1 Primary convention — typed native entry per OxFunc

Signature derived from `locals[0..param_count]` (each carries full `OxTy` + `OxParamInfo { by_ref, variadic }`):

```
fn(ctx: *mut JitRun, [me: instance ptr — class methods], ret: *mut <ret machine type / Variant>, params…) -> i32 status
```

- **ByVal scalar** → by-value machine register (i32/i64/f32/f64; Bool i8; Currency i64).
- **ByVal Variant/owning** → `*const Variant` / handle by pointer; callee clones into its own local (O8).
- **ByRef** → `*mut` typed slot: scalar params point at the caller's escaped machine-typed slot; String/Object/Array/Record at the caller's bare-handle slot (exactly `BSTR*` / `IUnknown**` shapes, §5.1); Variant at the 16-byte slot. Type-mismatched ByRef never reaches the boundary — elaboration already inserts compound copy-in/copy-out with the `VariantChanged` guard.
- **Return** via typed out-pointer (dead-on-entry, O2) rather than multi-return: keeps every compiled entry directly callable from Rust shims (ProcInvoker) over plain C ABI. The status `i32` is the sole register return.
- **Omitted** reaches only Variant params (finding 1): caller passes a scratch holding `MISSING_ARG`; callee writes to it are discarded (vm3 parity).
- **ParamArray**: one array argument (finding 2). Element-alias copy-out uses the shared helper vm3 uses. *Implementation-time confirmation:* verify the IR carries enough for caller-side copy-out from `ArrayLiteral{aliases}`; if not, a small IR-prep addition in M4-1.
- *Implementation-time confirmation:* mirror the `Me`-receiver convention exactly from vm3's class-method call path.

### 6.2 Dynamic-entry thunk

The uniform shape survives — demoted from "the convention" to a **JIT-generated per-function adapter**:

```
extern "C" fn(ctx, ret: *mut Variant, argv: *const *mut Variant, argc: usize) -> i32
```

Generated mechanically from the known signature: unbox/coerce each argv slot to the param's machine type (the same coercion semantics as vm3's late-bound path, via the shared coerce helpers — err 13 on mismatch), call the typed entry, box the result. Consumers: `CallByName`, late-bound class member dispatch (Untyped receivers), Property Get/Let/Set resolution, ProcInvoker (event delivery, Terminate drain, host sessions, the differential harness), and `CallProcRef` where the signature cannot be statically proven. Both entries coexist permanently by design — the dynamic entry is required for late binding regardless. Generate dynamic thunks lazily / only for dynamically-reachable functions if code size warrants.

- **Static dispatch uses typed entries**: `CallProc`/`CallExtern` are direct typed calls; `CallProcRef` with statically-known signature is a typed indirect call, else the dynamic entry via the fn tables.
- **Bring-up without a convention migration**: early worksets may route static calls through dynamic entries (they exist first, M4-3), but the typed entry is the convention of record — static call sites switch to typed calls in M4-4 as an optimization inside one architecture, not a migration.

### 6.3 NativeThunk generator

One component with a shared signature-derivation core; its consumers span the milestone:

1. **COM interface vtable implementations** (a stated aim of the JIT): per-member native slots `extern "system" fn(this, typed COM wire args…, retval*) -> HRESULT` — wire↔runtime conversions via the existing `oxvba-com` helpers (`disp_params_to_runtime_call_frame` / `variant_to_com_value`), call the typed entry, map `ST_FAULT` → HRESULT + IErrorInfo. This replaces and generalizes comhost's bounded hand-written dual tier to **arbitrary** TypeLib shapes (M4-13).
2. **AddressOf → native callbacks** (to vm3's current parity level only — presently declined; the JIT declines identically).
3. **DLL export functions** (M4-14; arbitrary signatures per the generic-native-export goal).
4. (Reverse direction, Phase B) **direct typed calls into native COM vtables and Declare targets**, replacing libffi on statically-descriptored sites — the same signature model, caller side (M4-12).

### 6.4 Recursion and stack overflow (error 28)

vm3 uses heap frames (effectively unbounded); the JIT uses the native stack. Two measures, both adopted:

1. Run compiled code on a **dedicated big-stack thread** (e.g. 64 MB) owned by the engine.
2. **Prologue guard** against `ctx.stack_limit`: on trip, seat err 28 "Out of stack space" and return `ST_FAULT` *before* `rt_frame_enter` (nothing to clean up) — a catchable VBA fault flowing through normal R2 machinery, like real VBA. Additionally match vm3's frame-depth ceiling constant so both engines raise 28 at the same depth. **Never** a Cranelift trap (not resumable, not catchable).

---

## §7 Coverage boundary and re-entrancy

**Compile 100% of the OxIR vocabulary; no vm3 fallback in the product path** (JIT-v1's epitaph). What routes through *shared runtime facilities* rather than JIT-specific code — in Phase A: Declare FFI marshaling, COM transports, the event fabric, member resolution, Terminate drain, builtins. Phase B moves statically-descriptored native calls to direct typed emission (§6.3 reverse direction); everything else stays helper-mediated.

Re-entrancy: `CompiledImage::invoke` (Rust → dynamic entry → status) is the entry thunk ProcInvoker uses for event sinks, Terminate drain, and RaiseEvent fan-out — plain native recursion under the shared `draining`/`pumping` guards in ExecState. Since ExecState is `!Send` and the activation stack is strictly LIFO, re-entrancy needs no special machinery beyond those guards.

---

## §8 Cranelift specifics

- **Crates**: `cranelift-codegen`, `cranelift-frontend`, `cranelift-module`, `cranelift-jit`, `cranelift-native` (+ transitive `target-lexicon`), pinned as one version family (wasmtime-3x era, ≥ 0.116; record the exact pin at M4-3). Win64 `windows_fastcall` fully supported for JIT code and extern "C" boundary calls.
- **Settings**: `opt_level=speed` (`none` under a debug env var for compile-debug loops), `is_pic=false` (JITModule), `enable_verifier=true` in debug/trace builds, `unwind_info=false` (nothing consumes it; helpers never unwind — `catch_unwind` in every shim), **NaN canonicalization ON** (the harness canonicalizes NaN for comparison, but journal-visible formatting must match too).
- **Traps are compiler bugs only**: `br_table` default, `Unreachable`, debug tag asserts. Trap handler formats function/offset and aborts with "JIT internal error" — never surfaced as a VBA error.
- **No i128 in CLIF**: Decimal is a 16-byte memory value handled by helpers; Currency mul/div overflow needs 128-bit intermediates → permanent helpers on `oxvba-eval`'s i128 kernels (bit-exact by construction).
- **Floats**: Phase A all-helper. Phase B may inline f64 add/sub/mul (IEEE on both sides). `/`, `^`, `Mod`, float→int narrowing (banker's rounding!), and comparisons stay helpers permanently unless proven — CLIF `fcvt*` trapping/saturating semantics do not match VBA's error-6/rounding rules.
- **Memory ops**: Variant copies as two i64 load/store pairs (slots 8-aligned); `MemFlags::trusted` on frame slots.

---

## §9 Verification strategy

vm3 is the oracle; the JIT earns trust the way vm3 did against vm2 — full-corpus differential on all six axes — plus new axes the JIT specifically endangers. The live-Excel oracle remains ground truth transitively (vm3 is oracle-validated; JIT ≡ vm3 ⇒ JIT ≡ Excel on the captured set).

- **`Executor::Jit`** in `oxvba-differential`; every runner works once `HostConfig.backend` exists. `RunOutcome` gains `handle_balance: Option<HandleBalance>` and `compile: Option<CompileStats>` (per-program compile time + declined-op list).
- **`vm3_jit_differential`**: every corpus program, both engines, axis-by-axis assertion. JIT `unsupported` ⇒ skip-with-record; vm3-unsupported-but-JIT-supported ⇒ **hard fail** (scope inversion is a bug).
- **`jit_golden`**: JIT outcomes rendered with the *same* `render_outcome`, diffed line-by-line against the existing `vm3_golden.snap`. **The JIT never gets its own truth file** — it matches vm3's or fails.
- **`jit_scope.snap` ratchet**: a blessed `{program → compiled | declined(reason)}` list. Transitions are reviewed diffs; compiled-count is monotone non-decreasing across M4-3..M4-11; exit criterion at M4-11 is declines == 0.
- **Live com_matrix × 2 backends** (`OXVBA_TEST_BACKEND`): Excel/Scripting/DAO, in- and out-of-proc, events V7/V8 — the only tests where compiled code meets a real COM apartment; mandatory from M4-9.
- **Handle-balance axis** (the #1 new risk class — leaks/UAF the value axes can't see): `oxvba-runtime` gains test-feature atomic counters for live BSTRs, object boxes, SAFEARRAYs, and record buffers at their single alloc/free choke points. Balance == 0 asserted per corpus program for **both** engines. Established on vm3 in M4-0 — flushing any pre-existing interpreter imbalance before the JIT can hide behind it (intentional `Box::leak` class descriptors exempted at the leak site). Plus: nightly ASAN (`-Zsanitizer=address`, MSVC nightly) over the differential subset, and poison-on-release (0xDD tag) canary Variants in debug so a UAF becomes a deterministic type-mismatch divergence rather than silence.
- **Fuzz differential (scoped narrow)**: a proptest generator emitting single-module programs — scalar/Variant expression trees to depth ~6 across all numeric carriers, `Dim` of every scalar type, If/Select/For/Do to depth 3, ByVal/ByRef call mixes, `On Error` wrappers. Full `RunOutcome` compared vm3-vs-JIT; shrunk finds land as **permanent corpus programs** (every fuzz find compounds the golden net). Nightly + on-demand. Grammar-complete VBA fuzzing (objects/COM/events) is an explicit non-goal — re-entrancy bugs are found by the corpus and live legs.
- **Numeric micro-corpus** (~15–20 programs, written in M4-0 so they harden the vm3 golden before any JIT exists): integer overflow at ±bounds (err 6 exactness), Currency scaled-i64 overflow, banker's rounding at .5 boundaries (incl. `CLng(2147483647.5)` → 6), `Int`/`Fix` vs conversion rounding, NaN/±Inf production and comparison, `-0.0`, Single↔Double round-trips, `\`/`Mod` at INT_MIN.
- **Bisection** (mixed vm3↔native frame interop as a product feature: **rejected** — Loc-into-native-slot lifetime hazards, split state, and its own bug surface would poison the signal):
  1. corpus-program minimization (programs are small);
  2. **statement-trace diff**: both engines emit identical `{prog, func, stmt, err.number, journal_len, depth}` records under trace; the first divergent record localizes to one statement — finer than function-granular toggling, with zero interop machinery;
  3. `OXVBA_JIT_DENY_FUNCS` deny-list mixed execution via the ProcInvoker seam — `jit-debug` feature only, loudly logged, never product-reachable; a binary-search driver isolates the diverging function, then diff its CLIF/disassembly.
- **Test-gate table**:

| Gate | What | When | Budget |
|---|---|---|---|
| `cargo test --workspace` | existing nets incl. vm3 golden | per-commit | unchanged |
| `vm3_jit_differential` | full corpus, both engines, all axes + balance | per-commit from M4-3 | target < 5 min for the pair (measure in M4-0; parallelize by program if over) |
| `jit_golden` | JIT ≡ `vm3_golden.snap` | per-commit from M4-3 | ~0 (one execution feeds both assertions) |
| `jit_scope` ratchet | compiled-set monotone, declines reviewed | per-commit | ~0 |
| fuzz differential | 10k cases | nightly / on-demand | ≤ 30 min |
| ASAN differential | sanitized subset | nightly | ≤ 60 min |
| live com_matrix × 2 | real COM + events | milestone close (M4-9+), release | operator-run |
| criterion benches | §10 suite | per-milestone + perf PRs | ≤ 10 min |

- **Lockstep process rule**: every vm3 semantic change (the remaining Tier-3/4/5 spec-gap work continues in parallel) lands **with a corpus program in the same commit** — the differential then covers the JIT automatically. A vm3-only fix without corpus cover is treated as an unverified change.

---

## §10 Performance program

**Baseline before the JIT exists** (M4-0): a criterion suite in `crates/oxvba-differential/benches/` (it owns the canonical run entry points and parameterizes over `Executor` for free), vm3 numbers recorded to `docs/evidence/perf/`. Bench fixtures (~50–200 ms vm3 runtime each; execution measured net of compile, compile time its own group):

1. `scalar_loop` — Long/Double arithmetic loop (the Phase-B headline)
2. `string_concat` — `s = s & …` and a `Mid$`-builder variant (BSTR churn)
3. `array_loop` — element read/modify/write over `Long()` and `Variant()`
4. `udt_fields` — record field get/set loop
5. `call_overhead` — tight ByVal + ByRef proc-call loops
6. `error_loop` — `On Error Resume Next` around a faulting op per iteration (fault-path tax, quantifying risk 3)
7. `collection_ops` — Add/Item/Remove churn
8. `com_late_vs_early` — fixed dispatch counts, both transports (proves the JIT doesn't regress COM)
9. `image_load` — `.oxi` JSON parse + prepare (feeds risk 10 and §14's conditional binary format)

**Checked-vs-Widening lane census** (M4-0): instrument counts over the corpus and real projects — what fraction of Arith/Compare ops carry static types? This grounds the expectations below; and if the binder is conservatively choosing Widening where types are provable, improving its Checked coverage lifts **both** engines and raises the JIT ceiling — file follow-on beads if the census shows headroom.

**Honest expectations**:

| Tier | Expected vs vm3 | Notes |
|---|---|---|
| Phase A, typed-dense code | **3–10×** | dispatch elimination + registers + typed shims (no boxing/tag-dispatch on Checked lanes) |
| Phase A, Widening/Variant-dominant | 2–5× | the census tells us the real mix |
| Phase A, string/builtin-heavy | 1.5–2× | helper-bound |
| Phase A, COM-bound | ~1× | transport-dominated |
| Phase B | 10–50× `scalar_loop`, 5–15× `array_loop`, 2–4× `udt_fields` | strings/COM ≈ Phase A |

Untyped user code (all-Variant) stays in Widening helpers until the M6 speculation tier — said plainly, not hidden.

**M4 perf exit gate** (with the full verification suite green): ≥ 10× vm3 on `scalar_loop`, ≥ 5× on `array_loop`, ≥ 2× geomean over benches 1–7, ≥ 0.95× (no regression) on benches 8–9.

**Compile budget**: < 100 ms typical project, < 1 s largest corpus image; per-function compile time tracked in the compile criterion group. If a real project breaks the budget: the lazy-compile fallback is pre-designed behind the function-table indirection (entries start as compile-me trampolines behind ProcInvoker) — a contained change, not built speculatively.

**M6 hooks preserved, not built**: patchable fn-table entries (tier-up point), StmtBoundary lowering centralized in one function (profiling/DAP hooks later without touching every op), the `Backend` trait (JITModule/ObjectModule).

---

## §11 AOT PE export

In M4 scope (user decision). Builds on `AOT_CRANELIFT_PE_EXPORT_DESIGN_2026-06-20.md`.

- **Artifact**: the `Backend` trait instantiated with `ObjectModule`/a serialized relocatable blob (the design note's "skip linking" camp, wasmtime-`.cwasm`-style). Per-build output = relocatable code blob + descriptor arena + metadata. **No system linker at user build time.**
- **Stub loader**: an author-side pre-linked stub PE (built once with MSVC/lld) containing the loader (map blob RW → copy → `VirtualProtect` RX, apply cranelift relocations, W^X hygiene) and the **host import table** — fixed slots of shim/runtime function pointers. The rt-abi surface compiles as a staticlib into the stub; this is precisely why the shims live in `oxvba-rt-abi`, not `oxvba-jit` (§1.1).
- **Export wiring — resolving the design note's open Option A/B question**: OxVba has *both* export shapes, so both options ship, phased:
  - (i) **WrappedComServer `.dll`** — fixed ABI (`DllGetClassObject`/`DllCanUnloadNow`/`DllRegisterServer`) → **Option B** (fixed thunk exports in the stub, slots wired at load) fits perfectly and keeps the append-a-blob model. Ships first (M4-14).
  - (ii) **Generic native `.dll` export** — per-program export names (the build-time-reflection generic mechanism) → **Option A** (bake the export table by PE surgery via the `object` crate). Ships second, sharing the blob+loader substrate; export bodies are NativeThunk-generated (§6.3).
- **Verification**: the corpus differential re-runs against AOT-loaded code — same ExecState/shims, so identical behavior is expected; the gate proves relocation/loader correctness. AV/SmartScreen risk (the packer shape) recorded: sign the stub, strict W^X, prefer reserved-section patching if flagged in practice.

---

## §12 Worksets (DAG; critical path M4-0 → … → M4-14)

```
M4-0 ──┬── M4-1 ──┐
       └── M4-2 ──┴── M4-3 → M4-4 → M4-5 → M4-6 → M4-7 → M4-8 → M4-9 → M4-10 → M4-11 → M4-12 → M4-13 → M4-14
                              (M4-6.5 fuzz ∥ from M4-3)
```

Sizes: S ≈ ≤ 2 days, M ≈ 3–5 days, L ≈ 1–2 weeks. The corpus lights up incrementally via the `jit_scope` ratchet; unimplemented constructs decline with clean `Unsupported` throughout.

### M4-0 — Baselines, gates, and plumbing (M)
Criterion suite + vm3 numbers recorded to `docs/evidence/perf/`; corpus wall-clock measured (CI budget baseline); live-handle counters in `oxvba-runtime` + `handle_balance` in `RunOutcome` + vm3 corpus balance green (imbalances found → beads filed/fixed first); numeric micro-corpus programs added and vm3 golden re-blessed; **Checked-vs-Widening lane census** recorded (follow-on binder beads if headroom); `ExecBackend` replaces `enable_jit` end-to-end (CLI/host/comhost/differential); `Executor::Jit` added (standard decline).
**Verify:** workspace green; vm3 handle balance == 0 on all corpus programs; benches < 5% run-to-run noise; census in evidence; `oxvba run --backend jit` exits with the standard not-implemented diagnostic. **Depends:** —

### M4-1 — IR-prep passes (L)
Temp type table on `OxFunc` (`temps: Vec<OxTy>`; elaboration records at `new_temp`; `.oxi` version bump); escape analysis per §5.3 (+ escaped-temps analysis set); Assign-normalization Box/Unbox/Coerce insertion (§5.4); fixed-array shape refinement where declaration bounds are static; verifier extensions (escape soundness, Assign representation-preservation, Bool never identity-fused, fault-closure/dispatch-set domains, temp typing). **Passes always-on for both engines — one canonical image** (a JIT-only IR pipeline would mean the differential compares different programs; disqualifying).
**Verify:** vm3 golden **byte-identical** with passes on; verifier green over every corpus image; escape corner unit tests (ParamArray alias, AsNew, ByRef-of-element). **Depends:** M4-0. ∥ M4-2.
**Implementation note (2026-07-03):** Complete in `bd-h4oh.2`; evidence is recorded in `docs/evidence/jit/JIT_M4_IR_PREP_20260703.md`. The remaining record-layout identity cleanup needed to remove the temporary `Object(Untyped) <- Variant` assign-normalization exception is split to M4-7 follow-up bead `bd-h4oh.9.1`. The only non-green lane is the non-blocking formal runner, which could not start in the current Linux environment because PowerShell is not installed.

### M4-2 — `oxvba-rt-abi`: ExecState, ErrEngine, kernels, shims v1 (L)
New crate per §1; extract `ErrEngine` (cells + `route_fault`) and `ExecState` (§4.1 table) out of vm3, re-point vm3; ProcInvoker seam with the vm3 adapter; run-protocol contract documented (the §2 driver sequencing); extract `maybe_drain` core, ByRef/ParamArray copy-out, and native-call marshaling out of `Vm3` methods into shared functions; **`oxvba-eval` typed-kernel facade refactor** (checked-i64 / currency-i128 / f64 kernels as public typed entry points; Variant facades delegate — one semantic kernel, two facades); shim surface v1: typed families for Checked lanes + Variant shims for Widening/dynamic, clone/release (incl. type-specific handle releases), `rt_lib_invoke`, `rt_maybe_drain`, err shims.
**Verify:** vm3 golden byte-identical after extraction (the behavior-preservation proof); balance still 0; shim unit tests incl. property-test equality of typed and Variant facades over the same kernels; `cargo tree`: no cycle, vm3 cranelift-free. **Depends:** M4-0. ∥ M4-1.
**Implementation note (2026-07-03):** Complete in `bd-h4oh.3`; evidence is recorded in `docs/evidence/jit/JIT_M4_RT_ABI_20260703.md`. The only non-green lane is the non-blocking formal runner, which could not start in the current Linux environment because PowerShell is not installed.

### M4-3 — Cranelift skeleton: straight-line code runs (M)
Pinned cranelift deps in `oxvba-jit` only; `Backend` trait over `JITModule`; JitRun/frame layout + dynamic-entry ABI; lower StmtBoundary/Assign/consts/arith-via-shims/Return; host compiles the image when `backend == Jit`; `jit_scope.snap` ratchet live.
**Verify:** first corpus slice green in the differential (values/err/balance axes); everything else declines cleanly; ratchet active. **Depends:** M4-1, M4-2.

### M4-4 — Control flow, calls, ByRef (M–L)
Branch/loops; **typed primary entries become the convention of record** (signature derivation from OxFunc params; typed direct calls for CallProc/CallExtern); dynamic-entry thunk generator (unbox/coerce/box adapters); activation save/restore; ByRef aliasing via typed slot pointers; ParamArray (incl. the alias copy-out confirmation flagged in §6.1); err-28 stack guard with vm3-matching ceiling.
**Verify:** control/call corpus slices green **through typed entries**; late-bound programs green through dynamic entries; deep-recursion program raises 28 identically on both engines (promote vm3's test shape to corpus). **Depends:** M4-3.

### M4-5 — Error model (L)
Fault-code convention on every fallible op; FaultDispatch lowering (§3.4); Resume/ResumeNext/Resume-label; On Error mode sets; GoSub/Return + err 3; `Erl`; `Err.Raise` defaults + inheritance; R10 Exit clears; R13 `GoTo -1`; R14 `End`.
**Verify:** the entire error-family corpus green on all axes (the largest thematic slice); `error_loop` bench recorded (fault-path tax visible early). **Depends:** M4-4.

### M4-6 — Strings, refcounting, temp lifetime (M)
BSTR ops via shims; O1–O8 discipline live; `clear_temps_from`; LSet/RSet/fixed strings; Mid-statement.
**Verify:** string corpus green **including the balance axis on every program compiled so far** (leaks would first appear here — the gate is already merciless); nightly ASAN job enabled. **Depends:** M4-5.

### M4-6.5 — Fuzz differential online (∥ from M4-3, S)
Generator per §9; finds land as permanent corpus programs.

### M4-7 — Arrays and records (M)
SAFEARRAY element load/store (typed + Variant lanes), Bound, ReDim(Preserve), Erase, unallocated semantics; record field get/set, With receivers.
**Verify:** array+record corpus green + balance; `array_loop`/`udt_fields` benches recorded. **Depends:** M4-6.
**Split-in from M4-1:** bead `bd-h4oh.9.1` owns threading record-layout identities into OxIR typing and removing the temporary `Object(Untyped) <- Variant` assign-normalization exception.

### M4-8 — Objects, classes, lifecycle (L)
New/AsNew, predeclared singletons, Is/TypeOf, member dispatch on project classes, Release{may_terminate}, DrainTerminations fixpoint through ProcInvoker (Terminate runs **compiled** code re-entrantly), project RaiseEvent → WithEvents fan-out.
**Verify:** lifecycle micro-corpus green under `Executor::Jit` incl. the Terminate-timing axis; drain re-entrancy (a Terminate dropping the last ref of another object) covered. **Depends:** M4-7.

### M4-9 — COM late/early, Declare, pointer helpers (L)
ComCall lowering (rich HRESULT→Err through the same shims vm3 uses); late IDispatch + early vtable transports (axis-5 counts must match); typed-arg writebacks (axis 6); Declare lane + `last_dll_error`; GetObject; pointer helpers.
**Verify:** COM corpus slice green; **live com_matrix legs green on the JIT** in- and out-of-proc. **Depends:** M4-8.

### M4-10 — Events, sessions, product surface (M)
COM WithEvents subscriptions + event pump delivering into compiled handlers (agile sink, thread-aware arg order); `ProjectRuntimeSession` on the JIT backend; comhost env selection; CLI `--backend jit` end-to-end incl. `.oxi` load-and-compile.
**Verify:** live com_matrix events V7/V8 green on the JIT; session twins green. **Depends:** M4-9.

### M4-11 — FULL PARITY GATE (S–M)
Close the scope ratchet: declines == 0; full corpus, all 6 axes + balance, 0 mismatches; `jit_golden` identical; fuzz ≥ 100k cumulative cases clean or findings landed; ASAN nightly clean; CI within budget.
**Verify:** the numbers above, recorded in `docs/evidence/jit/` (W9-style parity proof). **Depends:** M4-10, M4-6.5.

### M4-12 — Phase B: typed fast paths (L)
Register promotion of non-escaped scalars (consumes M4-1); inline Long/Integer/Double add/sub/mul/cmp with **exact overflow parity** vs the `oxvba-eval` kernels; inline array element access + bounds check; direct global-slot addressing; **direct typed calls into native COM vtables and Declare targets on statically-descriptored sites** (NativeThunk reverse direction, replacing libffi there). Currency/Variant/String/objects stay helper-called.
**Verify:** the §10 perf gate AND the entire M4-11 gate re-run unchanged (fast paths ship only behind green differentials; the numeric micro-corpus is the overflow-parity net); COM transport-count axis unchanged (same vtable slots, different call mechanism). **Depends:** M4-11.

### M4-13 — JIT-generated COM vtable implementations + AOT blob/loader (L)
NativeThunk vtable tier: per-member typed native slots for arbitrary TypeLib shapes, replacing comhost's bounded hand-written dual tier (HRESULT + IErrorInfo mapping, wire conversions via the `oxvba-com` helpers); `ObjectModule`/serialized-blob backend behind the `Backend` trait; author-side stub PE with loader + host import table (rt-abi staticlib); load-and-run parity. Conditionally per §14: binary `.oxi` format folded in here if the `image_load` bench shows JSON dominating (one serialization decision, not two).
**Verify:** com_matrix early-bound legs green against JIT-generated vtables (a live COM client calls OxVba-served typed slots); corpus differential green against AOT-loaded code (subset acceptable for runtime-host reasons; full where feasible). **Depends:** M4-12.

### M4-14 — AOT export packaging (M)
Option B thunk exports for the WrappedComServer `.dll`; Option A PE-surgery export table for generic native export (arbitrary signatures via NativeThunk); signing/W^X/AV posture documented.
**Verify:** live COM smoke against an AOT-exported COM server; an exported generic `.dll` callable from a host harness. **Depends:** M4-13.

---

## §13 Risk register

| # | Risk | L / I | Mitigation |
|---|---|---|---|
| 1 | **BSTR/ObjectRef/SafeArray leaks & UAF in generated code** | High / Critical | Handle-balance axis per program per commit, established on vm3 in M4-0 before the JIT exists; helper-mediated alloc/free in audited Rust shims through Phase A; O1–O8 discipline with the temp-release contract gated at M4-6 before arrays/objects; nightly ASAN + poison-on-release canaries; fuzz shrinker turns leaks into permanent corpus programs. |
| 2 | **R1–R14 divergence** | Med / High | Shared `ErrEngine` cells + routing — one implementation; extraction proven by byte-identical golden; the error-family corpus is the largest thematic slice and gates M4-5; the rules encoded in IR structure are shared by construction. |
| 3 | **No Win64 unwinding ⇒ status checks on every fallible site** | Certain / Med (perf) | Accepted as the model — it matches the IR's explicit fault edges (vm3 performs the same check interpretively). Cost = one branch per fallible op, quantified by the `error_loop` bench from M4-5. Phase B reduces check density by proving pure ops. Do not attempt SEH. |
| 4 | **Re-entrancy: event sink / Terminate drain → compiled code mid-run** | Med / High | Single ProcInvoker spine + shared `draining`/`pumping` guards in ExecState — the same fixpoint discipline as vm3 by construction; live com_matrix events on the JIT (M4-10) is the ground-truth gate; drain-fires-event-raises-error covered by corpus program. |
| 5 | **Native-stack overflow on deep recursion** (vm3 heap frames → native frames) | Med / High (process crash vs err 28) | Frame-depth ceiling matched to vm3's + prologue watermark guard ⇒ err 28 parity (M4-4); big-stack thread; cranelift stack probes as backstop; recursion corpus program. |
| 6 | **Cranelift version churn** | Med / Med | Pin one release family; cranelift confined to one crate; upgrades only at milestone boundaries with a full gate re-run; `Backend` trait limits `JITModule`-specific surface to one file. |
| 7 | **Numeric edges: NaN payloads, overflow flags, banker's rounding, Currency i128** | Med / Med | NaN canonicalization on; Currency permanently helper-called on the i128 kernel; conversions/rounding permanently helper-called; Phase-B inline arith gated on the numeric micro-corpus + fuzz; one-kernel-two-facades makes lane divergence structurally impossible. |
| 8 | **vm3/JIT lockstep drift as vm3 absorbs remaining Tier-3/4/5 fixes** | High / Med | Process rule: every vm3 semantic change lands with a corpus program in the same commit ⇒ the differential auto-covers the JIT; golden re-bless diffs make changes reviewable; the scope ratchet prevents silent JIT decline of new constructs. |
| 9 | **Compile-time blowup on pathological functions** | Low–Med / Med | Per-function compile time tracked in the bench compile group; lazy-compile fallback pre-designed behind the fn-table indirection — contained change if a real project hits it. |
| 10 | **`.oxi` JSON load dominating session start** | Med / Low–Med | `image_load` bench makes it visible from M4-0; binary format conditionally folded into M4-13 (§14). |
| 11 | **Silent-fallback rot** (JIT-v1's death) | Med / High (methodology) | Mixed execution exists only under the `jit-debug` feature + env deny-list, loudly logged, unreachable from product config; product decline is whole-program `Unsupported` tracked by a monotone ratchet. |
| 12 | **CI time doubling** | Med / Low | Measured in M4-0 before commitment; one execution feeds both differential and golden assertions; program-parallel runner if > 5 min; fuzz/ASAN nightly. |
| 13 | **AOT AV/SmartScreen** (packer shape) | Med / Med (product feel) | Sign the stub; strict W^X (RW→copy→RX); reserved-section patching fallback if the overlay form gets flagged. |

---

## §14 Explicit non-goals for M4

1. **Speculation / profiling tier** — M6. Seams preserved: patchable fn-table entries, centralized StmtBoundary lowering, ProcInvoker.
2. **Inline caches for late-bound COM** — transport-dominated; revisit with M6 profiling data.
3. **Multi-threading** — ExecState is `!Send`; the termination queue stays thread-local; one session per thread, as today.
4. **Debugger (DAP) integration** — but the StmtBoundary hook site is never compiled away: in Phase A boundaries are real (temp release + drain check makes them semantically load-bearing); Phase B may elide only the *drain check* where a statement provably creates no object references, keeping the boundary as a labeled site. A future DAP tier recompiles with hooks rather than patching.
5. **AddressOf → native-callback thunks beyond vm3 parity** — currently declined by vm3; the JIT declines identically so the differential stays symmetric.
6. **Grammar-complete VBA fuzzing** — the narrow scalar/control-flow generator (§9) is in scope; object/COM/event generation is not.
7. **Binary `.oxi` format** — *conditionally in scope*: if the M4-0 `image_load` bench shows JSON parse dominating session start, fold a binary image format into M4-13, whose AOT blob already needs serialized metadata/descriptor-arena design — one format decision, not two. Otherwise deferred.

---

## Implementation-time confirmations (carried forward)

1. **ParamArray element-alias copy-out**: confirmed in M4-1. `ArrayLiteral { aliases }` carries enough for caller-side static copy-out; no IR extension was needed.
2. **`Me`-receiver convention**: mirror vm3's class-method call path exactly (hidden receiver parameter placement per §6.1).
3. **Cranelift version pin**: record the exact crate versions at M4-3 in this document.
