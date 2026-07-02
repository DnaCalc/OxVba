# Enum Initializer Invalid Value Evidence

Date: 2026-07-02
Bead: `bd-aprs.9.9.2` under `bd-aprs.9.9`
Workset:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Target Behavior

The target is real VBA compile-time behavior. A VBA `Enum` member's optional
constant expression evaluates to a `Long`; if no expression is specified, the
member receives zero for the first item or one more than the previous member.
Invalid explicit initializers should not be treated as if no initializer was
present.

No legacy OxVBA fallback behavior is accepted as the target for this slice.

Source:
- `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/enum-statement`

## Outcome

Clean symbol enum folding no longer uses the running auto-counter when an
explicit enum initializer is not a valid `Long` constant. Invalid explicit
initializers now leave that enum block's member values absent, so downstream
binding reports the enum value as an unresolved constant instead of consuming a
fabricated numeric value.

Valid enum values still fold through the same path. In particular, VBA radix
Long bit-pattern literals such as `&HFFFFFFFF` remain signed `Long` values
(`-1`), and the next implicit member still wraps through zero.

## Regression Shape

- `AllBits = &HFFFFFFFF` folds to `CoreConst::I32(-1)`.
- The following implicit `AfterBits` folds to `CoreConst::I32(0)`.
- `Bad = 1.5` no longer folds to the current auto-counter value.
- `AfterBad` no longer receives a fabricated value after the invalid explicit
  initializer.
- `TooWide = 5000000000^` no longer wraps into a fake `Long`.

## Checks

- `cargo test -p oxvba-symbol enum_initializers_do_not_auto_counter_invalid_explicit_values -- --nocapture`
- `cargo test -p oxvba-symbol scanner_declares_enum_members -- --nocapture`
- `cargo test -p oxvba-symbol const_and_enum_values_fold_into_the_type_system -- --nocapture`
- `cargo test -p oxvba-symbol referenced_project_resolves_through_its_export_surface -- --nocapture`
- `cargo test -p oxvba-symbol --quiet`
- `cargo test -p oxvba-bind --quiet`
- `cargo clippy -p oxvba-symbol -p oxvba-bind --tests -- -D warnings`
- `cargo fmt --all --check`
- `cargo check --workspace`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`

## Boundary

This closes the bounded fabricated enum-value fallback. It does not claim a full
compile-time diagnostic model for every invalid enum initializer; today, the
symbol layer leaves invalid values absent and the binder reports unresolved
constant use sites.
