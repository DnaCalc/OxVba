# Workset: Front-End Refactor — Tokenizer, Parser, Resolved AST, and Binder

Date: 2026-05-31
Owner: DNA Kode
Status: proposed

Architecture decision, 2026-05-31:

The syntactic layer will be a **Roslyn-style green/red concrete syntax tree** (lossless,
immutable) with **typed AST facades**, as used by `rust-analyzer` via the `rowan`/`cstree`
crates — chosen because interactive tooling (language-server diagnostics, formatting,
refactoring, incremental recompile) is a goal, not only batch compile-to-bytecode. The
resolve → bound-IR → lower backbone is unchanged; this decision sets the *syntax layer* and
adds a semantic-overlay + incremental capability. (Lean-AST/rustc shape was the alternative;
see §5.5 and the decision log §10.)

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

This workset is a **plan**, not a started migration. It defines the target architecture, the
library/pattern decisions, the formal-grammar question, and a phased, behavior-preserving,
test-gated route. No front-end behavior changes until a phase ships behind differential tests.

## 2. Correctness authority (unchanged repo convention)

1. Actual VBA running in Excel on Windows.
2. Published specifications — primarily **[MS-VBAL] VBA Language Specification** for the
   grammar, plus COM/Automation/ABI specs.
3. Existing OxVba behavior as a **regression anchor only** (a baseline to diff against, not a
   source of truth). If a string rewriter encodes a divergence from Excel/MS-VBAL, the new
   pipeline must not inherit it.

The new pipeline must produce **byte-identical bytecode + runtime metadata** to the current
pipeline for every passing corpus program, *except* where the current output is a known
divergence (tracked explicitly). That equivalence is the migration's gate.

## 3. Motivation — the shortcut inventory (grounded in current code)

| # | Shortcut (today) | Where | Traditional shape |
|---|---|---|---|
| S1 | No lexer for the main grammar; precedence by substring scanning | `parse_expr`, `split_at_lowest_precedence_op`, `split_compare_keyword_top_level`; tell-tale patch `parse_typed_suffix_literal` (disambiguates `100&` from `x & y`) | lexer → token stream → Pratt parser |
| S2 | Names are strings; AST is **not resolved** | `BoundExpr::Var(String)`, `ProcCall { name: String }`, `Member { member: String }`; resolution recovered later via `slot_map` (emit) + `project.rs` | symbol table; AST carries `SymbolId` |
| S3 | String-rewriting front-end (macro-by-text) | `project.rs`: member dispatch, default members, property Get/Let/Set, qualified names, `New`, WithEvents, collections, F3c diagnostics | resolve in the binder against the symbol table |
| S4 | Stringly-typed intrinsics as an escape hatch (~25 magic names) | `IntrinsicCall { name }` (`__empty`, `__null`, `__nothing`, `__oxvba_project_instance`, `__oxvba_withevents_*`, `dispatchinvoke`, `__omitted`, `vbnullstring`, …); giant `match name.as_str()` in `emit.rs` | dedicated AST/IR nodes (or a typed `enum Intrinsic`) for structural concepts |
| S5 | Under-modeled operators / postfix | `Is` (object identity) unsupported as a binary op (only `TypeOf x Is T`); indexing is the `__oxvba_array_get` intrinsic, not a uniform `Index`; `New`, bang `obj!field` are string-rewritten | unified postfix grammar: call / index / member / bang; `CompareOp::Is` |
| S6 | Peephole optimization baked into AST shape | `BoundExpr::AddConst`/`SubConst` produced directly by the parser, special-cased in every consumer | uniform `BinaryOp`; recognize in an optimizer pass |

Precedent already in-repo: a real tokenizer+parser exists for `#If` preprocessor expressions
(`tokenize_pp_expr` → `PpToken`, `resolve.rs:1323`), and `BoundExpr::Member` (added 2026-05-31,
commit `f7cb6b85`) is the first expression-level node that began collapsing S5 for call-result
receivers. This workset generalizes that direction.

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

Each is a decision to lock during Phase 0 via a small spike. Defaults below are recommendations.

### 5.1 Lexer
- **Recommended: hand-written lexer.** VBA's lexical quirks (case-insensitive keywords, `_`
  line continuation joining physical lines, `:` statement separators, type sigils `%&!#@$`,
  `[bracketed]` identifiers, `'` and `Rem` comments, `#…#` date literals, `&H`/`&O` literals,
  `""` string escaping) are easier to get exactly right by hand, and we already have the
  `tokenize_pp_expr` precedent.
- Alternative: **`logos`** (derive-based, very fast) with callbacks. Viable; revisit if lexer
  perf matters or the quirks fit cleanly.

