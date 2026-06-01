# Workset: Front-End Refactor — Tokenizer, Parser, Resolved AST, and Binder

Date: 2026-05-31
Owner: DNA Kode
Status: proposed / preparation-review

Architecture decision, 2026-05-31:

The syntactic layer will be a **Roslyn-style green/red concrete syntax tree** (lossless,
immutable) with **typed AST facades**, chosen because interactive tooling (language-server
diagnostics, formatting, refactoring, incremental recompile) is a goal, not only batch
compile-to-bytecode. This shape is already partially present in `oxvba-syntax` through a custom
green/red tree, hand-written lexer, Pratt-capable parser, and typed accessor helpers. The
front-end rework must build on that substrate unless Phase 0 proves a library migration is worth
the churn.

`rowan` and `cstree` are helper libraries for the same general syntax-tree pattern used by
rust-analyzer. They are not semantic requirements. The requirement is the Roslyn-style shape:
lossless syntax tree, syntax/semantics separation, typed facades, stable node identity/spans, and
an IDE-capable semantic query layer. Whether that is backed by the current custom tree, `rowan`,
or `cstree` is a Phase 0 engineering decision.

The resolve → bound-IR → lower backbone is unchanged; this decision sets the target *front-end
architecture* and adds a semantic-overlay + incremental capability. (Lean-AST/rustc shape was
the alternative; see §5.5 and the decision log §10.)

## 1. Purpose

Move OxVba's compiler front-end from its current **string-rewriting + string-splitting**
shape toward a conventional, Roslyn/rust-analyzer-shaped pipeline:

```
source text
  → lexer (tokens + trivia + spans)
  → parser (recursive-descent + Pratt, drives a green-node builder)
      → green/red CST (lossless, immutable) + typed AST facades
  → resolver / binder (symbol table, scopes)
      → bound HIR (resolved IR for lowering) + SemanticModel overlay (queries over the CST)
  → lowering → bytecode (+ runtime metadata)
  → (later) incremental recompute via a query engine (salsa)
```

Note the two semantic layers, mirroring Roslyn/rust-analyzer: a **bound HIR** (the resolved
tree that lowering and typecheck consume — Roslyn's "bound tree", rust-analyzer's HIR) *and* a
**SemanticModel overlay** (lazy, cached symbol/type queries keyed by CST nodes, for the
tooling/IDE API). Compilation lowers the bound HIR; tools query the overlay. The CST itself
carries no binding/type information.

Today, name resolution and a large amount of VBA member/statement semantics are implemented
as **source-text surgery** (`crates/oxvba-compiler/src/project.rs`) that runs *before* a thin
binder (`resolve.rs`), and operator-precedence parsing is done by repeatedly scanning raw
`&str` substrings (`parse_expr`). This works and is heavily tested, but it is fragile at the
edges, hard to extend, and splits single concepts (e.g. member access) across two paradigms.

This workset is a **plan for the compiler front-end migration**, not a claim that no supporting
syntax work exists. Git history shows `oxvba-syntax` was scaffolded in the initial workspace
bootstrap (`68965e4e`, 2026-02-26), then substantially expanded for language-service work
(`5f4da2f3`, 2026-03-23: Pratt expression parser, typed accessors, provider trait). The workset
therefore starts from a partial syntax/IDE substrate that is not yet wired into the production
compiler pipeline. No production compiler front-end behavior changes until a phase ships behind
the planned gate and evidence.

## 2. Correctness authority (unchanged repo convention)

1. Actual VBA running in Excel on Windows.
2. Published specifications — primarily **[MS-VBAL] VBA Language Specification** for the
   grammar, plus COM/Automation/ABI specs.
3. Existing OxVba behavior as a **regression anchor only** (a baseline to diff against, not a
   source of truth). If a string rewriter encodes a divergence from Excel/MS-VBAL, the new
   pipeline must not inherit it.

The new pipeline must preserve or improve observable VBA behavior against Excel/MS-VBAL evidence.
During migration, the current compiler output is a regression baseline and a diagnostic aid, not a
byte-for-byte contract. Bytecode, slot choices, temporary layout, helper selection, and metadata
ordering may legitimately differ when the new front-end emits cleaner lowering, provided the
differences are explained and the resulting execution behavior, diagnostics, and public metadata
contracts are correct for the scoped construct. Differential comparison remains valuable, but its
gate is semantic equivalence plus documented intentional improvements, not byte identity.

## 3. Motivation — the shortcut inventory (grounded in current code)

| # | Shortcut (today) | Where | Traditional shape |
|---|---|---|---|
| S1 | Production compiler path does not use the existing general syntax lexer/parser; precedence is still recovered by substring scanning | `parse_expr`, `split_at_lowest_precedence_op`, `split_compare_keyword_top_level`; tell-tale patch `parse_typed_suffix_literal` (disambiguates `100&` from `x & y`) | lexer → token stream → Pratt parser feeding the compiler front-end |
| S2 | Names are strings; AST is **not resolved** | `BoundExpr::Var(String)`, `ProcCall { name: String }`, `Member { member: String }`; resolution recovered later via `slot_map` (emit) + `project.rs` | symbol table; AST carries `SymbolId` |
| S3 | String-rewriting front-end (macro-by-text) | `project.rs`: member dispatch, default members, property Get/Let/Set, qualified names, `New`, WithEvents, collections, F3c diagnostics | resolve in the binder against the symbol table |
| S4 | Stringly-typed intrinsics as an escape hatch (~25 magic names) | `IntrinsicCall { name }` (`__empty`, `__null`, `__nothing`, `__oxvba_project_instance`, `__oxvba_withevents_*`, `dispatchinvoke`, `__omitted`, `vbnullstring`, …); giant `match name.as_str()` in `emit.rs` | dedicated AST/IR nodes (or a typed `enum Intrinsic`) for structural concepts |
| S5 | Under-modeled operators / postfix | `Is` (object identity) unsupported as a binary op (only `TypeOf x Is T`); indexing is the `__oxvba_array_get` intrinsic, not a uniform `Index`; `New`, bang `obj!field` are string-rewritten | unified postfix grammar: call / index / member / bang; `CompareOp::Is` |
| S6 | Peephole optimization baked into AST shape | `BoundExpr::AddConst`/`SubConst` produced directly by the parser, special-cased in every consumer | uniform `BinaryOp`; recognize in an optimizer pass |

