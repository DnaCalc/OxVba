# CORE-1 Windows VARIANT-pointer BSTR ownership

Date: 2026-07-11

Bead: `bd-59co.2.2.14`

Effect: delivery

Result: implementation and evidence complete for this observed accounting
seam; the Core capability profile remains in-progress

## Contract and scope

This slice advances `CONF-QUALITY-001`, `RUNTIME-VALUE-001`, and
`SEC-BOUNDARY-001` on canonical row
`CORE-READINESS/CORE-BASELINE-BALANCE-LIFECYCLE`.

On Windows, `VarPtr` over a Variant materializes a native `VARIANT` cell. A
String projection cloned the canonical `BStr`, transferred its raw BSTR into
that cell, and later relied on `VariantClear` for the native free. The transfer
disarmed the Rust owner, however, so the OxVba live-BSTR counter retained its
allocation credit after `VariantClear`. This was an accounting leak even when
the OLE allocation itself was released correctly.

This leaf repairs only that already-observed native-VARIANT projection seam.
It audits the top-level `OwnedVariant`, nested `SAFEARRAY(VT_VARIANT)` element
projection, stable registry identity across simultaneous pins and map growth,
`free_pins` release order, null and non-BSTR cells, panic-safe partial
array-construction failure, extreme Windows `LONG` array bounds, and native
clearing or replacement of a writable cell. It does not claim general
pointer/native-call parity, arbitrary foreign VARIANT validity, COM
marshalling, or VBA oracle conformance.

## Ownership model

`OwnedVariant` now owns two independent resources:

1. the mutable native `VARIANT` cell and whichever valid payload native code
   leaves in it; and
2. an optional `TrackedBstrAccountingToken` for the one canonical BSTR
   allocation transferred into the original String projection.

String projection starts from a fresh zero/`VT_EMPTY` cell. It deep-clones the
canonical BSTR, installs the raw pointer and `VT_BSTR`, then attaches the
accounting token. There is no fallible operation after raw ownership transfer.
A null BSTR installs `VT_BSTR` plus null and creates no token because no
allocation was credited.

Drop ordering is deliberately fixed and pointer-independent:

1. `VariantClear` first releases the cell's **current** valid payload exactly
   once; then
2. dropping the token debits the original transferred OxVba allocation exactly
   once.

The token never stores or compares the original pointer and never inspects the
cell's current `vt`. Consequently:

- an unchanged cell has its original BSTR freed by `VariantClear`, followed by
  the matching counter debit;
- a native-cleared cell makes the later `VariantClear` a no-op, but retains the
  independent original debit until registry release; and
- a native replacement is the only payload freed by the final `VariantClear`,
  while the token debits only the original OxVba allocation. The replacement
  receives no false counter debit.

`Drop` does not panic on the `VariantClear` result. Duplicate or absent
`free_pins` addresses remain no-ops in the registry, so the owner and token can
be dropped only once.

`PointerEntry::VariantCell` heap-owns its `OwnedVariant` before
`PointerRegistry::insert` derives the pin ID from the cell address. Moving the
entry into a `HashMap`, growing or rehashing that map, and moving its buckets
therefore move only the `Box`, never the native VARIANT cell. Simultaneous pins
have distinct IDs and cells; removing one entry cannot replace, alias, or
invalidate another entry or settle its token early.

Every temporary native VARIANT used to populate a
`SAFEARRAY(VT_VARIANT)` is now an `OwnedVariant` as well. `SafeArrayPutElement`
deep-copies the element; the temporary is then cleared and its token settled.
Every successful `SafeArrayCreate` or `SafeArrayCreateVector` is immediately
wrapped by `OwnedWindowsSafeArray`. The guard destroys the array and all
already-copied elements on every error or Rust unwind and is disarmed only
after a complete array VARIANT has taken ownership. If element projection,
insertion, checked index progression, or later construction fails, temporary
tokens settle through RAII and no registry entry is published. Multidimensional
progression uses representable `i64` relative indices, never increments after
the final element, and returns an error instead of overflowing a Windows
`LONG`. This also retains the earlier repair of the one-dimensional
index-conversion exit that bypassed partial SAFEARRAY cleanup.

The audit found no other call to the raw VARIANT projector and no
`mem::forget`/`forget(` occurrence under `crates`.

## Named Windows-isolated proof

The exact bead acceptance command now runs the runtime unit test
`pointer_helpers::tests::windows_variant_pointer_bstr_balance` rather than a
zero-test filter. It uses thread-local carrier counts to prove two simultaneous
cells, independent null/native-BSTR mutations, staggered idempotent release,
and exact `+2 -> +1 -> 0` BSTR accounting even when the full runtime suite runs
in parallel.

The companion `windows_variant_pointer_bstr` differential target runs as its
own process with global `live-counters` enabled. It additionally proves:

- a top-level unchanged `VT_BSTR` cell preserves exact UTF-16 including an
  embedded NUL, retains exactly one token while pinned, and returns all carrier
  counts to baseline after repeated `free_pins`;
- a native `VariantClear` followed by `VT_NULL` retains the original token
  until pin release and then balances;
- a native `VariantClear` followed by an independently allocated BSTR
  replacement preserves the exact replacement value, balances after release,
  and would expose either a missed debit (`+1`) or false extra debit (`-1`);
