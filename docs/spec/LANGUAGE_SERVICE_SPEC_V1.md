# OxVBA Language Service Spec V1

> [!CAUTION]
> **Superseded and not currently implemented.** The described service was removed from the clean stack. Use [`OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md`](OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md).

**Status:** superseded historical design for the deleted language-service stack
**Date:** 2026-03-23
**Implements:** §3.1.6, §4.8 of `HOSTING_PROJECT_TOOLING_PROPOSAL.md`

**Successor note:** This document remains the authority for the current bounded internal language-service surface tracked by `LSF-0001`. Forward first-class platform execution now proceeds under `docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md`.

---

## 1. Context

OxVBA has a complete VBA compiler (resolver: 6,097 lines, typechecker: 2,140 lines, emitter: 3,265 lines) with 2,098 passing tests. The design-locked spec (§3.1.6, §4.8 of `HOSTING_PROJECT_TOOLING_PROPOSAL.md`) defines a `LanguageServiceProvider` trait for in-process hosts, with LSP as a Phase 2 wrapper. All methods accept `ProjectManifest` — source comes from the host, not the filesystem.

The current compiler works on normalized lines (continuations merged, comments stripped, With blocks rewritten) and produces `BoundModule` with full semantic info but zero source positions. The `oxvba-syntax` crate (282 lines total) is a stub — completely unused by the compiler pipeline (zero imports from it anywhere in the workspace).

This spec defines: a lossless syntax tree, span-tracked semantic model, workspace abstraction, and the `LanguageServiceProvider` trait — architected so that the LSP server becomes a trivial wire adapter.

---

## 2. Key Decisions

### 2.1 Syntax Tree: Custom Enhanced Green/Red (not rowan, not tree-sitter)

**Chosen over:**
- **rowan 0.16**: Battle-tested (rust-analyzer, Zed) but adds external deps. VBA modules are small enough (<2K lines, lex <1ms) that structural sharing is irrelevant.
- **tree-sitter**: C FFI overhead, overkill for sub-millisecond parsing. Doesn't integrate with Rust compiler pipeline.

**Rationale:** Zero consumers of current syntax crate — building from scratch. A custom ~500-line implementation gives typed nodes, positions, parent pointers, and lossless trivia with zero dependency cost. Aligns with project's "no frameworks in the compiler" philosophy. Migration to rowan is straightforward later if needed (similar node abstraction).

### 2.2 Incremental Strategy: Document-Level (not salsa, not statement-level)

**Chosen over:**
- **salsa**: ~2,500 LoC framework overhead. Pays off for 500+ module projects, but VBA projects are typically 20-200 modules.
- **Statement-level**: ~2,000 LoC incremental parser. Only benefits modules with 5,000+ lines (rare in VBA).

**Rationale:** Resolver runs in <50ms for large VBA modules. Document-level caching (re-analyze only the changed module, serve cached results for others) gives the right granularity for VBA project sizes. Architecture uses `Arc<SemanticSnapshot>` per document — same shape salsa would manage — so upgrading later requires no architectural changes.

### 2.3 Span Tracking: Hybrid CST + SymbolTable (not inline spans in resolver)

**Chosen over:**
- **Add TextSpan to BoundExpr/BoundStmt**: Touches ~200 call sites in resolver. Full coverage but high churn.
- **Post-pass correlation only**: Zero resolver changes but fragile on duplicate names.

**Rationale:** CST owns positions by construction. The `SymbolTable` maps CST node identifiers to `SymbolInfo { kind, type, definition_node_id, scope }` for O(1) lookups. Resolver needs zero changes for Phase 1 — SymbolTable is built as a post-pass over BoundModule + CST correlation.

### 2.4 Architecture Influences

| Innovation | Source | Adopted |
|-----------|--------|---------|
| Resilient parsing | tree-sitter, Swift | Yes — ErrorNode in CST |
| Typed syntax | rust-analyzer | Yes — generated accessors per VBA node kind |
| Snapshot isolation | Roslyn | Yes — `Arc<SemanticSnapshot>` |
| Host-agnostic source | OxVBA spec §3.1.6 | Yes — per spec requirement |
| Demand-driven queries | salsa | Deferred — document-level cache sufficient at VBA scale |

---

## 3. Crate Structure

```
crates/oxvba-syntax/              — Lossless VBA syntax tree (rewritten)
crates/oxvba-languageservice/     — Workspace + SemanticModel + LanguageService
crates/oxvba-lsp/                 — Thin LSP wrapper (Phase 2, after core is stable)
```

---

## 4. Step 1: Lossless VBA Syntax Tree (`oxvba-syntax` rewrite)

The current crate is 282 lines, zero consumers, replaced entirely.

### 4.1 `SyntaxKind` Enum

Three categories:

