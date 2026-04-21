# Correctness Result

Status: in-progress

Current result:

1. paired boundary correctness is green for the currently migrated string seams
2. string conformance is not yet green repo-wide
3. no old/new-only divergence was observed in the boundary bundle.

Canonical paired correctness artifacts:

1. `vmd6-corr-boundary-final`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmd6-corr-boundary-final/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmd6-corr-boundary-final/comparison/correctness_summary.md`

Paired boundary lanes covered:

1. `pointer_helpers`
2. `native_string`
3. `dispatch_boundary`
4. `dispatch_exception_details`
5. `dispatch_exception_resume_next`
6. `dispatch_exception_rich_excepinfo`

Authority-hierarchy classification:

1. boundary lanes:
   - baseline tag and migrated head both passed the selected string-sensitive
     pointer/native/COM boundary lanes
   - current classification: no migration regression observed in the covered
     boundary surface
2. `string_slice_ops_dollar.bas`:
   - the string-focused VM conformance subset failed on both the fixed baseline
     and current `HEAD`
   - observed mismatch: expected slots `12,45,234`; actual slots `0,0,0`
   - current classification: pre-existing OxVba correctness bug, not a
     migration-specific divergence
   - hierarchy outcome: VBA/conformance expectation remains authoritative over
     both old and new OxVba behavior.

Blocked or deferred rows:

1. full string conformance cannot yet be called green because
   `string_slice_ops_dollar.bas` still fails on both baseline and current
   `HEAD`
2. the string migration lane can continue, but final correctness closure for
   the string family still depends on fixing that repo-wide semantic gap.

See also:

1. [LATEST_ARTIFACT_MAP.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/LATEST_ARTIFACT_MAP.csv)
