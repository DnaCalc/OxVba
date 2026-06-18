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
3. **Then** start deleting the parts of the old code we will **not** need. The
   retained `_legacy_harvest/` reference tree has now been deleted; future
   debugger/language-service/web work should be rebuilt against the clean stack
   rather than copied from the removed legacy crates.

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
  clean run path — the legacy-executor fallback was **removed** with the legacy stack, so
  `oxvba run-project <convention-dir>` now errors ("no .basproj or .vbp project file
  found"). Decide whether to support via a synthesized single-/multi-module manifest fed to
  `load_project_closure` (the loader already builds a convention `LoadedProject`; it just is
  not wired to the closure path).

## Deferred features / capabilities

- **True COM server export + `.tlb`/native export** (DLL / EXE / COM-server / XLL): re-target
  `oxvba-build`'s bundle-embed + reflection-driven signature emit at the clean `oxvba-bundle`
  `Bundle`. The old retained reference implementation has been deleted; current
  COM-server work lives in `oxvba-build`, `oxvba-com`, and `oxvba-comhost`.
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
- **`Debug.Print` / console output** — DONE. `Debug.Print`/`Debug.Assert` (statement-level
  special-case in `stmt.rs`, mirroring `Err`) and bare `Print` (a `ConsolePrint` catalog name)
  now route through the existing HAL diagnostics/console callbacks. Multi-arg join +
  display formatting live in `oxvba_runtime::print_display_text` (the HAL's
  `variant_to_display_text` delegates to it). Covered by
  `oxvba-host/tests/debug_and_console_print.rs`. **Remaining fidelity** (deferred): the `;`
  no-space separator (only `,`→tab is rendered), `Tab(n)`/`Spc(n)` positioning, trailing
  `;`/`,` newline suppression, and VBA's leading/trailing space around printed numbers;
  console `Input`/`Line Input` (no file number) still route to the file intrinsics, not the
  `ConsoleInput`/`ConsoleLineInput` ones.
- **Native `Declare` execution (`VarPtr`/`StrPtr`/`ObjPtr` + the FFI lane)** — DONE for the
  pointer-driven path (L1–L3). `Declare Lib` now invokes real DLLs: the binder emits the
  `m1-native-ffi` lane for real libraries (HAL gates actual execution on policy, so sandboxed runs
  stay safe); `StrPtr`/`VarPtr`/`ObjPtr` bind value-based (r-values like `StrPtr("x")` work);
  `VarPtr(a(i))` points at the whole array buffer; and a `StrPtr(x)`/`VarPtr(x)` argument over an
  l-value writes the pinned buffer back into `x` after the call (`CoreCallee::Declare.ptr_writebacks`
  → vm2 `read_back_*`). Proven by `oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs`
  (15/15 on Windows: LoadLibraryA, GetModuleHandleExW, MultiByteToWideChar/WideCharToMultiByte
  buffers, SysReAllocString, oleaut32 ByRef numeric conversions, msvcrt sqrt).
  **Remaining (separate features, not the pointer path):** `pointer_helpers_end_to_end`'s
  byte-array Declare *parameter* passing + `VarPtr` of a Variant-Decimal/i64; the registry leak
  (next bullet); and the **true `As String` marshalling** further down.
  **VarPtr over an unallocated array Variant — DONE.** `set_windows_variant_array_arg` used to
  reject a Variant whose SAFEARRAY has a declared shape but null `pv_data` (a `Dim a()` never
  `ReDim`'d, or a zero-length dynamic array). It now synthesizes default (`Empty`) elements
  sized to the descriptor's bounds (empty when there are no bounds either) and marshals an
  array of that shape, matching VBA. Windows-only marshalling; end-to-end coverage is via the
  native-Declare lane (no portable unit test — the bounds-but-null-data state has no public
  constructor).
- **Conditional compilation (`#If`/`#ElseIf`/`#Else`/`#End If`/`#Const`)** — DONE.
  `oxvba_symbol::cond_comp::preprocess` runs before each module is parsed: it evaluates the directives
  against the predefined host constants (`Win64`/`VBA7`/`Win32` = True on the 64-bit runtime, others
  False), the project `DefineConstants`, and module `#Const`s, and blanks directive + inactive-branch
  lines (offset-preserving) so inactive branches are never parsed. `#If` conditions reuse the real
  expression grammar (`oxvba_syntax::parse_expression`) + the const-expr folder, so their semantics
  match VBA `Const`. Covered by `cond_comp` unit tests + `feature_coverage` end-to-end
  (`conditional_compilation_*`). **Follow-up:** referenced projects evaluate `#If` against the
  predefined constants only (a `ReferencedProjectManifest` does not carry its own `DefineConstants`);
  predefined constants are hardcoded 64-bit (parameterize by target bitness when 32-bit support is needed).
- **Predeclared instances (`VB_PredeclaredId`)** — DONE. A class/document module with
  `VB_PredeclaredId = True` (e.g. `ThisWorkbook`, `Sheet1`, `UserForm1`) is reachable as a **global
  singleton by its module name** — created lazily on first access, persisting for the run, distinct
  from `New` (which still allocates a fresh instance). Two IR forms mirror `New`/`NewExtern`:
  `CoreValue::Predeclared { class }` (active project) and `PredeclaredExtern { import }` (a referenced
  project's exposed predeclared class) → `Op::PredeclaredInstance` / `Op::PredeclaredInstanceExtern`.
  The VM caches one instance per class per `LoadedBundle` (cached before `Class_Initialize` runs so a
  re-entrant access sees the same object), carrying the owning bundle's id so members dispatch into the
  right bundle (reusing the cross-bundle object path). Resolution: the binder's
  `ids.predeclared_class_of` (active) + `env.resolve_extern_predeclared` (referenced, via a new
  `SurfaceType.predeclared` flag). Covered by `oxvba-bind/tests/cross_project.rs`
  (`predeclared_instance_singleton_in_active_project`, `predeclared_new_makes_independent_instance`,
  `cross_project_predeclared_instance_property` — the `ThisWorkbook.Path` shape — and
  `cross_project_predeclared_instance_persists_state`).
- **`Err.LastDllError`** — DONE. Binds as a `Long` member read (`call.rs::err_field` →
  `ErrField::LastDllError` → `Op::LoadErrLastDllError`) returning the OS last-error the VM captured
  after the most recent native `Declare` call. The standard HAL adapter captures `GetLastError`
  (`std::io::Error::last_os_error()`) immediately after `invoke_stdcall` into a shared cell, exposed via
  a new `DynamicLinkHal::last_dll_error()` (default 0 for non-native/null/wasm adapters); the VM stores
  it after each `declare_call` and `Err.LastDllError` reads it. Covered by a portable bind/default test
  (`feature_coverage::err_lastdllerror_binds_and_defaults_to_zero`) and a Windows faithful-capture test
  (`native_declare…::err_lastdllerror_reads_os_error_after_native_declare…`, `SetLastError 12345` →
  `12345`). **Not done** (no tested path needs them): the other `Err` members as member reads
  (`HelpFile`/`HelpContext`) — `Raise`/`Clear` already lower as statements.
- **VBA numeric/type conversion intrinsics (`CDbl`/`CLng`/`CInt`/…)** — DONE (except `CDec`). Added
  `CBool`/`CByte`/`CInt`/`CLng`/`CLngLng`/`CLngPtr`/`CSng`/`CDbl`/`CCur`/`CVar` to the catalog +
  `NativeImplId` + oxvba-lib bodies (`CStr`/`CDate`/`CVErr` were already present). Each coerces its
  argument to the named type with VBA banker's rounding (half-to-even) and raises Overflow (6) when the
  rounded value is out of range; numeric strings parse (`CDbl("3.5")`), `CVar` is identity. Covered by
  `feature_coverage` (`numeric_conversion_intrinsics_with_bankers_rounding`,
  `conversion_intrinsic_overflow_is_error_6`). **`CDec` deferred** (loud "unresolved name `CDec`", not a
  silent stub): faithful `CDec` needs real f64→`Decimal96` conversion (`Decimal96` has only `from_parts`,
  no `from_f64`), a separate sub-feature; no tested path needs it.
- **`Kill` file statement** — DONE. `Kill pathname` (delete files) now resolves: `FileKill` got the
  catalog name `"Kill"`. Unlike Open/Close/Print#/Name/Lock/…, `Kill` is not a lexer keyword, so it
  parses as an ordinary statement-call and must resolve by name (a 1-arg native). Covered by
  `bind_roundtrip::kill_statement_resolves_to_file_kill_native`.
- **Module-qualified-call recursion (the demo's ~45 GB "crash") — FIXED.** The runaway allocation was
  NOT native marshalling: it was the VM frame stack growing under **infinite recursion**. The closure
  loader injects a startup shim (module `__OxVbaStartupEntryShim`, `Public Sub Main()`) whose body is
  `Call <EntryPoint>` = `Call Main.Main` (the demo's entry is module `Main`'s `Sub Main`). Binding
  `Main.Main`, `is_module_qualifier("Main")` asked `resolve("Main")` whether it's a module — but
  `resolve` prioritises the **Procedure** namespace over **Module**, so `Main` resolved to the *sub*
  `Main`, the qualified path was skipped, and `Main.Main` mis-bound as member access on the `Main`
  proc-call (the shim, proc 0) → self-recursion. Fix: `is_module_qualifier` reads module-ness from the
  authoritative module list (`all_modules`), and only a **local/parameter** variable of the same name
  shadows a qualifier — a same-named `Sub`/`Function` does not. Regression tests in
  `oxvba-bind/tests/cross_project.rs` (`module_qualified_call_resolves_when_qualifier_also_names_a_sub`
  + two more).
- **VM robustness against guest-triggered unbounded allocation/recursion.** A VBA program must never
  abort the host: (1) `vm2` bounds the call-frame depth → "Out of stack space" (28) instead of OOM
  (`guard_call_depth`, applied at every frame push); (2) `String`/`Space` bound their count to `Long`
  range → Overflow (6) (`oxvba-lib::alloc_count`); (3) `ReDim` bounds its element count → "Out of
  memory" (7) (`vm2::build_bounds`). These caught/contained the symptom class; the recursion fix above
  is the actual cause.
- **SQLiteForExcel acceptance test — now drives REAL `sqlite3.dll`; next gap is dynamic-array typing.**
  Recursion fixed + the test runs from the workspace root (so the fixture's relative `ThisWorkbook.Path`
  resolves for `LoadLibrary`), so the bounded demo now genuinely exercises native SQLite:
  `SQLite3LibVersion` returns `"3.11.1"` and `SQLite3Open`/`SQLite3Close` succeed (real DB open/close).
  It reaches `TestOpenCloseV2` → `SQLite3OpenV2` → `StringToUtf8Bytes`, which assigns a `Variant` byte
  array to a `Dim bufFileName() As Byte` and then fails: **"unsupported coercion from ArrayVariant to
  Double"**.
- **Dynamic/array declarators typed as their scalar element — FIXED.** `declared_var_type`
  (`oxvba-symbol/src/scanner.rs`) now wraps an array declarator (`x()` or `x(1 To 3)`) in
  `VarTypeRef::Array(element)`. The binder was already built for this (`bind_redim`/`bind_dim`/`Erase`
  peel `Array(inner)`; `array_element` reads it; indexing discards the base type), so a whole-array
  assignment (`x = arr`) no longer scalar-coerces, and `ReDim x(n)` now allocates the declared element
  type instead of `Variant`. No existing test regressed; regression test
  `feature_coverage::dynamic_array_whole_assignment_is_a_copy_not_a_scalar_coercion`.
- **SQLiteForExcel — PASSES end to end.** The acceptance test
  `sqliteforexcel_declare_integration::bounded_demo_completes_on_vm_via_native_sqlite` is **un-ignored**
  and green: the real SQLiteForExcel VBA project drives the real `sqlite3.dll` through the clean stack
  (closure → bind → linearize → vm2 → HAL native FFI) for the entire bounded demo — `TestVersion`
  ("3.11.1") → `TestOpenClose` → `TestError` → `TestInsert` → `TestSelect` (INTEGER/TEXT/FLOAT/NULL read
  back correctly) → `TestBinding` → `TestDates` → `TestStrings` (10 000-char `String`/`String(n,c)`) →
  `TestBackup` → `TestBlob` (`SQLite3ColumnBlob` byte-array round-trip) → `TestWriteReadOnly`
  (read-only enforcement), printing `----- All Tests Complete -----`. The native string/int/float/null
  marshalling round-trip is proven correct. Library/binder gaps cleared to get here, all with regression
  tests in `oxvba-bind/tests/`: `#If` conditional compilation; the `ThisWorkbook` predeclared instance;
  `Err.LastDllError`; the numeric/type conversion intrinsics (`CDbl`/`CLng`/`CInt`/`CSng`/`CByte`/
  `CBool`/`CCur`/`CLngLng`/`CLngPtr`/`CVar`) with banker's rounding + overflow; `Kill`; `DateValue`'s
  `d mmm yyyy` text form and `CDate` of a numeric serial; array declarators typed as `Array(element)`
  (whole-array assignment is a copy); and array-return functions (`Function F() As Byte()` — parser +
  `build_signature` wrap the return type in `Array`). The earlier "45 GB allocation" was **infinite
  recursion**, not marshalling: `is_module_qualifier` resolved `Main.Main` as a self-call because
  `resolve()` ranks the Procedure namespace over Module — fixed to read module-ness from the authoritative
  module list. VM hardening so guest code can never abort the host: frame-depth limit → VBA error 28,
  `String`/`Space` count bound → error 6, `ReDim` element-count bound → error 7.
- **`VarPtr`/`StrPtr`/`ObjPtr` binding** — DONE (folded into native Declare execution above).
- **Pointer-registry lifetime** — DONE. `oxvba_runtime::pointer_helpers` still backs every
  `VarPtr`/`StrPtr` with a process-global `PointerRegistry`, but pins are now **scoped to the native
  call that consumes them** instead of leaking. `vm2::declare_call` collects the `LongLong`-carried
  registry addresses of its arguments and calls the new `pointer_helpers::free_pins` after the call
  returns and any pointer write-back has read the pins back (and on the invoke-error path), so a
  looping `Declare` no longer accumulates one cell per iteration. This matches VBA's "the pointer is
  valid for the duration of the call" contract. Chosen over a statement-boundary drain because ordinary
  project calls run in the flat `run()` loop, so a pin created mid-statement can straddle a nested
  call's statement boundaries (`Foo(StrPtr(s), Bar())`) — per-call freeing is the hazard-free point and
  also supports the split idiom (`p = VarPtr(buf): CopyMemory p, …`). **Residual:** a pin never passed
  to any `Declare` (e.g. `Debug.Print StrPtr(x)`) is not reclaimed — a degenerate case, since the
  helpers exist to feed native calls. Tests: `pointer_helpers::free_pins_releases_only_the_named_addresses`,
  `feature_coverage::pointer_helper_pins_are_freed_per_native_call_not_leaked`.
- **Native `Declare` string marshalling — DONE (faithful ANSI model).** VBA marshals every
  `Declare` String as **system-codepage ANSI** (never wide, regardless of the export's A/W
  name) — the previous A-suffix-alias heuristic (wide for non-A exports) was a deviation and
  is removed. All three As-String shapes now implemented:
  - **`ByVal … As String`** — ANSI buffer in a marshal cell; after the call the (possibly
    callee-mutated) buffer converts back into the variable at its **full length** (embedded
    NULs preserved) — `ByVal` notwithstanding; that is VBA's pre-sized-buffer idiom. The
    binder (`bind_byval_string_arg`) binds a String-typed, non-parenthesised l-value ByRef so
    the marshaled-back value reaches the variable; literals/expressions/`(s)`/non-String
    l-values stay ByVal (their conversion temp is discarded, as in VBA).
  - **`ByRef … As String`** — an `LPSTR*` cell over the ANSI buffer. Read-back follows
    whichever pointer the cell finally holds: unchanged → full-length buffer decode; replaced
    → capped NUL-terminated ANSI read. *Deliberate safety deviation:* the replacement pointer
    is **not freed** (real VBA frees its temp here, which crashes on static/CRT-owned
    pointers — guest code must never abort this host), accepting a leak in the rare
    callee-allocated case.
  - **`VarPtr(s As String)` read-back trusts the native callee** (review finding
    W1-runtime-002): the write-back decodes whatever BSTR pointer the call left in the
    cell (`SysStringLen` + length read). A callee that stores a non-BSTR pointer there
    causes an out-of-bounds read — exactly as in real VBA, where the String variable IS
    a BSTR slot and a corrupted slot faults identically. A native callee already has
    arbitrary-code power, so no in-process defense exists; the "guest never aborts the
    host" doctrine covers OxVba's own logic over data it controls, not a native callee's
    ABI violations. Accepted, documented here.
  - **`String` return** — the VB contract: the callee returns a BSTR of ANSI bytes
    (`SysAllocStringByteLen`); the runtime decodes it to Unicode and frees it.
  ANSI decode (`utf16_from_ansi`, CP_ACP, length-bounded) lives next to `ansi_c_string` in
  `oxvba_com::windows_ffi_bridge`. Windows e2e tests: `lstrcpyA` (ByVal write-back +
  full-length), `_get_pgmptr` (ByRef replaced pointer), `SysAllocStringByteLen` (String
  return); bind shape: `declare_byval_string_lvalue_binds_byref_for_ansi_writeback`.
  - **Raw (non-libffi) invocation paths reject float lanes and arity > 6** (review finding
    W1-com-006, P0): the Unix and Windows-non-x86_64 paths dispatch through i64 transmutes,
    which cannot express floating-point argument/return registers or stack-passed arguments —
    they previously miscalled (floats read garbage registers; args 7+ silently dropped).
    Those shapes now fail deterministically with a clear error. **Follow-up:** extend the
    libffi path (today Windows-x86_64-only) to all targets, which lifts both limits and
    retires `raw_invoke_shape_error`; needs the `libffi` dependency un-gated from the
    Windows-only target table plus Linux/macOS CI verification.
  **Parked:** fixed-length `String * N` as a *Declare param* (no length in
  `DeclareParamType`; scanner folds it to `String` — fine for the descriptor, but a
  fixed-string variable arg binds ByVal/no-write-back); Variant-variable args get no
  write-back (VBA discards the conversion temp too); Unix keeps wide ByVal marshalling, no
  ByRef-String, and rejects String returns (no BSTR/codepage ABI there).

## Legacy stack removed (this pass)

The legacy execution stack is gone; the workspace builds and tests green on the clean stack
only. Removed: `oxvba-compiler`, `oxvba-vm`, and the retained `_legacy_harvest/`
tree. `oxvba-jit` is a `thiserror`-only stub (kept). `oxvba-host` was rewritten to a thin clean `Engine` (two entry points:
`execute_source_with_variant_snapshot_clean`, `execute_project_closure_with_variant_snapshot`).

- **`oxvba-project`** severed off the deleted crates: the project-manifest types it borrowed
  from `oxvba-compiler` are localized in `src/manifest.rs`; the COM-typelib-diagnostic
  injection + legacy reference resolution were dropped from `load.rs` (the clean closure
  builder in `closure.rs` owns reference resolution); the host-IDE tooling modules
  (`com_selection`, `host_helpers`, `generate`, `validate`, `resolve`) were deleted. The
  crate now depends only on `oxvba-symbol` + `quick-xml` + `thiserror`. `load_basproj` no
  longer populates `manifest.reference_projects` (empty; the closure path resolves the graph).
- **`oxvba-cli`** rewritten fresh to the clean `run` / `run-project` subcommands +
  runner-bootstrap flags only. Dropped: `compile`, `build`, `com-ref`, `repl`/`immediate`,
  `native-ready-runner`, `explain`, `init`, `import-vbp`, native-export packaging, the XLL
  shim, and the `oxvba-reflect-wrapper` bin. Re-add project-authoring conveniences
  (`import-vbp`, `init`) against the clean modules if wanted — they are not the execution path.

## Re-implement on the clean stack

- `oxvba-languageservice` / `oxvba-lsp` — semantic model + LSP over `oxvba-symbol` + CST.
- `oxvba-debug` — debugger (DAP) over `oxvba-vm2` (needs a vm2 debug surface).
- `oxvba-web-host` / `oxvba-web-shell` — web/wasm host + shell on the clean stack.
- `oxvba-launcher` — fold into the clean `oxvba-host`/`oxvba-cli` run path.
