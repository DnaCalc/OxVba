# Qualified Const Visibility Evidence

Date: 2026-07-02
Bead: `bd-aprs.9.9.1` under `bd-aprs.9.9`
Workset:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Target Behavior

The target is real VBA compile-time scoping. Module-level `Const` declarations
default to `Private`; a private constant is usable inside its declaring module,
including through same-module qualification, but it is not a public member that a
sibling module can consume as `Module.Const` or `Project.Module.Const`.

No legacy OxVBA behavior is accepted as the target for this slice.

Sources:
- `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/const-statement`
- `https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/understanding-scope-and-visibility`

## Outcome

The symbol table now carries optional declared visibility on symbols. The scanner
sets that visibility for module-level members from the same source-owned
visibility facts already used by export-surface synthesis.

During compile-time constant folding, qualified constant resolution still folds
public same-project `Module.Const` and `Project.Module.Const` references, but it
requires `Public` visibility when the qualified target is in a different module.
Private constants remain foldable from inside their declaring module.

## Regression Shape

- `Public Const Derived As Long = ModB.Base + 1` still folds when `Base` is
  `Public`.
- `Public Const SameModule As Long = ModA.Secret + 1` folds inside `ModA` even
  when `Secret` is default-Private.
- `Public Const SameExplicit As Long = ModA.ExplicitSecret + 1` folds inside
  `ModA` when `ExplicitSecret` is explicitly `Private`.
- `Public Const FromPrivate As Long = ModA.Secret + 1` in sibling `ModB` no
  longer folds.
- `Public Const FromProjectPrivate As Long = Proj.ModA.Secret + 2` likewise no
  longer folds.
- `Public Const FromExplicitPrivate As Long = ModA.ExplicitSecret + 3` likewise
  no longer folds.
- `env.resolve_qualified(&["ModA", "Secret"])` does not publish the private
  constant as a module-qualified member.

## Checks

- `cargo test -p oxvba-symbol module_qualified_const_values_honor_private_module_scope -- --nocapture`
- `cargo test -p oxvba-symbol module_qualified_const_values_fold_across_modules -- --nocapture`
- `cargo test -p oxvba-symbol --quiet`
- `cargo test -p oxvba-bind --quiet`
- `cargo clippy -p oxvba-symbol -p oxvba-bind --tests -- -D warnings`
- `cargo fmt --all --check`
- `cargo check --workspace`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`

## Boundary

This closes the bounded qualified-constant visibility leak. It does not claim
complete cross-project constant folding, all constant-expression diagnostics, or
complete attribute/session option parity; those remain owned by the broader
FE-8.5.e lane.
