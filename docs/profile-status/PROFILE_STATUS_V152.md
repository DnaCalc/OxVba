# PROFILE_STATUS_V152.md

## Profile
- ID: mvp-profile-v152
- Ladder step: v152

## Scope Summary
- UDT/value semantics hardening: deterministic whole-UDT assignment lowering (`b = a`) into flattened field-alias copies.

## Gate Artifacts
- `docs/worksets/PROFILE_LADDER_2026-03-01_MACH1000_V147_V166_NON_HAL_COMPLETION.md`
- `docs/worksets/WORKSET_2026-03-01_UDT_VALUE_SEMANTICS_V152.md`
- `conformance/tests/udt_whole_assignment_copy.bas`
- `conformance/golden/smoke.csv`
- `docs/evidence/language/COVERAGE_INDEX.csv`

## Closure Signals
- Profile is complete when whole-UDT assignment behavior is deterministic in compile/runtime/conformance lanes and associated formal/evidence/profile artifacts are updated.
