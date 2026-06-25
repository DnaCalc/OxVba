# ByRef As Any Record Marshaling Evidence - 2026-06-25

## Scope

This slice replaces the old GUID-shaped `ByRef As Any` UDT special case with
descriptor-backed native `VbaRecord` staging for records whose fields are plain
native ABI storage:

- numeric primitive fields;
- `Boolean`;
- nested UDT records;
- fixed arrays of supported field shapes.

Records with owning `String` or `Variant` fields still decline for this native
pointer lane. Record `VarPtr`/source-record `As Any` addressability and fixed
bounds expressed through named constants remain residual work.

## Evidence

- `cargo test -p oxvba-runtime --lib vba_record`
- `cargo test -p oxvba-host --test native_declare_lane --quiet`
- `cargo test -p oxvba-symbol --quiet`
- `cargo test -p oxvba-bind --quiet`
- `cargo check -p oxvba-symbol -p oxvba-bind -p oxvba-vm2 -p oxvba-hal -p oxvba-host --tests`

Covered behavior:

- fixed-array UDT field metadata reaches `VbaRecordFieldKind::FixedArray`;
- fixed-array record fields project to array values and write back into inline
  record storage;
- real `ole32!IIDFromString` writes a GUID UDT through the general record path;
- a nested UDT with fixed byte-array tail is written through `ByRef As Any` from
  a bounded native buffer;
- String-containing records decline deterministically before native invocation.

## Fresh-Eyes Review

Review looked for blunders, mistakes, oversights, omissions, logical gaps,
misconceptions, hidden assumptions, regressions, and bugs.

Findings and rework:

- The first host command used a Cargo name filter and did not run the integration
  test binary; reran with `--test native_declare_lane`.
- Fixed-array fields were initially still mapped as `Variant`; added fixed-array
  metadata in `VarTypeRef`, `ArrayElementType`, binder layout mapping, and VM
  record layout mapping.
- Inline fixed-array fields initially could not participate in normal field
  syntax; added SAFEARRAY projection/readback over inline storage.
- The general UDT test initially depended on source-record `ByRef As Any`, which
  is a separate addressability residual; reshaped it to copy from a bounded
  native byte buffer into the UDT destination.
- The test also used `LenB(record)`, which is still not the subject of this
  slice; replaced it with the descriptor-known 16-byte fixture size.
- The full binder suite exposed regressions in existing fixed-array UDT tests:
  inline `Single` fixed-array fields needed numeric write coercion, and indexed
  fixed arrays of UDT elements needed to preserve their element type through
  binder place/member access. Both were fixed and the full bind suite passed.

After rework, the targeted runtime and host integration checks passed.
