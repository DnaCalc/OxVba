# Value Substrate, Numeric, And UDT Cleanup Workset

Status: `planned`
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

- `value-clean-001`: convert remaining numeric helper families to `Variant`.
- `value-clean-002`: write mixed numeric result matrix and tests.
- `value-clean-003`: pin `Currency`/`Decimal` exactness expectations.
- `value-clean-004`: design descriptor-backed UDT semantic model.
- `value-clean-005`: classify UDT/native ABI residuals.

## Terminal Gate

This workset is complete when native-facing compiler/runtime planning can rely
on:

- retained `Variant` as canonical slot/snapshot carrier,
- exact numeric behavior or explicit residual classification,
- descriptor-backed UDT semantics or an approved implementation path,
- native ABI layout as a separate materialization layer.