**Token kinds (~70):** All VBA keywords (`KwSub`, `KwFunction`, `KwEnd`, `KwIf`, `KwThen`, `KwElse`, `KwFor`, `KwNext`, `KwDo`, `KwLoop`, `KwWhile`, `KwSelect`, `KwCase`, `KwWith`, `KwDim`, `KwConst`, `KwPublic`, `KwPrivate`, `KwStatic`, `KwSet`, `KwLet`, `KwAs`, `KwNew`, `KwNothing`, `KwByRef`, `KwByVal`, `KwOptional`, `KwParamArray`, `KwProperty`, `KwGet`, `KwDeclare`, `KwLib`, `KwAlias`, `KwType`, `KwEnum`, `KwEvent`, `KwRaiseEvent`, `KwWithEvents`, `KwImplements`, `KwGoTo`, `KwGoSub`, `KwReturn`, `KwCall`, `KwExit`, `KwOn`, `KwError`, `KwResume`, `KwErase`, `KwReDim`, `KwPreserve`, `KwOption`, `KwExplicit`, `KwBase`, `KwCompare`, `KwMe`, `KwTrue`, `KwFalse`, `KwNot`, `KwAnd`, `KwOr`, `KwXor`, `KwMod`, `KwLike`, `KwIs`, `KwDebug`, `KwStop`, etc.), literals (`IntLiteral`, `FloatLiteral`, `HexLiteral`, `StringLiteral`, `DateLiteral`), operators/punctuation, trivia (`Whitespace`, `Newline`, `Comment`, `LineContinuation`).

**Composite node kinds (~40):** `SourceFile`, `SubDecl`, `FunctionDecl`, `PropertyDecl`, `DimStmt`, `ConstStmt`, `IfStmt`, `ElseIfClause`, `ElseClause`, `ForStmt`, `ForEachStmt`, `DoStmt`, `WhileStmt`, `SelectStmt`, `CaseClause`, `WithStmt`, `AssignStmt`, `CallStmt`, `OnErrorStmt`, `ResumeStmt`, `ReDimStmt`, `EraseStmt`, `ExitStmt`, `GoToStmt`, `ParamList`, `Param`, `TypeRef`, `Block`, `BinaryExpr`, `UnaryExpr`, `CallExpr`, `MemberExpr`, `IndexExpr`, `NewExpr`, `IdentExpr`, `LiteralExpr`, `ParenExpr`, `TypeBlock`, `EnumBlock`, `DeclareStmt`, `ErrorNode`.

**Sentinels:** `Eof`, `Root`, `Error`.

### 4.2 Lossless Lexer

Takes `&str`, produces `Vec<(SyntaxKind, &str)>` — kind + slice into original source (zero-copy). Every byte of input is covered by exactly one token. Whitespace, newlines, comments, line continuations are all explicit trivia tokens.

Key capabilities:
- All ~60 VBA keywords (case-insensitive match)
- Multi-char operators: `<=`, `>=`, `<>`, `:=`
- Type suffixes: `%&!#@$`
- Hex/octal: `&H`, `&O` prefixes
- Float literals: `1.5`, `1E10`, `1.5E-3`
- Date literals: `#1/1/2000#`
- Bracketed identifiers: `[Sheet1]`
- Line continuation: `_` at EOL → `LineContinuation` trivia

### 4.3 Green Tree (Custom, No rowan)

```rust
pub struct GreenNode {
    kind: SyntaxKind,
    width: u32,
    children: Vec<GreenChild>,
}

pub enum GreenChild {
    Token { kind: SyntaxKind, text: Box<str> },
    Node(Arc<GreenNode>),
}
```

`Arc<GreenNode>` enables cheap snapshot cloning. `width` enables O(1) offset computation during red-tree traversal.

### 4.4 Red Tree (Position-Aware Facade)

```rust
pub struct SyntaxNode<'a> {
    green: &'a GreenNode,
    offset: u32,
}
```

Methods: `kind()`, `text_range()`, `children()`, `parent()` (via re-walk from root), `first_token()`, `last_token()`, `text()`.

### 4.5 Parser — Recursive Descent with Error Recovery

Event-based parser producing a `GreenNode` tree:

```rust
struct Parser<'a> {
    tokens: &'a [(SyntaxKind, &'a str)],
    pos: usize,
    events: Vec<Event>,
}
```

**Error recovery:** On unexpected token, parser wraps unexpected tokens in an `ErrorNode` and resynchronizes at the next statement boundary (newline starting a keyword). Tree is always complete — every byte of source is represented.

### 4.6 Public API

```rust
pub struct Parse {
    green: Arc<GreenNode>,
    errors: Vec<ParseError>,
}
impl Parse {
    pub fn syntax(&self) -> SyntaxNode<'_>;
    pub fn errors(&self) -> &[ParseError];
}
pub fn parse(source: &str) -> Parse;
```

### 4.7 Invariants

- **Round-trip:** `parse(source).syntax().text() == source` for every input
- **Error recovery:** Partial source still produces a tree
- **Token coverage:** Every VBA keyword, operator, literal type recognized

---

## 5. Step 2: Semantic Snapshot with Symbol Table

### 5.1 Core Types

