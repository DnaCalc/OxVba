# Private Class Member Visibility Parity

Date: 2026-07-02
Bead: `bd-wuhl`
Parent: `bd-aprs.9.10` / FE-8.5.f broader declaration/type surface

## Scope

This slice makes typed receiver member lookup honor VBA class visibility.
`Private` class members remain callable from their declaring class, but another
module cannot bind `receiver.PrivateMember`. `Friend` and `Public` members
remain visible inside the same project.

The target is real Excel/VBA compile behavior, not legacy OxVBA member lookup.

## Excel/VBA Oracle

Oracle run:
`docs/evidence/conformance/vm3_scoping_visibility_private_class_20260702T1948Z/summary.md`

The harness made the VBE visible, invoked Debug -> Compile VBAProject
(`CommandBar` control id `578`), captured owned modal dialog text with UI
Automation scoped to the owned Excel PID, and cleaned up only the owned Excel
process.

Key rows:

| Case | Excel/VBA result |
| --- | --- |
| `SCOPING-CLASS-PRIVATE-INTERNAL` | Compile ok; `Main.RunProbe` returns `23` through a public class method that calls private `Secret`. |
| `SCOPING-CLASS-PRIVATE-EXTERNAL` | Compile error: `Method or data member not found` for `w.Secret()`. |
| `SCOPING-FRIEND-CLASS-MODULE` | Compile ok; same-project external call to a `Friend` class method returns `19`. |

## Implementation

- `ResolutionEnvironment` now exposes context-aware member/default-member
  resolution from a source `ScopeId`.
- Contextual resolution rejects `Private` member bindings when the caller's
  enclosing module differs from the member's declaring module.
- Context-free member/default-member resolution no longer publishes `Private`
  source members without a source context.
- The binder now uses contextual member and default-member resolution from the
  current procedure scope.

## Checks

- `cargo test -p oxvba-symbol context_member_resolution_honors_private_class_visibility --quiet`
- `cargo test -p oxvba-bind private_class_method --quiet`
- `cargo test -p oxvba-bind private_class_field_is_not_accessible_from_other_module --quiet`
- `cargo test -p oxvba-symbol --quiet`
- `cargo test -p oxvba-bind --quiet`
- `cargo fmt --all --check`
- `cargo clippy -p oxvba-symbol -p oxvba-bind --tests -- -D warnings`
- `cargo check --workspace`
- `git diff --check`
- `br dep cycles --json`
- `scripts\check-governance.ps1`

## Residuals

This bead covers project class receiver visibility for `Private`, `Friend`, and
`Public` member/default-member lookup. It does not claim the whole VBA access
control surface for referenced projects, type-library import visibility, or
all diagnostic wording beyond the oracle-backed reject-vs-accept behavior above.