### 5.2 Parser
- **Recommended: hand-written recursive descent + Pratt (precedence climbing)** for
  expressions. Maximum control, best diagnostics, easiest to integrate.
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
- **Syntax = green/red CST via `rowan`** (the `rust-analyzer` library) — untyped `SyntaxKind`
  nodes with widths/offsets (green), lazy parent/position-aware facades (red), lossless (all
  trivia retained), immutable with structural sharing. **Typed AST = thin accessor wrappers**
  over `SyntaxNode` (e.g. `ast::BinExpr`, `ast::CallExpr`, `ast::MemberExpr`), generated or
  hand-written — *not* a separate owned `enum` tree. The hand-written parser (§5.2) stays
  hand-written; it drives a `GreenNodeBuilder` (`start_node`/`token`/`finish_node`) emitting the
  green tree, exactly as rust-analyzer's hand-written parser does.
  - Alternative library: **`cstree`** (a rowan-compatible fork with built-in token interning and
    `Send`/threading support) — evaluate in Phase 0; pick `cstree` if cross-thread CSTs or memory
    from large projects matter, else `rowan`.
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
            ──parse──▶ green/red CST (rowan): untyped SyntaxKind nodes, lossless, immutable
                       └─ typed AST facades (ast::CallExpr, ast::MemberExpr, …) over SyntaxNode
            ──resolve──▶ bound HIR (Var→SymbolId; Member→{early-bound proc | late dispatch};
                          default members, property forms, New, WithEvents resolved) — arena IR
                       └─ SemanticModel overlay: lazy/cached symbol+type queries keyed by CST node
            ──typecheck──▶ diagnostics over the HIR / SemanticModel
            ──lower──▶ Bytecode + ProcedureRuntimeMetadata   (UNCHANGED output contract)
            ──(later) salsa──▶ memoized parse/resolve/typecheck queries → incremental recompute
```

Key invariant: the **lowering output** (bytecode + metadata + descriptors) is the stable
contract. The refactor changes everything *upstream* of lowering while keeping the bytecode
identical, so the VM, JIT, host, and the 861 + conformance suites are unaffected by construction
until we deliberately change semantics. The CST/HIR/SemanticModel/salsa are all upstream of that
boundary; batch lowering needs the CST + HIR only, so the IDE-oriented layers (full SemanticModel
surface, salsa) can land after the batch pipeline reaches parity.

### 6.1 Lowering-target maturity (bytecode audit, 2026-05-31) & the activation-frame dependency

A maturity audit of the VM bytecode *as a lowering target* (ahead of this rework) concluded:

- **The instruction set is a mature, appropriate target with no stringly shortcuts.** ~229 typed
  instructions on a register/slot machine; calls and jumps are resolved to instruction PCs at emit
  time (`CallProc.target_pc`, `Jump.target_pc` via `call_patches`/`proc_labels`) — no name-keyed
  runtime dispatch. Strings appear only where late binding genuinely needs them (late-bound COM /
  IDispatch member names, named-argument names, `TypeOf … Is Name`, external `Declare` metadata).
  The stringly intrinsics are a *front-end* artifact (`BoundExpr::IntrinsicCall { name }`, S4) that
  is resolved away during lowering into typed opcodes. Value model = refcounted IUnknown `Variant`;
  serialization = versioned `rkyv` (`OXVB`, `FORMAT_VERSION`). So the front-end can lower cleanly.
- **One genuine immaturity: the slot / activation model.** The register file is a single **flat,
  global** `Vec<RuntimeSlot>`; `call_stack` saves only `(return_pc, error_frame)` — there are **no
  per-call activation frames / slot windows**. Verified consequence: **recursion is broken** —
  `Fact(5)` returns `16`, not `120`, because a recursive callee reuses the caller's slots
  (regression probe: `recursion_preserves_caller_locals`, `#[ignore]`d). This is the **same root
  cause** as the object-slot-lifetime gaps (cascade through object fields, the `gMT` trailing-`T`,
  Me-param / return-slot retention): nothing pops a frame, so object references linger.

**Dependency on the activation-frame model (the full "A").** This front-end rework lowers to the
bytecode; the instruction **format** is stable, but the slot **semantics** are slated to change
from absolute-global to **frame-relative** when per-call activation frames land. When that happens,
emit must produce frame-relative slot indices and the VM maintains a frame base. The activation-
frame + object-lifetime work is tracked as a **separate back-end workstream/bead ("A")**, not in
this front-end workset. Coordination point: prefer landing (or co-designing) the activation-frame
model **before or alongside** the resolver/lowering phases (Phase 4+), since it changes the slot
contract emit produces; Phases 0–3 (grammar, lexer, CST parser) are independent of it.

