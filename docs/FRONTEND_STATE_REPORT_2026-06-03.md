# OxVBA Front-End State Report — Compiler & Runtime

> [!CAUTION]
> **Historical pre-clean-stack report.** Current compiler architecture and status are in `docs/ARCHITECTURE.md`, the 2026-07-10 review and compiler contract V2.

- **Report date:** 2026-06-03
- **Branch surveyed:** `single-package-descriptor-vm`
- **Scope:** Current state of the VBA **front-end** (lexer → parser → binder → HIR → lowering) and how far the migration *away from the legacy string-replacement front-end toward a new HIR* has progressed. The back-end/VM and COM/host areas are touched only where they bound the front-end.
- **Nature:** Read-only snapshot. No repository code or tracker state was modified to produce this report.
- **Primary sources:** `crates/oxvba-compiler/src/` (the compiler crate), `crates/oxvba-syntax/` (the new syntax substrate), `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`, `CURRENT_BLOCKERS.md` (entry **FE-PROD-001**), `docs/evidence/frontend_rework/*`, and the `bd-aprs` bead tree in `.beads/`.

---

## 1. TL;DR — where the migration actually stands

The front-end is **mid-migration and the controlling workset is explicitly OPEN/REOPENED**. The honest one-line status is:

> The new front-end (lossless CST → binder → bound HIR → HIR-to-bytecode lowering) is **built and is the default route for a large, route-audited subset of single-module VBA**, but the **legacy string-replacement front-end (`project.rs` rewrites + `resolve.rs`/`parse_expr` substring scanning) is still load-bearing in production** for project/class/COM/default-member semantics and is reached as a **fallback** for everything the HIR route does not yet own. The goal of *"no legacy, full VB-compatible front-end"* is **not yet reached.**

Quantitatively, from the migration's own bookkeeping:

| Signal | Value | Source |
|---|---|---|
| Front-end constructs defaulted to the new path (`V2`) | **9 of 10** (`ProjectSemantics` is the lone residual) | `frontend_route_policy.rs` |
| Tracked legacy paths | **7 rows: 3 Replaced, 4 Quarantined-Residual** | `frontend_retirement_inventory.rs` |
| `bd-aprs` migration beads | **72 total — 53 closed, 18 open, 1 in-progress** | `.beads` tracker |
| Milestone epics | **FE-0…FE-6 CLOSED; FE-7, FE-8, FE-9 OPEN** | workset §13 / tracker |
| Active blocker | **FE-PROD-001 — OPEN** | `CURRENT_BLOCKERS.md:54` |

The single most important nuance in this whole report (see §3): **there are two different things called "HIR" in this repo.** The old `oxvba-ir` HIR/MIR/CFG crate was deleted in the April native-ready rebase. The *new* "HIR" is the front-end **bound HIR** in `oxvba-compiler/src/frontend_hir*.rs`, built on the `oxvba-syntax` CST. This report is about the new one. `docs/IR_DESIGN.md` and `docs/ARCHITECTURE.md` still say "no HIR pipeline" — that refers to the *deleted* one and has not been updated for the new front-end work.

---

## 2. What "legacy string-replacement" means here

The front-end being retired is not a parser in the usual sense. As the workset states verbatim:

> "name resolution and a large amount of VBA member/statement semantics are implemented as **source-text surgery** (`crates/oxvba-compiler/src/project.rs`) that runs *before* a thin binder (`resolve.rs`), and operator-precedence parsing is done by repeatedly scanning raw `&str` substrings (`parse_expr`)."

Concretely the legacy front-end is three cooperating mechanisms, all still present in `crates/oxvba-compiler/src/`:

1. **`project.rs` — source-text rewriting.** Project/class/COM/default-member/property semantics are implemented by *rewriting VBA source text* before compilation: e.g. emitting helper-call source like `__oxvba_project_instance(handle)`, `__oxvba_withevents_set(...)`, `__oxvba_array_field_get/set/redim(...)`, and `property_*_pmr_*` helper lines, which the downstream resolver then parses. This is the "carrier" terminology that recurs throughout the commit log and beads: a *compatibility carrier* is a generated helper-source string standing in for a real semantic binding.
2. **`resolve.rs` — the thin binder + `parse_expr`.** Operator precedence is resolved by repeated substring scanning of the rewritten source (`parse_expr`), and symbol binding is derived from that. `resolve.rs` also still hosts the `#If` preprocessor expression parser (`tokenize_pp_expr`).
3. **`syntax_bridge.rs` — the transitional CST→legacy bridge.** A stop-gap that lowered the new CST back into the legacy `BoundExpr` shape. This is now `pub(crate)` and `#[cfg(test)]`-gated (see §6).

