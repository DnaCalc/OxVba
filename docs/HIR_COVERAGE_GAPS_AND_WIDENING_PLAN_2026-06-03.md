# HIR Coverage Gaps & Widening Plan

- **Date:** 2026-06-03
- **Why this exists:** The `bd-aprs` retirement beads keep stalling in the same place — we can't make the new HIR path own a construct, so we wrap the old path in more scaffolding (carriers, quarantines, residual rows). This document inverts that: it maps the *real* HIR coverage gaps so we can widen the new path until the legacy boundary collapses and deletion becomes trivial.
- **Scope:** front-end only (CST → binder → bound HIR → lowering). Read-only audit; no code changed to produce this.

## The root finding (read this first)

The recurring wall is **not** a long list of unrelated missing features. It is **two missing layers** in the binder:

1. **The HIR binder is single-module.** `build_hir_from_source(module_name, source)`, `collect_type_hooks_from_source("Main", source)`, and `build_symbol_model_from_source` all take **one module's source**. There is no project-wide symbol table in the binder. Multi-module projects "work" in HIR today only because `project.rs` *flattens the whole project into one concatenated source string* (`full_hir_source` / `active_project_hir_source`) and feeds that single string to the single-module binder. `resolve_name` (`frontend_hir.rs:2058`) resolves only the current module's locals/params/procedures/properties/intrinsics and errors `UnresolvedName` on anything else — but calls lower to `BoundExpr::ProcCall { name }` *by string* (`frontend_hir_lowering.rs:2716`), so flattened cross-module names survive without ever being *bound*.

2. **The HIR has no type/member model.** Member access `x.Foo` lowers blindly to `BoundExpr::Member { receiver, member }` *by name* (`frontend_hir_lowering.rs:2718, 2769`). The binder has no notion that a local is typed as a class or a COM coclass, and no member-resolution step. So it cannot bind `x.OpenTextFile(...)` to a dispatch token or `obj.Method` to a class method — that resolution exists **only** in the legacy `project.rs` rewrites (`rewrite_early_bound_member_dispatch`, `project.rs:5078`).

Everything labelled "references HIR can't bind", "class modules force legacy", "default-member needs PMR carriers", etc. is a *consequence* of these two gaps. The metadata needed to close them already exists (see below) — it just isn't wired into the binder.

## What already works in HIR (so we don't re-solve it)

- All single-module procedural language: expressions, control flow, declarations, a large intrinsic library, `Const`, arrays/`ReDim`, simple UDTs.
- **Native `Declare` (Lib) calls are already fully HIR-bound** (`frontend_hir_lowering.rs:492`, `collect_declared_external_procedures`) — param types, alias/ordinal, ByRef writeback. This is the proof that reference binding *can* live in HIR.
- `ProjectSymbolIndex` (`frontend_project_symbols.rs`) is already complete for in-source symbols across **all** active and referenced-project modules (modules, classes, members, properties, default members, events, WithEvents fields, field-array descriptors). It is built from the manifest but **not consulted by the binder** — only by `project.rs`.
- COM typelib metadata is already available as data: `oxvba_com::TypeLibMetadataBlob` (`member_name → dispatch token`, vtable slot, invoke kind, param specs, default members, events). Consumed today only by legacy `project.rs` rewrites.

## The gaps, by layer

### A. Project-shape boundary (`project_compile_boundary`, `project.rs:1449`)
Routes to `FullLegacy` (no HIR attempt) when any of:
- the active project contains **any** Class / Document / Form / Extension module (the predicates require `all(== Procedural)`); the *only* escape today is the `New`/`As New` construction-rewrite override (`hir_construction_source_from_project_rewrites`, `project.rs:957`);
- a **predeclared-id / global-instance** class in the *active* project (predeclared-safe is allowed only for *referenced* modules);
- **used** `Project` or `HostInjected` references, mixed reference kinds, or non-synthetic typelib references;
- `WithEvents` steady-state wiring or `Implements` in the active project;
- predeclared-member property routes (PMR markers) — disables the construction override.

### B. Binder layer 1 — cross-module / project-wide symbol binding
`resolve_name` sees one module. Cross-module and referenced-project symbols are not bound to real symbols; they ride on source-flattening + name-based `ProcCall`. **Data exists** (`ProjectSymbolIndex`); this is a *plumbing* gap.

