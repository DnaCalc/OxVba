# Post-cleanup backlog

Tracking deferred work and decisions after the cross-project epic + the start of the
legacy-stack removal. Source of truth for "things we chose to defer, on purpose."

## Current priorities (in order)

1. **Build confidence in the new compiler stack** (`source → oxvba-syntax → oxvba-symbol
   → oxvba-bind → oxvba-bundle → oxvba-vm2`): put in place **testing infrastructure** —
   a reusable harness to run VBA source / `.basproj` projects through the clean path and
   assert user-visible results, and re-point the highest-value legacy VBA-semantics test
   corpora at it.
2. **Gap analysis vs the old compiler**: re-pointing the old corpora at the clean path is
   the primary signal — what fails reveals which VBA features/semantics the new stack does
   not yet cover. Catalog the gaps.
3. **Then** start deleting the parts of the old code we will **not** need (selective, after
   the gaps are understood — not a blind one-pass delete).

Keep `oxvba-debug` (debugger) and `oxvba-languageservice`/`oxvba-lsp`/`oxvba-web-*`
(language support) in `_legacy_harvest/` **as reference until we re-implement them** on the
clean stack — do not delete.

## Clean-stack gaps (from the re-pointed `feature_coverage` corpus)

`crates/oxvba-bind/tests/feature_coverage.rs` re-points the legacy `vm_feature_coverage`
VBA-semantics corpus at the clean stack (bind→linearize→vm2). First run: 27 pass / 26 fail
across 6 categories (A–F). Round 2 (typed arithmetic + coercion) closed A, B, F and the
pure-Boolean-const cases (→ 35/18). Round 3 closed D, G, H (→ 50/5). Round 4 closed E
(indexed Property Let), I (the non-standard `^` test, rewritten with `As LongLong`), and
the G follow-ups (fixed-array Dim hoisting + module-level array globals) (→ 55/2).
**Round 5 closed C (UDTs) → the whole corpus passes: 57 pass, 0 ignored.**

**Closed (commit: typed arithmetic in the bytecode/VM + store coercion):** the numeric
type/regime lives **on the arithmetic op** (so the bundle fully describes the code for the
Cranelift JIT), not as a separate binder coercion node:
- **typed-op ISA** — `Op::{Add,Sub,Mul,Neg,IntDiv,Mod}` carry `mode: NumericMode =
  Widening | Checked(ty)`. The binder picks the regime from the operands' static types
  (`types::numeric_mode`); the VM computes integers **exactly in i64** and raises Overflow
  (error 6) for `Checked`, or Integer→Long→Double promotion for `Widening`.
