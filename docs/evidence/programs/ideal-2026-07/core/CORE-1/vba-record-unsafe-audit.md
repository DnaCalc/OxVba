# `VbaRecord` unsafe ownership audit

Date: 2026-07-11

Bead: `bd-59co.2.2.2`

Current code baseline: `c0ff1de7a61dce2df6c83eff730582f2fd6f969b`

Integrated audit origin: `37811fd5ecb06bbdb9690d3ff5929ff675242f7a`

Host used for the executable checks: Windows `10.0.26200`, x64,
`x86_64-pc-windows-msvc`, Rust/Cargo `1.94.1`.

Status: **not certified**. The 15 focused tests and strict runtime Clippy gate
pass, and no lint suppression was added, but the review confirmed invalid
public layout/field-handle invariants and non-transactional owning-write failure
paths. Independent review also confirmed that borrowed raw BSTR/SAFEARRAY
carriers are temporarily wrapped as owning values and forgotten only after a
fallible or panicking operation, so unwind can destroy the source. P0 delivery
successors `bd-59co.2.2.15`, `bd-59co.2.2.16`, and `bd-59co.2.2.17` own those
separate defects. Existing high-risk successor `bd-59co.2.7.2` owns the hidden
Variant/object thread-transit question.

This is a support-audit result, not capability closure, a workspace-wide gate,
cross-platform evidence, an allocator-failure execution proof, or an Office/VBA
parity claim.

## Scope and contract trace

The audit covers every current production `unsafe {}` block and `unsafe fn` in
`crates/oxvba-runtime/src/vba_record.rs`, its four test-only unsafe blocks, and
the directly coupled ownership routes in `variant.rs`, `bstr.rs`,
`object_ref.rs`, and the record-payload portions of `safe_array.rs`.

It traces to `CONF-QUALITY-001`, `RUNTIME-VALUE-001`, and
`SEC-BOUNDARY-001`, and to canonical row
`CORE-READINESS/CORE-BASELINE-UNSAFE-CLIPPY`.

The current file contains 69 lexical production `unsafe {}` blocks, 12
production `unsafe fn` declarations, no unsafe impl, and four test-only unsafe
blocks. The grouping below covers all of them by owning function/path rather
than treating repeated typed scalar reads and writes as unrelated contracts.

Commit `37811fd5` added invariant comments to the then-current 1,079-line file.
The current file is 1,455 lines; relative to that commit it has 436 additions
and 60 deletions. Later work added direct array-element access, fixed strings,
multidimensional fixed-array bounds, and `LSet` behavior. The original field-
handle, drop-before-clone, and raw-borrow/late-`forget` arguments are still
present, so this review does not certify the integrated comments merely because
strict Clippy accepts their placement.

## Site-by-site findings

