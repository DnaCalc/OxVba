# ABI And Layout Matrix

Date: `2026-04-22`

## Paired Correctness Run

- run id: `vmg5-abi-layout-r3`
- baseline ref: `pre-value-model-migration-2026-04-20`
- baseline commit: `dd1c295b2a3d3a1530dd034d9bb4a6b4c38ea57a`
- candidate ref: `HEAD`
- candidate commit: `7d5bba2d15ac36eb9267897a4ee08f1425b1f650`

Paired correctness result:

- all seven selected ABI/layout-sensitive lanes pass on both baseline and
  candidate
- no migration-induced regression remains in the covered UDT, pointer-helper,
  or native writeback rows

Lanes covered:

1. `conformance_vm`
2. `conformance_jit`
3. `pointer_helpers`
4. `native_string`
5. `pointer_variant_scalar_container`
6. `pointer_variant_decimal_container`
7. `native_string_writeback_array_slot`

Artifacts:

- summary csv:
  `docs/evidence/value_model_migration/runs/value_model_correctness_vmg5-abi-layout-r3/correctness_summary.csv`
- comparison summary:
  `docs/evidence/value_model_migration/runs/value_model_correctness_vmg5-abi-layout-r3/comparison/correctness_summary.md`

## Layout Delta Source

The authoritative layout-size comparison for the post-`ObjectRef` migrated
runtime remains:

- `docs/evidence/value_model_migration/runs/value_model_memory_vmf2-mem-identity-smoke/comparison/layout_metrics.csv`

That comparison already captures the representation changes that matter for the
ABI/layout matrix:

1. `Variant`: `16 -> 80` bytes
2. `ObjectIdentityCarrier`: `4/4 -> 8/8`
3. `ComCallbackPayload`: `40 -> 48` bytes
4. `RuntimeValue`: unchanged at `64/8`
5. `ComValue`: unchanged at `64/8`

These are observable representation deltas, not unresolved bugs.

## Intentional Old/New Boundary Differences

The remaining old/new differences in the ABI/layout lane are intentional
migration outcomes and are already classified by the authority hierarchy.

1. `ObjPtr` and runtime object pointer truth
   - old baseline exposed token-era object identity
   - migrated runtime exposes real runtime `IUnknown` pointer truth
   - classification: intentional improvement required by the new object model
2. `VarPtr(Variant)` object payload
   - old baseline explicitly rejected object-valued container materialization
   - migrated runtime materializes `VT_UNKNOWN`
   - classification: intentional improvement aligned with COM/VBA boundary truth
3. `VarPtr(Variant)` array payload
   - old baseline explicitly rejected array-valued container materialization
   - migrated runtime materializes `VT_ARRAY | VT_VARIANT`
   - classification: intentional improvement aligned with Automation boundary truth

The detailed evidence for those boundary deltas is in:

- `docs/evidence/value_model_migration/POINTER_HELPER_ABI_CELL_RECONCILIATION_2026-04-22.md`
- `docs/evidence/value_model_migration/NATIVE_DECLARE_WRITEBACK_RECONCILIATION_2026-04-22.md`
- `docs/evidence/value_model_migration/UDT_LAYOUT_BOUNDARY_STATUS_2026-04-22.md`

## Matrix Interpretation

1. The bounded non-boundary UDT subset remains preserved after the migration.
2. ABI-sensitive pointer/native rows covered by the paired correctness run do
   not regress.
3. The remaining observable ABI/layout differences versus the old baseline are
   representation upgrades required by the Windows/VBA/COM-aligned migration,
   not unclassified regressions.
4. Broad native struct-overlay parity and unconstrained UDT-byref native ABI
   parity remain explicitly bounded outside this closed matrix.