- two pairs of simultaneous BSTR Variant pins have distinct IDs and addresses,
  survive independent native mutation, and remain live when released in either
  order with exact `+2 -> +1 -> 0` accounting;
- 128 simultaneous BSTR Variant pins retain unique, stable IDs and cell
  addresses through repeated `HashMap` growth, then balance under staggered
  even/odd release and repeated no-op release;
- null-BSTR String, `VT_EMPTY`, `VT_NULL`, and non-BSTR `VT_I4` projections
  preserve their exact type/value and create no BSTR accounting token;
- a `SAFEARRAY(VT_VARIANT)` BSTR element is copied exactly, while the temporary
  projected BSTR token is already settled before the outer pin is released;
- a multidimensional array that copies a BSTR before encountering an
  unsupported procedure-reference element fails without publishing a pin,
  leaves the source readable, destroys the partial native array, and returns
  every carrier count to baseline; and
- a valid one-element dimension beginning at `LONG::MAX` succeeds without
  unwinding, while an unrepresentable two-element dimension fails through
  checked progression after its first successful BSTR copy; the armed native
  SAFEARRAY guard destroys that partial array and returns every tracked count
  and pin count to baseline; and
- the process ends at its initial pointer-registry count and exact BSTR,
  object-box, SAFEARRAY, record-buffer, and total carrier counts.

## Observable evidence

| surface | evidence |
|---|---|
| Result | Unchanged, replacement, null-BSTR, Empty, Null, Long, and nested-array controls preserve exact native `vt` and payload values. Unsupported nested ProcRef returns the existing projection error. |
| Full Err | This low-level helper returns `Result<_, String>` and does not mutate VBA `Err`. The failure control retains the complete message `procedure references cannot be marshaled as VARIANT values`; no partial pin is observable. |
| Side effects | Registry publication occurs only after complete projection. Native mutation is confined to a distinct heap-stable owned cell. Failed or unwound nested projection leaves the OxVba source array readable and the armed guard releases the partial Windows SAFEARRAY. |
| Lifecycle/event order | Heap-own fresh cell -> clone/transfer BSTR -> attach accounting token -> derive stable pin ID -> publish pin -> optional valid native clear/replacement -> remove exactly that pin -> `VariantClear(current)` -> debit original token. Nested array construction arms a SAFEARRAY guard before population, uses temporary `OwnedVariant` owners, transfers a completed array into the outer VARIANT, and only then disarms the guard. No runtime event dispatch occurs. |
| Transport | The public pointer-helper and native `VARIANT`/SAFEARRAY layouts are unchanged. This is process-local Windows x64 native transport, not serialization or a cross-process artifact. |
| Balance | The named process-isolated test observes the exact live vector at each ownership transition and requires zero BSTR and total drift after every release/failure case and after final source drop. |

## Checks

- `cargo test -p oxvba-runtime windows_variant_pointer_bstr_balance -- --nocapture` — the exact bead acceptance command ran 1 named runtime test and passed (not a zero-test filter).
- `cargo test -p oxvba-differential --test windows_variant_pointer_bstr -- --test-threads=1 --nocapture` — 1 named Windows-isolated test passed.
- `cargo test -p oxvba-differential --test pointer_bstr_ownership -- --test-threads=1 --nocapture` — 1 neighboring Windows BSTR-cell ownership test passed.
- `cargo test -p oxvba-runtime pointer_helpers::tests --all-features -- --test-threads=1` — 18 neighboring pointer-helper tests passed.
- `cargo test -p oxvba-runtime --all-features -- --test-threads=1` — 179 unit tests, 2 isolated integration tests, and 8 compile-fail doctests passed.
- `cargo clippy -p oxvba-runtime --all-targets --all-features -- -D warnings` — passed with zero warnings.
- `cargo clippy -p oxvba-differential --test windows_variant_pointer_bstr --all-features --no-deps -- -D warnings` — passed with zero warnings in the touched Windows-isolated target.
- `cargo +nightly miri test -p oxvba-runtime borrowed_carrier_unwind_safety -- --test-threads=1` — 5 portable neighboring ownership tests passed; the four already-documented exposed-provenance warnings remained visible.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- `rg -n "mem::forget|forget\\(" crates` — no matches.

## Residual disposition

- No accepted residual remains in the accounting provenance or release order
  for an OxVba BSTR transferred through this pointer-helper native VARIANT cell
  or its temporary `SAFEARRAY(VT_VARIANT)` element projections.
- Callers/native code must still leave a valid clearable VARIANT in a pinned
  writable cell. A fabricated type/pointer, stale pointer, concurrent mutation,
  or replacement that abandons the old payload without first releasing it is
  outside the unsafe boundary contract and cannot be repaired by accounting.
- Windows OS allocation/`VariantClear` behavior cannot execute under Miri. The
  neighboring portable borrowed-carrier Miri lane proves OxVba raw-carrier
  error/unwind ownership, while this leaf uses the real isolated Windows OLE
  APIs for its native proof.
- Broader WIN-10 pointer/native-call parity remains explicitly owned by
  `bd-59co.3.11.1`. That producer covers ABI shapes, writeback, native-call
  integration, and Windows/Excel evidence beyond this already-observed
  accounting seam. This leaf does not close or substitute for it.
