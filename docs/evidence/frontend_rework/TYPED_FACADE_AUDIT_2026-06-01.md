# Typed Facade Audit

Date: 2026-06-01
Bead: `bd-aprs.3.3`
Crate: `crates/oxvba-syntax`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Current Surface

`SyntaxNode` currently exposes generic red-tree operations:

- `kind`
- `text_range`
- `width`
- `children`
- `child_nodes`
- `child_tokens`
- `text`
- `first_token`

It also exposes a small typed-accessor surface:

- `name_token`
- `param_list`
- `params`
- `return_type`
- `statements`
- `body_block`

Existing focused tests cover Sub/Function/Property names, parameter lists, return types, body
blocks, and parameter enumeration. The FE-2.1 tests also cover red ranges and nested token
traversal.

## Minimal Facade Needed Next

| Consumer | Needed facade shape | Current status |
|---|---|---|
| Parser tests | generic traversal, text, node kind, round-trip text | sufficient |
| Binder declarations | typed accessors for module items, procedure/property/event/declare declarations, parameter names/modifiers/types/defaults | partial |
| Binder statements | typed accessors for assignment target/value, call callee/args, block children, labels, control-flow clauses | missing |
| Binder expressions | typed expression facade for binary/unary/call/member/index/new/literal/identifier nodes | missing |
| SemanticModel | stable node key strategy, source range, symbol/type query attachment points | partial; range exists, durable identity absent |
| Formatting/refactoring | trivia-preserving token traversal, parent/child context, separators | partial; traversal exists, parent links/separator APIs absent |

## Required Follow-Up API Rows

- `SourceFile`: module items, options, attributes, diagnostics.
- Declaration nodes: name, visibility, declaration kind, type reference, initializer.
- Procedure/property nodes: name, visibility, parameters, return/value type, body block.
- Parameter nodes: name, `ByRef`/`ByVal`, `Optional`, `ParamArray`, type, default expression.
- Statement nodes: labeled line, assignment target/value, call callee/args, block clauses.
- Expression nodes: operator, operands, callee, receiver, member name, index args, literal token.
- Token/trivia helpers: leading/trailing trivia grouping or equivalent formatting iterator.
- Stable query key: root identity plus text range/node kind as the short-term key; revisit with
  salsa/SemanticModel integration.

## Gaps

- There are no typed wrapper structs yet; all typed accessors are methods on `SyntaxNode`.
- Accessors return raw `SyntaxNode`/`SyntaxToken` values, not typed node wrappers.
- No parent links are available from red nodes.
- No separator-aware list helpers exist for comma-separated params/args/cases.
- No durable cross-edit node identity exists yet.

## Checks

- Existing focused accessor tests in `crates/oxvba-syntax/src/red.rs`.
- `cargo test -p oxvba-syntax --quiet`: passed, 60 tests.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.

## Fresh-Eyes Notes

The current facade is adequate for parser smoke tests but not adequate for binder or IDE work.
That is expected at this phase; the important point is to harden the current tree with typed
wrappers deliberately instead of pretending generic `SyntaxNode` traversal is already a Roslyn-style
API.
