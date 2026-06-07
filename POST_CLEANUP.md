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
