# Optional Default Invalid Fallback Parity

Date: 2026-07-02
Bead: `bd-9dqh`
Parent: `bd-aprs.9.10` / FE-8.5.f broader declaration/type surface

## Scope

This slice removes the legacy fallback where an explicit `Optional` parameter
default that failed declared-type coercion could still be published as a raw
default value, or disappear and be treated like a missing/default-zero optional.

The target is real Excel/VBA compile behavior, not legacy OxVBA behavior.

## Excel/VBA Oracle

The oracle used VBE Debug -> Compile VBAProject, with dialog handling scoped to
the owned Excel process. The first UIA probe attempt wedged while walking the
VBE accessibility tree after compile; cleanup was PID-scoped to the owned Excel
PID. The retry used Win32 window enumeration for owned `#32770` dialogs, read
child text/button captions, dismissed only the owned dialog, and killed only the
recorded owned Excel PID.

| Case | VBA declaration | Excel/VBA compile result |
| --- | --- | --- |
| `long_string_non_numeric` | `Sub S(Optional ByVal n As Long = "abc")` | `Compile error: Type mismatch` |
| `long_fractional` | `Sub S(Optional ByVal n As Long = 1.5)` | No compile dialog |
| `long_too_wide_longlong` | `Sub S(Optional ByVal n As Long = 5000000000^)` | `Compile error: Overflow` |
| `object_zero` | `Sub S(Optional ByVal o As Object = 0)` | No compile dialog |
| `object_nothing` | `Sub S(Optional ByVal o As Object = Nothing)` | No compile dialog |

## Implementation

- `fold_optional_defaults` now returns `Result<_, SymbolModelError>` and raises
  `SYM-E-INVALID-OPTIONAL-DEFAULT` when an explicit default expression cannot be
  folded/coerced to the declared parameter type.
- Scanner default metadata now distinguishes absent, unparsed, valid, and invalid
  literal defaults. Invalid literal defaults no longer fall back to the raw
  uncoerced carrier.
- Declared enum parameter types keep VBA's underlying `Long` default-coercion
  behavior, rather than being mistaken for object defaults.
- Object optional defaults accept `Nothing` and numeric zero as the `Nothing`
  carrier.

The stable OxVBA diagnostic code is not a claim that the displayed diagnostic
text is byte-for-byte Excel's modal text. The compatibility claim here is that
invalid explicit defaults are rejected at compile/bind time instead of running
through legacy fallback behavior.

## Checks

- `cargo test -p oxvba-symbol invalid_optional_defaults_reject_instead_of_falling_back --quiet`
- `cargo test -p oxvba-symbol invalid_literal_optional_default_does_not_publish_raw_metadata --quiet`
- `cargo test -p oxvba-symbol optional_object_defaults_accept_nothing_and_zero --quiet`
- `cargo test -p oxvba-bind invalid_optional_default_is_bind_error --quiet`
- `cargo test -p oxvba-bind --test feature_coverage optional_enum_member_defaults_are_bound_for_omitted_args --quiet`
- `cargo test -p oxvba-symbol --quiet`
- `cargo test -p oxvba-bind --quiet`
- `cargo fmt --all --check`
- `cargo clippy -p oxvba-symbol -p oxvba-bind --tests -- -D warnings`
- `cargo check --workspace`
- `git diff --check`
- `br dep cycles --json`
- `scripts\check-governance.ps1`

## Residuals

This bead does not claim the whole `Optional` default-expression language. In
particular, `Empty`/`Null` default behavior and broader exact Excel diagnostic
text mapping remain separate oracle-backed work if they enter the accepted
surface.
