# Workset: Front-End Refactor — Tokenizer, Parser, Resolved AST, and Binder

Date: 2026-05-31
Owner: DNA Kode
Status: reopened / production front-end replacement incomplete (2026-06-02)

Reopen note, 2026-06-01:

The original workset wording already pointed at the intended outcome: replace the production
compiler's string-splitting and string-rewriting front-end end to end with a Roslyn-style
lexer/parser/binder/HIR/SemanticModel pipeline. The first execution run produced useful syntax,
HIR, SemanticModel, route-policy, metadata, and terminal-test evidence, but it closed the workset
on scaffolding plus documented residuals. That was not sufficient for this workset's intended
terminal gate. Passing terminal tests with `resolve.rs` and `project.rs` still load-bearing is not
closure; it is a foundation checkpoint.

Rework note, 2026-06-02:

Fresh review confirms the workset was already meant to cover the full production compiler
front-end replacement. The desired outcome is not a second workset and not a new independent bead
set. It is the same workset, with sharper terminal requirements and a repaired bead graph:
completed scoped slices remain valid evidence, but every area that still relies on legacy
front-end behavior must stay open or have a new child bead that finishes the accepted production
replacement. Bounded fixture passes are checkpoints only. They cannot close the workset while
accepted compiler surfaces still depend on legacy `parse_expr` string parsing, CST-to-legacy
lowering, `project.rs` source rewriting, duplicate language-service semantics, or fallback
eligibility for constructs that are inside the intended production surface.

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

During migration, the current resolve → bound-IR → lower backbone may be used as a compatibility
bridge. At closure, it must no longer be the production front-end route for the scoped language
surface. The target outcome is production source text flowing through the new syntax, binder, HIR,
SemanticModel, and HIR lowering contracts by default, with retired legacy paths removed or
quarantined outside production. (Lean-AST/rustc shape was the alternative; see §5.5 and the
decision log §10.)

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

This workset is the **production compiler front-end migration**, not merely a plan or scaffold.
Git history shows `oxvba-syntax` was scaffolded in the initial workspace bootstrap (`68965e4e`,
2026-02-26), then substantially expanded for language-service work (`5f4da2f3`, 2026-03-23:
Pratt expression parser, typed accessors, provider trait). The workset therefore starts from a
partial syntax/IDE substrate that must be wired into the production compiler pipeline before this
workset can close.

Non-closure examples:

- adding a `frontend_v2` flag that only validates CST and then calls the legacy compiler;
- adding HIR/SemanticModel data structures without routing production binding through them;
- documenting `project.rs` text rewrites as residuals while they remain production behavior;
- passing terminal tests while `parse_expr` / source-rewrite paths are still the default route;
- claiming "per-construct flip" without executable proof that the flipped construct no longer
  uses the legacy production path.

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

### 6.2 Production replacement terminal gate

This gate is binding for workset closure. The workset is not complete until all of these are true:

1. The default production compile path for the scoped language surface is:

   ```
   source → oxvba-syntax CST → binder → bound HIR → type/coercion facts
          → HIR lowering → bytecode + ProcedureRuntimeMetadata
   ```

2. `frontend_v2` is no longer merely a CST-validation bridge. If a switch remains, it selects
   compatibility behavior; the new pipeline is the default for scoped constructs.
3. `resolve::parse_expr_for_syntax_bridge`, legacy `parse_expr` substring splitting, and
   `project.rs` source-text rewrite bridge paths are removed from production execution for the
   scoped constructs, or are behind explicit compatibility/test-only gates.
4. Every reopened bead area has executable proof that its production call path reaches the new
   front-end implementation, not just a data structure or evidence document.
5. The language-service query path answers symbol/type/diagnostic questions from the same
   compiler-owned SemanticModel/HIR facts used by production compilation.
6. Full compiler, syntax, VM, host, conformance, and selected Excel oracle lanes pass. Bytecode may
   differ from the old compiler, but each difference must be classified as equivalent,
   improvement, or defect.
7. Residuals may exist only outside the claimed scoped language surface. A residual inside the
   claimed surface keeps the relevant bead and workset open.
8. Every accepted compiler entry point has route proof, not only the explicit frontend-v2 helper:
   lightweight single-source compile, project compile, host/session compile, language-service
   semantic queries, and comparison harnesses must either use the new front-end facts or be
   explicitly marked as legacy comparison/test-only paths.
9. Fallback is treated as a migration mechanism, not a semantic owner. If a construct is inside the
   intended production surface, "fallback-eligible" means the owning bead is still open until HIR
   binding/lowering and route evidence land.
10. Project/class/COM/default-member/property semantics must be bound as front-end semantics, not
    by generating helper-source strings that are then parsed by the old resolver. A compatibility
    helper may remain only when the replacement route is already production-owned and the helper is
    behind an explicit comparison or out-of-scope boundary.
11. Terminal route-audit evidence must cover the accepted grammar matrix and corpus lanes, not only
    a hand-picked smoke set. A passing bounded audit row may close that row; it does not close the
    production replacement workset.

### 6.3 Lowering-target maturity and current VM contract

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

Closure evidence must include **route proof**. A test that passes through the old compiler path
does not prove a front-end migration bead. Each migration bead must show the production call path
that enters the new parser/binder/HIR/lowering surface and must show the corresponding legacy
production path deleted, disabled, or explicitly quarantined.

## 11.1 Reopen audit after first execution run

The first execution run produced useful artifacts but closed too much. This audit governs the
second run.

