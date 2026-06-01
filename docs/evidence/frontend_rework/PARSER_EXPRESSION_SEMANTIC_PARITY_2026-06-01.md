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

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - Result: passed, 71 unit tests plus 2 integration tests.
- `cargo fmt --check -p oxvba-syntax`
  - Result: passed.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The main concrete bug was exponent associativity. The code comment said right-associative, but the
binding powers encoded left-associative Pratt behavior. The test now checks for the nested
right-hand `3 ^ 4` shape.

`TypeOf ... Is ...` was another real omission: `TypeOf` was previously just an identifier, so the
parser could not represent the special expression cleanly. It is now tokenized as `KwTypeOf` and
parsed as one expression surface that consumes the required `Is` token.

This bead does not claim full compiler-front-end routing through the new syntax parser. The bridge
and statement/postfix parser work are still separate FE-4 beads. Existing legacy conformance tests
continue to own execution semantics until the bridge is introduced.

Residuals left for later beads:

- unifying call/member/index/default-member grammar belongs to FE-4.2;
- broad statement parser coverage belongs to FE-4.3;
- parser diagnostics and recovery snapshots belong to FE-4.5;
- compiler execution parity through a bridge belongs to FE-4/FE-5 bridge and harness beads.
