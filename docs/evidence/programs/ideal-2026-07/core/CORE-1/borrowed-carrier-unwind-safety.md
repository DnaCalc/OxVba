# CORE-1 borrowed carrier unwind safety

Date: 2026-07-11

Bead: `bd-59co.2.2.17`

Effect: delivery

Result: implementation and evidence complete for this runtime-safety slice; the
Core capability profile remains in-progress

## Contract and scope

This slice advances `RUNTIME-VALUE-001`, `SEC-BOUNDARY-001`, and
`CONF-QUALITY-001`. Its explicit ownership and raw-helper design also informs
the downstream `SYS-OWN-001` and `RUNTIME-ABI-001` realization, but this leaf
does not claim those additional clauses.

Exact-layout carriers are owning values. A raw pointer projected from an owning
`Variant`, record slot, SAFEARRAY element, or pointer registry remains owned by
that source until an API explicitly transfers it. A temporary read/clone view
must therefore be unable to run the source owner's destructor, including when a
fallible operation returns early or panics.

The prior local pattern constructed an owning `BStr` or `SafeArray` wrapper from
a borrowed raw pointer, performed work, and called `mem::forget` afterward. The
final forget did preserve ownership on the success path, but it left a material
window: an error or unwind between wrapper construction and the late forget
would run `Drop` and free the allocation still owned by the source. The same
shape made failure injection inside a borrowed SAFEARRAY projection unsafe, so
the preceding transactional-write slice had to suppress those nested test
boundaries.

The audited scope is:

| source owner | borrowed carrier | audited operations |
|---|---|---|
| `BStr`, String `Variant`, record String slot, typed BSTR SAFEARRAY slot | raw BSTR payload pointer | read, deep clone, trusted-wire clone, record clone/read/write, typed-array decode/replace |
| Array `Variant` or caller-owned `SafeArray` | OxVba-owned raw SAFEARRAY descriptor | deep clone, bounds, Variant/I4 read and set, record/fixed-array materialization |
| Object `Variant`, object SAFEARRAY slot, pointer registry | retained `RawRuntimeIUnknown` | ownership transfer into carriers/registries |
| temporary owned BSTR/SAFEARRAY clones | raw pointer result | explicit ownership transfer without a late forget window |

This is local OxVba carrier safety. It does not claim adoption of an arbitrary
foreign SAFEARRAY, a fabricated/stale pointer, COM marshalling parity, or an
Excel/VBA semantic result.

## Delivered ownership model

`BorrowedBStr<'a>` is an internal, destructor-free shared view. Its unsafe
constructor requires the caller to keep the source BSTR live for the view's
complete lifetime. Cloning first crosses a fallible testable boundary, then
allocates a distinct BSTR and immediately installs that fresh allocation in an
owning `BStr`. Returned errors and unwinds can drop only independent owners;
the view itself has no destructor that could claim the source pointer.

Raw SAFEARRAY helpers validate the OxVba owner prefix and immediately place the
temporary `SafeArray` value in `ManuallyDrop`. Every fallible or panic-injectable
operation happens only after that destructor suppression is established. The
view is kept local to the helper and is never returned as an owner. Fallible
`SafeArray::try_clone` and `try_variant_elements` substrates propagate nested
BSTR, Variant, record, object, and SAFEARRAY clone failures without crossing an
infallible `Clone::expect` seam. Existing public infallible compatibility APIs
remain thin adapters over those fallible internals and retain their prior
observable signatures.

Ownership transfer no longer relies on a late `mem::forget` anywhere in
`oxvba-runtime/src`:

- `BStr::into_raw_bstr` takes its optional pointer, leaving ordinary `Drop`
  inert before returning the raw owner.
- Pointer helpers retain `OwnedBstr(Option<BStr>)`, so ordinary pin release
  continues through the canonical tracked owner. `OwnedBstr::into_raw` takes
  that option and delegates to `BStr::into_raw_bstr`, leaving both destructors
  inert before returning the transferred owner.
- A mutable `OwnedBstrCell` whose native callee consumed its original first
  applies the one explicit live-counter debit, then disarms the now-dangling
  Rust owner with `BStr::into_raw_bstr`. It never dereferences or frees that
  consumed address and frees only a non-null native replacement. An unchanged
  cell still drops its original canonical `BStr` normally.