---

## 3. The target architecture (intended end-state)

Decided 2026-05-31 (workset Decision D0): a **Roslyn / rust-analyzer-style pipeline**, chosen because interactive tooling (LSP diagnostics, formatting, refactoring, incremental recompile) is a first-class goal, not just batch compile-to-bytecode:

```
source text
   │
   ▼
oxvba-syntax: hand-written lexer ──► green/red CST (lossless, immutable)   [crate: oxvba-syntax]
   │                                  + typed AST facades
   ▼
binder ──► bound HIR  +  SemanticModel overlay        [frontend_hir.rs, frontend_type_hooks.rs,
   │        (two layers: HIR for lowering;             frontend_semantic_model.rs, frontend_symbols.rs,
   │         SemanticModel for IDE queries)            frontend_project_symbols.rs]
   ▼
HIR-to-bytecode lowering                              [frontend_hir_lowering.rs]
   │
   ▼
Bytecode + ProcedureRuntimeMetadata ──► OxBundle (executable semantic package) ──► oxvba-vm
```

This deliberately differs from the deleted `oxvba-ir` (`VbaHir`/`VbaMir`/`CfgIr`) optimization scaffold removed in `WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`. The new HIR is a **front-end bound tree**, not a mid-level optimization IR. A future procedure-lowering IR (`ProcLoweringIr`) for JIT/native is still a separate, not-yet-built layer.

### 3.1 The new `oxvba-syntax` substrate — state

The CST crate is **mature and greenfield** (no references to legacy/bridge/HIR inside it). ~3,465 lines across 7 files:

| File | ~Lines | Role |
|---|---|---|
| `parser.rs` | 2,384 | Hand-written recursive-descent statements/declarations + **Pratt** expression parser (13 precedence levels, right-assoc `^`, postfix `.`/`!`/`(`) |
| `lexer.rs` | 588 | Zero-copy tokenizer; lossless trivia (whitespace, comments, line-continuations); all literal forms |
| `syntax_kind.rs` | 350 | 150+ token/node kinds; keyword + contextual-keyword classification |
| `red.rs` / `green.rs` | 297 / 172 | Lossless red/green CST (immutable green, positioned red cursors) |
| `lib.rs` | 14 | Public facade |
| `tests/lexer_snapshot_corpus.rs` | 157 | Lexer snapshot corpus |

**Grammar coverage is broad** (all major control flow, operators incl. `Mod`/`Like`/`Imp`/`Eqv`, member/bang/index access, `New`, `TypeOf…Is`, named args, all literal forms, round-trip-tested). **Known structural gaps at the CST layer**, handled elsewhere or deferred:
- `#If`/`#Const`/`#Else` conditional compilation is **not** parsed in the CST — it is pre-filtered from source by `resolve::apply_conditional_compilation_to_source` *before* parsing.
- `DefType` (`DefLng A-Z`, etc.) is **not** structurally parsed in the CST — handled in the binder layer.
- `Declare`, `Dim`/`Const` initializers, and `Type` members are **parsed conservatively** (keyword/modifiers structured; the remainder consumed losslessly to end-of-line). The detailed semantics are reconstructed by the binder.

These gaps are why some constructs the route audit marks "HIR production" still rely on non-CST front-end stages (preprocessor filtering, binder-side DefType handling) rather than a single clean CST→HIR path.

---

## 4. How the pipeline is actually wired today (the truth in code)

The default compile entry point is **HIR-first with a legacy fallback**, behind an **eligibility gate**. From `crates/oxvba-compiler/src/lib.rs`:

- `CompileOptions { frontend_v2: bool }` — **defaults to `false`** (`lib.rs:104`).
- **Strict mode** `compile_with_options(src, { frontend_v2: true })` (`lib.rs:159`): runs front-end diagnostics + binder/HIR, then `compile_source_with_runtime_metadata_via_hir`. **No legacy fallback** — an unsupported construct returns `frontend_v2 HIR unsupported: …`. This is the "clean" path and is currently opt-in/test-facing.
- **Default mode** (`frontend_v2: false`, `lib.rs:169`): tries HIR; on `HirProductionLoweringError::Unsupported`, **falls back to `compile_with_runtime_metadata_legacy`**.
- The lightweight production path `compile_with_runtime_metadata_object_locals_class` (`lib.rs:251`): first calls `apply_conditional_compilation_to_source`, then checks **`source_is_eligible_for_lightweight_hir_default(...)`**. If *not* eligible → straight to legacy. If eligible → try HIR, fall back to legacy on `Unsupported`.
- **Project compile:** "single active procedural-module projects with no reference projects now enter the HIR-capable metadata compiler at the project boundary, while broader project shapes remain on the legacy project backend" (FE-PROD-001).

So the route is gated three times: (a) preprocessor still external; (b) an eligibility predicate decides whether HIR is even attempted; (c) HIR may still answer `Unsupported` and drop to legacy. The workset is explicit that **this fallback is a migration mechanism, not a semantic owner**, and that its continued existence keeps the workset open.

---

## 5. Migration milestone map (FE-0 … FE-9/10 → `bd-aprs.N`)