| Area | Beads | Current disposition |
|---|---|---|
| Workset truth, decisions, corpus inventory | `bd-aprs.1.*` | Keep closed. These were preparation/support outcomes and remain useful. |
| Grammar foundation | `bd-aprs.2.1`, `bd-aprs.2.2`, `bd-aprs.2.4` | Keep closed as foundation. |
| Grammar coverage matrix | `bd-aprs.2.3` | Reopen. The matrix must become a production migration gate, not only a scaffold. |
| Syntax substrate audit and narrow hardening | `bd-aprs.3.*` | Keep closed unless later route proof exposes syntax gaps. |
| Lexer corpus proof | `bd-aprs.4.4` | Reopen. Token snapshots must cover the accepted production corpus, not just focused syntax fixtures. |
| Parser and CST bridge | `bd-aprs.5.*` | Reopen. Prior work validated syntax/bridge pieces, but did not make the parser the production compiler parser. |
| Frontend gate and diff harness | `bd-aprs.6.*` | Reopen. Prior gate was opt-in bridge/scaffold; the new gate must prove production routing and classify real v2 output. |
| Binder, HIR, SemanticModel | `bd-aprs.7.*` | Reopen. Prior work created surfaces; production binding must now consume them. |
| Project semantics migration | `bd-aprs.8.*` | Reopen. `project.rs` rewrite paths remain load-bearing. |
| Typed intrinsics and lowering cleanup | `bd-aprs.9.*` | Reopen. Some typed surfaces exist, but production HIR lowering and retirement are incomplete. |
| Flip, retirement, IDE query foundation | `bd-aprs.10.*` | Reopen. Terminal closure was premature while lowering/rewrite residuals remained production paths. |

Partial work from the first run should be reused aggressively. Reopened beads should start by
auditing what is already present, then turn scaffold/evidence into production behavior with route
proof.

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
| FE-6.6 Production binder integration | `bd-aprs.7.6` |
| FE-6.7 Bare object `Is` identity binding/lowering | `bd-aprs.7.7` |
| FE-7.1 Qualified names and project/module lookup | `bd-aprs.8.1` |
| FE-7.2 Member dispatch classification | `bd-aprs.8.2` |
| FE-7.3 Property and assignment semantics | `bd-aprs.8.3` |
| FE-7.4 Class construction and fields | `bd-aprs.8.4` |
| FE-7.5 Events and Implements migration | `bd-aprs.8.5` |
| FE-7.6 External references binding | `bd-aprs.8.6` |
| FE-7.3.a Property/default-member production semantics | `bd-aprs.8.7` |
| FE-7.6.a Reference/COM activation and member binding | `bd-aprs.8.8` |
| FE-8.1 Typed structural intrinsic enum | `bd-aprs.9.1` |
| FE-8.2 Operator normalization optimizer split | `bd-aprs.9.2` |
| FE-8.3 HIR lowering contract cleanup | `bd-aprs.9.3` |
| FE-8.4 Metadata normalization for harness | `bd-aprs.9.4` |
| FE-8.5 Production HIR-to-bytecode lowering | `bd-aprs.9.5` |
| FE-8.5.a Direct project construction on HIR | `bd-aprs.9.6` |
| FE-8.5.b As New initializer construction metadata | `bd-aprs.9.7` |
| FE-8.5.d Arrays/indexing/ReDim parity | `bd-aprs.9.8` |
| FE-8.5.e Compile-time options/declarations/constants | `bd-aprs.9.9` |
| FE-8.5.f Broader declaration/type surface | `bd-aprs.9.10` |
| FE-9.1 Per-construct default flip | `bd-aprs.10.1` |
| FE-9.2 Legacy parser/rewriter retirement | `bd-aprs.10.2` |
| FE-9.3 Salsa/query integration | `bd-aprs.10.3` |
| FE-9.4 Language-service reconciliation | `bd-aprs.10.4` |
| FE-9.5 Terminal evidence and closure | `bd-aprs.10.5` |
| FE-9.6 Production legacy-route audit gate | `bd-aprs.10.6` |
| FE-9.7 Broad matrix/corpus route audit | `bd-aprs.10.7` |
| FE-9.8 Legacy route retirement finalization | `bd-aprs.10.8` |

### 13.1 Reworked continuation graph after partial first run

The first run produced substantial valid slices but left the intended end goal unfinished. The bead
graph is therefore repaired in place:

| Work area | Existing evidence state | Required bead state after rework |
|---|---|---|
| FE-0 through FE-3 preparation, grammar, syntax substrate, and lexer foundation | Adequately completed as support/foundation work. | Keep closed unless later execution finds a concrete defect. These beads do not by themselves imply production replacement. |
| FE-4 parser/CST bridge | Parser and bridge evidence exists, but terminal closure cannot depend on CST validation before legacy lowering. | Keep completed parser slices closed; reopen/create only if accepted grammar rows still fail to parse into CST for HIR/binder consumption. |
| FE-5 harness and route gate | Harness exists and can classify non-byte-identical output. | Keep closed as support/delivery foundation, but FE-9 terminal audit must expand corpus coverage and fail if "v2" means fallback. |
| FE-6 binder/HIR/SemanticModel | Core structures and selected production facts exist. | Keep scoped closed slices, but any missing SymbolId/type/coercion facts discovered during FE-7/FE-8 delivery reopen the owning FE-6 bead or spawn a focused child. |
| FE-7 project semantics | Active-project slices landed, but project/class/default-member/COM/property behavior is not fully retired from text rewrites. | Reopen FE-7 epic and affected child beads for direct production replacement or explicit compatibility quarantine. Partial work must be noted as already done. |
| FE-8 production HIR lowering | Many statement/expression families are HIR-routed. `bd-aprs.9.5` remains open and is too broad to be the only executable unit. | Add child delivery beads under FE-8/FE-8.5 for remaining concrete HIR-lowering lanes: direct project construction, properties/default members, arrays/ReDim/indexing, compile-time options/constants, declarations/attributes, and final broad matrix sweep. |
| FE-9 flip/retirement/IDE | Default route and audit scaffolding exist; bounded audit fixtures pass. | Reopen or add terminal retirement beads so no accepted route closes on bounded smoke evidence. Terminal closure waits for broad matrix/corpus route proof. |

