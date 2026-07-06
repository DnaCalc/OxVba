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
- `https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/da1d4885-946f-4937-9487-488143b97f08`

## Outcome

Clean symbol enum folding no longer uses the running auto-counter when an
explicit enum initializer is not a valid `Long` constant. As of the 2026-07-06
continuation, invalid explicit enum initializers now reject during symbol
environment construction with `SymbolModelError::InvalidConstValue`, and the
normal binder path reports the same failure before execution.

Valid enum values still fold through the same path. In particular, VBA radix
Long bit-pattern literals such as `&HFFFFFFFF` remain signed `Long` values
(`-1`), and the next implicit member still wraps through zero. Active-project
enum initializers can also read exported referenced-project constants through
the same final export-surface fold used for `Const` values.

## Regression Shape

- `AllBits = &HFFFFFFFF` folds to `CoreConst::I32(-1)`.
- The following implicit `AfterBits` folds to `CoreConst::I32(0)`.
- `Bad = 1.5` rejects as `InvalidConstValue`.
- `TooWide = 5000000000^` rejects as `InvalidConstValue`.
- Forward and self references in explicit enum initializers reject as
  `InvalidConstValue`.
- `FromLib = Seed` in an active-project enum folds when `Seed` is an exported
  referenced-project constant.

## Checks

- `cargo test -p oxvba-symbol enum_initializers_keep_long_bit_patterns_and_auto_increment -- --nocapture`
- `cargo test -p oxvba-symbol enum_initializers_reject_invalid_explicit_values -- --nocapture`
- `cargo test -p oxvba-symbol enum_initializers_fold_referenced_project_constants -- --nocapture`
- `cargo test -p oxvba-bind invalid_enum_initializer_is_bind_error -- --nocapture`
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

This closes the bounded fabricated enum-value fallback and the direct diagnostic
gap for the covered explicit initializer surface. It does not claim every Enum
static rule is complete; duplicate names, project/type namespace conflicts, and
any parser-unsupported expression shapes remain governed by their existing
diagnostic lanes.
