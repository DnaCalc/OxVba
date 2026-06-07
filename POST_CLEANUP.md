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
VBA-semantics corpus at the clean stack (bind→linearize→vm2). First run: **27 pass, 26
fail**. The clean stack handles all the core *shapes* (scalar/double arithmetic, most
const folding, strings + `Left/Mid/UCase/Len`, fixed+dynamic arrays, `For`/`While`/
`If`, logical ops as r-values, type-suffix literals). The 26 failures are 6 specific,
fixable gaps (each failing test is `#[ignore = "gap X: …"]` in the corpus — un-ignore as
fixed):

- **A — store coercion to declared type** (~10 tests): an assigned value isn't coerced to
  the target variable's declared type. `Dim x As Long: x = x*3+4` → stored `Double(10)`
  not `Long(10)`; a boolean const folds to `Long(0)` and stays `Long` in a `Boolean` var.
  (Note: in some cases the clean stack is *more* VBA-correct than the legacy expectation —
  e.g. `Dim b As Byte: b = 3` → `Byte(3)` clean vs `Long(3)` legacy; those expectations
  should be re-checked against the Excel oracle, not just "fixed".) Biggest cluster.
- **B — overflow detection** (2): fixed-integer overflow (`Dim x As Long: x = 2e9: x = x +
  2e9`) widens to `Double` instead of raising run-time error 6. Tied to A (no coerce/check
  on store).
- **C — UDTs** (2): `Type … End Type` + `p.X = 3` → `424 Object required`. User-defined
  types not supported in the clean stack.
- **D — Optional parameter defaults** (9): an omitted `Optional ByVal x As T = <default>`
  passes `Missing` (an Error variant) instead of binding the default value.
- **E — indexed `Property Let`** (2): `Item(i) = x` → bind error "not an assignable
  variable" (indexed `Property Get` works).
- **F — division-by-zero error code** (1): `1 / 0` yields error 13 (Type mismatch) instead
  of 11 (Division by zero).

Next gap-analysis steps: re-point the host `com_*`/`file_io`/`pointer`/`invoke` suites
(objects, COM, file I/O, pointers) — those will surface the object/COM/host-call gaps the
scalar corpus can't.

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
