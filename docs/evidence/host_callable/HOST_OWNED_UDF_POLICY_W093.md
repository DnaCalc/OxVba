# Host-Owned UDF Policy And W093 Projection

Date: 2026-05-24
Bead: `bd-hjys.8`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`

## Implementation summary

Added an example host-owned policy layer in `crates/oxvba-host/src/udf_policy_example.rs`:

- `UdfAdmissionPolicy`
- `UdfAdmissionReport`
- `AdmittedUdf`
- `RejectedUdfCandidate`
- `W093RegistrationRequest`
- `W093SourceIdentity`
- `W093CallableMetadata`
- `W093InvocationTarget`
- `W093Capability`

The policy consumes neutral `ProjectReflection` only. It does not mutate registry state, implement formula precedence, or add compiler/runtime UDF metadata.

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| Example host admits/rejects functions through host-owned policy. | `udf_policy_example_tests::host_owned_policy_admits_public_functions_and_projects_w093_shape` admits a public scalar Function; rejection test covers excluded shapes. |
| Subs, private functions, class methods, and unsupported signatures are rejected by policy, not compiler metadata. | `udf_policy_example_tests::host_owned_policy_rejects_non_admitted_shapes` asserts `POLICY-NOT-FUNCTION`, `POLICY-NOT-PUBLIC`, `POLICY-CLASS-MEMBER`, and `POLICY-RETURN-TYPE`. |
| W093 projection has source identity, callable metadata, invocation target, capability, and change facts. | The admission test asserts `source_identity.callable_id`, public metadata, `TypedScalarFirstTier` invocation target, capability policy name, and `host-udf-policy` change fact. |
| Changing host policy changes admission output without changing neutral descriptors. | `udf_policy_example_tests::changing_host_policy_changes_admission_without_changing_descriptors` toggles `allow_option_private_modules`, changes admission output, and verifies descriptor fingerprint is unchanged. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-host --test udf_policy_example_tests -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- UDF admission is explicitly outside compiler/runtime facts and operates only as an example host projection over neutral descriptors.
- Rejections are policy results, not compiler metadata flags.
- W093-shaped data is a registration request shape only; no registry mutation or formula name precedence is implemented here.