The migration is one bead tree rooted at **`bd-aprs`**. Status as of 2026-06-03 (tracker cross-checked against the workset's 2026-06-02 bead-graph repair):

| Milestone | Bead (epic) | Status | What it covers |
|---|---|---|---|
| FE-0 Workset prep / truth repair | `bd-aprs.1` | **CLOSED** | Foundation, truth audit |
| FE-1 Grammar & coverage foundation | `bd-aprs.2` | **CLOSED*** | EBNF/grammar matrix (coverage-matrix child reopened once) |
| FE-2 Syntax substrate audit | `bd-aprs.3` | **CLOSED** | `oxvba-syntax` hardening |
| FE-3 Lexer completion | `bd-aprs.4` | **CLOSED** | Lossless lexer + trivia |
| FE-4 Parser + CST→legacy bridge | `bd-aprs.5` | **CLOSED** | Pratt/statements; bridge now test-only |
| FE-5 Semantic harness + `frontend_v2` gate | `bd-aprs.6` | **CLOSED** | `CompileOptions.frontend_v2`, diff harness |
| FE-6 Binder, HIR, SemanticModel core | `bd-aprs.7` | **CLOSED** | Core bound-tree + semantic model |
| **FE-7 Project semantics migration from `project.rs`** | `bd-aprs.8` | **OPEN** | Children open: **8.3, 8.4, 8.6, 8.7, 8.8** (8.1/8.2/8.5 closed) |
| **FE-8 Typed intrinsics, lowering, optimizer split** | `bd-aprs.9` | **OPEN** | Open: **9.5, 9.9, 9.10, 9.12**; closed: 9.1–9.4, 9.6–9.8, 9.11, **9.13** |
| **FE-9 Flip, retirement, IDE query foundation** | `bd-aprs.10` | **OPEN** | Open: **10.2, 10.5, 10.6, 10.7, 10.8**; closed: 10.1, 10.3, 10.4 |
| FE-10 Final default flip (planned) | — | **NOT STARTED** | Make `frontend_v2` the default; delete legacy |

`*` The closed FE-0…FE-6 beads are, in the workset's own words, *"scoped foundation evidence, not proof that the production compiler front-end has been replaced."*

2026-07-06 superseding note: `bd-aprs.9.13` has since been closed in the current crate graph after
the old compiler/project rewrite bridge was removed and class field-array lowering was proved
through Core IR/OxIR/VM3 fused field-array operations. The remaining legacy-code deletion pressure
is now concentrated in `bd-aprs.10.2` (legacy parser/rewriter retirement) and `bd-aprs.10.8`
(final legacy-route retirement), gated by audits `10.6`/`10.7` and terminal closure `10.5`, plus
the still-open FE-7 binding beads.

---

## 6. What is migrated to the new HIR (route-audited "HirProduction")

`frontend_legacy_route_audit.rs` runs ~100 representative source fixtures through the live route classifier (`syntax_bridge::production_route_for_source`) and asserts each lands on `HirProduction`. As of this snapshot the in-code **terminal gate passes** (`run_production_legacy_route_audit().terminal_gate_passed() == true`), and the grammar-matrix overlay (`frontend_grammar_matrix_route_audit.rs`) maps **110 grammar productions → HirProduction**. The audited surface includes:

- **Expressions:** arithmetic/comparison/logical, `Not`/unary, `^`, `&` concat, `Mod`, integer-div `\`, `Like`, `TypeOf…Is`, qualified identifiers, member `.` reads, bang `!` reads, index/call.
- **Statements:** assignment; `If`/`ElseIf`/`Else` (block + single-line); `Select Case` (single/range/multi/`Is`); `Do While/Until` + post-check; `While/Wend`; `For`; `For Each`; `Exit Do/For/Sub`; `On Error`/`Resume` (label, numeric, zero, resume-next); `GoTo` (label + numeric); `GoSub`/`Return`; `Erase`; `ReDim` (1-D/2-D, explicit lower bound, dynamic element r/w, fixed-array alias + `ReDim Preserve` rematerialization); `RaiseEvent`; `With` reads; member/bang assignment targets; `Call` (incl. bare-arg and statement-form, member-call args).
- **Declarations:** `Dim`/`Static`/`Const` (incl. typed + simple constant expressions, Currency/Date/Single/Double, type-char, Boolean/Xor/Eqv/Imp/Like); `Enum` members; `Type`/UDT layout (nested, fixed-string, array fields); `Declare` (typed signatures, ordinal alias, `Any`, `LongPtr`, missing-`PtrSafe` diagnostic); `Event`; single-source `Implements`; `Optional`/`ParamArray` parameters; `Property Get/Let/Set` incl. indexed and named-indexed.
- **Module directives:** `Option Explicit`, `Option Compare Text/Database`, `Option Private Module`, `Option Base 0/1`, `DefType`, `#Const`/`#If` (via preprocessor), `Attribute VB_*` (module + member, boolean attrs).
- **Intrinsics (very large library now lowering through HIR):** string (`Len/Left/Right/Mid/InStr/InStrRev/Replace/StrComp/LCase/UCase/Trim/Space/String/Chr/Asc/StrReverse/StrConv/Format/Split/Join`), math (`Abs/Int/Fix/Sgn/Round/Sqr/Sin/Cos/Log/Exp/Atn/Tan`), conversion (`CStr/Str/Val/CDate/Hex/Oct`), financial (`FV/PV/Pmt/NPV/IRR/MIRR/Rate/NPer`), pointers (`StrPtr/VarPtr/ObjPtr`), `Array`, `Rnd/Randomize`, host intrinsics (`Date/Time/Now/Timer/FreeFile/DoEvents/EOF/LOF/Seek/Loc/MsgBox/InputBox/Shell/Environ/Dir`), `CreateObject`, `DispatchInvoke`, console/file I/O (`Print`, `Debug.Print`, `Kill`, `Open`, `Close`, `Print#`, `Write#`, `Input#`, `Line Input#`).
- **Project construction (project-compile audits):** direct active-project `Set obj = New Widget` and `Dim x As New Widget` now lower through `HirNewExpressionBinding` → `LoadProjectObjectRef`, **without** leaving a `__oxvba_project_instance(...)` helper-source artifact; narrowed WithEvents `Set src = New Emitter` routes through a HIR temporary. `Class_Initialize`/`Class_Terminate` metadata and bare-object `Is` identity are covered for the scoped active-project case.
- **Language service:** `oxvba-languageservice` semantic snapshots now consume compiler HIR/query facts rather than rebuilding a legacy `BoundModule`.

### 6.1 Why "the audit passes" ≠ "the migration is done"

This is the crux. The in-code gates are green, yet the workset is reopened and FE-PROD-001 is open, because the gates measure something narrower than completion:

- The audits are anchored to **curated representative fixtures** and a **110-row grammar-matrix overlay**, not the full accepted compiler/host/IDE/Excel-oracle corpus. The grammar-matrix doc itself says it is *"route-proof overlay, not full matrix/documentation closure."*
- A fixture being classified `HirProduction` proves *that fixture routes to HIR*. It does **not** prove the legacy code is deleted, nor that arbitrary combinations/broader project shapes route to HIR.
- The standalone single-source classifier still returns `HirUnsupportedResidual` for object-heavy single-source snippets (e.g. `Set obj = New Widget` with no project graph), which in default mode **falls back to legacy**. The same construct only reaches HIR inside a full *project* compile with the class present.

---

## 7. What is still on the legacy path (residuals & open work)

### 7.1 Route policy residual (`frontend_route_policy.rs`)

9 of 10 `FrontendConstruct`s default to `V2`. The lone `LegacyResidual` is **`ProjectSemantics`**, reason recorded in code:

> "FE-7 project semantics has binder-owned symbol tables and a module-aware production lowering route, but **project.rs line lowering/rewrite glue remains load-bearing**."

### 7.2 Retirement inventory (`frontend_retirement_inventory.rs`) — 7 tracked paths

| Legacy path | Disposition | Owner | Note |
|---|---|---|---|
| `resolve::parse_expr_for_syntax_bridge` | **Replaced** | bd-aprs.10.2 | superseded by HIR scoped lowering |
| `syntax_bridge::lower_cst_expr` CST→legacy bridge | **Replaced** (test-gated) | bd-aprs.9.6 | now `pub(crate)` + `#[cfg(test)]` |
| stringly structural intrinsic names | **Replaced** | bd-aprs.9.1 | now typed `StructuralIntrinsic` enum |
| `compile_with_options` legacy fallback after HIR `Unsupported` | **Quarantined residual** | bd-aprs.9.6 | the live default-mode fallback |
| `resolve::parse_expr` substring splitting in resolver | **Quarantined residual** | bd-aprs.9.6 | still exists; bypassed by HIR fixtures, not deleted |
| `bundle.rs` module-fact `resolve_symbols` fallback | **Quarantined residual** | bd-aprs.10.8 | HIR `BoundModule` facts preferred; resolver fallback for unsupported modules |
| `project.rs` text rewrites (project/class/COM/default-member) | **Quarantined residual** | bd-aprs.7.* , 9.6 | the single biggest open area |

### 7.3 The hard remaining frontier (open FE-7/FE-8/FE-9 beads)

- **Project/class/COM property & default-member writeback** (`bd-aprs.8.7`, `8.8`, lowering half `9.12`). Large progress recorded (imported-COM dispatch-id validation, host-injected `HostGlobal` validation, statement-form named args through HIR, late-bound `obj(42)` default-member, `BoundStmt::AssignDefaultMember`, default-member ambiguity rejection) — **but** broad writeback breadth, type-overload validation, early-bound COM property-put, and **deletion of the remaining `property_*_pmr_*` rewrite bodies** remain open. Several routes still *validate then retain* a compatibility carrier rather than owning the semantics in HIR.
- **Project/class array-field carriers** (`bd-aprs.9.13`, closed after this report). The old
  `lower_module_source_module_aware` / helper-source carrier residual is superseded in the current
  crate graph: field-array `ReDim` is bound as a `CorePlace::Field` compound resize/writeback, and
  element get/set lower to fused `FieldArrayGet`/`FieldArraySet` VM3 operations.
- **Reference / imported-COM activation & member binding** (`bd-aprs.8.6`, `8.8`). Referenced-project class construction and early-bound COM activation still partly legacy.
- **Broad compile-time evaluation breadth** (`bd-aprs.9.9`, `9.10`): richer constant/default expressions, locale-sensitive `Date`, full `LongPtr`, remaining DefType/preprocessor/typed-constant breadth, broader declaration/type surface.
- **Broad route audit + final retirement** (`bd-aprs.10.7` broad matrix/corpus/host/IDE/Excel audit; `10.2`/`10.8` delete/quarantine legacy `parse_expr`, CST→legacy lowering, `project.rs` rewrites, duplicate language-service semantics; `10.5` terminal closure). None complete.

---

## 8. Governance / bookkeeping machinery (how progress is tracked in-code)

The migration carries an unusually explicit, test-enforced ledger inside the compiler crate. For a reader auditing status, these are the files to read first:

| File | ~Lines | Purpose |
|---|---|---|
| `frontend_route_policy.rs` | 96 | Per-construct V2-vs-LegacyResidual policy + residual list |
| `frontend_retirement_inventory.rs` | 152 | The 7 legacy-path rows with disposition/owner/closure-condition |
| `frontend_legacy_route_audit.rs` | 1,629 | ~100 fixtures classified HirProduction / LegacyFallbackResidual / StaticResidual; terminal gate |
| `frontend_grammar_matrix_route_audit.rs` | 264 | 110 grammar productions → audit-area route findings |
| `frontend_diff.rs` | 2,761 | Differential harness comparing HIR vs legacy output (semantic-equivalence gate; byte-identical bytecode is explicitly **not** a closure condition) |
| `frontend_lowering_contract.rs` | 543 | Validates emitted `ProcedureRuntimeMetadata` against typed-HIR contracts |
| `frontend_hir.rs` / `frontend_hir_lowering.rs` | 3,734 / 9,933 | The bound HIR types and HIR→bytecode lowering (the bulk of the new front-end) |
| `frontend_type_hooks.rs` / `frontend_semantic_model.rs` | 1,131 / 513 | Typed HIR collection + IDE-facing SemanticModel overlay |
| `frontend_symbols.rs` / `frontend_project_symbols.rs` | 1,071 / 1,639 | Binder symbol tables (local + project) |
| `frontend_member_dispatch.rs` / `frontend_assignment_semantics.rs` / `frontend_class_semantics.rs` / `frontend_event_semantics.rs` / `frontend_external_references.rs` / `frontend_structural_intrinsics.rs` | 169 / 346 / 180 / 136 / 277 / 119 | Per-area semantic classifiers migrating logic out of `project.rs` |

Legacy side still compiled into production: `resolve.rs` (10,480 lines), `project.rs`, `emit.rs` (11,875), `typecheck.rs` (2,620), `syntax_bridge.rs` (804, test-gated entry points).

**Reproduce the status locally** (read-only): the gate tests in these files (`audit_terminal_gate_passes_after_audited_residuals_retire`, `grammar_matrix_route_audit_maps_broad_anchored_rows_to_hir_production`, `route_policy_defaults_*`, the `retirement_inventory_*` tests) encode the current claims and will fail if a residual regresses.

---

## 9. Blockers

| ID | Status | Meaning for the front-end |
|---|---|---|
| **FE-PROD-001** | **OPEN** (since 2026-06-01 scope audit) | The controlling blocker: front-end production replacement not complete; legacy `frontend_v2`-as-bridge, `resolve.rs`, and `project.rs` rewrites remain load-bearing. |
| FE-TERM-001 | RESOLVED (2026-06-01) | Front-end terminal metadata failure (argument binding descriptors); superseded by FE-TERM-002. |
| FE-TERM-002 | RESOLVED (2026-06-01) | Host-snapshot regressions from the front-end pass; VM-owned completed-frame snapshot surface added. |
| RV-BRIDGE-001..004 | RESOLVED | `RuntimeValue` value-carrier removed (`bd-0w46`, commit `8d5fdfc0`); unrelated to front-end string-rewriting but shares the "carrier" vocabulary. |

---

## 10. Relationship to the runtime / VM (context for "and runtime")

The front-end lowers to **Bytecode + metadata packaged as an `OxBundle`**, executed by `oxvba-vm`. Two recent VM worksets bound this contract:

- `WORKSET_2026-05-28_STRICT_PACKAGE_ONLY_VM_EXECUTION.md` (`bd-embl`, handoff-passed): made the executable semantic package the only accepted VM input; bundle format bumped (v15→v16+), raw `Bytecode` execution removed as a production path.
- `WORKSET_2026-05-29_SINGLE_PACKAGE_DESCRIPTOR_VM.md` (`bd-eura`, **in progress**, this branch): "**one** VM that runs the compiler's bytecode+metadata package directly and runs the full build-target feature set correctly, without non-object memory leaks." It is **deleting** the consumption-evidence/support-report *gating* apparatus built by `bd-embl` (per the project's own direction note) in favour of a single correct interpreter path. Object reference-cycle leaks are treated as VBA-consistent, not bugs in scope.

For the front-end this matters because the new HIR lowering must emit a package the strict VM accepts; the VM-side simplification (`bd-eura`) and the front-end migration (`bd-aprs`) are concurrent, separate efforts that meet at the `OxBundle` boundary. JIT (`oxvba-jit`) is disabled pending a v2 design and is not part of either.

---

## 11. Honest assessment — "where are we in the process?"

**Foundation: done.** The lexer, lossless CST, Pratt expression parser, binder, bound HIR, SemanticModel, the `frontend_v2` gate, the differential harness, and the route/retirement bookkeeping all exist and are closed (FE-0…FE-6). This is real, substantial infrastructure, not scaffolding-in-name.

**Scoped language surface: largely flipped.** For single-module procedural VBA — expressions, all control flow, declarations, a very large intrinsic library, scoped project construction — the default compile path **is** the new HIR, with legacy reached only as a fallback. The in-code terminal route gate passes for the curated fixture set.

**The tail — and it is the hard, semantics-dense part — is open.** Project/class/COM/default-member/property semantics and array-field handling still depend on `project.rs` source rewrites; `resolve.rs`/`parse_expr` still exist and are still authoritative for fallback paths; broad-corpus/Excel-oracle route auditing has not been done; and **no legacy code has been deleted** — the residuals are *quarantined and validated*, not removed. The project's own discipline (three reopen passes on 06-01/06-02/06-03) is precisely a guard against declaring victory on "tests pass / residuals documented."

**Distance to the stated goal ("no legacy, full VB-compatible front-end"):** the project is past the midpoint structurally but the remaining beads are the highest-semantic-density ones (FE-7 project semantics, FE-8 lowering breadth, FE-9 flip+retire). The terminal gate cannot close until FE-7/FE-8 land *and* the broad audit (FE-9.7) passes *and* the legacy parser/rewriter is actually deleted or hard-quarantined (FE-9.2/FE-9.8) *and* the default flips to `frontend_v2` (FE-10). On current evidence none of those four are complete.

### Documentation caveats found during this survey (worth fixing separately)
- `docs/ARCHITECTURE.md` ("Current IR Truth") and `docs/IR_DESIGN.md` previously stated there is no HIR pipeline, which was true of the *deleted* `oxvba-ir` optimization IR but misleading w.r.t. the new front-end bound HIR. **Both were reconciled on 2026-06-03** to distinguish the two and to state the unified End-State Destination (see §12); `ARCHITECTURE.md` now carries the canonical "End-State Destination (North Star)" section.
- `docs/IN_PROGRESS_FEATURE_WORKLIST.md` and `docs/INITIAL_SCOPE_STATUS_2026-03-24.md` predate the rework and do not track it.
- `docs/IMPLEMENTATION_LOG.md` contains **no** entries for the front-end migration — its authority lives in the workset, `CURRENT_BLOCKERS.md` (FE-PROD-001), and `docs/evidence/frontend_rework/`.

---

## 12. End-State Destination & Sequencing (confirmed 2026-06-03)

The migration's north star, confirmed by the project owner on 2026-06-03 and now
canonicalized in [`ARCHITECTURE.md`](ARCHITECTURE.md) → "End-State Destination
(North Star)":

> A state-of-the-art VBA compiler with **one compiler-owned front-end**
> (`oxvba-syntax` lossless CST → binder → bound HIR + SemanticModel → lowering)
> that emits a single **bytecode + metadata package**, consumed by **two runtime
> targets**: a portable interpreting VM (runs anywhere, incl. browser and Tauri
> desktop) that is the permanent reference oracle, and a Cranelift JIT that
> lowers from the same package. No production path rewrites source text or scans
> substrings.

**Two strictly ordered phases:**

1. **Phase 1 — full correctness on the VM** across the entire imaginable matrix:
   all COM scenarios (client + COM-server hosting), native interop
   (`Declare`/pointers), browser (WASM) and desktop (Tauri) execution, and all
   build targets — `Bundle`, `WrapperExe`, `WrapperLibrary`, `WrappedComServer`
   (`BuildTarget` in `oxvba-project/src/model.rs:54`; native-image
   `NativeExe`/`NativeDll` are a later evolution). The `bd-aprs` front-end
   migration is part of getting this phase correct.
2. **Phase 2 — Cranelift JIT**, only after Phase 1, off the *same* package, as the
   optimizing fast path, with the VM kept as the stable reference. JIT activation
   is gated on Phase-1 correctness, not on a schedule.

**Two design consequences that settle earlier open questions:**

- **No new front-end lowering IR is motivated.** The shared contract is the
  bytecode+metadata package (the CLR/JVM/Wasm pattern: bytecode *is* the IL, the
  interpreter runs it, the JIT compiles from it). A mid-IR between HIR and
  bytecode would be used by exactly one consumer (the emitter) and is unnecessary;
  the bytecode is already a mature low-level target (~229 typed instructions). The
  team already deleted one speculative mid-IR (`oxvba-ir`). The single motivated
  lowering IR is the JIT's consumer-side `ProcLoweringIr`, which is Phase 2.
- **The `Bound*`/`emit.rs` question is internal cleanup, not architecture.** Both
  "keep an owned mid-IR" and "HIR→bytecode directly" produce the same package, so
  the choice does not affect the destination. The destination-level requirement is
  that the front-end emits the **complete, JIT-ready package now** (typed slots,
  coercion facts, call/ABI descriptors, deopt/error-state), so Phase 2 need not
  reopen it — because every fact the JIT needs must already live in the package
  and be visible to VM execution.

## 13. Appendix — source & evidence index

**New front-end (compiler crate, `crates/oxvba-compiler/src/`):** `frontend_hir.rs`, `frontend_hir_lowering.rs`, `frontend_type_hooks.rs`, `frontend_semantic_model.rs`, `frontend_symbols.rs`, `frontend_project_symbols.rs`, `frontend_member_dispatch.rs`, `frontend_assignment_semantics.rs`, `frontend_class_semantics.rs`, `frontend_event_semantics.rs`, `frontend_external_references.rs`, `frontend_structural_intrinsics.rs`, `frontend_operator_normalization.rs`, `frontend_diagnostics.rs`, `frontend_query.rs`, `frontend_language_service.rs`, `frontend_lowering_contract.rs`, `frontend_diff.rs`, `frontend_route_policy.rs`, `frontend_retirement_inventory.rs`, `frontend_legacy_route_audit.rs`, `frontend_grammar_matrix_route_audit.rs`.

**New syntax substrate:** `crates/oxvba-syntax/` (lexer, green/red CST, Pratt parser).

**Legacy front-end (still compiled in):** `resolve.rs`, `project.rs`, `syntax_bridge.rs` (test-gated), plus `emit.rs`/`typecheck.rs`/`bundle.rs`.

**Controlling workset:** `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md` (status: reopened / incomplete; passes 2026-05-31, 06-01, 06-02, 06-03).

**Evidence directory:** `docs/evidence/frontend_rework/` (≈47 files; freshest 2026-06-03), incl. `PRODUCTION_HIR_LOWERING_2026-06-01.md`, `LEGACY_RETIREMENT_INVENTORY_2026-06-01.md`, `PER_CONSTRUCT_ROUTE_POLICY_2026-06-01.md`, `FRONTEND_V2_GATE_2026-06-01.md`, `PRODUCTION_LEGACY_ROUTE_AUDIT_2026-06-01.md`, `GRAMMAR_MATRIX_ROUTE_AUDIT_2026-06-02.md`, `HIR_LOWERING_CONTRACT_2026-06-01.md`, `FRONTEND_REWORK_TRUTH_AUDIT_2026-06-01.md`.

**Blocker:** `CURRENT_BLOCKERS.md` → **FE-PROD-001** (line 54).

**Tracker:** `.beads/` (CLI `br`), root epic `bd-aprs` (72 beads: 53 closed, 18 open, 1 in-progress). Milestones: FE-0..FE-6 = `bd-aprs.1`..`.7` (closed); FE-7 = `bd-aprs.8`, FE-8 = `bd-aprs.9`, FE-9 = `bd-aprs.10` (open). Key open/in-progress: `8.3, 8.4, 8.6, 8.7, 8.8, 9.5, 9.9, 9.10, 9.12, 9.13(IP), 10.2, 10.5, 10.6, 10.7, 10.8`.

---

*Prepared 2026-06-03 as a read-only snapshot. The most authoritative live status is the `bd-aprs` bead tree plus `CURRENT_BLOCKERS.md`; this report should be re-checked against those before being cited as current.*