| Current site | Ownership/layout finding | Verdict |
|---|---|---|
| `VbaRecordLayout::new`, `VbaRecordFieldKind::storage_shape`, `fixed_array_total_len`, `align_to` (`73-209`, `1008-1025`) | Field-size multiplication and most total-size additions are checked, dimensions must be nonempty/nonzero, and all ordinary carrier alignments are powers of two. However, `align_to` performs unchecked `value + align - 1`. A near-`usize::MAX` public `FixedString { len }`, followed by an aligned field, can panic with overflow checks or wrap before the later `checked_add`; release wrapping can seal overlapping fields and a small record size. Fixed-array stride uses the same helper. Large otherwise-valid layouts also reach infallible `Vec` allocation rather than a bounded/fallible admission gate. | **Invalid**; `bd-59co.2.2.15`. |
| `VbaRecord::new_default` and `init_field_at` (`265-289`, `589-620`) | For a valid sealed layout, `Vec<u64>` supplies the checked native alignment and enough rounded-up bytes. The zeroed allocation is a valid resource-empty baseline for scalar/fixed-string bytes, null BSTR slots, and `Variant::Empty`; recursive initialization preserves that state. The record live counter is incremented only after allocation and is balanced by local `Drop` on unwind. These arguments depend on layout arithmetic being sound. | Conditional on `.15`; ordinary path retained. |
| `VbaRecord::field_ptr` / `field_mut_ptr` (`311-322`) | The safety comment says the supplied `VbaRecordFieldLayout` can only come from this record. That is false: the type and all of `name`, `kind`, `offset`, `size`, and `align` are public. Safe code can forge an arbitrary offset or pass a descriptor from another independently constructed layout, with no identity validation. The safe methods then execute `ptr.add(field.offset)` without membership, range, extent, or alignment validation; an out-of-allocation offset violates `ptr.add`'s contract inside a safe API. | **Invalid**; `bd-59co.2.2.15`. |
| `field_bytes`, `read_field_variant`, `write_field_variant` (`324-349`) | Index lookup uses the record's internal immutable layout, so slice extent and typed alignment follow if the layout is sealed. Safe reads can still lose borrowed BSTR/SAFEARRAY source ownership during unwind, and writes inherit both source-borrow and destination-transaction defects described below. | Bytes conditional on `.15`; reads also blocked by `.17`; owning writes by `.16` and `.17`. |
| `lset_from` (`351-360`) | Both layouts are recursively rejected if they contain variable BSTR or Variant ownership. `ptr::copy` permits self-overlap, copies only the smaller live buffer prefix, and leaves a longer target tail intact. Fixed strings and scalar bit patterns require no destructor transfer. | Retained for sealed layouts; no ownership defect found. |
| `array_field_bounds_len`, `read_array_field_element`, `write_array_field_element` (`363-460`) | Internal field lookup justifies the Variant cast. Fixed-array length is checked, and `flat < len` plus a valid checked layout justifies stride arithmetic. `&mut self` makes the mutable Variant/inline element exclusive. Variant-array bounds/read/write delegate to raw SAFEARRAY borrow projections whose owning temporary can drop the source on early return/unwind. Inline writes also inherit the Variant, nested-record, and partial fixed-array transaction defects from `write_field_variant_at`. | Pointer arithmetic conditional on `.15`; borrowed source blocked by `.17`; owning destination writes also by `.16`. |
| `clone_from_raw` (`463-475`) | The public unsafe precondition requires a live source payload of the exact layout. The newly default-initialized destination can clean its completed fields, but BSTR and Variant cloning reaches owning-wrapper borrow projections. Panic before their late `forget` frees the caller-owned source, so destination RAII does not make the overall clone unwind-safe. | **Invalid source-unwind contract**; `bd-59co.2.2.17` (plus `.15`). |
| `clone_into_raw` and `clone_field_at` (`477-488`, `622-673`) | Scalar/fixed-string bytes are copied, BSTR and Variant fields are deep-cloned, and nested/fixed-array fields recurse. The destination contract is not failure-complete: if one owning field clones and a later BSTR clone returns `Err`, the caller receives a partially initialized raw payload without an initialized-prefix token or rollback route. `alloc_record_payload_from_records` drops only earlier complete records and deallocates the current partial slot, leaking the successful prefix. Separately, BSTR/Variant source cloning uses owning temporary wrappers; panic before `forget` can free the source. The internal `clone_record_into_ptr` comment also calls the destination default-initialized while this public API promises uninitialized storage. | **Invalid destination and source failure contracts**; `.16` and `.17`. |
| `drop_raw`, `VbaRecord::drop`, and `drop_field_at` (`490-502`, `565-577`, `675-714`) | For a fully initialized record dropped exactly once, Variant destruction recursively releases BSTR, object, SAFEARRAY, and record payloads; String reconstructs the uniquely owned BSTR; nested/fixed arrays visit each slot once. The helper deliberately does not clear freed bytes, which is valid only under its single-drop precondition. Drop-before-fallible-clone and borrowed-wrapper unwind sites can both cause later cleanup to violate that precondition. | Retained only for valid states; invalidating callers blocked by `.16` and `.17`. |
| `clone_into_native_words` / `clone_from_native_words` (`504-538`) | Native staging recursively rejects variable String and Variant fields, so accepted layouts contain plain scalars, fixed strings, nested plain records, and fixed arrays. The `u64` buffer is aligned on the checked Windows x64 baseline, size is checked, and no owning cleanup is needed. `clone_from_native_words` remains unsafe because the caller supplies the native image. | Retained on the checked Windows x64 baseline; no cross-platform ABI certification. |
| `VbaRecord::clone` (`541-563`) | Source and destination buffers are distinct, and a zeroed destination can clean its completed prefix. That destination argument does not protect the source: BSTR and Variant deep clones pass borrowed raw carriers through owning wrappers. If cloning panics before the late `forget`, unwinding drops the wrapper and frees the source field; later source cleanup can destroy it again. | **Invalid source-unwind contract**; `.17` (plus `.15`). |
| `read_field_variant_at` (`716-796`) | Scalar reads use their descriptor alignment; fixed-string UTF-16 uses unaligned reads intentionally. For a variable BSTR, line 754 constructs an owning `BStr` from the record-owned raw pointer, calls panicking `text.clone()`, then forgets the wrapper only on success. Unwind frees the source BSTR. Variant, nested-record, and fixed-array reads transit the same BSTR/SAFEARRAY projections. A partially built result vector protects only its new results, not the borrowed source. | **Invalid source-unwind contract**; `.17`. |
| `write_field_variant_at`: scalar and Boolean (`813-878`, `912-918`) | Conversion completes before a direct Copy-carrier write, with no raw owning projection. | Retained for sealed layouts. |
| `write_field_variant_at`: String and FixedString (`879-911`) | The target ordering is prepare-first on the normal path, but `value.as_bstr()` uses `variant::raw_bstr_to_bstr`: it temporarily owns the source Variant's BSTR and can unwind before `forget`. Thus even a failure before target mutation can free the source and make its later drop duplicate destruction. | **Invalid borrowed-source contract**; `.17`. |
| `write_field_variant_at`: Variant (`806-812`) | The current destination Variant is dropped before `value.clone()` is evaluated, so clone panic leaves stale freed destination bits for later drop. The clone can also free the source first when a borrowed BSTR/SAFEARRAY wrapper unwinds before `forget`. | **Invalid destination transaction and source borrow**; `.16` and `.17`. |
| `write_field_variant_at`: nested Record and FixedArray (`919-955`) | The nested-record arm recursively drops the target before fallible `clone_record_into_ptr(...)?`. On BSTR allocation `Err`, unrewritten slots still contain freed old pointers and later record drop repeats destruction; a multi-field target can also contain a mixed new/old partial state. Fixed-array assignment writes elements sequentially and inherits this corruption for Variant/Record elements. Preparatory `as_vba_record`/`as_safearray` clones also transit borrowed raw carriers whose owning wrapper can destroy the source on unwind. | **Invalid destination and source failure contracts**; `.16` and `.17`. |
| `clone_record_from_ptr` / `clone_record_into_ptr` (`960-987`) | The from-pointer wrapper owns a default destination, while the into-pointer helper has no transaction/initialized-prefix representation and is used with both default-initialized and raw destinations. Source BSTR/Variant/SAFEARRAY cloning also has the late-`forget` unwind window, so neither destination RAII nor success-path deep copy seals all failure exits. | **Invalid failure contracts**; `.16` and `.17`. |
| `borrow_bstr_raw` / `clone_bstr_raw` (`989-1005`) | `borrow_bstr_raw` creates an owning `BStr` for a pointer still owned by the record. At line 1002, `clone_bstr_raw` forgets it only after the clone call. A normal `Result::Err` is captured and then forgotten, but any panic before line 1004 drops the temporary and frees the source. The same direct wrapper at line 754 surrounds a clone that explicitly panics on allocation failure. | **Invalid borrowed-source unwind contract**; `.17`. |
| `variant::raw_bstr_to_bstr` (`variant.rs:218-225`) | `Variant::as_bstr`, String `Variant::clone`, and raw-core cloning construct an owning `BStr`, call `BStr::clone` (which can panic), and forget the wrapper only afterward. Unwind frees the BSTR still owned by the source Variant. | **Invalid borrowed-source unwind contract**; `.17`. |
| Test-only blocks (`1241-1250`, `1410-1433`) | Tests derive fields from the same live layout, write the declared carrier types, drop the default Variant before replacement, transfer one owned raw BSTR, and borrow raw String pointers only for comparison. | Retained. |
| `safe_array::raw_bstr_to_bstr` and raw descriptor borrow projections (`safe_array.rs:244-252`, `1514-1640`) | BSTR decode temporarily owns a slot-owned BSTR and calls panicking clone before `forget`. Raw SAFEARRAY clone/read/write helpers similarly construct an owning `SafeArray` for caller-owned storage and forget it only after clone/read/write. Panic frees the caller's descriptor; `raw_safearray_bounds_len` also has a fallible early `?` after wrapper construction. The sibling raw I32 projections share the same late-forget shape. | **Invalid borrowed-source unwind/early-return contract**; `.17`. |
| `SafeArray` record allocation/replacement (`safe_array.rs:881-959`, `1390-1427`) | Contiguous record allocation tracks completed whole records, but not a partial current record from `clone_into_raw`. Record element replacement prepares a cloned `VbaRecord`, then drops the live raw slot and performs a second fallible raw clone. Failure after the drop leaves the SAFEARRAY owning invalid/dangling element bytes that `SafeArray::drop` later traverses. The preparatory source clone can independently hit the `.17` borrowed-carrier unwind defect. This contradicts the earlier SafeArray audit's broad replacement-order statement. | **Invalid**; `.16` for destination transaction and `.17` for source borrow. |
| Variant/Object transit through `VbaRecord` | `Variant` itself is `!Send + !Sync` because its union includes a raw pointer, but `VbaRecord` stores live Variants behind `Vec<u64>` and therefore currently satisfies `Send + Sync`. A `VbaRecord` containing `Variant(ObjectRef)` can cross threads despite compat-object interior state, thread-local termination, and possible foreign COM apartment affinity. Single-thread clone/drop AddRef/Release accounting is intact; thread/apartment ownership is not certified. | Existing owner `bd-59co.2.7.2`. |

