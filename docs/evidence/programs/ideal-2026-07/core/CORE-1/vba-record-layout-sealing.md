# CORE-1 VbaRecord layout sealing

Date: 2026-07-11

Bead: `bd-59co.2.2.15`

Effect: delivery

Result: accepted implementation slice; capability profile remains in-progress

## Contract and authority

This slice advances `RUNTIME-VALUE-001`, `RUNTIME-ABI-001`,
`SEC-BOUNDARY-001`, and `CONF-QUALITY-001`.

The VBA-compatible admission bounds use Microsoft's published VBA rules:

- [Fixed or static data can't be larger than 64K](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/fixed-or-static-data-can-t-be-larger-than-64k) states that a user-defined type cannot exceed 64 KiB and separately identifies the procedure-local 32 KiB declaration rule.
- [Too many dimensions](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/too-many-dimensions) states that a VBA array cannot have more than 60 dimensions.

The runtime therefore admits a sealed record type payload of at most 64 KiB
and an inline fixed-array rank of at most 60. Declaration-context rules such as
the 32 KiB procedure-local limit remain compiler-owned; the runtime does not
invent a context when it receives a type layout.

## Delivered contract

- `VbaRecordFieldLayout` is now read-only outside its defining module. Its
  name, kind, offset, size, and alignment are exposed through accessors, so an
  external caller cannot fabricate or mutate a descriptor that purports to be
  layout-produced.
- Raw field projection now accepts `VbaRecordFieldHandle`, an indexed handle
  carrying the owning `Arc<VbaRecordLayout>` identity. A handle from an
  independently constructed, structurally equal layout is rejected.
- Before pointer arithmetic, the record validates, in order: layout identity,
  field index, kind-derived size/alignment, non-zero power-of-two alignment,
  checked field extent, sealed layout extent, and owned-buffer extent. The raw
  pointer is formed only after all checks pass.
- Layout construction performs a no-new-allocation shape preflight. Alignment,
  offsets, fixed-string bytes, fixed-array element counts, strides, products,
  and final size use checked arithmetic. Zero-sized fields/elements, zero-count
  bounds, rank above 60, arithmetic overflow, and payloads above 64 KiB fail
  before the field table or record buffer is allocated.
- Accepted field tables use fallible `try_reserve_exact`; accepted record
  buffers are capped at 64 KiB and use fallible reservation before zero-fill.
- Existing runtime, SAFEARRAY, Variant, and Windows COM record-layout callers
  now use handles/accessors. A whole-workspace all-target check proves there is
  no remaining repo caller of the forgeable pointer contract.

## Observable evidence

| surface | evidence |
|---|---|
| Result | Valid GUID-shaped, nested-record, fixed-array, fixed-string, scalar, Variant, and BSTR layouts retain their existing offsets and behavior. A valid nested/fixed-array layout is projected field-by-field with aligned, non-overlapping extents. The exact 64 KiB upper bound allocates and drops successfully. |
| Full Err | This is pre-execution runtime-carrier admission, so no VBA `Err` object exists or is mutated. Rejection is a deterministic `Result::Err(String)`. Covered exact errors include cross-layout handle, forged index, fixed-string multiplication overflow, zero-length fixed string, rank 61, fixed-array product overflow, invalid alignment, alignment overflow, and the 64 KiB bound. |
| Side effects | Rejected layouts do not create a record buffer. Rejected forged/cross-layout handles do not form a field pointer. Test-only per-thread event probes assert both properties without interference from parallel tests. |
| Lifecycle/event order | Successful construction records allocation before initialized use; `Drop` records one matching free after field cleanup. The maximum-size record and cross-layout probe each show one allocation and one free. Nested materialization shows equal allocation/free deltas. No event dispatch is involved. |
| Transport | The affected transport is the in-process native record-buffer projection. Layout instance identity and field index travel together in the handle; COM descriptor comparison remains read-only and continues to match native scalar offsets. No COM call or external native invocation is claimed by this slice. |
| Balance | The focused tests record per-thread record-buffer allocation/free events. Rejection deltas are exactly `(0, 0)`; successful bounded cases have equal allocation/free deltas. The complete runtime suite also remains green. |

## Checks

- `cargo test -p oxvba-runtime vba_record_layout_sealing -- --nocapture` — 4 passed.
- `cargo test -p oxvba-runtime vba_record -- --nocapture` — 19 passed.
- `cargo test -p oxvba-runtime -- --nocapture` — 168 passed, including doc tests.
- `cargo test -p oxvba-com native_vba_record_layout -- --nocapture` — 3 passed; neighboring integration targets had no matching tests and remained green.
- `cargo check -p oxvba-com --all-targets` — passed.
- `cargo check --workspace --all-targets` — passed; one pre-existing HAL dead-code warning remained outside this bead and did not affect the check.
- `cargo clippy -p oxvba-runtime --all-targets -- -D warnings` — passed.
- `cargo fmt -p oxvba-runtime -- --check` — passed.
- `cargo +nightly miri test -p oxvba-runtime vba_record_layout_sealing -- --nocapture` — 4 passed. Miri emitted one existing permissive-provenance warning in `variant.rs::bytes_to_raw_record_payload`; it did not report undefined behavior in the changed layout/field-pointer path.

## Residual disposition

- No accepted residual remains in the layout/field-access safety scope of this
  bead.
- The focused Miri run exposed an unowned strict-provenance residual at
  `crates/oxvba-runtime/src/variant.rs::bytes_to_raw_record_payload`: the record
  payload pointer is reconstructed through `u64 -> usize -> *const
  RecordPayload`. Miri warns that this can hide pointer bugs unless the path is
  moved to strict-provenance APIs or its carrier design is changed. The
  controller must assign this exact unsafe-carrier residual a successor before
  closing the audit lane; it is not suppressed or counted as layout-sealing
  completion.
- Compiler diagnostics for the published 32 KiB procedure-local UDT variable
  rule and module aggregate/static-data limits are not certified here. They
  belong to the compiler conformance lanes because the runtime layout has no
  declaration context. This is not used to broaden a compiler compatibility
  claim.
- Transactional replacement of owning record fields remains owned by
  `bd-59co.2.2.16`; unwind-safe projection of borrowed BSTR/SAFEARRAY carriers
  remains owned by `bd-59co.2.2.17`. This slice neither weakens nor claims those
  sibling outcomes.
- Foreign COM `VT_RECORD` admission and descriptor provenance remain Windows
  interop work. This slice proves only that the OxVba `VbaRecord` carrier cannot
  be driven out of bounds by a forged local layout descriptor.
