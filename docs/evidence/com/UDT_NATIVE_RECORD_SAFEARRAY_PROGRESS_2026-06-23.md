# UDT Native Record And SAFEARRAY(VT_RECORD) Progress

Date: 2026-06-23

## Result

This pass advances the UDT/native-record lane from array-only native record
storage to a unified scalar/array UDT storage path:

- `CoreValue::NewRecord` and `Op::NewRecord` now carry recursive field layout
  metadata instead of a field count.
- VM scalar UDT initialization now creates native `VbaRecord` values, matching
  the UDT array element storage path.
- Live Windows typelib record descriptors now carry optional record layout
  evidence: record size, field names, offsets, field kinds, and an
  unknown-field marker.
- `SAFEARRAY(VT_RECORD)` projection now admits native VBA record arrays only
  when descriptor layout proof matches the runtime layout. Field population uses
  `IRecordInfo::PutField`, not raw byte copying, so BSTR/VARIANT ownership is
  independent across the COM boundary.

## Current Boundaries

The COM projection path is deliberately strict:

- missing descriptor layout declines;
- unknown descriptor fields decline;
- layout size, field count, offset, or scalar kind mismatches decline;
- nested records, fixed-array fields, and object/interface record fields remain
  explicit future work until their ownership/writeback semantics have tests.

## Checks

- `cargo test -p oxvba-vm2 --test linearize_roundtrip --quiet`
- `cargo test -p oxvba-bind --test feature_coverage --quiet`
- `cargo check -p oxvba-com --tests`
- `cargo test -p oxvba-com windows_variant::tests::native_vba_record_layout --quiet`
- `cargo test -p oxvba-com windows_typelib_loader::tests::testeventserver_typed_record_safearray_descriptors_carry_record_info --quiet`

## Fresh-Eyes Review

Review focused on descriptor proof strictness, owned-field copying, legacy
SAFEARRAY-backed record compatibility, and false closure language. One issue was
found and fixed: failed field conversion now clears any partially initialized
Windows `VARIANT` before returning. After rework, the focused checks above pass.