```rust
pub struct TextSpan { pub start: u32, pub end: u32 }

pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub bound_type: BoundType,
    pub definition_span: TextSpan,
    pub scope: ScopeId,
}

pub type ScopeId = u32;  // 0 = module-level, N = procedure index
```

### 5.2 SemanticSnapshot

```rust
pub struct SemanticSnapshot {
    pub source: Arc<str>,
    pub parse: Arc<Parse>,
    pub bound: Arc<BoundModule>,
    pub symbols: SymbolTable,
    pub diagnostics: Vec<SpannedDiagnostic>,
}
```

Build pipeline:
1. `parse(source)` → CST with parse errors
2. `resolve_symbols(source)` → BoundModule with resolution diagnostics
3. `check_types(bound_module)` → type-checked module
4. **Correlation pass:** Walk CST for `SubDecl`/`FunctionDecl`/`DimStmt` nodes. Match by name against BoundModule. Record `TextSpan` from CST, `BoundType` from resolver. Build `SymbolTable`.
5. **Diagnostic mapping:** Map resolution diagnostics to CST positions.

### 5.3 Compiler Integration

Expose 4 private functions via `pub mod lsp_support` shim in oxvba-compiler:
- `detect_proc_kind` — identifies procedure lines
- `parse_proc_signature` — name + params + return type
- `normalize_ident` — VBA identifier normalization
- `intrinsic_spec` — intrinsic function catalog (85+ entries)

---

## 6. Step 3: Workspace Model

### 6.1 Document

```rust
pub struct Document {
    pub id: DocumentId,
    pub version: u64,
    pub source: Arc<str>,
    pub snapshot: Option<Arc<SemanticSnapshot>>,
}
```

### 6.2 Workspace

```rust
pub struct Workspace {
    documents: HashMap<DocumentId, Arc<Document>>,
    project: Option<ProjectManifest>,
    cross_module_exports: HashMap<String, Vec<SymbolInfo>>,
}
```

### 6.3 LanguageServiceProvider

Implements the trait from §4.8. Each method delegates to the SemanticSnapshot:

| Method | Implementation |
|--------|---------------|
| `diagnostics` | Return `snapshot.diagnostics` for each module |
| `symbols` | Return `snapshot.symbols` flattened across all modules |
| `completions` | Find scope at position → keywords + intrinsics + procedures + variables in scope |
| `signature_help` | If cursor inside call arg list → return procedure's `BoundParam` list |
| `go_to_definition` | Find identifier at position → SymbolTable lookup → return definition_span |
| `find_references` | Find symbol at position → walk all CSTs → return matching spans |
| `hover` | Find symbol → format type/signature as markup |

---

## 7. Step 4: LSP Server (Phase 2)

Deferred until in-process API is stable.

New crate `crates/oxvba-lsp/` with deps: `oxvba-languageservice`, `tower-lsp`, `tokio`. ~200 lines translating JSON-RPC to `LanguageServiceProvider` calls.

---

## 8. File Manifest

### Create

| File | Est. Lines | Purpose |
|------|-----------|---------|
| `crates/oxvba-languageservice/Cargo.toml` | 15 | New crate manifest |
| `crates/oxvba-languageservice/src/lib.rs` | 20 | Crate root, re-exports |
| `crates/oxvba-languageservice/src/span.rs` | 60 | TextSpan, SymbolInfo, SymbolKind, ScopeId |
| `crates/oxvba-languageservice/src/semantic.rs` | 400 | SemanticSnapshot, correlation pass, diagnostic mapping |
| `crates/oxvba-languageservice/src/document.rs` | 80 | Document, DocumentId |
| `crates/oxvba-languageservice/src/workspace.rs` | 200 | Workspace, cross-module export tracking |
| `crates/oxvba-languageservice/src/service.rs` | 500 | LanguageServiceProvider impl |

### Rewrite (oxvba-syntax — 282 lines, replacing entirely)

| File | Est. Lines | Purpose |
|------|-----------|---------|
| `crates/oxvba-syntax/src/syntax_kind.rs` | 250 | Full VBA SyntaxKind enum |
| `crates/oxvba-syntax/src/lexer.rs` | 350 | Complete VBA lexer |
| `crates/oxvba-syntax/src/parser.rs` | 800 | Recursive-descent parser with error recovery |
| `crates/oxvba-syntax/src/green.rs` | 80 | GreenNode + GreenChild |
| `crates/oxvba-syntax/src/red.rs` | 120 | SyntaxNode facade with traversal |
| `crates/oxvba-syntax/src/lib.rs` | 30 | Parse struct + public API |

### Modify

| File | Change |
|------|--------|
| `Cargo.toml` (root) | Add `oxvba-languageservice` to workspace members |
| `crates/oxvba-compiler/src/lib.rs` | Add `pub mod lsp_support` (~20 lines) |
| `crates/oxvba-compiler/src/resolve.rs` | 4 functions `fn` → `pub(crate) fn` |

**Total new/rewritten code: ~2,900 lines**

---

## 9. Verification

```bash
cargo check --workspace
cargo test --workspace
cargo test --package oxvba-syntax
cargo test --package oxvba-languageservice
```