## Minimal failure shapes for the P0 delivery routes

These are reachable semantic shapes; the current test harness has no
deterministic allocation-failure injection, so the green tests below do not
exercise them.

### `.16`: destination transaction and partial initialization

1. Direct Variant field: create a record with one `Variant` field holding a
   nonempty String Variant, then replace it with another owning Variant. If the
   replacement clone fails after `drop_in_place`, unwind leaves the old freed
   carrier bits for the record destructor.
2. Nested record field: use one nested record whose layout contains one
   nonempty variable String. Let `value.as_vba_record()`'s preparatory clone
   succeed, then fail the BSTR allocation in the second clone after
   `drop_field_at`. The method returns `Err`; later outer drop frees the stale
   old BSTR pointer again.
3. Record SAFEARRAY element: use a one-element record SAFEARRAY with the same
   one-String layout. Let the preparatory record clone succeed, then fail the
   second clone after `drop_raw`. The safe setter returns `Err`; array drop later
   calls `drop_raw` on the invalid slot.
4. Partial raw initialization: use a two-String record layout. Let the first
   raw-field clone succeed and the second return allocation `Err`. Current
   SAFEARRAY construction deallocates that partial current slot without dropping
   the successful prefix, leaking its BSTR.

The successor needs a scoped failpoint capable of failing the Nth BSTR/Variant/
record/SAFEARRAY clone so each postcondition and allocation balance is
deterministic.

