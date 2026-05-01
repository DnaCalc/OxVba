# Value Substrate, Numeric, And UDT Cleanup Workset

Status: `in-progress`
Date: 2026-04-30
Parent: `WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`

## Purpose

Make the retained value model precise enough for VM/JIT/wrapper correctness and
future native compilation. This workset turns native compilation into a forcing
function for value, numeric, and UDT clarity without starting the native backend
itself.

## Scope

In scope:

- Make numeric helpers `Variant`-native.
- Define exact behavior for numeric tags and mixed-type operations.
- Preserve exact carriers for `Currency`, `Decimal`, `Date`, and Boolean truth.
- Replace flattened-only UDT thinking with descriptor-backed semantic UDT
  planning.
- Keep native ABI materialization separate from internal semantic UDT storage.

Out of scope:

- Direct native code generation.
- General native UDT-byref ABI parity.
- Arbitrary struct overlay/packing parity.

## Evidence

- Spec lock baseline:
  [`../evidence/native_ready/VALUE_SUBSTRATE_SPEC_LOCK_2026-05-01.md`](../evidence/native_ready/VALUE_SUBSTRATE_SPEC_LOCK_2026-05-01.md)
- RuntimeValue bridge public-API blocker register:
  [`../evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md)
- Variant-native numeric helper evidence:
  [`../evidence/native_ready/VARIANT_NATIVE_NUMERIC_HELPERS_2026-05-01.md`](../evidence/native_ready/VARIANT_NATIVE_NUMERIC_HELPERS_2026-05-01.md)
- Phase-2 RuntimeValue/IR search gate feeding this workset:
  [`../evidence/native_ready/RUNTIMEVALUE_IR_SEARCH_GATE_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_IR_SEARCH_GATE_2026-05-01.md)

## Execution Epics

1. **Value Substrate Spec Lock**
   - Close condition: `NATIVE_READY_VALUE_SUBSTRATE_V1.md` defines the intended
     canonical carriers and residual boundaries.
2. **Numeric Helper Migration**
   - Close condition: arithmetic/coercion helpers operate over `Variant`
     directly without `RuntimeValue` bridges.
3. **Numeric Matrix Expansion**
   - Close condition: tag/result/overflow/null/error/string-coercion behavior is
     recorded for all in-scope numeric families.
4. **UDT Descriptor Model**
   - Close condition: a design and first implementation path exists for
     `UdtTypeId`, field descriptors, copy/init rules, and semantic storage.
5. **ABI Boundary Split**
   - Close condition: native ABI materialization is documented as a separate
     layer from internal UDT semantics.

## First Beads

Rolled out on 2026-05-01 under bead epic `bd-9xmu.3`:

- `bd-9xmu.3.1` / `value-clean-000`: roll out this executable bead path.
- `bd-9xmu.3.2` / `value-clean-001`: retire residual `RuntimeValue`
  bridge methods or record an explicit public-API blocker. Done 2026-05-01;
  residual bridge methods are registered as `RV-BRIDGE-*` blockers with owners,
  removal targets, and unblocking steps.
- `bd-9xmu.3.3` / `value-clean-002`: lock value substrate spec
  boundaries against the post-phase-2 baseline. Done 2026-05-01; the spec
  names `Variant` as canonical, limits `RuntimeValue` to approved residuals,
  and maps phase-3 gates to bead IDs.
- `bd-9xmu.3.4` / `value-clean-003`: migrate remaining numeric helper
  families to retained `Variant` carriers on the normal path. Done 2026-05-01;
  normal VM/JIT helper call-site searches are clean for legacy arithmetic/math
  helpers, and VM/JIT retained-Variant helper tests pass.
- `bd-9xmu.3.5` / `value-clean-004`: expand mixed numeric result matrix
  and tests.
- `bd-9xmu.3.6` / `value-clean-005`: pin exact `Currency`, `Decimal`,
  `Date`, and Boolean carrier expectations.
- `bd-9xmu.3.7` / `value-clean-006`: design descriptor-backed UDT semantic
  model.
- `bd-9xmu.3.8` / `value-clean-007`: classify native ABI UDT materialization
  residuals.

## Terminal Gate

This workset is complete when native-facing compiler/runtime planning can rely
on:

- retained `Variant` as canonical slot/snapshot carrier,
- exact numeric behavior or explicit residual classification,
- descriptor-backed UDT semantics or an approved implementation path,
- native ABI layout as a separate materialization layer.

