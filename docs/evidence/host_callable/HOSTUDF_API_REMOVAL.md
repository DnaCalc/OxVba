# HostUdf API Removal

Date: 2026-05-24
Bead: `bd-hjys.7`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`

## Implementation summary

Removed the old public `HostUdf*` API surface from `oxvba-host` instead of adapting it:

- Removed `HostUdfCatalog` and nested descriptor DTOs from `engine.rs` and `lib.rs` exports.
- Removed `HostUdfCallContext`, `HostUdfTypedValue`, `HostUdfTypedSignature`, `HostUdfTypedInvokeResult`, `HostUdfInvokeResult`, and `HostUdfTypeMapEvidence`.
- Removed `Engine::host_udf_catalog`, `Engine::host_udf_typed_signature`, `Engine::invoke_host_udf_typed`, and `Engine::invoke_host_udf_with_variants`.
- Removed old host-UDF helper functions and policy fabrication helpers.
- Replaced `RuntimeCallSource::HostUdf` with neutral `RuntimeCallSource::HostCallable`.
- Deleted old host-UDF tests from `invoke_procedure_tests.rs`; neutral coverage now lives in `vba_host_facade_tests.rs` and bundle/session descriptor tests.

Superseded older claims:

- `docs/evidence/HOST_UDF_W093_METADATA_DESCRIPTOR_2026-05-22.md` is historical evidence for the removed old API shape.
- `docs/evidence/conformance/WRAPPED_COM_SERVER_HOST_UDF_*.md` remains historical conformance evidence only; active callable truth is the neutral descriptor/invocation evidence under `docs/evidence/host_callable/`.
- Future host-owned UDF admission/W093 projection is explicitly deferred to `bd-hjys.8` and must not resurrect core `HostUdf*` APIs.

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| HostUdf* public surfaces are removed, not adapted. | `rg -n "HostUdf|host_udf|Host UDF|host UDF|RuntimeCallSource::HostUdf" crates -g'*.rs'` returns no old-shape code references after the rename/removal. |
| Tests are migrated to neutral API names and semantics. | Old host-UDF tests were removed; neutral callable reflection/invocation tests are in `crates/oxvba-host/tests/vba_host_facade_tests.rs`. Existing procedure invocation tests still cover runtime invocation without UDF terminology. |
| Compiler/runtime contain no independent UDF-specific policy engine. | Bundle policy fields were removed in `bd-hjys.4`; engine host-UDF descriptor/policy helpers were removed here; runtime source is now `HostCallable`. |
| Searches for deprecated old-shape API names pass except allowed audit/evidence references. | Code search over `crates/**/*.rs` passes with no old names. Historical/audit docs intentionally retain references for provenance. |
| Evidence docs state which older claims are superseded and removed. | This file lists superseded old evidence and the active replacement evidence set. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-host --test invoke_procedure_tests -- --nocapture
cargo test -p oxvba-host --test vba_host_facade_tests -- --nocapture
cargo test -p oxvba-runtime call_frame -- --nocapture
cargo test -p oxvba-compiler reflect_project -- --nocapture
cargo check --workspace --all-targets
rg -n "HostUdf|host_udf|Host UDF|host UDF|RuntimeCallSource::HostUdf" crates -g'*.rs'
```

Results: all passed; the final code search returned no deprecated old-shape references.

## Fresh-eyes review notes

- The removal did not leave compatibility adapters, aliases, or deprecated re-exports.
- Neutral callable invocation is through `VbaHost` / `PreparedVbaProject` APIs only.
- Runtime terminology now says `HostCallable`, not UDF.
- Old evidence is retained only as historical/audit provenance and is explicitly superseded.
