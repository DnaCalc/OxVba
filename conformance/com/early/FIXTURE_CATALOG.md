# COM Early Lane Fixture Catalog

- `typelib_ingest_known_identity.bas`: identity resolution and metadata ingest floor.
- `early_bind_member_exists_success.bas`: member lookup success path.
- `early_bind_member_unknown_error.bas`: deterministic unsupported-member diagnostic path.
- `early_bind_runtime_success.bas`: runtime success over early-bind rewrite lane.
- `early_bind_runtime_missing_arg_resume_next.bas`: runtime failure-route with `On Error Resume Next`.
- `early_late_mix_project.bas`: mixed early/late dispatch path in one module.
