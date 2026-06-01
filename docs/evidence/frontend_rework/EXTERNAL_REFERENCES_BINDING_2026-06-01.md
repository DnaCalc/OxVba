# External References Binding Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.6`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_external_references.rs`, a descriptor-backed external
reference binding surface for typelib, project, and native/host-injected references. The reopened
continuation moved this from scaffold-only to production use in `project.rs`.

The model records reference symbols, descriptor identities, member bindings, and stable unresolved
reference diagnostics. Production reference-kind authority now flows through
`build_external_reference_index` for:

- imported typelib qualification before `Dim ... As New OxVba.Type` and related imported COM
  metadata lowering can bind;
- reference-project priority ordering; and
- host-injected/native implicit receiver classification.

The remaining COM metadata construction still uses the existing compatibility metadata helpers
after the frontend reference route has accepted the declared reference. That lowering bridge is
bounded to typelib metadata materialisation and activation/member-token compatibility, not to
deciding whether a dependency reference exists or what kind it is.

## Checks

- `cargo test -p oxvba-compiler frontend_external_references --quiet`
- `cargo test -p oxvba-compiler expand_bound_source_line_stores_imported_typelib_metadata_in_early_bound_binding --quiet`
- `cargo test -p oxvba-compiler expand_bound_source_line_requires_frontend_external_typelib_reference_route --quiet`
- `cargo test -p oxvba-compiler compile_project_accepts_imported --quiet`
- `cargo test -p oxvba-compiler compile_project_rewrites_as_new_external_type_to_createobject_progid --quiet`
- `cargo test -p oxvba-compiler host_injected --quiet`
- `cargo test -p oxvba-compiler reference --quiet`
- `cargo check -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- Reference kind is normalized into one binder-owned route model instead of dependency-specific
  routing branches.
- Descriptor identity is carried with the symbol route, so later lowering can consume package-owned
  descriptor facts.
- Route proof is executable: qualified imported COM declarations fail unless the frontend index
  resolves the qualifier as a typelib reference, and host/project ordering tests continue to pass
  with kind lookup routed through the same index.
- The old direct manifest scans for reference kind in the touched production paths were replaced.
  Legacy compatibility remains only after route acceptance, where existing typelib metadata helpers
  build activation/member descriptors for runtime behavior.
