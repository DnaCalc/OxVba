# Workset: Front-End Refactor — Tokenizer, Parser, Resolved AST, and Binder

Date: 2026-05-31
Owner: DNA Kode
Status: proposed

## 1. Purpose

Move OxVba's compiler front-end from its current **string-rewriting + string-splitting**
shape toward a conventional compiler pipeline:

```
source text
  → lexer (tokens + spans)
  → parser (recursive-descent + Pratt) → syntax AST
  → resolver / binder (symbol table, scopes) → resolved AST (HIR)
  → lowering → bytecode (+ runtime metadata)
```

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

### 5.5 AST / IR storage
- **Index-based arenas** (newtype `ExprId`/`StmtId`/`SymbolId` into `Vec`s; the rustc /
  rust-analyzer pattern), or `la-arena`/`id-arena`. Avoids `Box` graphs and makes the resolved
  AST + symbol tables ergonomic.
- **`rowan`** (lossless red-green CST) is noted but **deferred** — it is the right tool if/when
  we want full-fidelity round-trip + IDE tooling; start with a plain AST.

### 5.6 Patterns to adopt
- **Distinct passes & IRs**: syntactic AST (names unresolved) → resolved AST / HIR (names →
  symbols; member/default-member/property/`New` resolved) → bytecode lowering. The `project.rs`
  rewriting dissolves into the resolver + lowering.
- **Error recovery**: the parser produces a partial tree + a diagnostic list rather than bailing
  on first error (matters for a developer-facing tool).
- **Symbol table / scopes** modelling VBA scoping: procedure locals, parameters, module-level,
  project-level/`Public`, predeclared singletons, `With`-block targets, `Const`.
- **Salsa / query incrementalism**: noted for a future IDE/incremental story; **out of scope**.

## 6. Target architecture

```
SourceFile ──lex──▶ [Token{kind, span}]
            ──parse──▶ SyntaxTree (Stmt/Expr arena; identifiers = interned, UNRESOLVED)
            ──resolve──▶ ResolvedAst/HIR (Var→SymbolId; Member→{early-bound proc | late dispatch};
                                          default members, property forms, New, WithEvents resolved)
            ──typecheck──▶ (annotations / diagnostics on HIR)
            ──lower──▶ Bytecode + ProcedureRuntimeMetadata  (unchanged output contract)
```

Key invariant: the **lowering output** (bytecode + metadata + descriptors) is the stable
contract. The refactor changes everything *upstream* of lowering while keeping the bytecode
identical, so the VM, JIT, host, and the 861 + conformance suites are unaffected by construction
until we deliberately change semantics.

## 7. Phased plan (each phase: behind a flag, differential-tested, independently shippable)

A `frontend_v2` build/config flag selects the new pipeline. A **differential harness** compiles
the full corpus (compiler unit fixtures + `conformance/` + host integration projects) through
both pipelines and asserts identical bytecode/metadata; divergences are triaged (bug vs known).

- **Phase 0 — Foundations & decisions.**
  Capture the EBNF grammar (`docs/spec/VBA_GRAMMAR_V1`) + coverage matrix. Lock library choices
  via spikes (lexer, parser, interner, arena, diagnostics). Build the differential harness.
  *Exit:* grammar + matrix committed; harness compiles the corpus both ways (old==old as a
  sanity baseline); decisions recorded in §10.

- **Phase 1 — Lexer.**
  Hand-written tokenizer with spans for the whole language; handle line continuation, `:`,
  comments, sigils, bracketed idents, date/hex/octal literals, case folding. Fuzz/round-trip
  against the corpus (every source file tokenizes without loss).
  *Exit:* lexer tokenizes the entire corpus; token snapshot tests.

- **Phase 2 — Expression parser (Pratt).**
  Parse expressions into the AST behind the flag; initially target the existing `BoundExpr`
  (or a new `Expr` lowered to `BoundExpr`) so downstream is unchanged. Differential-test against
  `parse_expr` over a large expression corpus. Add the missing nodes: `Is`, unified `Index`,
  `New`, and route all member access (incl. `name.member`) through one postfix grammar.
  *Exit:* expression differential parity on the corpus; S1/S5/S6 addressed at the syntactic level.

- **Phase 3 — Statement parser.**
  Full statement grammar (Dim/Const, `Set`/`Let`/`Call`, `If`/`For`/`Do`/`While`/`Select`,
  `With`, `On Error`/`Resume`, `RaiseEvent`, `Property`, declarations, attributes). Differential
  against current bound statements.
  *Exit:* statement differential parity; the new parser fully replaces ad-hoc statement parsing
  (still behind the flag).

- **Phase 4 — Resolver / binder (the deep one).**
  Symbol table + scopes; produce a resolved AST. Move member dispatch, default-member
  resolution, property Get/Let/Set selection, qualified-name resolution, `New`, WithEvents, and
  the F3c diagnostics **out of `project.rs` string rewriting** into the resolver. This is where
  the member-access dual-path collapses into one and where S2/S3 are resolved.
  *Exit:* resolver produces equivalent lowering for the corpus; `project.rs` rewriters begin
  retirement.

- **Phase 5 — Typed intrinsics & optimizer split.**
  Replace structural stringly-intrinsics (S4) with typed nodes (null/`Nothing`, `New`,
  dynamic-dispatch, WithEvents ops, omitted-arg) — exhaustive enum matches instead of
  `match name.as_str()`. Move `AddConst`/`SubConst` (S6) into an optimizer pass over uniform
  `BinaryOp`.
  *Exit:* emit's magic-string dispatch shrinks to genuine library intrinsics; consumers match
  exhaustively.

- **Phase 6 — Flip & retire.**
  Make `frontend_v2` the default; remove the legacy `parse_expr` string-splitting and the
  retired `project.rs` rewriters; delete the member-access dual-path. Keep the differential
  harness as a regression guard for one release, then archive.
  *Exit:* single pipeline; legacy paths deleted; full suite + conformance green.

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

## 10. Open decisions / decision log (to fill in Phase 0)

- D1: Lexer — hand-written (default) vs `logos`.
- D2: Parser — hand-written RD+Pratt (default) vs `chumsky`.
- D3: Interner — `lasso` vs `string-interner`.
- D4: AST storage — newtype-index arenas (default) vs `la-arena`/`id-arena`; CST via `rowan` (deferred?).
- D5: Diagnostics renderer — `ariadne` vs `codespan-reporting`.
- D6: New `Expr`/`Stmt` AST vs incrementally reshaping `BoundExpr` in place.
- D7: Grammar source of truth — EBNF-from-MS-VBAL (default) with Rubberduck cross-check.

## 11. Test & evidence strategy

- Grammar-coverage matrix (Phase 0) — one fixture per production.
- Lexer round-trip + token snapshots (Phase 1).
- Expression/statement **differential** parity vs the legacy parser (Phases 2–3).
- Resolver equivalence: identical lowering for the corpus (Phase 4).
- Full suites green at every phase: `oxvba-compiler` (861+), `oxvba-vm`, `oxvba-host`,
  `conformance/`, plus the Excel oracle where member/lifetime semantics move.
- Evidence docs under `docs/evidence/` per phase; final closure report.

## 12. Out of scope (unless a later workset expands)

- IDE/incremental (salsa), lossless CST (`rowan`), language-server features.
- The runtime object-slot-lifetime work (cascade / `gMT` trailing-`T`) — that is the parked "A"
  item, a VM concern, independent of this front-end refactor.
- Any change to the bytecode/metadata contract or to the VM/JIT.