## 7. Phased plan (each phase: behind a flag, differential-tested, independently shippable)

A `frontend_v2` build/config flag selects the new pipeline. A **differential harness** compiles
the full corpus (compiler unit fixtures + `conformance/` + host integration projects) through
both pipelines and asserts identical bytecode/metadata; divergences are triaged (bug vs known).

- **Phase 0 — Foundations & decisions.**
  Capture the EBNF grammar (`docs/spec/VBA_GRAMMAR_V1`) + coverage matrix. Choose `rowan` vs
  `cstree` and lock the other library choices (lexer, interner, diagnostics, salsa) via small
  spikes — including a spike that hand-writes a `GreenNodeBuilder`-driven parser for a tiny slice
  to validate the typed-facade ergonomics. Build the differential harness + a CST→`BoundExpr`
  bridge (so the new CST can feed the *existing* lowering during transition).
  *Exit:* grammar + matrix committed; rowan/cstree chosen; harness compiles the corpus both ways
  (old==old baseline); decisions recorded in §10.

- **Phase 1 — Lexer.**
  Hand-written tokenizer producing tokens with spans **and retained trivia** (whitespace,
  comments, line continuation) for the lossless CST; handle `:`, sigils, bracketed idents,
  date/hex/octal literals, case folding. Round-trip the corpus (CST text == source byte-for-byte).
  *Exit:* lexer tokenizes the entire corpus losslessly; token snapshot + round-trip tests.

- **Phase 2 — Expression parser (Pratt) → green CST.**
  Hand-written RD+Pratt parser drives a `GreenNodeBuilder` to emit the expression CST + typed
  facades; a CST→`BoundExpr` bridge feeds existing lowering. Differential-test against `parse_expr`
  over a large expression corpus. Add the missing forms: `Is`, unified `Index`, `New`, and one
  postfix grammar covering call/index/member/bang (incl. `name.member`).
  *Exit:* expression differential parity on the corpus; S1/S5/S6 addressed at the syntax level.

- **Phase 3 — Statement parser → green CST.**
  Full statement grammar (Dim/Const, `Set`/`Let`/`Call`, `If`/`For`/`Do`/`While`/`Select`,
  `With`, `On Error`/`Resume`, `RaiseEvent`, `Property`, declarations, attributes) into the CST,
  with error-node recovery. Differential against current bound statements via the bridge.
  *Exit:* statement differential parity; the CST fully represents the corpus (still behind the flag).

- **Phase 4 — Resolver / binder + SemanticModel (the deep one).**
  Symbol table + scopes; produce the **bound HIR** from the typed CST, and the **SemanticModel
  overlay** (CST-node → symbol/type). Move member dispatch, default-member resolution, property
  Get/Let/Set selection, qualified-name resolution, `New`, WithEvents, and the F3c diagnostics
  **out of `project.rs` string rewriting** into the resolver. Lowering now consumes the HIR
  directly (retire the CST→`BoundExpr` bridge). This is where the member-access dual-path
  collapses and S2/S3 are resolved.
  *Exit:* resolver/HIR produces equivalent lowering for the corpus; `project.rs` rewriters begin
  retirement.

- **Phase 5 — Typed intrinsics & optimizer split.**
  Replace structural stringly-intrinsics (S4) with typed HIR nodes (null/`Nothing`, `New`,
  dynamic-dispatch, WithEvents ops, omitted-arg) — exhaustive enum matches instead of
  `match name.as_str()`. Move `AddConst`/`SubConst` (S6) into an optimizer pass over uniform
  binary ops.
  *Exit:* emit's magic-string dispatch shrinks to genuine library intrinsics; consumers match
  exhaustively.

- **Phase 6 — Flip & retire.**
  Make `frontend_v2` the default; remove the legacy `parse_expr` string-splitting and the
  retired `project.rs` rewriters; delete the member-access dual-path. Keep the differential
  harness as a regression guard for one release, then archive.
  *Exit:* single pipeline; legacy paths deleted; full suite + conformance green.

- **Phase 7 — Incrementality & tooling surface (`salsa`).**
  Wrap parse → resolve → typecheck in salsa queries for incremental recompute; expose the
  SemanticModel as a query API. Foundation for a language server / editor diagnostics / formatter
  (the lossless CST already enables exact-span refactors and round-trip formatting).
  *Exit:* incremental recompile on edits; a minimal semantic-query API; (full LSP is a separate
  workset).

## 8. Coexistence & migration strategy

