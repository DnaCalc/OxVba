# Bare Object `Is` Identity Binding/Lowering Evidence

Date: 2026-06-01
Bead: `bd-aprs.7.7`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Bare VBA object identity comparisons now have an explicit front-end and runtime
shape instead of being rejected by the syntax bridge or confused with value
equality.

Implemented route:

- `oxvba-syntax` parses `a Is b` and `a Is Nothing` as `BinaryExpr` with
  `KwIs`.
- FE-6 HIR lowers that CST to `HirBinaryOp::Is` and lowers `Nothing` to
  `HirLiteral::Nothing`.
- The temporary CST bridge lowers bare `Is` to `CompareOp::Is` for expression
  tests and rewrites production bridge source for this construct only to
  `__oxvba_object_is(lhs, rhs)`, avoiding the legacy raw substring expression
  parser for the `Is` surface.
- The bridge rejects obvious scalar bare-`Is` operands (for example `1 Is 2`)
  instead of compiling them as object identity.
- Emit lowers `CompareOp::Is` and `__oxvba_object_is` to the typed bytecode
  instruction `Instruction::CmpObjectIsSlots`.
- The VM executes `CmpObjectIsSlots` through
  `runtime_object_identity_is`, comparing retained `ObjectRef` identity and
  treating `Nothing`/cleared object slots as the null object identity.
- Operator metadata now uses `VbaOperatorDescriptor::ObjectIs`, not ordinary
  equality metadata.

## Fixtures

Added conformance fixture sources:

- `conformance/tests/object_identity_is_nothing.bas`
- `conformance/tests/object_identity_is_same_and_different.bas`

These are currently compiled through the opt-in frontend v2 route because the
default production route has not yet flipped for this workset.

## Checks

- `cargo check -p oxvba-compiler`
- `cargo test -p oxvba-compiler object_is --quiet`
- `cargo test -p oxvba-compiler object_identity --quiet`
- `cargo test -p oxvba-compiler frontend_v2_compiles_bare_object_is_identity --quiet`
- `cargo test -p oxvba-vm object_identity_is --quiet`

## Fresh-Eyes Review

- The default compiler route still does not parse bare object `Is`; FE-9 owns
  the default flip after the broader production route is ready.
- The temporary production bridge still rewrites this construct to a structural
  intrinsic before legacy lowering. This is an explicit quarantine for the
  bridge phase, not the terminal HIR-to-bytecode route; FE-8.5 owns direct HIR
  lowering for this and other scoped constructs.
- `CompareOp::Is` is no longer represented as value equality in bytecode or
  metadata. The metadata descriptor now identifies object identity explicitly.
- The VM test covers same object, different object, object-vs-`Nothing`, and
  `Nothing`/empty identity. The conformance fixtures cover source forms for
  `a Is b`, `a Is a`, and `a Is Nothing`.
- Fresh review found and fixed the scalar-operand hole where `1 Is 2` could
  have reached `CmpObjectIsSlots`; the syntax bridge test now covers that
  rejection.
