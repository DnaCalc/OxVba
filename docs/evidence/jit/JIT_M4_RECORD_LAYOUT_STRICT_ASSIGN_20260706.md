# JIT M4-7 Record Layout Strict Assign Evidence

Date: 2026-07-06

Scope: `bd-h4oh.9.1` / M4-7 record-layout identities for strict `Assign`
normalization.

## Landed

- `assign_repr_preserving` no longer treats `Object(Untyped) <- Variant` as a
  representation-preserving assignment.
- OxIR elaboration threads harvested UDT record-layout ids into locals,
  parameters, and globals declared as the UDT name.
- `CoreValue::NewRecord` assigned into a known UDT destination now materializes a
  `Record(layout)` temp, including global destinations, so strict assign
  verification does not depend on legacy untyped object carriers.
- Inline UDT fixed-array field shape remains descriptor-owned through
  `ArrayElementType::FixedArray` inside the enclosing record layout; it is not a
  standalone SAFEARRAY-shaped `OxTy`.

## Checks

- `cargo test -p oxvba-oxir new_record_assignments_use_record_layout_identity -- --nocapture`
- `cargo test -p oxvba-oxir object_untyped_from_variant_is_not_repr_preserving -- --nocapture`
- `cargo test -p oxvba-oxir -- --nocapture`
- `cargo test -p oxvba-differential --test record_array_access_vm3 -- --nocapture`
- `cargo test -p oxvba-differential --test compound_place_vm3 -- --nocapture`
- `cargo test -p oxvba-differential --test fixed_array_erase_vm3 -- --nocapture`
- `cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`
- `br lint bd-h4oh.9.1 --json`
- `br dep cycles --json`

The staged-scope PowerShell guard could not run in the local Linux container
because `pwsh` is not installed.

## Existing Oracle Evidence

- `docs/evidence/conformance/vm3_record_array_field_oracle_20260702T_bdvt0r_bounds/summary.md`
  covers UDT fixed-array field bounds against Excel/VBA.

## Golden Refresh Note

The first `vm3_golden_snapshot` run exposed a pre-existing stale row for
`conformance/jit_v2/tracer_bullets/tb05_safearray_foreach_bounds.bas`: clean
HEAD (`a9f38990`) failed on the same row before this bead's record-layout
changes. The row now records the binder diagnostic for `For Each v As Long In
a()`. This matches Microsoft VBA documentation for the diagnostic, which states
that array `For Each` control variables must be `Variant`:
`https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/for-each-control-variable-must-be-variant-or-object`.

## Boundary

Record-field value reads remain conservatively `Variant`-typed at the `RecordGet`
result because `CorePlace::RecordField` does not carry the base record-layout id.
The runtime record instructions still consume the descriptor-backed layout for
field offsets and inline fixed-array storage.