### `.17`: borrowed-source unwind and early return

1. Record String read: create a record with one nonempty variable String and
   force `text.clone()` at `vba_record.rs:755` to panic. The temporary owning
   wrapper drops during unwind and frees the record's BSTR; later record drop
   frees the stale pointer again.
2. String Variant projection: call `Variant::as_bstr` or clone a nonempty String
   Variant and force `BStr::clone` to panic in `variant::raw_bstr_to_bstr`. The
   source Variant loses its BSTR before its own destructor.
3. BSTR SAFEARRAY element read: force the clone in
   `safe_array::raw_bstr_to_bstr` to panic. The temporary wrapper frees the BSTR
   still stored in the array slot; array destruction later repeats the free.
4. Raw SAFEARRAY descriptor projection: call raw clone/read/write on a live
   caller-owned descriptor and force the delegated operation to panic before
   `forget`. The temporary owning `SafeArray` frees descriptor and payload
   during unwind while the caller still holds the raw carrier. The bounds helper
   has the same ownership loss on its post-wrapper fallible early return.
5. Raw record cloning: force an unexpected panic inside the clone call after
   `clone_bstr_raw` constructs its owning temporary but before line 1004. The
   normal returned `Err` path executes `forget`; the unwind path does not.

`.17` requires a non-owning projection or a `ManuallyDrop`/guard shape whose
drop behavior is correct on every return and unwind edge. Its proof obligation
is that the borrowed source remains byte-for-byte readable and drops once.
That is separate from `.16`, whose obligation is target commit/rollback and
partial-destination cleanup.

