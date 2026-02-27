# Formal Run Report

- Timestamp (UTC): 2026-02-27T11:50:06Z
- Profile scope: mvp-formal-foundation-v3
- Overall mode: non-blocking
- cargo-kani: unavailable

| Obligation | Profile | Blocking | Status | Command | Artifact | Note |
|---|---|---|---|---|---|---|
| FO-V2-001 | v2 | no | skipped | cargo kani -p oxvba-vm --harness pc_progression_is_safe_for_valid_jump_target | crates/oxvba-vm/src/interpreter.rs | cargo-kani not available |
| FO-V2-002 | v2 | no | skipped | cargo kani -p oxvba-compiler --harness temp_slots_do_not_overlap_declared_slots | crates/oxvba-compiler/src/emit.rs | cargo-kani not available |
| FO-V3-001 | v3 | no | skipped | cargo kani --version | scripts/run-formal.ps1 | cargo-kani not available |
