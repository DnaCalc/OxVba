# CORE-1 VM3 foreign-object fixture provenance

Date: 2026-07-14

Bead: `bd-59co.2.2.22`

Effect: support

Result: the bounded VM3 test-fixture provenance repair and focused lifecycle
proof are complete; fresh-eyes review and the separately owned strict-Clippy
baseline remain outstanding

## Scope and contract

This support leaf exercises `CONF-QUALITY-001` and `SEC-BOUNDARY-001` for the
test-only foreign-IUnknown fixture in `oxvba-vm3`. It advances no VM3
capability row and makes no VBA compatibility claim.

`FakeForeignObject` is a `repr(C)` allocation whose first field is the
COM-shaped `RawRuntimeIUnknown` and whose following fields include its atomic
reference count. The former constructor projected `&mut (*raw).unknown` and
passed that field-derived pointer to `ObjectRef`. Its numeric address matched
the box address, but its provenance was bounded to the first field. The fake
vtable later cast that interface pointer back to `FakeForeignObject` in order
to access the adjacent reference count and reconstruct the complete `Box` on
the final `Release`. Miri correctly treats those operations as requiring the
provenance of the complete allocation, not merely an equal address.

The constructor now follows the already accepted runtime fixture and product
allocation pattern: it casts the pointer returned directly by `Box::into_raw`
to `RawRuntimeIUnknown`. Because `unknown` is the first field, this preserves
the interface address while retaining complete-allocation provenance for
`AddRef`, `Release`, and `Box::from_raw`.

## Lifecycle regression

The named
`fake_foreign_object_addref_release_preserves_complete_allocation_provenance`
test keeps an independent `Arc<AtomicU32>` destruction observer and executes
the complete reference sequence:

1. the owned raw constructor starts at reference count 1;
2. `ObjectRef::clone` invokes the fake vtable's `AddRef`, reaching 2;
3. `ObjectRef::from_raw_iunknown_addref` retains the borrowed raw interface,
   reaching 3;
4. dropping the three owners reaches 2, then 1, then 0; and
5. the final `Release` reconstructs and drops the full box exactly once,
   leaving only the test's observer `Arc`.

Every intermediate adjacent-refcount read uses the same interface pointer, so
Miri would reject a regression to first-field-bounded provenance before the
test could report success. The existing VM3 `For Each` foreign-object failure
test remains green and still exercises the fixture through the host and VM
path.

## Sibling audit

A bounded search of `crates/oxvba-vm3` for `RawRuntimeIUnknown`,
`Box::into_raw`, raw-IUnknown constructors, `repr(C)` test objects, and
field-address-to-container casts found this single fake foreign-object
allocation. The two casts in `fake_foreign_add_ref` and
`fake_foreign_release` are the required interface-to-owner recovery operations;
they are sound after the constructor preserves the complete box provenance.
No VM3 sibling fixture required a code change.

The equivalent runtime foreign-object fixture already uses
`raw.cast::<RawRuntimeIUnknown>()`, and its neighboring descriptor-projection
test remains green.

## Observable evidence

| surface | evidence |
|---|---|
| Result | Reference counts follow exactly `1 -> 2 -> 3 -> 2 -> 1 -> 0`; the final box destruction count is exactly 1. |
| Full Err | Not applicable: this is a test-fixture ownership boundary and does not execute VBA `Err`. The VM foreign-enumeration neighbor still reports its expected host error 438. |
| Side effects | Only the fake object's atomic refcount and test-owned destruction observer change. Product behavior and public interfaces are unchanged. |
| Lifecycle/event order | Allocate full box -> transfer initial reference -> AddRef twice -> Release three times -> reconstruct and drop the same full box once. No event dispatch occurs. |
| Transport | The exposed pointer remains the address of the first `RawRuntimeIUnknown` field; only its Rust provenance source changes from the field projection to the complete allocation. |
| Balance | The destruction observer stays 0 while any owner remains, becomes 1 on the final Release, and its `Arc` strong count returns to 1. |

## Checks

- `cargo test -p oxvba-vm3 fake_foreign_object -- --nocapture` - 1 named
  lifecycle test passed; this is not a zero-test filter.
- `cargo +nightly miri test -p oxvba-vm3 fake_foreign_object -- --nocapture`
  - the same named test passed under Miri with no warning or failure.
- `cargo test -p oxvba-vm3 --lib -- --test-threads=1` - all 34 VM3 unit
  tests passed.
- `cargo test -p oxvba-vm3 foreach_over_foreign_object_surfaces_enumeration_failure -- --nocapture`
  - the existing VM/host foreign-object neighbor passed.
- `cargo test -p oxvba-runtime foreign_iunknown_has_no_runtime_descriptor_projection -- --nocapture`
  - the accepted runtime sibling passed.
- `cargo fmt --all -- --check` - passed.
- `git diff --check` - passed.

The exact strict command
`cargo clippy -p oxvba-vm3 --all-targets -- -D warnings` is not recorded as
green. On the accepted base it stops on pre-existing `too_many_arguments`
warnings in `oxvba-oxir/src/elaborate/lower.rs:577` and
`oxvba-oxir/src/verify.rs:1313`. Re-running with dependency linting excluded
also exposes the pre-existing `too_many_arguments` warning at
`oxvba-vm3/src/lib.rs:134` and `collapsible_if` warning at line 3068. None is
introduced by this leaf. They are neither suppressed nor broadened into this
fixture repair; the open strict workspace baseline owner is `bd-59co.2.2.3`.

## Residual disposition

- This support result does not advance or close any VM3 capability state.
- Product Variant carrier provenance remains the delivered scope of
  `bd-59co.2.2.20`; safe complete `VariantCore` initialization remains the
  delivered scope of `bd-59co.2.2.21`.
- Strict workspace Clippy and ordinary lifecycle certification remain open
  under `bd-59co.2.2.3`. This bead must not be used to claim that baseline is
  already green.
- Fresh-eyes non-author review is still required before integration.