## Observable axes

| Axis | Current observation |
|---|---|
| Result | All 15 focused `vba_record`-filtered tests and all 28 `safe_array`-filtered tests pass. Strict runtime Clippy is clean. The audit result is nevertheless **not certified** because the suites do not exercise either destination rollback (`.16`) or borrowed-source unwind/early return (`.17`). |
| Full Err | This audit changes no runtime behavior or Err state. The focused runtime tests do not establish VBA `Err` number/description/source/help/Erl parity. Nested/raw record clone allocation failure can return `Err(String)` after invalidating destination ownership, direct Variant clone can panic during mutation, and a borrowed-wrapper unwind can destroy the source before any stable result is seated. `.16` owns rollback/result ordering; `.17` owns source preservation across every return/unwind. |
| Side effects | Scalar Copy-carrier writes retain their ordinary behavior. Owning Variant/nested/fixed-array/record-SAFEARRAY failures can partially mutate or invalidate the target. Supposedly prepare-first String/fixed-string and safe read/clone paths can instead free the source before target mutation when a borrowed owning wrapper unwinds. No failure-side-effect claim is made until `.16` and `.17`. |
| Lifecycle/order | Success-path construction, deep clone, single drop, BSTR transfer, Variant payload destruction, object AddRef/Release, and record-buffer counters were traced. Drop-before-clone and untracked partial initialization violate target single-drop lifecycle; raw-from/late-`forget` projections violate borrowed-source single ownership on unwind or early return. |
| Transport | Current native-word staging rejects owning fields and was checked only on Windows x64. Raw field handles are not sealed to their owner; raw BSTR/SAFEARRAY projections temporarily adopt ownership instead of expressing a borrow; and `VbaRecord` hidden-Variant object transit is not thread/apartment-safe evidence. No Windows COM, Linux, 32-bit, ARM64, or external ABI certification is claimed. |
| Balance | The ordinary-path deep-clone tests pass, but they do not inject allocation failures or report isolated BSTR/object/record/SAFEARRAY balances. Confirmed failure shapes include leaked partial destinations, double destruction of invalidated destinations, and double destruction of sources freed by borrowed-wrapper unwind. Balance remains unverified under `.16` and `.17`; thread/apartment balance remains with `.2.7.2`. |

## Commands and exact results

Executed from the isolated
`codex/bd-59co-2-2-2-vba-record-audit` worktree:

| Command | Result |
|---|---|
| `git rev-parse HEAD` | `c0ff1de7a61dce2df6c83eff730582f2fd6f969b` before this evidence-only commit. |
| `git diff --numstat 37811fd5..HEAD -- crates/oxvba-runtime/src/vba_record.rs` | `436` additions, `60` deletions. |
| `cargo test -p oxvba-runtime vba_record -- --nocapture` | Pass: 15 passed, 0 failed, 0 ignored, 149 filtered out. This includes 14 `vba_record` module tests and `variant::tests::vba_record_variant_clone_deep_copies_native_payload`. |
| `cargo clippy -p oxvba-runtime --all-targets -- -D warnings` | Pass: exit 0, zero warnings. This is the runtime crate only, not workspace Clippy. |
| `cargo fmt -p oxvba-runtime -- --check` | Pass: exit 0. |
| `cargo test -p oxvba-runtime safe_array -- --nocapture` | Pass: 28 passed, 0 failed, 0 ignored, 136 filtered out. No allocation-failure case is present. |
| Current-file and post-`37811fd5` added-line scan for `#[allow]`, `#[expect]`, `clippy::`, or `undocumented_unsafe_blocks` overrides | No suppression/override found. Workspace lint remains `undocumented_unsafe_blocks = "deny"`. |
| Compile-time trait probe `assert_send_sync::<oxvba_runtime::VbaRecord>()` | Compiles: `VbaRecord` currently satisfies `Send + Sync`. The equivalent `Variant` probe fails with `E0277` because `*mut c_void` is neither Send nor Sync. |
| `./scripts/check-governance.ps1` | Not green on the inherited baseline: earlier sections pass, then `pmr-event-snippets` reports stale generated `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`. This audit did not touch its registry/generated inputs, and the bead explicitly forbids generated-summary edits. This is not counted as a `vba_record` gate pass. |