- **Feature flag** (`frontend_v2`) gates the new pipeline end-to-end; the old pipeline stays the
  default until a phase proves parity.
- **Differential testing is the gate.** The same corpus compiled both ways must yield identical
  `Bytecode` + `ProcedureRuntimeMetadata` (+ descriptors). Any diff is triaged as (a) a bug in
  the new path → fix, or (b) a known legacy divergence from Excel/MS-VBAL → record and allow.
- **Per-construct flip.** Within a phase, route only the constructs at parity through v2 and fall
  back to v1 for the rest, so the flag can advance incrementally rather than big-bang.
- **Grammar-coverage matrix** is the running checklist of what v2 covers.

## 9. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Huge test surface (861 compiler + conformance + host integration) | Differential harness as the gate; identical-bytecode invariant; per-construct flag flips |
| `project.rs` rewriters encode subtle, hard-won semantics (default members, F3c diagnostics, COM early/late, WithEvents) | Treat each as a spec item with a fixture before moving it; never delete a rewriter until its resolver replacement passes its fixtures |
| VBA lexical quirks (case, line continuation, `:`, sigils) | Hand-written lexer + corpus round-trip tests in Phase 1 before any parsing |
| Scope creep / long-lived branch | Phases ship independently behind the flag; each is mergeable; avoid a multi-month dark branch |
| Performance regression | Bench the new pipeline vs old on the corpus; interning + arenas should be ≥ parity |
| Output-contract drift breaking VM/JIT | Bytecode/metadata is the fixed contract; differential harness asserts it |

## 10. Decision log

Resolved:
- **D0 (2026-05-31): Syntactic layer = Roslyn-style green/red CST with typed facades** (not a
  lean AST), because interactive tooling/incremental is a goal. Backbone (resolve → bound HIR →
  lower) unchanged.
- **D4: Syntax storage = `rowan` (or `cstree`) green/red CST**; bound HIR + symbol tables = index
  arenas. `rowan` (deferred) is no longer deferred — it is the chosen syntax layer.
- **D6: Two layers, not a reshaped `BoundExpr`** — typed CST facades for syntax; a *new* bound
  HIR for the resolved IR. `BoundExpr` is retired in favor of the HIR (with a temporary
  CST→`BoundExpr` bridge during Phases 2–3 to keep existing lowering).

Open (settle in Phase 0):
- D1: Lexer — hand-written (default) vs `logos`.
- D2: Parser — hand-written RD+Pratt driving a `GreenNodeBuilder` (default) vs `chumsky`.
- D3: Interner — `lasso` vs `string-interner` (or `cstree`'s built-in interning).
- D5: Diagnostics renderer — `ariadne` vs `codespan-reporting`.
- D7: Grammar source of truth — EBNF-from-MS-VBAL (default) with Rubberduck ANTLR cross-check.
- D8: CST crate — `rowan` vs `cstree` (interning + `Send`/threading); decide on a Phase-0 spike.
- D9: Incremental engine — `salsa` version/shape (Phase 7); confirm it wraps the same queries.

## 11. Test & evidence strategy

- Grammar-coverage matrix (Phase 0) — one fixture per production.
- Lexer round-trip + token snapshots (Phase 1).
- Expression/statement **differential** parity vs the legacy parser (Phases 2–3).
- Resolver equivalence: identical lowering for the corpus (Phase 4).
- Full suites green at every phase: `oxvba-compiler` (861+), `oxvba-vm`, `oxvba-host`,
  `conformance/`, plus the Excel oracle where member/lifetime semantics move.
- Evidence docs under `docs/evidence/` per phase; final closure report.

## 12. Scope notes

In scope (per D0): lossless green/red CST (`rowan`/`cstree`), the SemanticModel overlay, and
`salsa`-based incrementality (Phase 7) — these are now goals, not deferrals.

Out of scope (unless a later workset expands):
- A full **language server / LSP** product surface (Phase 7 builds the foundation — incremental
  queries + semantic API — but the editor integration, completion, code actions, etc. are a
  separate effort).
- The **activation-frame model + object lifetime** ("A") — per-call activation frames / slot
  windows that fix recursion (`recursion_preserves_caller_locals`) **and** object-slot lifetime
  (cascade, `gMT` trailing-`T`, Me-param / return-slot retention). This is a back-end VM/emit
  workstream tracked as its own bead; see §6.1 for the slot-contract dependency. It is *not* in
  this front-end workset, but Phase 4+ should coordinate with it.
- Any change to the **bytecode/metadata instruction format** beyond the frame-relative slot
  semantics introduced by "A", or to the VM/JIT.
