## Interface And Event Matrix

Date: `2026-04-22`

Run:

- run id: `vmf6-interface-event-matrix-r3`
- baseline ref: `pre-value-model-migration-2026-04-20`
- baseline commit: `dd1c295b2a3d3a1530dd034d9bb4a6b4c38ea57a`
- candidate ref: `HEAD`
- candidate commit: `59e572a05c5c174a73cba8448a614e2c20bda041`

Result:

- all seven interface/event lanes pass on both baseline and candidate
- no migration-induced divergence remains in the bounded interface/event matrix

Lanes covered:

1. `dispatch_boundary`
2. `events_identity`
3. `dispatch_exception_details`
4. `dispatch_exception_resume_next`
5. `dispatch_exception_rich_excepinfo`
6. `event_callback_handler_body`
7. `event_callback_value_payload`

Artifacts:

- summary csv:
  `docs/evidence/value_model_migration/runs/value_model_correctness_vmf6-interface-event-matrix-r3/correctness_summary.csv`
- comparison summary:
  `docs/evidence/value_model_migration/runs/value_model_correctness_vmf6-interface-event-matrix-r3/comparison/correctness_summary.md`

Required harness reconciliation during this bead:

1. `crates/oxvba-host/tests/com_client_end_to_end.rs`
   - broad VM/JIT snapshot comparisons were still comparing retained `ObjectRef`
     allocation identity directly
   - the test surface now canonicalizes object-valued snapshots by observable
     object identity (`raw()`) before comparing VM and JIT results
2. `crates/oxvba-host/tests/com_early_project_end_to_end.rs`
   - the same stale pointer-equality assumption existed in early-bound project
     snapshot comparisons
   - the snapshot surface now canonicalizes object-valued results before
     equivalence assertions

Interpretation:

1. the migrated `ObjectRef` identity model is compatible with the bounded
   interface/event behavior already covered by the old/new matrix
2. the required test changes were harness-truth fixes, not semantic downgrades
3. VM/JIT equivalence in object-valued interface/event lanes should be judged by
   observable object identity and payload behavior, not by retained pointer
   allocation details inside the test process