### C. Binder layer 2 — typed symbols + member dispatch (the big one)
No declared-type tracking for objects, no member resolution. Blocks: early-bound COM members, class methods/properties, default members, predeclared-member access. **Metadata exists** (`TypeLibMetadataBlob`, class member descriptors in `ProjectSymbolIndex`); the binder has no layer to consume it. `ExternalReferenceIndex` currently carries only name→kind, **no members**.

### D. Host globals
Host-injected references have **no member metadata source at all** (unlike typelibs). Closing this needs *new data* (a host-global member catalog), not just wiring.

### E. Per-construct lowering tail (`HirProductionLoweringError::Unsupported`)
Smaller, incremental gaps discovered lazily: `New` without a project construction binding (`frontend_hir_lowering.rs:2503`), non-static array/`ReDim` bounds, cross-type UDT assignment (deferred marker), indexed-property arities, integer literals over `i32`, non-`randomize` bare intrinsic statements, `Select Case` on non-constant values. Plus eligibility pre-gate rejects (`source_is_eligible_for_lightweight_hir_default`, `lib.rs:339`): unsupported `Def*` / `Option` forms, parse errors, parameter-signature mismatches, unsupported `Property` arities.

## Widening plan (leverage-ordered)

- **Tier 0 — Project-aware binder (foundation, mostly plumbing).** Give `resolve_name` a project symbol table so cross-module and referenced-project names bind to real symbols instead of relying on source-flattening. Immediately lets *used* procedural cross-module / referenced-project shapes leave `FullLegacy`. Prereq for everything below.
- **Tier 1 — Typed symbol + member-dispatch layer (the decisive one).** Track declared object types (class / coclass / predeclared singleton) on symbols; resolve `x.Member` against that type to a method symbol or dispatch token, consuming `ProjectSymbolIndex` class members and `TypeLibMetadataBlob`. Unblocks early-bound COM, class methods/properties, default members, and predeclared-member access in one architectural move — and lets the PMR carriers and `rewrite_early_bound_member_dispatch` retire.
- **Tier 2 — Active-project class modeling.** HIR-native class instances/fields/construction/WithEvents/Implements so the boundary admits class-bearing active modules generally (not only `New`-constructed).
- **Tier 3 — Host-global member catalog.** New metadata describing host-injected globals' members, ingested by the binder.
- **Tier 4 — Per-construct tail.** Knock down the remaining `Unsupported` cases incrementally.

As Tiers 0–2 land, the boundary predicates widen to admit class/reference shapes, the legacy `project.rs` rewrites lose their reason to exist, and the field-array carrier (`bd-aprs.9.13`) and broader legacy retirement (`10.2`/`10.8`) become safe deletions rather than risky ones.

## Key design decision (Tier 0/1 — needs a call before coding)

**How does the binder get its symbol/type knowledge?**

- **Option A — Inject `ProjectSymbolIndex` (+ typelib/host catalogs) into the existing single-module binder.** `build_hir_from_source` gains an optional project-context parameter; `resolve_name` consults it after local scope. Lower risk, incremental, reuses the complete index. Keeps the "one flattened source" compile shape initially.
- **Option B — Make the binder genuinely multi-module.** Build one `SymbolModel` spanning all project + referenced modules with real visibility, and bind each module against it; retire source-flattening. Cleaner end-state and the honest "final HIR state", but a larger change touching the project compile boundary.

Recommendation: **A first** (foundation + fast wins, de-risks Tier 1), evolving toward **B** as the flattening is retired. Tier 1's member-dispatch layer is where the real design weight is and should be specced before building.

## Mapping to existing beads
- Tier 0/1 reference binding ↔ FE-7.6 / FE-7.6.a (`bd-aprs.8.6`, `8.8`).
- Tier 1/2 class + property/default-member ↔ FE-7.3/7.4 (`bd-aprs.8.3`, `8.4`, `8.7`) + FE-8.5.c (`9.12`).
- These are the OPEN FE-7 beads; widening them is what unblocks the FE-9 retirements (`10.2`/`10.8`) and `9.13`.
