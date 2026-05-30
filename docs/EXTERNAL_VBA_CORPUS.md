# External VBA Corpus — Test Summary & Findings

Companion to [`INTERESTING_VBA_PROJECTS.md`](INTERESTING_VBA_PROJECTS.md). That file
is the *watchlist*; this file is the *running summary* of what we actually exercised
against OxVBA and what we learned.

## How this corpus is stored

Web-sourced sample code is **deliberately not committed**. The samples and any
adapted `.basproj` projects live in a gitignored working area
(`.external/vba-corpus/`, see `.gitignore`); **only this summary and the findings
below are committed.** This keeps the clean-room boundary: we observe and describe
external projects, we don't redistribute their code in the OxVBA tree.

Clean-room rule (inherited from the watchlist): use these projects only through
public docs, high-level source review, and reproducible black-box behavior. Inclusion
here is not a compatibility claim.

## Status legend

`watchlisted` → noted only · `gathered` → source pulled into the local area ·
`building` → `.basproj` authored · `running` → executes under the VM ·
`characterized` → behavior/limitations documented below.

## Corpus

| Project | Reference | What we're checking | Status |
| --- | --- | --- | --- |
| Riff | https://github.com/uesleibros/riff | `Declare`/Win32 interop, COM vtable dispatch, pointer-heavy VBA, conditional 32/64-bit compilation, UDTs/arrays, machine-code callback thunks, host-reset safety | gathered |
| Wasabi | https://github.com/uesleibros/wasabi | Win32 networking `Declare`s, byte-array/string transport, async callback/event patterns, handler-class lifetimes, `DoEvents` host behavior, TLS/proxy surfaces | gathered |
| Awesome VBA | https://github.com/sancarn/awesome-vba | Index for sourcing further candidates (JSON/CSV/XML, data structures, parsers, Win32, add-ins, …) | watchlisted |

## Method note (how to read sweep failures)

A first pass compiled each module **in isolation** (`oxvba-cli compile <one-file>`).
Two failure classes from that are test-method artifacts, not engine defects, and are
verified as such:

- **Cross-module calls** (`call to unknown procedure: wasabi_tcpconnect`, etc.): OxVBA is
  a whole-program compiler; a lone module calling a sibling module's `Public` proc has
  nothing to resolve against. A 2-module project resolves it (`Helper.Add(2,3)` → `5`). ✓
- **`.cls VERSION 1.0 CLASS / BEGIN…END` header**: single-file `compile` chokes on the
  exported class header, but loading the same `.cls` as a `ClassModule` in a `.basproj`
  does **not** error — so this is a single-file-`compile` limitation, not a class-ingest
  bug. (Originally mis-described as expected; corrected after checking.)

## Findings (compiler/runtime gaps surfaced by the corpus)

Each has a minimal standalone repro (kept under the gitignored `temp/` while iterating).

- **VBA7 dialect** — `VBA7` was already predefined `True`, so the corpus's `#If VBA7`
  `PtrSafe` branch was taken; made the predefined `#If` set explicit/complete for VBA 7.1
  (`Vba6`/`Win32`/`Win16` added, `Win64`/`Win32` keyed to pointer width). Status: **done.**
- **F1 — intrinsic `vb*` constants unresolved.** `vbCrLf`, `vbCr`, `vbTab`, `vbObjectError`,
  `vbBinaryCompare`, `vbFromUnicode`, … reported `use of undeclared variable` under
  `Option Explicit`. Fixed by binding the always-available `vbConstants` family to literal
  values in `resolve.rs`. Status: **FIXED** (regression test
  `resolve_intrinsic_vb_constants_to_literals`).
