# Non-HAL Completion Milestone (`v166`)

## Scope
- Ladder: `v147..v166`
- Terminal profile: `mvp-profile-v166`

## Exit criteria
1. Non-HAL rows in coverage/spec/runtime checklists are reconciled to implemented status where deterministic execution is available.
2. Remaining oracle-dependent non-HAL questions are tracked in Deferred Oracle Gates with explicit foldback notes.
3. VM/JIT conformance matrix remains green with current corpus.

## Evidence Anchors
- Coverage index: `docs/evidence/language/COVERAGE_INDEX.csv`
- Library checklist: `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
- Spec checklist: `docs/evidence/SPEC_CHECKLIST.md`
- Deferred Oracle Gates: `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
- Integrated gate artifacts: `docs/evidence/profiles/v166/`

## Notes
- Formal lane remains non-blocking per current run policy; unresolved Kani items continue through deferred gates.
- HAL-adjacent semantics remain out of scope for this ladder and carry forward to HAL planning/implementation phases.