- `ObjectRef::into_raw_iunknown` and `SafeArray::into_raw_safearray_ptr`
  establish `ManuallyDrop` immediately, then expose the transferred pointer.
- Variant String/Object/Array construction consumes those explicit transfers.
- record String reads/clones and typed SAFEARRAY BSTR decoding use
  `BorrowedBStr`; record and SAFEARRAY fallible paths use `try_as_bstr`,
  `try_as_safearray`, `try_variant_elements`, and `try_clone`.
- trusted process-local ArrayVariant reconstruction uses the fallible raw
  SAFEARRAY clone substrate.

The test-only nested-boundary suppression introduced by
`bd-59co.2.2.16` is removed. Nested borrowed-carrier boundaries are now visible
and are swept normally. The transactional constructor test consequently admits
additional nested element-clone events while still requiring at least every
top-level element event and sweeping the complete successful trace.

The invariant at each raw borrow is now:

1. the unsafe caller keeps a documented live source owner;
2. raw provenance/owner-prefix validation occurs where applicable;
3. a destructor-free view or immediate `ManuallyDrop` guard is constructed;
4. only then may cloning, allocation, decoding, validation, error return, or
   panic occur;
5. a successful clone becomes an independent owner, while an explicit transfer
   clears or suppresses the old owner's destructor before returning the raw
   pointer.

`rg` reports no `mem::forget`/`forget(` occurrence in
`crates`. Remaining `from_raw_bstr` sites are ownership
adoption/free paths, not temporary borrows; remaining raw SAFEARRAY adoption is
the explicitly owning API and Variant drop path.

## Deterministic error and unwind proof

The per-thread owning-boundary injector can make a selected boundary return
`Err` or panic. Every new test catches panic mode, clears the injector, verifies
the complete source carrier or descriptor is unchanged and still readable,
then allows the source to drop normally. Exact thread-local live-handle counts
must return to their starting vector after each test.

The five filterable tests cover:

- a direct destructor-free BSTR borrow over an odd-byte payload, including
  independent successful clone, returned error, panic, unchanged raw pointer
  and byte-for-byte source readability;
- String Variant projection and deep clone at the nested borrowed-BSTR
  boundary, plus ArrayVariant projection after the raw SAFEARRAY guard exists;
- record String read, owned record clone, and raw-record clone at the nested
  borrowed-BSTR boundary, with exact record-byte preservation and later
  SAFEARRAY/object-field readability;
- typed BSTR SAFEARRAY direct read, raw element read, raw clone, and raw set,
  failing both immediately after raw descriptor suppression and inside the
  nested borrowed BSTR view; descriptor identity, payload identity, flags,
  bounds, old BSTR bytes, and replacement source remain intact;
- raw I4 read, raw I4 set, and raw bounds helpers after descriptor suppression,
  followed by a successful mutation; and
- a composite Variant SAFEARRAY owning BSTR, object, nested SAFEARRAY, and
  nested VbaRecord carriers, proving semantic readability and exact BSTR,
  object-box, SAFEARRAY, and record-buffer balance after raw-clone errors,
  unwinds, and the final ordinary source drop; and
- a Windows process-isolated pointer-helper test covering an ordinary pin plus
  unchanged, native-consumed-to-null, and native-replaced writable BSTR cells.
  It reads back the exact value before `free_pins`, calls `free_pins` again to
  prove idempotent no-double-free behavior, and requires both zero BSTR drift
  and zero total carrier drift for every case and the complete test process.

The focused Miri run executes the same five error/unwind tests. It reports no
invalid access, use-after-free, leak, double drop, or other undefined behavior.
Its four visible warnings are the already accepted Exposed Provenance recovery
sites for process-local BSTR, IUnknown, SAFEARRAY, and record pointer bytes from
`bd-59co.2.2.20`; the run does not suppress those warnings.

## Observable evidence

