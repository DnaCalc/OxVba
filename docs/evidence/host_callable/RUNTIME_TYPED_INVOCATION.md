# Runtime Typed Invocation

Date: 2026-05-24
Bead: `bd-hjys.6`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`
Contract source: `docs/evidence/host_callable/NEUTRAL_DESCRIPTOR_MODEL.md`

## Implementation summary

Extended the `VbaHost` facade with neutral callable invocation lanes:

- `HostCallContext`
- `HostCaller`
- `HostContextValue`
- `HostContextObservations`
- `TypedValue`
- `InvocationResult`
- `TypedInvocationResult`
- `PreparedVbaProject::invoke_callable_variant(callable_id, context, args)`
- `PreparedVbaProject::invoke_callable_typed(callable_id, context, args)`
- `PreparedVbaProject::last_context_observations()`

Variant invocation resolves neutral `callable_id` from `ProjectReflection`, validates arity, records context observations on the prepared runtime object, and invokes through the existing runtime session.

Typed invocation uses explicit conversion lane name `TypedScalarFirstTier`, validates first-slice scalar types against descriptor-declared VBA types, converts to `Variant`, invokes by callable ID, and converts the return value back to `TypedValue`.

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| Variant invocation by callable ID works for public Functions. | `vba_host_facade_tests::vba_host_invokes_by_callable_id_with_context_observation_and_typed_lane` calls `PreparedVbaProject::invoke_callable_variant` using a reflected callable ID and receives `Variant::from_i32(5)`. |
| Typed first-slice invocation works with explicit conversion-lane naming. | Same test calls `PreparedVbaProject::invoke_callable_typed` with `TypedValue::Long` arguments, receives `TypedValue::Long(10)`, and asserts `conversion_lane == "TypedScalarFirstTier"`. |
| Arity/type/runtime diagnostics are structured and tested. | `vba_host_facade_tests::vba_host_callable_invocation_reports_structured_diagnostics` asserts `HostDiagnosticPhase::ValidateCall` plus `HOST-CALL-ARITY`, `HOST-CALL-TYPE`, and `HOST-CALL-NOT-FOUND` codes. Runtime conversion failures use `HOST-CALL-RETURN-CONVERSION`; runtime engine failures are mapped through `HOST-PHASE-DIAGNOSTIC`. |
| HostCallContext caller/locale/metadata reaches execution path or documented observation point. | `PreparedVbaProject` stores `last_context_observations`; the test passes caller, locale, and metadata and verifies both result observations and `PreparedVbaProject::last_context_observations()`. |
| Evidence artifact required. | This file: `docs/evidence/host_callable/RUNTIME_TYPED_INVOCATION.md`. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-host --test vba_host_facade_tests -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- New invocation names are neutral (`callable`, `typed`, `variant`) and do not add new `HostUdf*` APIs.
- The context is recorded on the prepared project as an observation point before runtime invocation; it is not solely an output echo.
- Typed conversion intentionally accepts only the first scalar slice plus explicit `TypedValue::Variant` escape lane.
- Callable-ID lookup and arity/type errors fail before runtime with structured diagnostics.