Required newly explicit delivery beads:

- FE-8.5.a Direct project construction on HIR: consume already materialized
  `HirNewExpressionBinding` facts in project compilation so `New <Class>` and generated object
  handles no longer travel as `__oxvba_project_instance(...)` helper source. Partial work already
  done: HIR `New` shape, construction binding payload, compile entry point, and project boundary
  fact materialization.
- FE-8.5.b `As New`, `Class_Initialize`, and construction source maps: extend the direct-HIR
  project construction route to lazy `As New`, initializer invocation, object lifetime metadata,
  and correct source-map accounting. Partial work already done: active-project construction
  analysis and downstream WithEvents direct-source workaround.
- FE-7.3/FE-8.5.c Property/default-member semantics: bind property Get/Let/Set/default-member
  selection and writeback through front-end facts for project/class/COM/host members. Partial work
  already done: simple late-bound dot/bang/With member reads and simple member assignment targets
  lower through HIR with Let/Set hints.
- FE-8.5.d Arrays/indexing/ReDim parity: finish array element read/write, fixed-array `ReDim`
  alias materialization, lower-bound `To` forms, multidimensional arrays, and project/class array
  fields through HIR. Partial work already done: one-dimensional dynamic-array `ReDim` runtime
  route and `Option Base` default-route policy.
- FE-8.5.e Compile-time declarations and module options: implement HIR-owned `Option Explicit`,
  `Option Compare Text/Database`, `Option Private Module`, DefType, attributes, conditional
  compilation/compile constants, and richer constant evaluation. Partial work already done:
  `Option Base 0/1`, `Option Compare Binary`, simple constants, enum constants, and same-statement
  constant expression substitution.
- FE-7.6/FE-8.5.f Reference/imported COM construction and member binding: route imported
  typelib/reference-project activation, early-bound COM member/property calls, and reference
  precedence through descriptor-backed front-end symbols. Partial work already done: reference kind
  indexing, imported/member dispatch classification, and basic Declare PtrSafe external call
  lowering.
- FE-9.7 Broad matrix/corpus route audit: extend the route audit from selected fixtures to the
  accepted grammar matrix, compiler fixture corpus, host project corpus, language-service corpus,
  and selected Excel oracle lanes. This bead must reopen the owning delivery bead for every
  accepted in-scope row that still reaches legacy fallback.
- FE-9.8 Legacy route retirement finalization: after delivery beads pass, delete or hard-quarantine
  legacy `parse_expr`, CST-to-legacy lowering, and `project.rs` helper-source rewrites from
  production entry points. Keep only explicitly named comparison/test-only helpers.

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
deferred reason. Reopened gate: the matrix must additionally record whether each production is
parsed by the production CST parser, bound through HIR/SemanticModel, lowered through the v2 route,
and covered by execution/diagnostic evidence. A row marked complete cannot rely only on legacy
compiler acceptance.

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
diagnostics surface. Reopened gate for FE-3.4: the snapshot corpus must include the production
migration corpus used by FE-5/FE-9, not only focused syntax examples.

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

Reopened production gate: FE-4 cannot close on CST validation plus legacy lowering alone. It must
prove that the accepted expression and statement subset used by production compilation is parsed
from `oxvba-syntax` and handed to binder/HIR facts. The CST-to-legacy bridge may remain only as a
temporary compatibility aid and must not be the terminal production route.

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

Reopened production gate: FE-5 must prove that v2 execution is not just CST validation before
legacy compilation. The harness must record the active route for each fixture and fail a "v2"
classification if the fixture enters `compile(source)` / legacy rewrite as its production path.

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
- FE-6.6 Production binder integration: wire scoped production compile paths so declarations,
  expressions, statements, calls, names, scopes, types, diagnostics, and coercions are bound from
  compiler-owned `SymbolId`/HIR/SemanticModel facts instead of legacy string recovery.
  Evidence: `docs/evidence/frontend_rework/PRODUCTION_BINDER_INTEGRATION_2026-06-01.md`.
- FE-6.7 Bare object `Is` identity binding/lowering: bind and lower `a Is b` and `a Is Nothing`
  as object identity through production front-end facts. This bead was added during the reopened
  FE-4.1 review after rejecting the incorrect shortcut of lowering bare `Is` as equality.
  Evidence: `docs/evidence/frontend_rework/OBJECT_IS_IDENTITY_BINDING_LOWERING_2026-06-01.md`.

Evidence gate: selected constructs bind through HIR, answer SemanticModel queries, and lower/run
with behavior matching or improving the legacy path.

Reopened production gate: FE-6 must replace scaffold-only facts with production binder facts.
Closure requires real compiler call paths to resolve names, scopes, types, calls, diagnostics, and
coercions through `SymbolId`/HIR/SemanticModel rather than reconstructing them later from strings.

### Epic FE-7 — Project Semantics Migration from `project.rs`

Outcome: source-text rewriting is retired construct by construct and replaced by resolver/HIR
semantics.

Status: reopened for production replacement completion. FE-7.1 through FE-7.6 contain useful
partial route proof and explicit compatibility classifications, but FE-7 cannot remain closed
while accepted project/class/default-member/property/COM semantics are still implemented by
source-text lowering internals or helper-source rewrites. FE-8 owns bytecode emission from HIR
facts; FE-7 owns the front-end semantic facts that make those emissions production-owned.