| surface | evidence |
|---|---|
| Result | Successful controls deep-clone odd-byte BSTRs, typed BSTR elements, complete SAFEARRAYs, and records into independent owners. After every injected failure the original source can still be read and, for mutable SAFEARRAYs, subsequently changed successfully. |
| Full Err | Error mode returns the injected runtime `Result::Err(String)` through every fallible BSTR/Variant/record/SAFEARRAY path. The bounds compatibility helper maps its internal injected error to its existing `None` result. No test-mode error crosses a public foreign/generated boundary or mutates VBA `Err`. |
| Side effects | Before commit, exact record bytes and SAFEARRAY descriptor/payload identity are unchanged. Raw set failures leave the old BSTR or I4 element in place and leave the replacement source readable. Borrow views allocate, retain, release, or mutate nothing by themselves. |
| Lifecycle/event order | Borrow: validate live source -> suppress destructor by construction -> perform fallible work -> return independent owner. Transfer: clear/take or establish `ManuallyDrop` -> expose pointer. Error/unwind drops only newly created independent owners; the source drops exactly once later. No runtime event dispatch occurs. |
| Transport | Variant payload bytes, BSTR layout, SAFEARRAY descriptor layout, record layout, and public raw-helper signatures are unchanged. The raw pointers remain process-local live-carrier transport, not persistent serialization. Native Windows tests exercise OLE BSTRs; Miri uses the documented layout-equivalent allocator. |
| Balance | Every focused test compares exact per-thread BSTR, object-box, SAFEARRAY, and record-buffer live counts before construction and after final drop. Composite paths exercise all four families. Miri independently validates the same cleanup paths. |

## Checks

- `cargo test -p oxvba-runtime borrowed_carrier_unwind_safety -- --test-threads=1` — 5 focused tests passed.
- `cargo test -p oxvba-differential --test pointer_bstr_ownership -- --test-threads=1` — 1 Windows process-isolated test passed; ordinary, unchanged, consumed-to-null, and native-replacement paths preserved exact values, tolerated duplicate `free_pins`, and restored zero BSTR/total carrier drift.
- `cargo test -p oxvba-runtime pointer_helpers::tests --all-features -- --test-threads=1` — 17 neighboring pointer-helper tests passed.
- `cargo test -p oxvba-runtime --all-features -- --test-threads=1` — 178 unit tests, 2 isolated integration tests, and 8 compile-fail doctests passed.
- `cargo clippy -p oxvba-runtime --all-targets --all-features -- -D warnings` — passed with zero warnings.
- `cargo clippy -p oxvba-differential --test pointer_bstr_ownership --all-features --no-deps -- -D warnings` — passed with zero warnings in the touched Windows-isolated test target.
- `cargo +nightly miri test -p oxvba-runtime borrowed_carrier_unwind_safety -- --test-threads=1` — 5 focused tests passed in 5.22 seconds with no Miri failure; four already documented Exposed Provenance warnings remained visible.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- `rg -n "mem::forget|forget\\(" crates` — no matches.

An exploratory differential Clippy invocation without `--no-deps` stopped on
five pre-existing `oxvba-symbol` and two pre-existing `oxvba-oxir` warnings
outside this branch's files. The strict runtime command and the strict
all-feature isolated-test command above are green; no warning was suppressed in
either touched target.

## Residual disposition

- No accepted residual remains in temporary raw BSTR or OxVba-owned SAFEARRAY
  borrow projection, clone/read/set helpers, record read/raw clone helpers, or
  local BSTR/ObjectRef/SAFEARRAY raw ownership transfer.
- Unsafe raw APIs still require a live correctly typed source for the complete
  call. Fabricated, stale, concurrently mutated, or foreign descriptors remain
  outside their documented contracts. Prefix validation is not permission to
  probe arbitrary unreadable memory.
- Address-only Variant carriers continue to use the explicit Exposed
  Provenance contract and retain the documented Miri warnings from
  `bd-59co.2.2.20`. This slice does not change that ABI or claim
  `-Zmiri-strict-provenance` compatibility.
- `bd-59co.2.2.14` remains explicitly open for the separate Windows
  `VarPtr(Variant)` ownership/accounting seam: `OwnedBstr::into_raw` transfers a
  tracked BSTR into a native `VARIANT`, whose later `VariantClear` frees it
  outside the OxVba live-counter path. The writable `OwnedBstrCell` regression
  in this leaf does not reach or claim that VARIANT/VariantClear residual.
- Foreign COM SAFEARRAY/BSTR ownership, external `IRecordInfo`, Windows
  marshalling, callbacks, and Excel/VBA certification remain in their Windows
  interop owners. This internal Rust ownership invariant has no separate VBA
  oracle question.
- Allocator exhaustion that aborts the process remains Rust/platform policy.
  The deterministic hooks prove every runtime boundary that can return or
  unwind; they do not claim recovery from an abort.
