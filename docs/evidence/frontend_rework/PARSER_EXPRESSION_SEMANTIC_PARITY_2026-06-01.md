# Parser Expression Semantic Parity Evidence

Date: 2026-06-01
Bead: `bd-aprs.5.1`
Workset lane: FE-4.1 Expression parser semantic parity

## Outcome

Hardened the `oxvba-syntax` expression parser for the FE-4.1 scope:

- fixed exponentiation binding power so `^` is right-associative in the Pratt parser;
- added `TypeOf` as a keyword token;
- added parser support for `TypeOf expr Is type-expr` as a single expression node shape;
- expanded keyword-operator fixtures to cover `And`, `Or`, `Xor`, `Mod`, `Like`, `Is`, `Imp`, and
  `Eqv`;
- retained existing coverage for precedence, unary/binary distinction, parenthesized expressions,
  member/index postfixes, and line-continuation expression trivia.

After the workset reopen, this bead was extended from parser-shape proof to production-route
bridge proof for the scoped expression surface:

- `syntax_bridge::lower_expression_to_legacy_bound_expr` now lowers the assignment RHS expression
  from the `oxvba-syntax` CST shape into the existing `BoundExpr` representation;
- the old `resolve::parse_expr_for_syntax_bridge` handoff was deleted, so FE-4.1 bridge expression
  parity no longer depends on legacy `parse_expr` as the authoritative parser;
- the scoped CST lowerer covers literals, identifiers, parenthesized expressions, unary negation,
  `Not`, arithmetic and concat operators, ordinary comparisons, `Like`, logical `And`/`Or`, and
  `TypeOf ... Is ...`;
- unsupported expression shapes now produce an explicit bridge `Unsupported` error rather than a
  silent legacy fallback.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - First-run result: passed, 71 unit tests plus 2 integration tests.
  - Reopen result: passed, 79 unit tests plus 2 integration tests.
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
  - Reopen result: passed, including CST lowering for right-associative `^`, parenthesized
    comparison/logical expressions, and `TypeOf ... Is ...`.
- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
  - Reopen result: passed after marking `resolve::parse_expr_for_syntax_bridge` replaced.
- `cargo fmt --check -p oxvba-compiler -p oxvba-syntax`
  - Reopen result: passed.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The main concrete bug was exponent associativity. The code comment said right-associative, but the
binding powers encoded left-associative Pratt behavior. The test now checks for the nested
right-hand `3 ^ 4` shape.

`TypeOf ... Is ...` was another real omission: `TypeOf` was previously just an identifier, so the
parser could not represent the special expression cleanly. It is now tokenized as `KwTypeOf` and
parsed as one expression surface that consumes the required `Is` token.

Reopen fresh-eyes review found the material gap: the first pass proved the CST parser could
represent the expression forms, but the bridge still parsed the expression text through legacy
`parse_expr`. The reopened implementation removes that special bridge hook and makes the CST
expression node the source of the `BoundExpr` shape for this bead's scoped operators.

The same review also rejected a tempting but wrong shortcut: bare `a Is b` must not lower as normal
equality, because VBA `Is` is object identity. The parser still represents the surface, but bridge
lowering leaves bare object `Is` as an explicit unsupported residual for `bd-aprs.7.7`
binder/object identity work. `TypeOf ... Is ...` is lowered because the current compiler already
has a dedicated `typeofis` intrinsic route for that semantics.

This bead still does not claim full compiler-front-end routing through the new syntax parser.
Statement/postfix parser work, broad source compilation, binder/HIR lowering, and final production
default routing remain separate FE-4 through FE-9 beads.

Residuals left for later beads:

- unifying call/member/index/default-member grammar belongs to FE-4.2;
- bare object `Is` lowering belongs to `bd-aprs.7.7` rather than arithmetic expression lowering;
- broad statement parser coverage belongs to FE-4.3;
- parser diagnostics and recovery snapshots belong to FE-4.5;
- compiler execution parity across the full corpus belongs to FE-5 bridge and harness beads.
