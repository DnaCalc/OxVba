# CORE-1 transactional VbaRecord writes

Date: 2026-07-11

Bead: `bd-59co.2.2.16`

Baseline: `2699082f`

Effect: delivery

Result: accepted runtime-safety slice; the Core capability profile remains
in-progress

## Contract and counterexample

This slice advances `RUNTIME-VALUE-001`, `SEC-BOUNDARY-001`, and
`CONF-QUALITY-001`.

An owning record mutation must not destroy the live destination and then run a
fallible clone into the resulting hole. That ordering made an allocation error
leave a dangling or partially initialized destination; an unwind could then
double-drop its previous BSTR, Variant, nested record, or fixed-array payload.
The same counterexample existed for a native record SAFEARRAY element, where
the old raw record was dropped before `clone_into_raw` could fail. Incremental
raw record construction also wrote successful prefix fields before a later
clone failure, making the caller's notion of whether the raw slot was live
depend on the exact failure point. Fresh non-author review then found the same
ordering one level out: intrinsic and record SAFEARRAY constructors tracked an
initialized raw-payload prefix with a local counter and explicit `Err` cleanup,
but no owner survived an unwind. A panic after the first element or after the
complete payload but before header adoption leaked the prefix and allocation.

The delivered invariant is prepare, commit once, then clean up the former
owner. Before the commit, the destination remains byte-for-byte unchanged and
fully live. After the commit, it owns the complete replacement and a temporary
guard owns the complete former value. A returned error or unwind before commit
drops only complete temporary owners.

## Delivered design

- `VbaRecord::new_default` now establishes the complete record invariant in
  one zero-filled allocation. Every admitted field has a valid zero default:
  numeric and fixed storage are zero, String is a null BSTR, Variant is
  `VT_EMPTY`, and nested records/fixed arrays recurse over the same defaults.
  There is no fallible per-field initialization phase and therefore no partial
  record for `Drop` to observe.
- `VbaRecord::try_clone` clones into a complete default record. Each field is
  replaced only after its owned clone is ready; an error or unwind drops the
  valid default/partial-success record through its ordinary destructor.
- Variant fields prepare fallible `value.try_clone()` before `ptr::replace`;
  String fields
  validate the source and prepare the owned BSTR before replacing the carrier.
  The former Variant/BSTR is dropped only after the slot owns its replacement.
- `OwnedFieldBuffer` is an aligned, all-zero, fully droppable owner for one
  nested record or fixed-array field. Recursive record fields and every fixed
  array element are cloned into this guard. One non-fallible byte swap commits
  the complete field; the guard then drops the complete former destination.
- Fixed-array assignment stages the entire array field, not each destination
  element. Source-array and element materialization finish before staging, all
  staged elements finish before commit, and one swap publishes the result.
  Failure at any element therefore preserves every destination element and its
  padding as one unchanged byte image.
- `VbaRecord::clone_into_raw` first creates a complete owned clone. Only after
  every fallible boundary succeeds does `move_into_raw` copy the full payload
  into the caller's uninitialized storage and release only the temporary Vec
  and layout owners. The destination is untouched on failure.
- `PayloadConstructionGuard` owns every nonempty intrinsic, Variant, record, or
  zeroed-scalar SAFEARRAY payload immediately after raw allocation. Its
  initialized count advances only after a complete element clone. On `Err` or
  unwind it drops exactly that live prefix and deallocates once. The guard stays
  armed after the payload is complete, across header validation/allocation and
  the explicit header-adoption boundary, and disarms only after the descriptor
  owns `pvData`.
- `Variant::try_clone` supplies the fallible substrate used by record fields
  and intrinsic Variant SAFEARRAY encoding/replacement. BSTR, object,
  SAFEARRAY, COM-record, and VbaRecord payload families keep their public Clone
  behavior, while constructor APIs returning `Result` now propagate injected
  clone errors instead of crossing a `Clone::expect` panic seam. The current
  SAFEARRAY public Clone adapter remains represented by a pre-borrow boundary
  so this change does not absorb the separate borrowed-projection work.
- Native `SAFEARRAY(VT_RECORD)` element replacement now borrows the source
  `VbaRecord`, prepares a complete `try_clone`, and swaps it with the live raw
  element. The SAFEARRAY descriptor and payload allocation never move; the
  temporary record drops the old element after the array owns the new one.
- `Variant::vba_record_ref` is the internal non-cloning projection used by the
  transactional consumers. The public `as_vba_record` behavior remains a deep
  clone, now layered on that projection.

The transaction buffers are `Vec<u64>` storage. The sealed x64 record
vocabulary has no field alignment above eight bytes; construction nevertheless
checks the requested alignment and rejects a future unsupported larger shape
rather than fabricating an under-aligned guard. Only the exact sealed field or
layout extent is swapped.

## Deterministic failure and unwind proof

Test-only, per-thread injection records every record-owned clone/allocation
boundary and can make the Nth boundary either return a deterministic `Err` or
panic. Each test first records the successful boundary sequence, then reruns
the operation once for every sequence position in both modes.

The sweep covers:

- a Variant field whose replacement owns a SAFEARRAY containing BSTR and
  object payloads;
- a variable-length String field;
- a nested record owning both String and Variant fields;
- a fixed array of nested owning records, with the complete fixed field as the
  transaction unit;
- a raw record containing String, Variant/SAFEARRAY, nested record, and fixed
  nested-record array fields; and
- a native record SAFEARRAY element containing String, Variant/object, and a
  nested owning record;
- a two-record SAFEARRAY constructor, including returned-error and panic paths
  after a nonempty initialized record prefix and after the complete two-record
  payload at header adoption; and