Candidate bead units:
- FE-7.1 Qualified names and project/module lookup: move module, class, procedure, field, and
  public symbol resolution into binder-owned tables. Partial work has already been done: a
  manifest-backed `ProjectSymbolIndex` exists and covers module/class/procedure/field/public
  routes, including `VB_Name` and `Option Private Module` handling. Continuation progress wires the
  index into production
  qualified procedure invocation lookup and active-project unqualified public procedure lookup
  while preserving ambiguity/reference-precedence behavior. Field/class member use is explicitly
  handed to FE-7.2, FE-7.3, and FE-7.4, which must consume this index rather than re-parsing text.
  Status: scoped delivery bead closed after qualified/unqualified active-project route proof and
  explicit handoff of member/property/class semantics to the downstream FE-7 beads.
  Evidence: `docs/evidence/frontend_rework/QUALIFIED_NAME_PROJECT_LOOKUP_2026-06-01.md`.
- FE-7.2 Member dispatch classification: resolve early-bound project members, imported COM
  members, late-bound dispatch, default members, and host-provided globals. Current continuation
  progress wires production classifier use for early-bound project procedure dispatch and imported
  COM member dispatch. Remaining executable categories are explicitly handed to FE-7.3, FE-7.4,
  FE-7.6, and FE-8.1, which must consume the same dispatch vocabulary rather than string-only
  decisions.
  Status: scoped delivery bead closed for project-procedure and imported-COM dispatch
  classification, with later categories explicitly owned by narrower beads.
  Evidence: `docs/evidence/frontend_rework/MEMBER_DISPATCH_CLASSIFICATION_2026-06-01.md`.
- FE-7.3 Property and assignment semantics: resolve Property Get/Let/Set, default member read/
  write/invoke, Let vs Set coercion, and object/scalar assignment diagnostics.
  Status: reopened continuation required. Partial work has already been done: active-project
  property/default-member route proof, typed-HIR assignment diagnostics, and simple dot/bang/With
  member assignment lowering with property Let/Set hints. Closure now requires project/class/COM/
  host property Get/Let/Set/default-member selection and writeback to be bound through front-end
  facts, with the corresponding text rewrite path deleted, compatibility-quarantined, or outside
  scope.
  Evidence: `docs/evidence/frontend_rework/PROPERTY_ASSIGNMENT_SEMANTICS_2026-06-01.md`.
- FE-7.4 Class construction and fields: resolve `New`, `As New`, predeclared instances,
  ordinary fields, WithEvents fields, and runtime object-field metadata. Current continuation
  progress wires active-project `New`/`As New` class construction through `ProjectSymbolIndex`
  class routes, gates active-project predeclared Property Get rewrite routes through frontend
  class/property routes, splits ordinary fields from `WithEvents` fields in the frontend project
  symbol table, emits active-project dynamic object field-token metadata from frontend
  ordinary-field routes, and classifies active-project typed class locals through the same
  frontend class route. Remaining fallback routes are explicitly bounded to referenced-project
  class/predeclared roots, imported COM activation metadata, and parser-incomplete compatibility
  enumeration outside the active-project symbol-index route.
  Status: reopened continuation required. Partial work has already been done: active-project class
  construction route analysis, typed class locals, predeclared Property Get roots, ordinary field
  metadata from frontend routes, WithEvents field separation, HIR `New` shape, construction
  binding facts, and a HIR compile entry point that accepts those facts. Closure now requires
  project compilation to consume those facts directly for active-project construction, `As New`,
  `Class_Initialize`, field/source-map metadata, and imported/COM activation or explicit
  compatibility quarantine.
  Evidence: `docs/evidence/frontend_rework/CLASS_CONSTRUCTION_FIELDS_2026-06-01.md`.
- FE-7.5 Events and Implements: migrate WithEvents, RaiseEvent, handler matching, Implements,
  and related diagnostics out of string rewriting. Current continuation progress makes
  active-project WithEvents source binding consume frontend class routes, makes event dispatch
  planning consume frontend `Event` and handler procedure routes before emitting guard wrappers,
  and makes active-project Implements lookup/coverage consume frontend class/procedure routes.
  Active-project `RaiseEvent` declared-event validation also consumes frontend event routes.
  Remaining fallback is explicitly bounded to reference/imported event and Implements sources plus
  text-shaped statement lowering glue.
  Status: scoped delivery bead closed for active-project event/Implements route decisions, with
  referenced/imported event sources classified to FE-7.6/reference composition.
  Evidence: `docs/evidence/frontend_rework/EVENTS_IMPLEMENTS_SEMANTICS_2026-06-01.md`.
- FE-7.6 External references: bind typelib/project/native references through descriptor-backed
  symbols without dependency-specific routing. Current continuation progress moves reference-kind
  authority into `frontend_external_references::ExternalReferenceIndex` for imported typelib
  qualification, reference-project ordering, and host-injected/native implicit receiver
  classification. Existing typelib metadata helpers remain as compatibility lowering after the
  frontend route accepts the declared reference.
  Status: scoped delivery bead closed after imported/reference/host route proof and compatibility
  classification for post-acceptance metadata materialisation.
  Evidence: `docs/evidence/frontend_rework/EXTERNAL_REFERENCES_BINDING_2026-06-01.md`.

Evidence gate: each migrated construct has before/after fixtures, semantic diff classification,
and deletion or quarantine of the corresponding text rewrite.

Reopened production gate: FE-7 remains open while any corresponding `project.rs` text rewrite or
source-text lowering internal is the production semantic owner for project/class/COM/default-member/
property/host behavior in the accepted surface. Evidence docs are not enough; closure requires
route proof plus deletion, production quarantine, or explicit out-of-scope classification of each
retired rewrite.

### Epic FE-8 — Typed Intrinsics, Optimizer Split, and Lowering Cleanup

Outcome: structural compiler concepts are typed HIR/lowering operations, not magic string
intrinsics or parser-shaped optimizations.