## Exact residual owners

### `bd-59co.2.2.15` — CORE-1 seal VbaRecord layouts and field access (P0)

Scope: make field handles immutable-layout-identity-bound or remove them from
the safe public raw-pointer API; reject forged and foreign-layout handles while
allowing an explicit handle capability to be reused across records sharing the
exact same immutable layout `Arc`; make alignment, stride,
offset, extent, and final-size arithmetic checked; reject hostile near-
`usize::MAX` layouts before pointer arithmetic/allocation; and make allocation
admission bounded/fallible.

Acceptance commands:

- `cargo test -p oxvba-runtime vba_record_layout_sealing -- --nocapture`
- `cargo test -p oxvba-runtime vba_record -- --nocapture`
- `cargo clippy -p oxvba-runtime --all-targets -- -D warnings`

Evidence route:
`docs/evidence/programs/ideal-2026-07/core/CORE-1/vba-record-layout-sealing.md`.

### `bd-59co.2.2.16` — CORE-1 make VbaRecord owning writes transactional (P0)

Scope: stage owning Variant, nested/fixed-array record, and record-SAFEARRAY
replacements before destroying the live target; represent and roll back partial
raw initialization; add deterministic failure injection; and prove result,
unchanged-on-failure state, single-drop lifecycle, and BSTR/object/Variant/
record/SAFEARRAY balance.

Acceptance commands:

- `cargo test -p oxvba-runtime vba_record_transactional_write -- --nocapture`
- `cargo test -p oxvba-runtime safe_array_record_transactional_write -- --nocapture`
- neighboring focused `vba_record` and `safe_array` tests
- `cargo clippy -p oxvba-runtime --all-targets -- -D warnings`

Evidence route:
`docs/evidence/programs/ideal-2026-07/core/CORE-1/vba-record-transactional-write.md`.

### `bd-59co.2.2.17` — CORE-1 make borrowed carrier projections unwind-safe (P0)

Scope: replace every runtime `from_raw`-plus-late-`forget` borrow projection
with a non-owning view or `ManuallyDrop`/guard whose ownership cannot transfer
on `Err`, early return, or panic. Deterministic failure/unwind tests must prove
the source remains unchanged and readable, its later destruction happens
exactly once, carrier balance has zero drift, and no late-forget window remains.

Acceptance commands:

- `cargo test -p oxvba-runtime borrowed_carrier_unwind_safety -- --nocapture`
- `cargo test -p oxvba-runtime vba_record -- --nocapture`
- `cargo test -p oxvba-runtime safe_array -- --nocapture`
- `cargo test -p oxvba-runtime variant -- --nocapture`
- `cargo clippy -p oxvba-runtime --all-targets -- -D warnings`

Evidence route:
`docs/evidence/programs/ideal-2026-07/core/CORE-1/borrowed-carrier-unwind-safety.md`.

### `bd-59co.2.7.2` — existing hidden-carrier object-thread owner

The existing CORE-5 SafeArray object-carrier thread-ownership successor is
explicitly extended to the `VbaRecord` hidden-Variant case. It must decide and
enforce Send/Sync, VM-thread, termination-queue, and COM-apartment ownership;
this audit gives no thread-transit certification.

## Certification verdict

The documented unsafe blocks remain warning-clean and the ordinary-path focused
tests are green, but the integrated ownership audit from `37811fd5` is **not
sound as a certification of current `vba_record`**. The public field-handle
premise was false at that commit, checked layout sealing is incomplete, and
owning writeback is not failure-transactional. In addition, raw BSTR/SAFEARRAY
borrow projections can destroy their source on unwind or early return because
they temporarily adopt ownership until a late `forget`. The lane remains in
progress under `.15`, `.16`, `.17`, and the existing `.2.7.2`
thread-ownership route, with no capability credit from this support audit.
