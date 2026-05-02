# Value Substrate, Numeric, And UDT Cleanup Workset

Status: `in-progress` (recovery audit reopened)
Date: 2026-04-30; recovery update 2026-05-02
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
- Superseded RuntimeValue bridge blocker register:
  [`../evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md)
  (historical only after `bd-0w46`; active Rust source now has no RuntimeValue
  bridge APIs)
- Variant-native numeric helper evidence:
  [`../evidence/native_ready/VARIANT_NATIVE_NUMERIC_HELPERS_2026-05-01.md`](../evidence/native_ready/VARIANT_NATIVE_NUMERIC_HELPERS_2026-05-01.md)
- Mixed numeric matrix evidence:
  [`../evidence/native_ready/MIXED_NUMERIC_MATRIX_2026-05-01.md`](../evidence/native_ready/MIXED_NUMERIC_MATRIX_2026-05-01.md)
- Exact carrier expectations:
  [`../evidence/native_ready/EXACT_CARRIER_EXPECTATIONS_2026-05-01.md`](../evidence/native_ready/EXACT_CARRIER_EXPECTATIONS_2026-05-01.md)
- Descriptor-backed UDT semantic model path:
  [`../evidence/native_ready/UDT_DESCRIPTOR_MODEL_PATH_2026-05-01.md`](../evidence/native_ready/UDT_DESCRIPTOR_MODEL_PATH_2026-05-01.md)
- UDT native ABI residual classification:
  [`../evidence/native_ready/UDT_NATIVE_ABI_RESIDUAL_CLASSIFICATION_2026-05-01.md`](../evidence/native_ready/UDT_NATIVE_ABI_RESIDUAL_CLASSIFICATION_2026-05-01.md)
- Phase-2 RuntimeValue/IR search gate feeding this workset:
  [`../evidence/native_ready/RUNTIMEVALUE_IR_SEARCH_GATE_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_IR_SEARCH_GATE_2026-05-01.md)
- Recovery audit:
  [`../evidence/native_ready/NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md`](../evidence/native_ready/NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md)

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
  bridge methods or record an explicit public-API blocker. Reopened/superseded
  by `bd-0w46`; bridge methods and active RuntimeValue source references are now
  removed, so this bead must be updated or closed under the stronger result.
- `bd-9xmu.3.3` / `value-clean-002`: lock value substrate spec
  boundaries against the post-phase-2 baseline. Reopened for recovery update;
  the spec now names `Variant` as canonical and active Rust `RuntimeValue`
  residuals as removed, while phase-3 executable proof remains open.
- `bd-9xmu.3.4` / `value-clean-003`: migrate remaining numeric helper
  families to retained `Variant` carriers on the normal path. Reopened for
  recovery audit; the post-`bd-0w46` code must prove Variant-native numeric
  helper behavior with executing tests.
- `bd-9xmu.3.5` / `value-clean-004`: expand mixed numeric result matrix
  and tests. Reopened for recovery audit; `mixed_numeric_matrix_current_variant_results`
  currently filters to zero tests and must be restored or replaced.
- `bd-9xmu.3.6` / `value-clean-005`: pin exact `Currency`, `Decimal`,
  `Date`, and Boolean carrier expectations. Done 2026-05-01; expectations are
  recorded and typed SAFEARRAY exact-carrier regression coverage was added.
- `bd-9xmu.3.7` / `value-clean-006`: design descriptor-backed UDT semantic
  model. Done 2026-05-01; `UdtTypeId`/`UdtFieldId`, descriptor contents,
  retained-Variant field-slot storage, copy/init rules, and implementation path
  are recorded.
- `bd-9xmu.3.8` / `value-clean-007`: classify native ABI UDT materialization
  residuals. Done 2026-05-01; internal descriptor-backed UDT semantics are
  separated from future native ABI materialization, with accepted/deferred/
  blocked rows recorded.
- `bd-9xmu.3.9` / recovery: reprove Variant-native value/numeric/UDT gates
  after RuntimeValue removal. Open 2026-05-02; this is now the active terminal
  recovery bead for phase 3.

## Terminal Gate

This workset returns to complete when native-facing compiler/runtime planning
can rely on:

- retained `Variant` as canonical slot/snapshot carrier,
- exact numeric behavior or explicit residual classification backed by executing
  tests,
- descriptor-backed UDT semantics or an approved implementation path backed by
  executing tests/evidence,
- native ABI layout as a separate materialization layer.

Recovery blocker: previous completion evidence cited tests that now filter to
zero after the RuntimeValue compatibility deletion. Reopened beads must restore
or replace that coverage before this workset can be called complete again.