Precedent already in-repo: `oxvba-syntax` contains a custom lossless green/red tree, lexer,
Pratt-capable expression parser, statement parser, and typed red-tree accessors, originally
introduced before this workset for language-service scaffolding. Separately, a real tokenizer+
parser exists for `#If` preprocessor expressions (`tokenize_pp_expr` → `PpToken`,
`resolve.rs:1323`), and `BoundExpr::Member` (added 2026-05-31, commit `f7cb6b85`) began
collapsing S5 for call-result receivers in the current compiler IR. This workset reconciles those
three strands into the production front-end.

## 4. Formal grammar

### 4.1 Is there a VBA grammar?

Yes — two authoritative references:

1. **[MS-VBAL]: VBA Language Specification** (Microsoft Open Specifications). Contains the
   lexical and syntactic grammar in an ABNF/EBNF-style notation, plus static and runtime
   semantics. This is our **grammar conformance authority**. The repo already cites MS-VBAL
   (`docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`).
2. **Rubberduck VBA ANTLR4 grammar** (`rubberduck-vba/Rubberduck`, `VBALexer.g4` /
   `VBAParser.g4`, MIT). The most battle-tested real-world VBA/VB6 grammar; handles the
   awkward lexical reality (line continuation `_`, statement separator `:`, `Rem`, bang
   member access, `With`, `On Error`, date literals, type-suffix sigils, bracketed
   identifiers). Excellent cross-reference and quirk checklist.

### 4.2 Recommendation: capture a grammar as the *spec*, hand-write the parser

- **Capture** an in-repo EBNF grammar derived from MS-VBAL, cross-checked against Rubberduck's
  ANTLR, as `docs/spec/VBA_GRAMMAR_V1.ebnf(.md)`. Use it to (a) drive a **grammar-coverage
  conformance matrix** (every production → a fixture + expected parse) and (b) guide the
  implementation. (We reference the grammars; we do not copy ANTLR/spec text into code.)
- **Implement** a hand-written recursive-descent parser with a Pratt expression sub-parser —
  *not* a generated parser. Rationale matches production compilers (rustc, Roslyn, Clang,
  Swift all hand-write despite formal grammars): error recovery and quality, control over
  VBA's context sensitivity (statement forms, `:` separators, line continuation, `Set`/`Let`/
  `Call`), and clean integration with existing bytecode lowering. Generators (LALRPOP LR(1),
  pest PEG) fight these and produce poor diagnostics.

The grammar artifact has value independent of implementation: it is a conformance reference
and a coverage checklist, and it documents the dialect we target (VBA 7.x x64 in Excel).

## 5. Rust libraries & compiler-authoring patterns (decision points)

Each is a decision to confirm during Phase 0 via a small spike or audit. Defaults below are
recommendations, but the current `oxvba-syntax` implementation is the starting point.

### 5.1 Lexer
- **Current/default: hand-written lexer.** VBA's lexical quirks (case-insensitive keywords, `_`
  line continuation joining physical lines, `:` statement separators, type sigils `%&!#@$`,
  `[bracketed]` identifiers, `'` and `Rem` comments, `#…#` date literals, `&H`/`&O` literals,
  `""` string escaping) are easier to get exactly right by hand. The current lexer already covers
  a useful subset and must be audited against the grammar matrix before its coverage is broadened.
- Alternative: **`logos`** (derive-based, very fast) with callbacks. Viable; revisit if lexer
  perf matters or the quirks fit cleanly.

### 5.2 Parser
- **Current/default: hand-written recursive descent + Pratt (precedence climbing)** for
  expressions. Maximum control, best diagnostics, easiest to integrate. The current
  `oxvba-syntax` parser validates the approach but is not yet the production compiler parser.
- Alternatives evaluated: **`chumsky`** (combinators; excellent error recovery/reporting, has
  Pratt helpers — strongest alternative if we want batteries-included diagnostics);
  **`winnow`/`nom`** (fast combinators, lower level); **`lalrpop`** (LR(1) from grammar — rigid
  recovery, awkward for VBA context sensitivity); **`pest`** (PEG — ordered-choice ≠ MS-VBAL
  BNF, precedence awkward). Recommend hand-written; keep `chumsky` as the fallback if hand-rolled
  diagnostics prove insufficient.

### 5.3 Spans & diagnostics
- Track a byte `Span` on every token and AST node from day one (today errors are stringly with
  the raw source line). Render with **`ariadne`** or **`codespan-reporting`**. This alone is a
  large quality-of-life and conformance-reporting win.

### 5.4 Identifier interning (case-insensitivity)
- **`lasso`** or **`string-interner`**: intern identifiers to a `Symbol`, folding case at intern
  time (VBA is case-insensitive). Replaces the pervasive `String`/`to_ascii_lowercase()` churn
  and gives cheap symbol comparison in the resolver.

### 5.5 Syntactic layer & IR storage — **green/red CST (decided)**
- **Syntax = green/red CST.** The semantic decision is the tree shape, not a specific helper
  crate. Untyped `SyntaxKind` nodes carry widths/offsets in the green tree; red wrappers provide
  lazy parent/position-aware traversal; the tree is lossless and immutable; typed AST facades are
  thin accessors over syntax nodes, not a separate owned enum tree. The existing custom
  `oxvba-syntax` green/red implementation already follows this direction and should be treated as
  the default unless Phase 0 finds concrete reasons to migrate.
  - **`rowan`** is the rust-analyzer syntax-tree library for this pattern.
  - **`cstree`** is a rowan-compatible alternative with built-in token interning and stronger
    multi-threading/memory options.
  - Migration to either helper library is optional. It requires evidence that the library removes
    real maintenance risk, enables required IDE/incremental behavior, or materially improves memory
    and threading behavior for large projects.