Candidate bead units:
- FE-8.1 Intrinsic enum: replace structural `IntrinsicCall { name }` paths for `Nothing`, `Null`,
  omitted arguments, project instances, WithEvents operations, dynamic dispatch, and pointer
  helpers where appropriate. Current continuation progress introduces typed
  `BoundExpr::StructuralIntrinsicCall` production nodes and migrates `Null`, `Nothing`, and
  omitted positional arguments through resolver, typecheck, optimization walks, emit, and metadata
  collection. Follow-up continuation also migrates project-instance materialisation and pointer
  helpers (`VarPtr`/`StrPtr`/`ObjPtr`), including external pointer writeback classification, plus
  WithEvents runtime helpers and DispatchInvoke/EarlyInvoke helper calls. Dynamic get/let/set
  placeholder enum variants were removed because no production helper names exist for them in this
  codebase.
  Evidence: `docs/evidence/frontend_rework/TYPED_STRUCTURAL_INTRINSICS_2026-06-01.md`.
- FE-8.2 Operator normalization: replace parser-produced `AddConst`/`SubConst` with uniform
  binary ops and a separate optimizer transform. Reopened continuation removed the direct
  `parse_expr` fast-path branch for simple `var + const` / `var - const` source forms and moved the
  fast-path conversion into recursive optimizer expression normalization, so parser/resolver shape is
  uniform while existing backend `AddConstI32`/`SubConstI32` bytecode support remains available after
  optimization.
  Evidence: `docs/evidence/frontend_rework/OPERATOR_NORMALIZATION_2026-06-01.md`.
- FE-8.3 Lowering contract cleanup: lower HIR into current bytecode/call-site metadata without
  relying on legacy name strings or flat-slot assumptions. Reopened continuation makes the contract
  executable in the normal compile route: typed HIR lowering contracts are derived from source and
  emitted `ProcedureRuntimeMetadata` is validated for procedure presence, symbol-backed frame slots,
  return slots, and scoped coercion overlays. Known current HIR modifier-token residue is quarantined
  rather than projected into runtime slots.
  Evidence: `docs/evidence/frontend_rework/HIR_LOWERING_CONTRACT_2026-06-01.md`.
- FE-8.4 Metadata normalization: define stable comparison projections for procedure metadata,
  descriptors, source maps, and diagnostics. Reopened continuation promotes the projection into
  executable harness output: frontend diff reports now include field-level semantic metadata drift
  paths, and the classifier carries those paths into reasons instead of reporting only an opaque
  metadata-summary mismatch.
  Evidence: `docs/evidence/frontend_rework/METADATA_NORMALIZATION_2026-06-01.md`.
