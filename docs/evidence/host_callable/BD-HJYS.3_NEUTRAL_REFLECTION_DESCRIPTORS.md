# BD-HJYS.3 Neutral Project Reflection Descriptors

Date: 2026-05-24
Bead: `bd-hjys.3`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`
Contract source: `docs/evidence/host_callable/NEUTRAL_DESCRIPTOR_MODEL.md`

## Implementation summary

Implemented a first neutral descriptor projection in `oxvba-compiler`:

- `ProjectReflection`
- `ProjectIdentity`
- `ModuleDescriptor` / `ModuleVisibility`
- `ProcedureDescriptor` / `ProcedureVisibility`
- `ProcedureSignature` / `ProcedureParameterDescriptor`
- `ProcedureKind`, `CallingShape`, `PassingMode`
- `VbaTypeDescriptor` / `VbaType`
- `RuntimeProcedureRoute`, `SourceSpan`, `ProcedureAnnotation`
- `CallableCapability`, `InvocationLane`, `UnsupportedReason`
- `reflect_project(&ProjectManifest) -> ProjectReflection`

The new surface is re-exported from `crates/oxvba-compiler/src/lib.rs`.

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| Public Functions, public Subs, private procedures, and class procedures project into neutral descriptors. | `reflect_project_projects_neutral_module_and_procedure_descriptors` covers public function `Add`, private sub `Hidden`, class property `Name`, and private class function `Internal` is included by projection inventory. |
| Parameter names/types, return types, visibility, procedure kind, and module kind are represented. | Same test asserts parameter names, `ByVal`/`ByRef`, scalar types, `Double` return type, public/private flags, option-private flag, class-member flag, `Function`/`Sub`/`PropertyGet`, and procedural/class module kinds. |
| Descriptor fields do not synthesize volatility, registry, worksheet, thread-safety, or formula policy. | `reflect_project_does_not_synthesize_host_udf_policy_fields` debug-scans reflection output for forbidden policy tokens: `udf`, `worksheet`, `volatile`, `registry`, `thread_safety`, `formula`, `selection_policy`. Code structs intentionally have no such fields. |
| Compiler-focused tests cover Suite A rows from the workset. | Tests are inline in `crates/oxvba-compiler/src/project.rs`, run by `cargo test -p oxvba-compiler reflect_project -- --nocapture`. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-compiler reflect_project -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- The projection is compiler-fact-only and does not adapt or rename old `HostUdf*` APIs.
- Capability fields describe implementation invocation lanes only; they do not carry worksheet/UDF admission policy.
- Source fingerprints are stable hash strings over source/descriptor inputs for first-pass identity; later beads may replace these with stronger content-hash types without adding policy.
- `runtime_route` is intentionally `None` in this bead because bundle inventory and prepared invocation are owned by subsequent beads (`bd-hjys.4` through `bd-hjys.6`).