- **Bound HIR + symbol tables = index-based arenas** (newtype `ExprId`/`StmtId`/`SymbolId` into
  `Vec`s; the rustc / rust-analyzer pattern). The HIR is the resolved tree lowering consumes; it
  references CST nodes by id for spans and for the SemanticModel mapping.
- **Incremental engine = `salsa`** (rust-analyzer's query framework) — memoizes/invalidates
  parse → resolve → typecheck queries for incremental recompute and tooling. Adopt **after** the
  CST + HIR exist (a later phase); the batch pipeline works without it first.

### 5.6 Patterns to adopt
- **Syntax ≠ semantics (the core Roslyn idea)**: the green/red CST carries *no* binding/type
  info. Two semantic layers sit above it: a **bound HIR** (resolved tree → lowering + typecheck)
  and a **SemanticModel overlay** (lazy, cached symbol/type queries keyed by CST nodes → the
  tooling API). `project.rs` text rewriting dissolves into the resolver that produces these.
- **Distinct passes**: lossless CST (names unresolved) → typed AST facades → resolve → bound HIR
  (names → symbols; member / default-member / property / `New` resolved) → lowering.
- **Error recovery**: the parser emits a partial CST with error nodes + a diagnostic list rather
  than bailing — required for both batch diagnostics and the IDE story.
- **Symbol table / scopes** modelling VBA scoping: procedure locals, parameters, module-level,
  project-level/`Public`, predeclared singletons, `With`-block targets, `Const`.
- **Incrementalism via `salsa`**: now **in scope** (interactive tooling is a goal), but sequenced
  as a later phase — the CST + HIR are built first, then wrapped in salsa queries.

## 6. Target architecture (green/red)

```
SourceFile ──lex──▶ [Token{kind, span, trivia}]
            ──parse──▶ green/red CST: untyped SyntaxKind nodes, lossless, immutable
                       └─ typed AST facades (ast::CallExpr, ast::MemberExpr, …) over SyntaxNode
            ──resolve──▶ bound HIR (Var→SymbolId; Member→{early-bound proc | late dispatch};
                          default members, property forms, New, WithEvents resolved) — arena IR
                       └─ SemanticModel overlay: lazy/cached symbol+type queries keyed by CST node
            ──typecheck──▶ diagnostics over the HIR / SemanticModel
            ──lower──▶ Bytecode + ProcedureRuntimeMetadata
                         (execution semantics and public metadata contract stable;
                          concrete bytecode shape may improve)
            ──(later) salsa──▶ memoized parse/resolve/typecheck queries → incremental recompute
```

Key invariant: the **execution semantics and public metadata contracts** are stable unless a
phase explicitly fixes a documented legacy divergence. The refactor changes everything upstream
of lowering and may produce different bytecode where that is a cleaner or more correct lowering.
The VM, JIT, host, and conformance suites are protected by semantic regression tests, metadata
contract checks, and targeted differential analysis rather than a byte-identical output rule. The
CST/HIR/SemanticModel/salsa are all upstream of the execution boundary; batch lowering needs the
CST + HIR only, so the IDE-oriented layers (full SemanticModel surface, salsa) can land after the
batch pipeline reaches parity.

### 6.1 Lowering-target maturity and current VM contract

A maturity audit of the VM bytecode *as a lowering target* (ahead of this rework) concluded:

- **The instruction set is a mature, appropriate target with no stringly shortcuts.** ~229 typed
  instructions on a register/slot machine; calls and jumps are resolved to instruction PCs at emit
  time (`CallProc.target_pc`, `Jump.target_pc` via `call_patches`/`proc_labels`) — no name-keyed
  runtime dispatch. Strings appear only where late binding genuinely needs them (late-bound COM /
  IDispatch member names, named-argument names, `TypeOf … Is Name`, external `Declare` metadata).
  The stringly intrinsics are a *front-end* artifact (`BoundExpr::IntrinsicCall { name }`, S4) that
  is resolved away during lowering into typed opcodes. Value model = refcounted IUnknown `Variant`;
  serialization = versioned `rkyv` (`OXVB`, `FORMAT_VERSION`). So the front-end can lower cleanly.

Post-review update, 2026-06-01: the activation-frame/object-lifetime dependency described in the
original audit has since landed through `bd-1ufc` and `bd-xkwq`. The VM now has activation-frame
slot overlays and explicit call/return transfer, and project-object ordinary fields live on the
runtime object. This front-end workset no longer needs to wait for the old "full A" model.

The remaining coordination point is contract fidelity: the new front-end/HIR lowering must target
the current descriptor-backed call convention, activation-frame slot overlay behavior, object
field storage, and return/writeback transfer rules. It should not resurrect assumptions from the
older flat-register lowering model.

## 7. Phased plan (each phase: gated, evidence-backed, independently shippable)

Planned, not yet created: a `frontend_v2` build/config gate will select the new pipeline. A
**differential and semantic harness** compiles and runs the full corpus (compiler unit fixtures +
`conformance/` + host integration projects) through both pipelines where both can handle the
construct. It records bytecode/metadata differences for review, but closes phases on correct
observable behavior, correct diagnostics, and stable public metadata contracts rather than
byte-identical bytecode.

- **Phase 0 — Foundations & decisions.**
  Capture the EBNF grammar (`docs/spec/VBA_GRAMMAR_V1`) + coverage matrix. Audit the existing
  `oxvba-syntax` lexer/parser/green/red/accessor surface against the target Roslyn-style shape
  and grammar matrix. Decide whether to keep the custom green/red tree or migrate to `rowan` /
  `cstree`; do not migrate merely for fashion. Lock the other library choices (interner,
  diagnostics, salsa shape) via small spikes. Build the planned frontend gate, semantic/diff
  harness, and a CST→legacy IR bridge so the existing lowering can be reused during transition.
  *Exit:* grammar + matrix committed; existing syntax substrate audited with gaps recorded; CST
  storage decision recorded; harness has an old-pipeline baseline and at least one v2 smoke route.

- **Phase 1 — Lexer.**
  Harden the existing hand-written tokenizer so it produces tokens with spans **and retained
  trivia** (whitespace, comments, line continuation) for the lossless CST; handle `:`, sigils,
  bracketed idents, `Rem`, date/hex/octal literals, case folding, and error tokens deliberately.
  Round-trip the corpus (CST text == source byte-for-byte).
  *Exit:* lexer tokenizes the accepted corpus losslessly; token snapshots, grammar-row fixtures,
  and known residual lexical gaps are recorded.

- **Phase 2 — Expression parser (Pratt) → green CST.**
  Harden the existing RD+Pratt parser and typed facades; a CST→legacy-expression bridge feeds
  existing lowering during transition. Compare against `parse_expr` over a large expression corpus,
  but allow cleaner bytecode when semantic behavior is proven. Add or verify the missing forms:
  `Is`, unified `Index`, `New`, bang/member access, and one postfix grammar covering call/index/
  member/bang (incl. `name.member`).
  *Exit:* expression semantic parity/improvement on the corpus; S1/S5/S6 addressed at the syntax
  level with gaps tracked explicitly.

- **Phase 3 — Statement parser → green CST.**
  Full statement grammar (Dim/Const, `Set`/`Let`/`Call`, `If`/`For`/`Do`/`While`/`Select`,
  `With`, `On Error`/`Resume`, `RaiseEvent`, `Property`, declarations, attributes) into the CST,
  with error-node recovery. Compare against current bound statements via the bridge, with semantic
  parity/improvement rather than bytecode identity as the gate.
  *Exit:* statement semantic parity/improvement; the CST fully represents the accepted corpus
  subset, with residual grammar gaps tracked explicitly.

- **Phase 4 — Resolver / binder + SemanticModel (the deep one).**
  Symbol table + scopes; produce the **bound HIR** from the typed CST, and the **SemanticModel
  overlay** (CST-node → symbol/type). Move member dispatch, default-member resolution, property
  Get/Let/Set selection, qualified-name resolution, `New`, WithEvents, and the F3c diagnostics
  **out of `project.rs` string rewriting** into the resolver. Lowering now consumes the HIR
  directly (retire the CST→`BoundExpr` bridge). This is where the member-access dual-path
  collapses and S2/S3 are resolved.
  *Exit:* resolver/HIR produces semantically equivalent or deliberately improved lowering for the
  corpus; `project.rs` rewriters begin retirement.

- **Phase 5 — Typed intrinsics & optimizer split.**
  Replace structural stringly-intrinsics (S4) with typed HIR nodes (null/`Nothing`, `New`,
  dynamic-dispatch, WithEvents ops, omitted-arg) — exhaustive enum matches instead of
  `match name.as_str()`. Move `AddConst`/`SubConst` (S6) into an optimizer pass over uniform
  binary ops.
  *Exit:* emit's magic-string dispatch shrinks to genuine library intrinsics; consumers match
  exhaustively.

- **Phase 6 — Flip & retire.**
  Make `frontend_v2` the default; remove the legacy `parse_expr` string-splitting and the
  retired `project.rs` rewriters; delete the member-access dual-path. Keep the semantic/diff
  harness as a regression guard for one release, then archive the old-pipeline comparison lane
  after the new pipeline is the only production route.
  *Exit:* single pipeline; legacy paths deleted; full suite + conformance green.

- **Phase 7 — Incrementality & tooling surface (`salsa`).**
  Wrap parse → resolve → typecheck in salsa queries for incremental recompute; expose the
  SemanticModel as a query API. Foundation for a language server / editor diagnostics / formatter
  (the lossless CST already enables exact-span refactors and round-trip formatting).
  *Exit:* incremental recompile on edits; a minimal semantic-query API; (full LSP is a separate
  workset).

## 8. Coexistence & migration strategy

- **Feature flag** (`frontend_v2`) gates the new pipeline end-to-end; the old pipeline stays the
  default until a phase proves parity. This flag/gate is planned work, not current repo state.
- **Semantic differential testing is the gate.** The same corpus compiled both ways should be
  compared at multiple levels: diagnostics, normalized metadata, bytecode summaries, execution
  traces, and observable outputs. Bytecode differences are triaged as (a) a bug in the new path,
  (b) harmless lowering/layout drift, or (c) an intentional improvement over a legacy divergence
  from Excel/MS-VBAL.
- **Per-construct flip.** Within a phase, route only the constructs at parity through v2 and fall
  back to v1 for the rest, so the flag can advance incrementally rather than big-bang.
- **Grammar-coverage matrix** is the running checklist of what v2 covers.

## 9. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Huge test surface (compiler + conformance + host integration) | Semantic/differential harness as the gate; per-construct flag flips; corpus rows for every migrated construct |
| `project.rs` rewriters encode subtle, hard-won semantics (default members, F3c diagnostics, COM early/late, WithEvents) | Treat each as a spec item with a fixture before moving it; never delete a rewriter until its resolver replacement passes its fixtures |
| VBA lexical quirks (case, line continuation, `:`, sigils) | Audit and harden the existing hand-written lexer; corpus round-trip tests and grammar-row fixtures before production routing |
| Existing `oxvba-syntax` and `oxvba-languageservice` semantics drift from compiler truth | Treat current syntax/language-service code as useful substrate, not authority; reconcile it through shared HIR/SemanticModel APIs before broad IDE claims |
| Custom green/red tree misses behavior helper libraries would have provided | Phase 0 explicitly audits node identity, span stability, sharing, memory behavior, traversal cost, and thread-safety needs before deciding to keep or replace it |
| Scope creep / long-lived branch | Phases ship independently behind the flag; each is mergeable; avoid a multi-month dark branch |
| Performance regression | Bench the new pipeline vs old on the corpus; interning + arenas should be ≥ parity |
| Output-contract drift breaking VM/JIT | Public metadata and execution semantics are the fixed contracts; bytecode/metadata diffs are normalized, explained, and backed by execution tests |

## 10. Decision log

Resolved:
- **D0 (2026-05-31): Syntactic layer = Roslyn-style green/red CST with typed facades** (not a
  lean AST), because interactive tooling/incremental is a goal. "Roslyn-style" names the
  architectural shape: lossless syntax, typed facades, syntax/semantics separation, bound HIR for
  lowering, and SemanticModel-style queries. It does not require Roslyn, rust-analyzer, `rowan`, or
  `cstree` as dependencies. Backbone (resolve → bound HIR → lower) unchanged.
- **D4: Syntax storage shape = green/red CST**; bound HIR + symbol tables = index arenas. The
  current custom `oxvba-syntax` green/red tree is the default substrate. `rowan`/`cstree` remain
  optional helper-library migration candidates, not goals by themselves; any migration must be
  justified by the Phase-0 substrate audit/spike.
- **D6: Two layers, not a reshaped `BoundExpr`** — typed CST facades for syntax; a *new* bound
  HIR for the resolved IR. `BoundExpr` is retired in favor of the HIR (with a temporary
  CST→`BoundExpr` bridge during Phases 2–3 to keep existing lowering).

Open (settle in Phase 0):
- D1: Lexer — keep and harden the current hand-written lexer (default) vs migrate to `logos`.
- D2: Parser — keep and harden the current hand-written RD+Pratt parser (default) vs migrate
  selected parsing/diagnostic pieces to `chumsky`.
- D3: Interner — `lasso` vs `string-interner` (or `cstree`'s built-in interning).
- D5: Diagnostics renderer — `ariadne` vs `codespan-reporting`.
- D7: Grammar source of truth — EBNF-from-MS-VBAL (default) with Rubberduck ANTLR cross-check.
- D8: CST storage implementation — keep custom `oxvba-syntax` tree (default) vs migrate to
  `rowan` or `cstree` if a Phase-0 spike proves concrete benefit.
- D9: Incremental engine — `salsa` version/shape (Phase 7); confirm it wraps the same queries.

## 11. Test & evidence strategy

- Grammar-coverage matrix (Phase 0) — one fixture per production.
- Lexer round-trip + token snapshots (Phase 1), including explicit residual rows for unsupported
  lexical forms.
- Expression/statement **semantic differential** parity vs the legacy parser plus intentional
  improvement tracking (Phases 2–3).
- Resolver equivalence: same observable behavior and correct diagnostics/metadata for the corpus
  (Phase 4), with bytecode differences reviewed but not automatically blocking.
- Full suites green at every phase: current `oxvba-compiler`, `oxvba-vm`, `oxvba-host`,
  `conformance/`, plus the Excel oracle where member/lifetime semantics move.
- Evidence docs under `docs/evidence/` per phase; final closure report.

## 12. Scope notes

In scope (per D0): lossless green/red CST (currently custom `oxvba-syntax`; possible `rowan` or
`cstree` migration only if justified), the SemanticModel overlay, and `salsa`-based incrementality
(Phase 7) — these are now goals, not deferrals.

Out of scope (unless a later workset expands):
- A full **language server / LSP** product surface (Phase 7 builds the foundation — incremental
  queries + semantic API — but the editor integration, completion, code actions, etc. are a
  separate effort).
- Back-end activation-frame or object-field lifetime implementation. Those prerequisites have
  landed separately; this workset only needs to respect their current contracts.
- VM/JIT behavior changes that are not required to execute the new front-end lowering correctly.

## 13. Hierarchical work plan for bead rollout

This section is the planning hierarchy used for the created bead graph.

Tracker root: `bd-aprs` — "Frontend compiler rework workset: Roslyn-style syntax, binder, HIR,
and SemanticModel".

Created epic mapping:

| Plan epic | Bead ID |
|---|---|
| FE-0 Workset preparation and truth repair | `bd-aprs.1` |
| FE-1 Grammar and coverage foundation | `bd-aprs.2` |
| FE-2 Syntax substrate audit and hardening | `bd-aprs.3` |
| FE-3 Lexer completion | `bd-aprs.4` |
| FE-4 Parser completion and CST-to-legacy bridge | `bd-aprs.5` |
| FE-5 Semantic harness and frontend gate | `bd-aprs.6` |
| FE-6 Binder, HIR, and SemanticModel core | `bd-aprs.7` |
| FE-7 Project semantics migration from `project.rs` | `bd-aprs.8` |
| FE-8 Typed intrinsics, optimizer split, and lowering cleanup | `bd-aprs.9` |
| FE-9 Flip, retirement, and IDE query foundation | `bd-aprs.10` |

Created child bead mapping:

| Plan bead | Bead ID |
|---|---|
| FE-0.1 Workset truth audit | `bd-aprs.1.1` |
| FE-0.2 Decision-record cleanup | `bd-aprs.1.2` |
| FE-0.3 Corpus inventory | `bd-aprs.1.3` |
| FE-0.4 Execution bead rollout refresh | `bd-aprs.1.4` |
| FE-1.1 MS-VBAL grammar capture | `bd-aprs.2.1` |
| FE-1.2 Rubberduck cross-check notes | `bd-aprs.2.2` |
| FE-1.3 Grammar coverage matrix | `bd-aprs.2.3` |
| FE-1.4 Fixture taxonomy | `bd-aprs.2.4` |
| FE-2.1 Green/red tree audit | `bd-aprs.3.1` |
| FE-2.2 Rowan/cstree library spike | `bd-aprs.3.2` |
| FE-2.3 Typed facade audit | `bd-aprs.3.3` |
| FE-2.4 Parser error recovery shape | `bd-aprs.3.4` |
| FE-3.1 Trivia and continuation semantics | `bd-aprs.4.1` |
| FE-3.2 Literal lexing completion | `bd-aprs.4.2` |
| FE-3.3 Identifier and keyword lexing | `bd-aprs.4.3` |
| FE-3.4 Lexer snapshot corpus | `bd-aprs.4.4` |
| FE-4.1 Expression parser semantic parity | `bd-aprs.5.1` |
| FE-4.2 Unified postfix grammar | `bd-aprs.5.2` |
| FE-4.3 Statement parser coverage | `bd-aprs.5.3` |
| FE-4.4 CST-to-legacy bridge | `bd-aprs.5.4` |
| FE-4.5 Parser diagnostic recovery fixtures | `bd-aprs.5.5` |
| FE-5.1 `frontend_v2` gate | `bd-aprs.6.1` |
| FE-5.2 Semantic/diff harness | `bd-aprs.6.2` |
| FE-5.3 Diff classifier | `bd-aprs.6.3` |
| FE-5.4 Corpus runner integration | `bd-aprs.6.4` |
| FE-6.1 Symbol identity model | `bd-aprs.7.1` |
| FE-6.2 Bound HIR arenas | `bd-aprs.7.2` |
| FE-6.3 SemanticModel query API | `bd-aprs.7.3` |
| FE-6.4 Type and coercion hooks | `bd-aprs.7.4` |
| FE-6.5 Diagnostic mapping | `bd-aprs.7.5` |
| FE-7.1 Qualified names and project/module lookup | `bd-aprs.8.1` |
| FE-7.2 Member dispatch classification | `bd-aprs.8.2` |
| FE-7.3 Property and assignment semantics | `bd-aprs.8.3` |
| FE-7.4 Class construction and fields | `bd-aprs.8.4` |
| FE-7.5 Events and Implements migration | `bd-aprs.8.5` |
| FE-7.6 External references binding | `bd-aprs.8.6` |
| FE-8.1 Typed structural intrinsic enum | `bd-aprs.9.1` |
| FE-8.2 Operator normalization optimizer split | `bd-aprs.9.2` |
| FE-8.3 HIR lowering contract cleanup | `bd-aprs.9.3` |
| FE-8.4 Metadata normalization for harness | `bd-aprs.9.4` |
| FE-9.1 Per-construct default flip | `bd-aprs.10.1` |
| FE-9.2 Legacy parser/rewriter retirement | `bd-aprs.10.2` |
| FE-9.3 Salsa/query integration | `bd-aprs.10.3` |
| FE-9.4 Language-service reconciliation | `bd-aprs.10.4` |
| FE-9.5 Terminal evidence and closure | `bd-aprs.10.5` |

### Epic FE-0 — Workset Preparation and Truth Repair

Outcome: the workset becomes executable without stale assumptions or hidden prerequisites.

Candidate bead units:
- FE-0.1 Workset truth audit: reconcile this plan with `oxvba-syntax`, `oxvba-languageservice`,
  current compiler lowering, activation-frame state, and project-object field state.
  Evidence: `docs/evidence/frontend_rework/FRONTEND_REWORK_TRUTH_AUDIT_2026-06-01.md`.
- FE-0.2 Decision-record cleanup: lock the meaning of "Roslyn-style" as a shape, and record
  `rowan`/`cstree` as optional helper migrations.
  Evidence: `docs/evidence/frontend_rework/FRONTEND_REWORK_DECISION_RECORD_2026-06-01.md`.
- FE-0.3 Corpus inventory: enumerate current compiler, host, conformance, language-service, and
  real-world fixture sources that will feed the semantic/diff harness.
  Evidence: `docs/evidence/frontend_rework/FRONTEND_REWORK_CORPUS_INVENTORY_2026-06-01.md`.
- FE-0.4 Bead rollout: create the actual bead tree from this hierarchy once the workset is
  accepted for execution.
  Evidence: `docs/evidence/frontend_rework/FRONTEND_REWORK_BEAD_ROLLOUT_2026-06-01.md`.

Evidence gate: workset text, architecture references, and corpus inventory agree; no execution
phase depends on an undocumented prerequisite.

### Epic FE-1 — Grammar and Coverage Foundation

Outcome: the target VBA grammar subset is explicit and measurable.

Candidate bead units:
- FE-1.1 MS-VBAL grammar capture: create `docs/spec/VBA_GRAMMAR_V1` with clean-room provenance
  and dialect notes.
  Artifact: `docs/spec/VBA_GRAMMAR_V1.md`.
- FE-1.2 Rubberduck cross-check notes: use Rubberduck as a quirk checklist without copying its
  grammar into product code.
  Evidence: `docs/evidence/frontend_rework/RUBBERDUCK_CROSSCHECK_2026-06-01.md`.
- FE-1.3 Grammar coverage matrix: create production-level rows with fixture anchors, parser
  status, binder status, execution status, and residual disposition.
  Evidence: `docs/evidence/frontend_rework/VBA_GRAMMAR_COVERAGE_MATRIX_2026-06-01.csv`.
  Summary: `docs/evidence/frontend_rework/VBA_GRAMMAR_COVERAGE_MATRIX_2026-06-01.md`.
- FE-1.4 Fixture taxonomy: split syntax-only fixtures, binder fixtures, execution fixtures,
  diagnostics fixtures, and Excel oracle fixtures.
  Evidence: `docs/evidence/frontend_rework/FRONTEND_REWORK_FIXTURE_TAXONOMY_2026-06-01.md`.

Evidence gate: every in-scope grammar production has an owned row or an explicit out-of-scope /
deferred reason.

### Epic FE-2 — Syntax Substrate Audit and Hardening

Outcome: `oxvba-syntax` is either confirmed as the syntax substrate or replaced by a justified
library-backed implementation.

Candidate bead units:
- FE-2.1 Green/red tree audit: verify losslessness, immutable sharing behavior, text ranges,
  token/node traversal, error-node representation, and stable handles needed by IDE queries.
  Evidence: `docs/evidence/frontend_rework/GREEN_RED_TREE_AUDIT_2026-06-01.md`.
- FE-2.2 Library spike: compare current custom tree against `rowan` and `cstree` for concrete
  gaps: node identity, interning, memory, threading, typed facade ergonomics, and maintenance cost.
  Evidence: `docs/evidence/frontend_rework/ROWAN_CSTREE_SPIKE_2026-06-01.md`.
- FE-2.3 Typed facade audit: define the minimal typed syntax API needed by parser tests, binder,
  SemanticModel, and formatting/refactoring.
  Evidence: `docs/evidence/frontend_rework/TYPED_FACADE_AUDIT_2026-06-01.md`.
- FE-2.4 Error recovery shape: standardize parser diagnostics and error nodes so incomplete
  source remains useful for IDE interactions.
  Evidence: `docs/evidence/frontend_rework/PARSER_ERROR_RECOVERY_SHAPE_2026-06-01.md`.

Evidence gate: a written keep-or-migrate decision exists, backed by tests or a spike, and the
chosen syntax substrate has an explicit hardening backlog.

### Epic FE-3 — Lexer Completion

Outcome: the lexer is lossless, span-stable, and broad enough for the accepted corpus.

Candidate bead units:
- FE-3.1 Trivia and continuation semantics: cover whitespace, comments, `Rem`, physical/logical
  line handling, and line continuation edge cases.
  Evidence: `docs/evidence/frontend_rework/LEXER_TRIVIA_CONTINUATION_2026-06-01.md`.
- FE-3.2 Literal lexing: cover strings, date literals, numeric suffixes, hex/octal forms,
  currency/decimal-relevant forms, and malformed literal recovery.
  Evidence: `docs/evidence/frontend_rework/LEXER_LITERAL_COMPLETION_2026-06-01.md`.
- FE-3.3 Identifier and keyword lexing: cover bracketed identifiers, type suffixes, case folding,
  contextual keywords, and host/library names that collide with keywords.
  Evidence: `docs/evidence/frontend_rework/LEXER_IDENTIFIER_KEYWORD_COMPLETION_2026-06-01.md`.
- FE-3.4 Lexer snapshot corpus: add token snapshots and round-trip tests across the grammar
  matrix and existing project fixtures.
  Evidence: `docs/evidence/frontend_rework/LEXER_SNAPSHOT_CORPUS_2026-06-01.md`.

Evidence gate: accepted corpus tokenizes losslessly, lexical residuals are matrixed, and any future
lexer diagnostics API must carry stable spans. FE-3 itself does not require a separate lexer
diagnostics surface.

### Epic FE-4 — Parser Completion and CST-to-Legacy Bridge

Outcome: the CST parser covers accepted expressions/statements and can feed existing lowering
through a temporary bridge.

Candidate bead units:
- FE-4.1 Expression parser parity: harden precedence, associativity, unary/binary distinction,
  `Is`, `Like`, `TypeOf ... Is`, and parenthesized expression behavior.
  Evidence: `docs/evidence/frontend_rework/PARSER_EXPRESSION_SEMANTIC_PARITY_2026-06-01.md`.
- FE-4.2 Postfix grammar: unify call, index, member, bang, default-member syntax, and statement
  call forms.
  Evidence: `docs/evidence/frontend_rework/PARSER_POSTFIX_GRAMMAR_2026-06-01.md`.
- FE-4.3 Statement parser coverage: harden declarations, blocks, inline statements, `With`,
  `On Error`/`Resume`, `RaiseEvent`, `Property`, `Declare`, `Type`, `Enum`, attributes, and
  statement separators.
  Evidence: `docs/evidence/frontend_rework/PARSER_STATEMENT_COVERAGE_2026-06-01.md`.
- FE-4.4 CST-to-legacy bridge: lower selected CST nodes into current `BoundExpr`/`BoundStmt`
  forms so syntax migration can proceed before HIR is complete.
  Evidence: `docs/evidence/frontend_rework/CST_TO_LEGACY_BRIDGE_2026-06-01.md`.
- FE-4.5 Parser diagnostic recovery: verify partial trees under common incomplete edit states.
  Evidence: `docs/evidence/frontend_rework/PARSER_DIAGNOSTIC_RECOVERY_2026-06-01.md`.

Evidence gate: parser fixtures round-trip, bridge-supported constructs compile/run through the
old lowering path, and unsupported constructs have clear fallback or residual rows.

### Epic FE-5 — Semantic Harness and Frontend Gate

Outcome: the new pipeline can be enabled per construct and compared safely against the existing
compiler.

Candidate bead units:
- FE-5.1 `frontend_v2` gate: introduce a config/feature/runtime switch with no default behavior
  change.
  Evidence: `docs/evidence/frontend_rework/FRONTEND_V2_GATE_2026-06-01.md`.
- FE-5.2 Semantic/diff harness: compare diagnostics, normalized metadata, bytecode summaries,
  execution traces, and observable outputs.
  Evidence: `docs/evidence/frontend_rework/SEMANTIC_DIFF_HARNESS_2026-06-01.md`.
- FE-5.3 Diff classifier: record bytecode differences as bug, harmless drift, or intentional
  improvement, with fixture links and close conditions.
  Evidence: `docs/evidence/frontend_rework/DIFF_CLASSIFIER_2026-06-01.md`.
- FE-5.4 Corpus runner integration: run compiler unit fixtures, host projects, conformance cases,
  and targeted Excel oracle-backed cases through the harness.
  Evidence: `docs/evidence/frontend_rework/CORPUS_RUNNER_2026-06-01.md`.

Evidence gate: the harness can prove old-vs-old stability, v2 smoke behavior, and meaningful
classification of at least one intentional non-byte-identical lowering.

### Epic FE-6 — Binder, HIR, and SemanticModel Core

Outcome: syntax is resolved into a compiler-owned bound HIR and an IDE-facing SemanticModel
without putting binding data into the CST.

Candidate bead units:
- FE-6.1 Symbol identity model: define `SymbolId`, scopes, module/project/library namespaces,
  case-insensitive interning, and source-span provenance.
  Evidence: `docs/evidence/frontend_rework/SYMBOL_IDENTITY_MODEL_2026-06-01.md`.
- FE-6.2 Bound HIR arenas: define expression, statement, declaration, call, member, property,
  and type nodes with CST backpointers.
  Evidence: `docs/evidence/frontend_rework/BOUND_HIR_ARENAS_2026-06-01.md`.
- FE-6.3 SemanticModel query API: expose symbol/type/diagnostic queries keyed by CST nodes and
  byte spans, reusing HIR facts rather than duplicating compiler semantics.
  Evidence: `docs/evidence/frontend_rework/SEMANTIC_MODEL_QUERY_API_2026-06-01.md`.
- FE-6.4 Type and coercion hooks: connect HIR to the current declared type model, call-site
  descriptors, Let/Set distinction, Optional/ParamArray, ByRef/ByVal, and default values.
  Evidence: `docs/evidence/frontend_rework/TYPE_COERCION_HOOKS_2026-06-01.md`.
- FE-6.5 Diagnostic mapping: route parser, binder, and type diagnostics to stable source spans
  with existing diagnostic family compatibility where applicable.
  Evidence: `docs/evidence/frontend_rework/DIAGNOSTIC_MAPPING_2026-06-01.md`.

Evidence gate: selected constructs bind through HIR, answer SemanticModel queries, and lower/run
with behavior matching or improving the legacy path.

### Epic FE-7 — Project Semantics Migration from `project.rs`

Outcome: source-text rewriting is retired construct by construct and replaced by resolver/HIR
semantics.

Candidate bead units:
- FE-7.1 Qualified names and project/module lookup: move module, class, procedure, field, and
  public symbol resolution into binder-owned tables.
  Evidence: `docs/evidence/frontend_rework/QUALIFIED_NAME_PROJECT_LOOKUP_2026-06-01.md`.
- FE-7.2 Member dispatch classification: resolve early-bound project members, imported COM
  members, late-bound dispatch, default members, and host-provided globals.
- FE-7.3 Property and assignment semantics: resolve Property Get/Let/Set, default member read/
  write/invoke, Let vs Set coercion, and object/scalar assignment diagnostics.
- FE-7.4 Class construction and fields: resolve `New`, `As New`, predeclared instances,
  ordinary fields, WithEvents fields, and runtime object-field metadata.
- FE-7.5 Events and Implements: migrate WithEvents, RaiseEvent, handler matching, Implements,
  and related diagnostics out of string rewriting.
- FE-7.6 External references: bind typelib/project/native references through descriptor-backed
  symbols without dependency-specific routing.

Evidence gate: each migrated construct has before/after fixtures, semantic diff classification,
and deletion or quarantine of the corresponding text rewrite.

### Epic FE-8 — Typed Intrinsics, Optimizer Split, and Lowering Cleanup

Outcome: structural compiler concepts are typed HIR/lowering operations, not magic string
intrinsics or parser-shaped optimizations.

Candidate bead units:
- FE-8.1 Intrinsic enum: replace structural `IntrinsicCall { name }` paths for `Nothing`, `Null`,
  omitted arguments, project instances, WithEvents operations, dynamic dispatch, and pointer
  helpers where appropriate.
- FE-8.2 Operator normalization: replace parser-produced `AddConst`/`SubConst` with uniform
  binary ops and a separate optimizer transform.
- FE-8.3 Lowering contract cleanup: lower HIR into current bytecode/call-site metadata without
  relying on legacy name strings or flat-slot assumptions.
- FE-8.4 Metadata normalization: define stable comparison projections for procedure metadata,
  descriptors, source maps, and diagnostics.

Evidence gate: emit magic-string matches shrink to genuine library/runtime intrinsics, and
lowering remains behavior-correct across compiler/host/conformance suites.

### Epic FE-9 — Flip, Retirement, and IDE Query Foundation

Outcome: the new compiler front-end becomes the production path and leaves behind an IDE-capable
semantic substrate.

Candidate bead units:
- FE-9.1 Per-construct default flip: route completed construct families through frontend v2 by
  default while retaining fallback only for tracked residuals.
- FE-9.2 Legacy parser/rewriter retirement: delete or quarantine legacy `parse_expr` string
  splitting and retired `project.rs` rewrite paths once their matrix rows are covered.
- FE-9.3 Salsa/query integration: wrap parse, bind, typecheck, diagnostics, and SemanticModel
  queries for incremental recompute.
- FE-9.4 Language-service reconciliation: replace duplicate `oxvba-languageservice` semantic
  logic with shared SemanticModel/HIR-backed queries.
- FE-9.5 Terminal evidence and closure: run full compiler, VM, host, conformance, syntax, and
  selected Excel oracle checks; archive the legacy comparison harness when no longer needed.

Evidence gate: frontend v2 is the single production compiler route for the scoped language
surface, interactive semantic queries use the same facts as compilation, and residual scope is
explicitly owned by follow-up worksets or beads.