- FE-8.5 Production HIR lowering: implement production bytecode and `ProcedureRuntimeMetadata`
  emission from bound HIR facts for the scoped language surface. Initial reopened continuation adds
  a scoped HIR production route for procedure/local/parameter declarations and assignment/expression
  bodies, tried before the CST/legacy bridge in frontend-v2 compilation. Unsupported constructs are
  rejected before HIR lowering and remain on the tracked fallback path rather than being silently
  partially lowered. Second reopened continuation adds simple same-module procedure call statement
  lowering and explicit `ByVal` / `ByRef` parameter mechanism projection. The call/coercion seed row
  now matches bytecode and call metadata; its remaining source-map metadata delta is classified as a
  deliberate HIR improvement. Follow-up call continuation preserves bare argument lists for
  same-module statement-form procedure calls without parentheses and adds an expression-statement
  discard path so statement-form member calls with bare arguments can lower through the existing
  late-bound member dispatch bytecode. Third reopened continuation adds simple multiline `If ... Then ...
  End If` HIR shape and production lowering through branch bytecode. Fourth reopened continuation
  adds front-checked `Do While ... Loop` HIR shape and production lowering through loop branch and
  backedge bytecode. Fifth reopened continuation adds parser expression nodes, HIR shape, and
  production lowering for simple single-value `Select Case` clauses. Partial work has already been
  done. Sixth reopened continuation widens `Do` loop support to `Do Until` and post-check
  `Loop While`/`Loop Until`. Seventh reopened continuation maps `While`/`Wend` onto the same
  front-checked loop HIR/backend shape. Eighth reopened continuation adds simple range `For` loops.
  Ninth reopened continuation gives `Select Case` typed value/range clauses and lowers integer
  ranges. This bead remains open for broader HIR lowering coverage outside that simple same-module
  call, simple If, conditional loop, simple For, and value/range/multi-value/`Case Is`
  Select Case subset. Twelfth reopened continuation adds simple `For Each` iterable loops, so the
  control-flow fixtures currently in the production legacy-route audit now classify as HIR
  production. Thirteenth reopened continuation wires the ordinary lightweight
  `compile()` / `compile_with_runtime_metadata()` path to try HIR production first for eligible
  sources, while retaining unsupported constructs on the tracked legacy fallback path and keeping an
  explicit legacy helper for differential comparison. Fresh-eyes correction in that slice narrowed
  eligibility so DefType, functions/properties, optional/default/ParamArray parameters, project
  rewrites, and class/object-local compatibility contexts stay on the residual path until HIR owns
  their semantics. Fourteenth reopened continuation adds multiline `ElseIf` lowering as nested HIR
  branch trees and records `If/Else` plus `If/ElseIf/Else` route-audit coverage. Fifteenth reopened
  continuation adds typed HIR and production lowering for `Exit Do`, `Exit For`, and procedure exit
  statements. Sixteenth reopened continuation changes the syntax parser to preserve single-line
  `If ... Then ... Else ...` bodies as inline blocks and routes that HIR shape through production
  lowering. Seventeenth reopened continuation adds typed HIR and production lowering for non-label
  `On Error Resume Next`, `On Error GoTo 0`, `Resume Next`, and bare `Resume`. Eighteenth reopened
  continuation adds syntax/HIR/lowering support for identifier and numeric labels plus `GoTo`.
  Nineteenth reopened continuation adds `GoSub` and `Return` lowering over the same label model.
  Twentieth reopened continuation adds label-targeted `On Error GoTo` and `Resume` lowering.
  Twenty-first reopened continuation adds a typed HIR route for `Erase` statements.
  Twenty-second reopened continuation adds declared function return-type projection into HIR,
  function return-slot metadata, function type-suffix parsing, and basic object assignment
  diagnostics on the HIR production route, allowing simple functions to leave the residual path.
  Twenty-third reopened continuation adds basic positional-argument `RaiseEvent` statement
  lowering to the existing backend event statement form. Fresh-eyes correction on that continuation
  makes event argument expressions visible to the compiler-owned SemanticModel and HIR lowering
  contract instead of only to bytecode emission. Follow-up continuation accepts module-level
  `Event` declarations as symbol/fact declarations on the HIR route when paired with `RaiseEvent`;
  event signature validation, WithEvents handler matching, Implements coupling, and project event
  binding remain broader FE-7/FE-8 event work. Follow-up continuation accepts the existing
  single-source `Implements` directive shape as a no-bytecode directive on the HIR route; project/
  class Implements validation and interface member matching remain broader FE-7/FE-8 work.
  Twenty-fourth reopened continuation adds simple literal `Const` substitution through HIR
  production lowering without allocating runtime slots for those constants. Follow-up continuation
  widens that subset to comma-separated literal `Const` declarators. Follow-up continuation accepts
  simple constant expressions such as `Const CBase = 1 + 2` by lowering them to bound expression
  trees and still keeping constants out of runtime local slots. Follow-up continuation allows later
  declarators in the same `Const` statement to reference earlier declarators, while full
  module/procedure-scoped constant evaluation remains broader FE-8.5 work.
  Twenty-fifth reopened continuation adds one-dimensional dynamic-array runtime `ReDim` /
  `ReDim Preserve` lowering from CST-preserved bound expressions through HIR and runtime array
  metadata. Follow-up default-route correction allows `Option Base 0`, `Option Base 1`, and
  default-equivalent `Option Compare Binary` on otherwise completed lightweight HIR sources, while
  leaving `Option Explicit`, `Option Compare Text`/`Database`, and `Option Private Module` outside
  the default route until their semantics are owned by HIR.
  Twenty-sixth reopened continuation adds explicit-receiver value-side dot-member read/call
  expressions through HIR member facts and the existing backend late-bound member expression shape.
  Follow-up continuation accepts read-side bang member access such as `obj!Field` through the same
  HIR member expression route and backend late-bound dispatch shape. Follow-up continuation adds
  simple explicit-receiver and `With` shorthand member assignment targets by emitting late-bound
  dispatch with explicit property Let/Set hints, including dot and bang member selectors. Object
  construction, default-member/property selection, project/class binding, COM binding,
  indexed/named writeback breadth, and type overload validation remain residual work.
  Follow-up continuation adds read-side `With obj ... .Member ... End With` lowering by binding
  dot-prefixed member reads to the active With receiver.
  Follow-up continuation adds module-level `Enum` blocks to the production HIR route by declaring
  enum members as module constants, substituting their integer values during HIR-to-bound lowering,
  and projecting enum descriptors into runtime metadata. The older enum bytecode test was adjusted
  away from byte-identical peephole expectations because HIR production may emit a different but
  equivalent add shape. Follow-up continuation adds `Declare PtrSafe` external function/sub
  declarations and calls to the HIR production route by projecting the existing external
  declaration descriptors into the lowered module and preserving `IntrinsicInvokeSymbolHost`
  bytecode emission. Unsupported declare signatures still return HIR `Unsupported` and remain
  fallback-eligible. Follow-up continuation adds simple module-level `Type` block layout projection
  for local UDT variables: HIR production now emits UDT descriptors and flattened field slots for
  declarations such as `Dim p As Point`. Follow-up UDT continuation maps simple field reads/writes
  such as `p.X = 1` and `y = p.X + 2` to flattened aliases, and preserves same-shape whole-UDT
  assignment as field-wise `UdtAssign` copy lowering.
  Follow-up continuation gives `New` a first-class HIR expression shape carrying the normalized
  constructor type name, and moves the residual from a raw CST syntax guard to a precise
  project-aware construction-binding error. Follow-up continuation adds an explicit HIR lowering
  construction-binding hook: supplied `HirNewExpressionBinding` facts lower `New <Class>` to typed
  `StructuralIntrinsic::ProjectInstance(handle)` instead of a helper-name rewrite. `New` remains an
  explicit production residual until project compilation consumes those facts for active-project
  class handles, imported/COM activation, generated instance metadata, `Class_Initialize`, and
  `As New` lazy-construction semantics without relying on `project.rs` source-text rewrites.
  Follow-up continuation adds a compile-to-bytecode HIR entry point that accepts those
  `HirNewExpressionBinding` facts and emits project-object reference bytecode, so the remaining
  project compile work can call HIR directly instead of duplicating lowering internals.
  Follow-up project-boundary continuation routes single active procedural-module projects with no
  reference projects through the HIR-capable metadata compiler, preserving legacy routing for
  multi-module, class/document, forced-object-local, and reference-project shapes.
  Follow-up project continuation now records normalized constructor type names in dynamic instance
  drafts and materializes `HirNewExpressionBinding` facts in source order at the project compile
  boundary; the direct `Set obj = New Class` gap is now closed by `bd-aprs.9.6`, while broader
  construction semantics remain split to the next construction/reference beads. Downstream cleanup
  has also removed the exact source-class public-field read
  regression (`c.Total`) and the direct WithEvents `Set field = New ActiveProjectClass` parser
  failure, but those fixes are compatibility-route reductions rather than direct-HIR project
  construction closure. `bd-aprs.9.6` continuation now consumes generated construction facts for
  the accepted direct active-project `Set obj = New Widget` shape by reconstructing the generated
  helper assignment as a HIR `New` expression and compiling it through
  `compile_source_with_runtime_metadata_via_hir_with_new_bindings`; `As New`,
  `Class_Initialize`, construction source maps, broader WithEvents construction, and imported/COM
  construction remain open under `bd-aprs.9.7` / `bd-aprs.8.8`.
  FE-8.5 remains open for unaudited broader language surfaces outside that subset.
  Evidence: `docs/evidence/frontend_rework/PRODUCTION_HIR_LOWERING_2026-06-01.md`.