- **F2 — omitted / Optional arguments rejected.** Two layered defects:
  1. **Root cause (high impact):** `parse_proc_signature` rejected any `Optional` parameter
     that was `ByRef`. VBA parameters are ByRef by default, so the ubiquitous
     `Optional b As Long` made the *whole procedure* fail to register — every caller then
     hit `call to unknown procedure`. So essentially all real-world `Optional` usage was
     broken, full-arity calls included.
  2. Omitted positional args via bare commas (`Foo(1, , 5)`) were rejected by
     `split_call_args` before binding.
  Fixed by removing the bogus `optional && by_ref` rejection and adding an omitted-allowing
  arg split that binds an `__omitted` sentinel (lowered to the parameter's Optional default).
  Verified end-to-end via `run-project`: `Foo(1)`→default, `Foo(1, , , 5)`→defaults fill the
  gaps. Status: **FIXED** (regressions `resolve_optional_byref_param_is_accepted`,
  `resolve_omitted_positional_arguments_bind_sentinel`).
- **F3 — parameterless `Function` member read without parens on a project class returns
  `Empty`.** _(Re-scoped twice: it is **not** a `New`-instantiation bug — `Dim w As New
  Widget` does instantiate; the `i32:1` slot is the project-object handle. And the site is
  **not** `emit.rs`/a VM property-get hint — the generic dynamic-dispatch path passes
  `call_kind_hint: None`, which already get-or-calls.)_ Boundary, all via `run-project`:
  `Property Get` (default or not) → OK; `widget.GetScore()` (parens) → OK;
  `widget.AddOne(6)` (args) → OK; **`widget.GetScore` (parameterless `Function`, no parens)
  → `Empty`.**

  **Real site:** the compiler rewrite `rewrite_internal_class_property_expression_reads`
  (and its statement-form sibling) in `project.rs` resolves a no-paren project-class member
  read with `allowed_kinds = [ProcedureDeclKind::PropertyGet]` **only** (`project.rs:~4710`).
  A parameterless `Function` matches nothing, the line is left unrewritten/unresolved, and
  the bind plan silently yields `Empty`. The COM early-bound read rewrite already probes
  `[PropertyGet, Method]` — so the internal-class path is the inconsistent one.

  **Observed today by receiver-declaration form (each should return `42` per VBA):**
  - `Dim a As New Widget` → `out = a.GetScore` → silent **`Empty`** (no error).
  - `Dim b As Widget : Set b = New Widget` → `out = b.GetScore` → **wrong compile error**
    `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` (a *different* resolution path; VBA would call).
  - `Dim c As Object : Set c = New Widget` → **`Set c = New Widget` is itself rejected**
    (`unsupported statement`): late-bound assignment of a project class into `Object`/`Variant`
    is unsupported — a separate prerequisite defect that must be fixed for the late-bound lane.

  Three forms, three different wrong outcomes for one operation, across ≥3 resolution paths
  (property-expression read, default-member read, late-bound `Set`). The fix must unify them.

  **Receiver-type matrix (the comprehensive fix spans compile-time *and* runtime — we do
  not always have a static type):**
  - **Statically-typed project class** (`As Widget`/`As New Widget`): fix at compile time —
    make the rewrite a multi-step probe `[PropertyGet]` → parameterless `Function`
    (precedence: a real property wins), mirroring the COM early-bound `[PropertyGet, Method]`.
  - **Late-bound to a VBA/project type** (`As Object`/`Variant` receiver): kind is unknown
    at compile time → must get-or-call at **runtime** in the VM project-dynamic dispatch.
    The unhinted (`None`) path already name+arity-matches; verify it is the path taken and
    that any property-get-hinted read also falls back. (Keep the `PropertyGet` matcher
    unchanged; *add* a method probe — do not relax the property-get predicate.)
  - **Late-bound to COM**: already covered by the combined `DISPATCH_METHOD |
    DISPATCH_PROPERTYGET` fix (DAO commit) and the COM early-bound `[PropertyGet, Method]`.

  **Diagnostic-parity obligation** (see `docs/CONFORMANCE.md#conformance-principle-diagnostic--error-behaviour-parity`):
  the get-or-call probe matches a *parameterless* `Function` only. The edges must match
  VBA's **error** behaviour, not silently return `Empty`:
  - `x = obj.NeedsArg` (required-arg function/indexed property, no parens) → VBA "Argument
    not optional"; OxVBA must raise an equivalent diagnostic at the same point.
  - `x = obj.SomeSub` (value context) → VBA "Expected Function or variable"; so the
    value-read probe is `[PropertyGet, Function]`, **excluding `Sub`**, and a `Sub` in value
    context must error rather than be called.
  These error cases are oracle-confirmable but the silent-`Empty`-on-unresolved-read
  behaviour is itself a parity bug to fix alongside.

  Status:
  - **F3b (no-paren read get-or-call): FIXED** (commit `a3f6ee34`). The internal-class
    no-paren read rewrite now probes a parameterless `Function` after `PropertyGet`, so
    `Dim w As New Widget : x = w.GetScore` → `42`. Regression test
    `pure_oxvba_class_no_paren_read_invokes_parameterless_function`; full suites green.
  - **F3a (`Set <var> = New <ProjectClass>`): mostly FIXED (4/5 receiver forms).**
    `expand_bound_source_line` now recognises `Set <var> = New <ProjectClass>` and lowers it
    like `As New` (allocate an instance handle, register/refresh the dynamic-object binding,
    emit the handle assignment + `Class_Initialize`), with a `referenced_typelib_blob`
    guard so COM `New` still routes to the early-bound rewrites. `As Widget`, `As Variant`,
    untyped, and `As New` now all give `42`. Regression test
    `pure_oxvba_class_explicit_set_new_instantiates_and_dispatches`; suites green (130).
    **Remaining: `Dim c As Object : Set c = New Widget`** still errors `cannot assign Long
    to Object variable c` — the instance handle is an integer literal, which a Variant/
    untyped slot accepts but an `Object`-typed slot rejects (`typecheck.rs:~1463`). Needs an
    object-typed representation of the project-instantiation handle (so it type-checks into
    an `Object` slot), tracked with F3c.
  - **F3c (edge diagnostics): pending oracle.** Required-arg read / `Sub`-in-value-context
    must raise VBA-equivalent diagnostics per the conformance parity principle.
- **F4 (minor, observed) — single-file `oxvba-cli run`/`compile` can't resolve sibling
  procedures.** A bare `.bas` with `Sub Main` + `Function Foo` reports `call to unknown
  procedure: foo`; the same code in a `.basproj` (`run-project`) resolves fine. Affects only
  the single-file CLI convenience path; noted so corpus testing uses `run-project`.

### Per project
- **Riff**: surfaces F2 (and WithEvents member-access in `If`), plus heavy
  `Declare PtrSafe`/`As Any`/`LongPtr` and `#If VBA7` paths (pending VBA7 predefinition).
- **Wasabi**: surfaces F1, F2, and (examples/tests) cross-module + project-class usage.

## Related prior corpus work

- `.external/sqliteforexcel/` — SQLite-for-Excel VBA modules, vendored earlier for
  `Declare`/FFI integration testing (this one **is** committed; provenance under
  `docs/evidence/SQLITEFOREXCEL_*`). Distinct from this gitignored watchlist area.
