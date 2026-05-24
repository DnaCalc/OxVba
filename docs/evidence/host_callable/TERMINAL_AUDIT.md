# Terminal Audit: Host Callable Reflection Rework

Date: 2026-05-24
Bead: `bd-hjys.15`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`

## Objective restated

Complete the `bd-hjys` host project callable reflection and wrapper generation rework: neutral callable descriptors/invocation, bundle descriptor truth, `VbaHost`, host-owned UDF policy example, wrapper plan profiles, PH-0011 refresh, and removal of deprecated `HostUdf*` API surfaces.

## Child bead state

All child delivery beads are closed:

- `bd-hjys.1` through `bd-hjys.14`: closed.
- `bd-hjys.15`: terminal audit bead, closed by this evidence/check cycle.

No blocker or deferred child is required for this workset terminal condition.

## Terminal condition checklist

| # | Requirement | Evidence |
| --- | --- | --- |
| 1 | Compiler and bundle metadata expose neutral callable facts without embedded UDF policy. | `BD-HJYS.3_NEUTRAL_REFLECTION_DESCRIPTORS.md`, `BUNDLE_DESCRIPTOR_TRUTH.md`; tests `reflect_project` and `bundle`. |
| 2 | In-process hosts can load, reflect, prepare, and invoke through `VbaHost`-style API. | `IN_PROCESS_HOST_API.md`; `vba_host_facade_tests`. |
| 3 | Runtime callable invocation uses neutral context and typed/variant conversion lanes. | `RUNTIME_TYPED_INVOCATION.md`; `vba_host_facade_tests::vba_host_invokes_by_callable_id_with_context_observation_and_typed_lane`. |
| 4 | Bundle descriptor inventory is packaged source of truth where present. | `BUNDLE_DESCRIPTOR_TRUTH.md`; `source_reflection_and_bundle_callable_inventory_match`; bundle prepared-session test. |
| 5 | UDF behavior is host-owned policy outside compiler/runtime. | `HOST_OWNED_UDF_POLICY_W093.md`; `udf_policy_example_tests`. |
| 6 | Generic wrapper-generation infrastructure is evidenced by generated introspection/reflection-caller EXE. | `WRAPPER_PLAN_ABSTRACTIONS.md`, `WRAPPER_GENERATION_EXE.md`; `wrapper_plan` and `reflection_exe` tests. |
| 7 | WrappedNativeLibrary and adjacent wrapper plans consume neutral descriptors. | `WRAPPED_NATIVE_LIBRARY_PROFILE.md`, `COM_XLL_WRAPPER_SUBSTRATE_ALIGNMENT.md`; native wrapper and substrate tests. |
| 8 | XLL remains a future special wrapper profile. | `COM_XLL_WRAPPER_SUBSTRATE_ALIGNMENT.md`; future XLL placeholders assert execution/registration deferred. |
| 9 | DNA Calc host examples avoid duplicated local function mirror/formula precedence ownership. | `DNA_CALC_HOST_CONSUMPTION.md`; DNA Calc host consumption tests. |
| 10 | PH-0011 and related evidence reflect new boundaries. | `PH0011_HOST_CALLABLE_REFRESH.md`; `PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv`; regenerated validation derived summary. |

## Deprecated-surface audit

Command:

```text
rg -n "HostUdf|host_udf|Host UDF|host UDF|RuntimeCallSource::HostUdf" crates -g'*.rs'
```

Result: no deprecated old-shape code references. Remaining `host_calls` references are host export test names, not `HostUdf*` APIs or bundle `host_calls` descriptor inventory.

Historical/audit docs retain old terms only to explain what was removed or superseded.

## Checks run

```text
cargo test -p oxvba-compiler reflect_project -- --nocapture
cargo test -p oxvba-compiler bundle -- --nocapture
cargo test -p oxvba-host --test vba_host_facade_tests -- --nocapture
cargo test -p oxvba-host --test udf_policy_example_tests -- --nocapture
cargo test -p oxvba-host --test dna_calc_host_consumption_examples -- --nocapture
cargo test -p oxvba-build wrapper_plan -- --nocapture
cargo test -p oxvba-build reflection_exe -- --nocapture
cargo test -p oxvba-build dll::wrapper_plan_tests -- --nocapture
cargo test -p oxvba-build substrate_alignment_tests -- --nocapture
cargo check --workspace --all-targets
scripts/check-governance.ps1
br lint / br dep cycles (run through `scripts/invoke-br-serialized.ps1`)
git diff --check
```

All checks passed in this audit cycle.

## Fresh-eyes review

- Bead evidence, code, tests, matrix updates, and workset terminal conditions are mutually aligned.
- No old public `HostUdf*` API surface remains in `crates/`.
- Old PH-0011 HostUdf evidence is superseded, not deleted as provenance.
- No compatibility adapters were added.
- XLL/Excel parity claims remain explicitly deferred.