- FE-8.5.a Direct project construction on HIR: finish the already-started `New <Class>` migration
  by making project compilation call the HIR compile entry point with the generated
  `HirNewExpressionBinding` facts instead of compiling rewritten `__oxvba_project_instance(...)`
  helper source. Partial work has already been done: HIR `New` expression shape, construction
  binding facts, source-order materialization at the project boundary, and a direct HIR compile
  entry point that emits project-object reference bytecode.
  Status: delivery complete for the accepted direct active-project `Set obj = New Class` shape.
  Remaining construction semantics are split to FE-8.5.b and FE-7.6.a.
- FE-8.5.b `As New`, initializer, and construction metadata: extend direct-HIR project
  construction to `Dim x As New T`, `Class_Initialize`, object lifetime/source-map metadata, and
  WithEvents construction interactions. Partial work has already been done: active-project
  construction analysis and the targeted WithEvents direct-source workaround. Continuation progress
  now derives a separate HIR construction candidate for active-project `Dim x As New T`: fallback
  source remains eager/legacy-compatible, while the accepted HIR route removes declaration-time
  helper construction and inserts guarded first-use/after-`Nothing` `New` sites, including
  field-mutating `Class_Initialize` bodies and source-map/dynamic-route checks. Source-class
  `WithEvents Set x = New T` also now restores its generated temporary construction to HIR `New`
  and preserves generated optional/default guard parameters. The accepted active-project reset
  regression now also checks private `Class_Terminate` retention, dynamic route termination
  metadata, and first-use/after-`Nothing` source maps.
  Status: delivery complete for the scoped accepted active-project `As New`/initializer/source-map/
  lifetime and source-class WithEvents construction lane. Imported/reference/COM activation remains
  owned by FE-7.6.a / `bd-aprs.8.8`; unsupported fallback shapes remain compatibility fallback
  until the broad route audit; broader event semantics remain under FE-7/FE-9 coverage.
- FE-8.5.c Property/default-member/writeback lowering: finish the semantic and lowering route for
  Property Get/Let/Set, default member read/write/invoke, early-bound COM property put/putref,
  indexed/named writeback, and type overload validation. Partial work has already been done: simple
  late-bound member reads/calls and simple dot/bang/With member assignment targets lower through
  HIR with Let/Set hints. Continuation progress now makes imported-COM dispatch classification
  carry typelib invocation kind and validates early-bound COM property read plus put/putref rewrite
  paths against that front-end dispatch fact before emitting the compatibility `DispatchInvoke`
  carrier; selected host-injected property/default-member routes now validate through the
  front-end `HostGlobal` dispatch classification before retaining their compatibility PMR rewrite
  carrier; statement-form named arguments now survive HIR and HIR production lowering into
  call-site argument binding metadata, including explicit no-paren `Call Proc name := value` and
  parenthesized `Call Proc(name := value)`; late-bound variable default-member calls such as
  `obj(42)` now lower through HIR into default-member dispatch metadata; late-bound variable
  indexed default-member assignments now lower through `BoundStmt::AssignDefaultMember`, preserve
  indexed argument names, and emit dispatch member id `0` with `PropertyLet`/`PropertySet` hints
  plus `LateBoundDefaultMember`/`SyntheticPropertyAssignment` call-site metadata with the
  synthetic `value` argument; multiple authoritative `VB_UserMemId = 0` candidates of the required
  accessor kind now reject as default-member ambiguity instead of selecting the first sorted
  candidate; selected active-project default-member accessors now validate source argument count
  before rewrite; selected active-project property/default-member rewrite routes now validate
  `EarlyBoundProject` member-dispatch classification before retaining the compatibility carrier.
- FE-8.5.d Arrays, indexing, and `ReDim` parity: finish array element read/write, fixed-array
  `ReDim` alias materialization, explicit lower-bound `To` forms, multidimensional arrays, and
  project/class array fields through HIR. Partial work has already been done: one-dimensional
  dynamic-array `ReDim` runtime lowering, array shape metadata, and `Option Base` default-route
  policy.
- FE-8.5.e Compile-time options/declarations/constants: route `Option Explicit`,
  non-binary `Option Compare`, `Option Private Module`, DefType, attributes, conditional
  compilation, typed constants, and broader compile-time constant evaluation through HIR. Partial
  work has already been done: `Option Base`, `Option Compare Binary`, enum constants, and simple
  same-statement constant expressions.
- FE-8.5.f Broader declaration and type surface: finish `Property` procedure declarations,
  optional/default/ParamArray parameters, richer `Declare` signatures, UDT nested/array/fixed-string
  fields, and corresponding diagnostics/metadata through HIR. Partial work has already been done:
  simple functions with return slots, `Declare PtrSafe` calls, simple UDT layout/field aliases, and
  same-shape UDT assignment.

