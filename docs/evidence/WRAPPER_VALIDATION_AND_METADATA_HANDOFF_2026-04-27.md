# Wrapper Validation And Metadata Handoff

Date: 2026-04-27
Bead: `bd-wrap1.5`

## Scope

Validate the current wrapper generation substrate and close the first metadata
handoff gap exposed by the wrapper validation pass.

## Finding

The pre-existing DLL shim generator emitted native export functions that
referenced `DeclareParamType`, `marshal_to_variant`, and
`marshal_from_variant`, but the generated source did not import or define those
symbols. String-shape tests covered the export signatures but not the generated
runtime handoff body.

## Change

`crates/oxvba-build/src/dll.rs` now emits:

- `use oxvba_compiler::{DeclareParamType, OxBundle};`
- bounded scalar/string pointer argument marshaling through
  `marshal_to_variant<T: IntoVariantArg>(...)`
- bounded scalar/string pointer return marshaling through
  `marshal_from_variant<T: FromVariantReturn>(...)`
- regression assertions that the generated DLL source contains the import and
  marshaling handoff layer beside the exported function body

The wrapper DLL source now has an explicit generated bridge from
`NativeExportDescriptor` parameter/return metadata to retained `Variant` invocation
arguments and return values.

## Validation

Commands:

```powershell
cargo fmt --check -p oxvba-build
cargo test -p oxvba-build --lib -- --nocapture
```

Results:

- `cargo fmt --check -p oxvba-build`: pass
- `cargo test -p oxvba-build --lib -- --nocapture`: pass, 31/31

Relevant regression rows:

- `dll::tests::dll_shim_generates_export`
- `dll::tests::dll_shim_sub_has_no_return`
- `exe::tests::exe_shim_contains_project_name`

## Remaining Boundary

This closes the current `bd-wrap1.5` validation slice for generated wrapper
source and metadata handoff. The later `bd-wrap1.6` COM/XLL handoff boundary
must still avoid claiming binary-packaging or Excel-facing closure until those
lanes have their own execution evidence.