- **A — store coercion to declared type** — a store into a declared scalar coerces to that
  type (`types::coerce_store`, an explicit `CoerceNumeric` op); declared vars hold their
  declared tag (`Dim b As Byte: b = 3` → `Byte(3)`, not the legacy VM's `Long(3)`). Several
  legacy corpus expectations encoded the old `Long`-biased tags and were corrected to the
  oracle-true tags (`Byte`/`Integer`/`Long`).
- **B — overflow detection** — integer literals take the smallest fitting static type
  (`int_literal_type`), so `Integer + Integer` is Integer arithmetic (the `30000*30000`
  gotcha). Intermediate overflow (`(al+al) Mod 7`) is caught at the inner op; 64-bit is
  exact (no f64 round-trip) — `longlong_multiplication_is_exact` guards it.
- **F — error codes** — the arith layer carries the VBA code structurally
  (`arith::ArithError`: Overflow 6, Division-by-zero 11, Invalid-use-of-Null 94, else 13);
  `Fault::from_arith` propagates it. No string→code matching.
- **Boolean coerce target** — `NumericCoerceTarget::Boolean` (`CBool` in vm2) so an
  `And`/`Xor`/`Eqv`/`Imp`-folded `I32` const stored into a `Boolean` var coerces to
  `True`/`False`.

**Closed (round 3 — D/G/H):**
- **D — Optional parameter defaults** (9): an omitted optional arg now binds the parameter's
  default. The default *expression* folds in the symbol layer via the const evaluator
  (`const_eval::fold_optional_defaults`, after `fold_const_values`), keyed by `(proc symbol,
  param index)` and exposed by `ResolutionEnvironment::optional_default`; the binder
  (`call.rs::omitted_optional_arg`) substitutes the folded default (coerced to the param
  type), else the declared-type zero (`Object`→`Nothing`), else `Missing` (a `Variant`
  optional with no default).
- **G — fixed-size array local** (1): a `Dim a(1 To 3) As Long` now allocates the array
  (`stmt.rs::bind_dim` emits a `ReDim` for fixed-bounds declarators, reusing the bounds
  parser; a dynamic `Dim a()` stays unallocated).
- **H — string relational / `Like` const folding** (3): `const_eval::fold_const_binary` folds
  string `=`/`<>`/`<`/… and `Like` (compact VBA matcher) under the module's `Option Compare`
  (threaded through the evaluator). Also unblocks the `Like`/string Optional defaults in D.

**Closed (round 4 — E/I + G follow-ups):**
- **E — indexed `Property Let`/`Set`** (2): `Item(index…) = rhs` lowers to a call of the
  accessor proc with the index args followed by the RHS (`call.rs::bind_indexed_property_let`,
  via the assignment binder's new `IndexExpr` arm); named index args reorder too. A
  member-qualified `obj.P(i)=x` still falls back to the place-store path (follow-up).
- **G follow-ups**: fixed-array `Dim`s are **hoisted** to proc entry
  (`collect_fixed_array_inits`, so a `Dim` in a loop allocates once), and module-level
  **fixed-array globals** allocate at program entry (`Lower::module_global_array_inits`,
  prepended to the entry proc).
- **I**: the legacy snippet's non-standard `^` LongLong type-char (not VBA 7.1) was rewritten
  to an explicit `As LongLong`; the clean parser correctly rejects `^`.

**Closed (round 5 — C):**
- **C — UDTs** (2): `Type … End Type` records are a **value-type aggregate**. The symbol layer
  captures each `Type`'s field table (`scanner::collect_udt_fields` → `env.udt_field`/
  `udt_field_count`); a declared `Object(name)` whose name is a `Type` becomes
  `VarTypeRef::Udt` (`Lower::resolve_udt_type`, at `symbol_type`). The bytecode has **dedicated
  record opcodes** — `NewRecord` / `RecordGet` / `RecordSet` (coreir `CoreValue::NewRecord` +
  `CorePlace::RecordField`) — so the VM and JIT see records distinctly from arrays. `Dim p As T`
  allocates a default record (`stmt.rs::udt_record_init`, hoisted to entry), `p.X` is a
  fixed-index `RecordField` (`place.rs::udt_field_place`, l-value + r-value), `q = p` is a plain
  `Let` (value copy). Generalizes through calls — a ByRef param writes a field back, a ByVal
  param copies (`udt_passed_byref_writes_through_byval_copies` test). The VM **backs** a record
  with a `SafeArray` of its fields (value-copy comes free from the `Variant` deep-clone); that
  storage is internal — see the interop constraint below.

**Design constraint — UDT native interop (before it ships):** a UDT value is backed at run
time by a `SafeArray` (`vtype = VT_ARRAY`), so a UDT `Variant` is indistinguishable from a
Variant array *at the runtime-`Variant` level*. UDT native interop (`Declare` / COM params,
typelib export) must therefore marshal a UDT by laying its fields out per the **static UDT
type** at the call site (read fields by index from the symbol field table → packed native
struct / `VT_RECORD`), **never** by passing or inspecting the backing `SafeArray` — otherwise
the callee would observe our `SAFEARRAY` storage instead of VBA's struct. The dedicated record
opcodes already distinguish records from arrays in the bytecode; the remaining hook is a
UDT-aware `CallExtern`/COM marshalling path (the alternative — a distinct `VT_RECORD` runtime
tag, touching the unsafe `Variant` — is heavier and deferrable). Today no tested path marshals
a UDT across the boundary, so nothing leaks; this note exists so it is handled when it does.

The re-pointed `feature_coverage` corpus now passes in full (57/0).

Next gap-analysis steps: re-point the host `com_*`/`file_io`/`pointer`/`invoke` suites
(objects, COM, file I/O, pointers) — those will surface the object/COM/host-call gaps the
scalar corpus can't. Of the remaining bind/VM gaps, **C** (UDTs) is the most substantial.

## Deferred decisions

- **CLI `--references` injection** — REMOVED for now (the `.basproj` reference graph is the
  source of truth for cross-project references). **Decide where ad-hoc reference injection
  is actually useful** before re-adding it on the clean path (e.g. scripting/one-off runs
  that reference a project not declared in the `.basproj`). If it is, re-implement against
  `load_project_closure` (merge the injected project's closure + add to the root's refs),
  not the legacy loader.
- **Convention-only directories** (a folder of `.bas`/`.cls` with no `.basproj`) on the
  clean run path — currently falls back to the legacy executor; decide whether to support
  via a synthesized single-/multi-module manifest.

## Deferred features / capabilities

- **True COM server export + `.tlb`/native export** (DLL / EXE / COM-server / XLL): re-target
  `oxvba-build`'s bundle-embed + reflection-driven signature emit at the clean `oxvba-bundle`
  `Bundle`. Reusable assets cataloged in `_legacy_harvest/CATALOG.md`
  (`registration.rs`, `deffile.rs`, `compile.rs`, `idl.rs`, `typelib_gen.rs`).
- **Host-sensitivity compile-time gate** (review item M1): re-express
  `preflight_host_sensitive_support` (matches HAL capability + host policy against
  host-sensitive intrinsics) over `oxvba-bundle` ops — the clean path currently lacks it.
- **Cross-bundle module variables / instance fields**: a referenced project's public module
  variable / class instance field is currently a clean bind error (no callable export).
  Support needs synthesized accessor procs (getter/setter) so they become exportable +
  dispatchable across a bundle boundary.
- **`End` → `Op::Halt` snapshot** (review item N7): if/when the VBA `End` statement is wired
  to `Op::Halt`, have the host snapshot read the entry bundle's globals explicitly (or have
  `run()` restore `cur` to the entry bundle on exit) — currently unreachable.

## Re-implement on the clean stack (kept as reference in `_legacy_harvest/`)

- `oxvba-languageservice` / `oxvba-lsp` — semantic model + LSP over `oxvba-symbol` + CST.
- `oxvba-debug` — debugger (DAP) over `oxvba-vm2` (needs a vm2 debug surface).
- `oxvba-web-host` / `oxvba-web-shell` — web/wasm host + shell on the clean stack.
- `oxvba-launcher` — fold into the clean `oxvba-host`/`oxvba-cli` run path.