Evidence gate: emit magic-string matches shrink to genuine library/runtime intrinsics, and
lowering remains behavior-correct across compiler/host/conformance suites.

Reopened production gate: FE-8 must implement real HIR-to-bytecode lowering for the scoped
constructs, not only define a lowering contract. Legacy bound-expression lowering may be used for
comparison, but closure requires production bytecode and metadata to be emitted from HIR facts.

### Epic FE-9 — Flip, Retirement, and IDE Query Foundation

Outcome: the new compiler front-end becomes the production path and leaves behind an IDE-capable
semantic substrate.

Candidate bead units:
- FE-9.1 Per-construct default flip: route completed construct families through frontend v2 by
  default while retaining fallback only for tracked residuals. Reopened continuation flips
  `CompileOptions::default()` to use the frontend-v2 route for completed constructs. Follow-up
  continuation also flips the ordinary lightweight single-source `compile()` /
  `compile_with_runtime_metadata()` route to try HIR production before legacy resolution for
  eligible completed constructs. Follow-up continuation removes simple functions from the
  residual set after declared return-slot facts are projected through HIR. The eligibility guard
  deliberately still excludes surfaces whose HIR semantics are partial, including DefType,
  properties, optional/default/ParamArray parameters, project rewrites, and class/object-local
  compatibility contexts. The legacy baseline remains available through an explicit comparison
  helper, and fallback is preserved only for unsupported residual constructs.
  Evidence: `docs/evidence/frontend_rework/PER_CONSTRUCT_ROUTE_POLICY_2026-06-01.md`.
- FE-9.2 Legacy parser/rewriter retirement: delete or quarantine legacy `parse_expr` string
  splitting and retired `project.rs` rewrite paths once their matrix rows are covered.
  Evidence: `docs/evidence/frontend_rework/LEGACY_RETIREMENT_INVENTORY_2026-06-01.md`.
- FE-9.3 Salsa/query integration: wrap parse, bind, typecheck, diagnostics, and SemanticModel
  queries for incremental recompute.
  Evidence: `docs/evidence/frontend_rework/QUERY_INTEGRATION_2026-06-01.md`.
- FE-9.4 Language-service reconciliation: replace duplicate `oxvba-languageservice` semantic
  logic with shared SemanticModel/HIR-backed queries. Reopened continuation removes the remaining
  legacy `BoundModule` fallback from `SemanticSnapshot`; unsupported front-end syntax now surfaces
  front-end diagnostics instead of a second semantic model.
  Evidence: `docs/evidence/frontend_rework/LANGUAGE_SERVICE_RECONCILIATION_2026-06-01.md`.
- FE-9.5 Terminal evidence and closure: run full compiler, VM, host, conformance, syntax, and
  selected Excel oracle checks; archive the legacy comparison harness when no longer needed.
  Evidence: `docs/evidence/frontend_rework/TERMINAL_CLOSURE_2026-06-01.md`.
- FE-9.6 Production legacy-route audit gate: prove before terminal closure that no scoped
  production compile path still depends on legacy `parse_expr`/string-splitting, `project.rs`
  source-text rewrite behavior, or duplicate language-service semantic fallbacks. Reopened
  continuation now passes the recorded audit fixtures/static checks and includes a direct check that
  completed lightweight compile fixtures use the HIR runtime-metadata route. Follow-up continuation
  expands the audit with statement-form procedure/member calls with bare arguments, multiline `If/ElseIf`,
  single-line If, basic `Exit`, and non-label
  error-control fixtures plus identifier/numeric-label `GoTo`, `GoSub`/`Return`, and
  label-targeted error-control fixtures, plus `Erase`, simple function declaration coverage, and
  basic `RaiseEvent`, `Event` declaration, single-source `Implements`, and literal `Const`. The workset remains open for broader terminal evidence
  and expanded route-audit coverage. Follow-up continuation also covers a one-dimensional
  dynamic-array runtime `ReDim` fixture, an explicit-receiver value-side dot-member read/call
  fixture, and a read-side `With` member fixture.
  Evidence: `docs/evidence/frontend_rework/PRODUCTION_LEGACY_ROUTE_AUDIT_2026-06-01.md`.
- FE-9.7 Broad matrix/corpus route audit: expand FE-9.6 from selected route fixtures to the
  accepted grammar matrix, compiler fixture corpus, host project corpus, language-service corpus,
  and selected Excel oracle lanes. Partial work has already been done: the bounded route audit,
  retirement inventory, corpus inventory, and diff classifier exist. Closure requires every
  accepted in-scope row to classify as HIR/SemanticModel production or to reopen/create the owning
  delivery bead.
- FE-9.8 Legacy route retirement finalization: after FE-7/FE-8 delivery beads pass, remove or
  hard-quarantine legacy production entry points for `parse_expr`, CST-to-legacy lowering,
  `project.rs` helper-source rewrites, and duplicate language-service semantic fallbacks. Partial
  work has already been done: selected structural intrinsics moved to typed forms, bounded route
  fixtures bypass legacy parsing, and the old project rewrite-bridge selector is no longer the
  unconditional production strategy. Closure requires code search and route proof that remaining
  legacy helpers are comparison/test-only or outside the accepted surface.

Evidence gate: frontend v2 is the single production compiler route for the scoped language
surface, interactive semantic queries use the same facts as compilation, and residual scope is
explicitly owned by follow-up worksets or beads.

Reopened production gate: FE-9 terminal closure must search the codebase and runtime evidence for
remaining production use of legacy parser/rewriter routes in the accepted surface. If any remain,
the relevant FE-4 through FE-8 bead stays open or a focused follow-up delivery bead is created
before terminal closure. Full test pass, bounded route-audit pass, or residual notes are not
sufficient.
