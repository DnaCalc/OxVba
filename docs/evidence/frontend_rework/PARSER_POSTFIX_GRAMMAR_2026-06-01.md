# Parser Postfix Grammar Evidence

Date: 2026-06-01
Bead: `bd-aprs.5.2`
Workset lane: FE-4.2 Unified postfix grammar

## Outcome

Hardened postfix and statement-call syntax in `oxvba-syntax`:

- added constrained contextual-name keyword handling for expression/call positions, so
  keyword-colliding callees such as `Name$` can participate in postfix chains;
- covered explicit `Call obj.Method(...)` and implicit `obj.Method arg1, arg2` statement forms;
- covered bang/member/index chaining such as `obj!Field(0).Value`;
- fixed the lexer/parser boundary where `obj!Field` was incorrectly tokenized as an identifier type
  suffix rather than a bang member operator;
- preserved `x!` as an identifier type suffix when `!` is not followed by a member-name start.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - Result: passed, 75 unit tests plus 2 integration tests.
- `cargo fmt --check -p oxvba-syntax`
  - Result: passed after formatting.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The most important issue found during this bead was not in the parser loop itself but in the token
stream feeding it. FE-3 made attached `!` a valid identifier type suffix, but `obj!Field` must be a
bang-member postfix. The lexer now leaves `!` as `Bang` when it is followed by an identifier or
bracketed member start, while still tokenizing `x!` as `TypeSuffix`.

The contextual-keyword path is intentionally constrained to keyword names commonly used as
statement/file/member surfaces (`Name`, `Line`, `Print`, file-mode keywords, etc.). It does not make
all reserved words valid expression names.

This bead verifies CST shape and syntax recovery only. Default-member semantics, call binding,
late-bound dispatch, and compiler execution parity remain binder/bridge work.

After the workset reopen, the bead was extended from parser-shape proof to bridge route proof for
the scoped postfix surface:

- `syntax_bridge::lower_expression_to_legacy_bound_expr` now lowers simple call/index expressions
  from CST `IndexExpr` nodes into `BoundExpr::ProcCall`;
- member and bang chains lower from CST `MemberExpr` nodes into `BoundExpr::Member`;
- member calls attach CST `ArgList` arguments to the lowered member route;
- the bridge test covers a keyword-colliding suffixed call, `obj.Method(1)`, and
  `obj!Field(0).Value`.

This still does not claim final dispatch/default-member semantics. The proof is that the production
bridge no longer has to reparse these scoped postfix expression forms from source text; binder/HIR
beads remain responsible for deciding whether a member is early-bound, late-bound, default-member,
or host-provided.

Residuals left for later beads:

- complete statement coverage belongs to FE-4.3;
- CST-to-legacy lowering belongs to FE-4.4;
- parser diagnostic snapshots belong to FE-4.5;
- semantic call/default-member resolution belongs to FE-7 and later binder/HIR lanes.