- an intrinsic Variant SAFEARRAY constructor whose four elements own a BSTR,
  object, nested SAFEARRAY containing BSTR/object, and VbaRecord. Every element,
  nested owned clone, partial-prefix, and final header-adoption boundary is
  swept in both modes.

For fixed-array source materialization, the existing infallible compatibility
SAFEARRAY clone and element projection are represented by one explicit
pre-boundary each. Nested injection is suppressed only inside those test-time
compatibility adapters so this bead does not inject after construction of a
borrowed raw SAFEARRAY wrapper; unwind-safe borrowed wrapper projection remains
the separate `bd-59co.2.2.17` contract. The staged record/element clones after
materialization retain their individual Nth-boundary coverage. Production code
does not suppress any operation.

Every injected `Error` boundary is required to return `Err`, and every injected
`Panic` boundary is required to unwind. After either result, tests compare the
complete destination record or raw-element bytes with the pre-call image, then
read all old semantic values to prove the destination is still live. Raw
construction uses sentinel-filled storage and proves no byte changes on every
failure. Record SAFEARRAY tests additionally prove descriptor address, payload
address, rank, flags, element size, lock count, and bounds remain unchanged.
Successful controls prove the complete new semantic values.

The constructor test uses per-thread BSTR, object-box, SAFEARRAY, and VbaRecord
live counts plus independent raw-payload allocation/free events. It asserts the
same starting live-handle vector after every individual Nth-boundary run and an
equal payload allocation/free delta. These probes remain deterministic even
when unrelated unit tests run concurrently.

## Observable evidence

| surface | evidence |
|---|---|
| Result | Successful Variant, String, nested-record, fixed-record-array, raw-clone, record-SAFEARRAY writes, and intrinsic/record SAFEARRAY construction expose the complete replacement. Record SAFEARRAY replacement retains the same descriptor and payload addresses. |
| Full Err | This carrier-level slice does not seat or mutate VBA `Err`. Deterministic clone/allocation failures return `Result::Err(String)` at every fallible boundary; the sweep rejects an unexpected success or error-mode unwind. The exact old semantic value remains readable after each error. |
| Side effects | Before commit, complete record bytes, raw sentinel storage, and SAFEARRAY descriptor/element bytes are unchanged. The commit is one `replace` for a leaf owner or one byte swap for a composite owner. Old-owner cleanup occurs only after the destination owns the replacement. |
| Lifecycle/event order | Mutation: prepare complete default owner -> clone all owned children -> one non-fallible commit -> drop complete former owner. Construction: allocate armed payload -> clone and count each complete element -> allocate/initialize header -> header adopts payload -> disarm guard. Error/unwind drops only the guard's exact live prefix. |
| Transport | Native raw record storage and `SAFEARRAY(VT_RECORD)` payloads retain the sealed x64 layout. Raw construction transfers the exact layout bytes only after full success; no wrapper format, descriptor ABI, field offset, or Variant carrier changes. |
| Balance | Mutation tests assert equal per-thread VbaRecord event deltas. Constructor sweeps assert exact per-thread BSTR, object-box, SAFEARRAY, and VbaRecord live-count restoration plus equal raw-payload allocation/free deltas after every Nth error and panic. Exact Miri reports no invalid access, uninitialized read, leak, double-drop, or other undefined behavior. |

## Checks

- `cargo test -p oxvba-runtime vba_record_transactional_write -- --nocapture --test-threads=1` — 1 focused test passed; every traced error and panic boundary was exercised.
- `cargo test -p oxvba-runtime safe_array_record_transactional_write -- --nocapture --test-threads=1` — 1 focused test passed; every traced error and panic boundary was exercised.
- `cargo test -p oxvba-runtime safe_array_payload_transactional_construction -- --nocapture --test-threads=1` — 1 focused test passed; two-record and four-Variant constructors swept every returned-error, panic, partial-prefix, and header-adoption boundary with exact handle/payload balance.
- `cargo +nightly miri test -p oxvba-runtime vba_record_transactional_write -- --nocapture --test-threads=1` — 1 passed in 18.79 seconds with no Miri failure.
- `cargo +nightly miri test -p oxvba-runtime safe_array_payload_transactional_construction -- --nocapture --test-threads=1` — 1 passed in 30.68 seconds with no Miri failure.
- `cargo test -p oxvba-runtime -- --test-threads=1` — 173 unit tests, 2 isolated integration tests, and 8 compile-fail doctests passed.
- `cargo clippy -p oxvba-runtime --all-targets --all-features -- -D warnings` — passed with zero warnings.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.

The Miri runs emitted only the already reviewed process-local Exposed
Provenance recovery warnings in `variant.rs` for BSTR, IUnknown, SAFEARRAY, and
record pointer carriers. Miri reported no defect in the transactional record
paths. Those warnings are part of the carrier decision/evidence established by
`bd-59co.2.2.20`, not an unrecorded residual of this slice.

## Residual disposition

- No accepted residual remains in destructive-before-fallible-clone ordering
  for VbaRecord field assignment, nested/fixed composite replacement, raw
  record construction, OxVba-owned record SAFEARRAY element replacement, or
  intrinsic/record/zeroed-scalar SAFEARRAY payload construction through header
  adoption.
- Unwind-safe borrowed BSTR and SAFEARRAY projection remains the deliberately
  separate `bd-59co.2.2.17` outcome. This slice avoids broadening its public
  projection contract and does not claim that work.
- Foreign COM `IRecordInfo`, external SAFEARRAY adoption, and Windows marshalling
  remain Windows interop scope. This slice proves the local OxVba-owned carrier
  transaction and does not make an Excel/VBA or foreign-COM transport claim.
- General allocator exhaustion behavior outside these fallible runtime APIs is
  Rust/platform policy. The deterministic hooks prove the ownership state at
  every runtime clone/allocation boundary; they do not claim that an aborting
  process can unwind.
