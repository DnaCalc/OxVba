# Green/Red Tree Audit

Date: 2026-06-01
Bead: `bd-aprs.3.1`
Crate: `crates/oxvba-syntax`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Scope

This audit verifies the current custom `oxvba-syntax` green/red tree against the frontend v2 needs:

- lossless text preservation;
- immutable/shareable green root behavior;
- red-tree text ranges and offsets;
- node/token traversal;
- explicit error-node recovery;
- stable handles needed by future IDE queries.

## Verified Behavior

| Need | Current state | Evidence |
|---|---|---|
| Lossless source preservation | `Parse::syntax().text()` reconstructs the source, including trivia | existing parser round-trip tests plus `cargo test -p oxvba-syntax --quiet` |
| Immutable/shareable green root | `Parse` stores `Arc<GreenNode>` and cloned roots preserve pointer identity | new `green_root_clone_preserves_shared_identity` test |
| Text ranges | red root reports `(0, source.len())`; nested tokens carry byte offsets | new `red_tree_nested_token_offsets_cover_source_ranges` test |
| Token/node traversal | red nodes expose `children`, `child_nodes`, and `child_tokens`; nested recursive traversal can reach tokens and trivia | new nested token traversal test |
| Error recovery node | unexpected statement recovery creates `SyntaxKind::ErrorNode` and keeps text lossless | strengthened `error_recovery_produces_tree` test |
| Typed accessors | existing red-tree helpers expose names, params, return types, body blocks | existing typed accessor tests |

## Gaps And Follow-Up Ownership

- Stable IDE handles are not yet durable identities. Red nodes are cheap cursors over a green tree
  plus offset; this is enough for current snapshots but not a cross-edit identity model. FE-6/FE-9
  should define SemanticModel node keys and incremental query identity.
- Traversal helpers allocate `Vec` results. This is acceptable for the current audit, but FE-2.3 /
  FE-9.3 should consider iterator-style APIs if language-service hot paths need them.
- Error recovery is present but incomplete. The parser creates explicit `ErrorNode` for unexpected
  statements and expression primaries, but later parser recovery work still needs targeted
  incomplete-edit fixtures.
- Losslessness is tested by source reconstruction, but there is no dedicated snapshot corpus yet.
  FE-3.4 owns broader lexer/parser snapshots.

## Checks

- `cargo test -p oxvba-syntax --quiet`: passed, 60 tests.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.

## Fresh-Eyes Notes

The custom tree is good enough to remain the default substrate for now. The material gaps are API
and coverage gaps, not evidence that `rowan` or `cstree` is required before frontend v2 can
continue.
