# OxVba Representation and Layout Doctrine V1

Date: 2026-06-22
Status: accepted direction, in-progress implementation
Owner: Codex

## Decision

OxVba's runtime carriers for `BSTR`, `VARIANT`, `SAFEARRAY`, `IUnknown`, and
the numeric primitives are intended to be exact Windows/VBA/COM storage
representations throughout the runtime stack. These carriers are not merely
temporary boundary projections.

This means:

- `oxvba-runtime` owns the exact-layout carrier types and their ownership rules.
- VM slots may still store high-level `Variant` values, but that `Variant` must
  itself be a faithful native `VARIANT`-compatible cell.
- `SafeArray` descriptors, bounds, feature flags, element sizes, and payload
  storage must match the declared element layout instead of normalizing typed
  arrays into `VT_VARIANT` arrays.
- `ObjectRef` must retain a COM-compatible `IUnknown` object/vtable pointer
  shape for object identity.
- `oxvba-com` still owns COM invocation and typelib semantics, but it should use
  the runtime's exact carriers directly where a value is addressable.
- Copy-in/copy-out remains valid only for temporaries, coercions, rvalues, or
  ABI-required conversions.

Non-Windows builds must keep compatible simulated Windows layouts for these
carriers so tests and cross-platform execution do not rely on a different
semantic-only representation.

## Carrier Requirements

### BSTR

`BStr` is an owned BSTR-shaped UTF-16 allocation. `StrPtr` exposes the BSTR data
pointer for addressable string storage or for a bounded temporary when the input
is not addressable.

### VARIANT

`Variant` is the canonical execution cell and must remain layout-compatible with
Windows `VARIANT`, including the 16-byte discriminant/data/`DECIMAL` overlay,
VARENUM tags, payload ownership, `VARIANT_BOOL`, `CY`, `DATE`, object, BSTR, and
SAFEARRAY pointer payloads.

APIs that expose a variant cell pointer must point at this real cell. Any helper
that serializes or clones bytes must not imply a second non-native wire format.

### SAFEARRAY

`SafeArray` is the runtime array carrier and must expose a COM-compatible
descriptor pointer. Declared scalar arrays must allocate typed SAFEARRAY payloads
such as `VT_I2`, `VT_I4`, `VT_R4`, `VT_R8`, `VT_CY`, `VT_DATE`, `VT_BSTR`, and
`VT_BOOL`; only declared `Variant` arrays should use `VT_VARIANT` by default.

The current implementation now preserves typed storage for VM `ReDim` scalar
arrays. UDT/record array elements still use a runtime record value carrier and
remain an in-progress exact-layout lane until record descriptors and element
payloads are made COM-compatible.

### IUnknown

`ObjectRef` must expose a COM-compatible identity pointer whose first field is a
vtable pointer with `QueryInterface`, `AddRef`, and `Release` shape compatible
with the supported runtime interfaces. COM-backed and runtime-backed objects may
have different dispatch implementations, but their object identity carrier must
not be an arbitrary integer token.

### Numeric Primitives

Numeric primitive payloads must match VBA/COM storage:

- `Integer`/`Long`/`LongLong` as signed 16/32/64-bit integers.
- `Byte` as unsigned 8-bit.
- `Single`/`Double` as IEEE 32/64-bit floats.
- `Currency` as scaled signed 64-bit `CY`.
- `Date` as OLE Automation `f64`.
- `Boolean` as `VARIANT_BOOL` (`-1` true, `0` false) where stored in VARIANT or
  SAFEARRAY carriers.

Semantic coercion and overflow behavior still belongs to arithmetic/binder/VM
logic; it must not change the physical layout of an already stored carrier.

## Migration Consequences

- Replace boundary-only materialization paths with direct use of exact runtime
  storage whenever the source expression denotes an addressable place.
- Add layout and pointer-stability tests for every carrier before widening
  compatibility claims.
- Treat `SAFEARRAY`, UDT/record, `Decimal`, and ByRef support as in-scope
  implementation work. Only truly unknown or foreign ABI cases should remain
  blocked.
- Keep docs, worksets, and evidence explicit about any carrier family that is
  exact only for a subset.

## Current Slice Evidence

The 2026-06-22 typed SAFEARRAY slices change VM `ReDim` so scalar declared
arrays allocate typed SAFEARRAY payloads instead of `VT_VARIANT` payloads, and
move VM array element assignment onto checked in-place SAFEARRAY element
replacement on the slot-owned ArrayVariant instead of descriptor rebuild through
`Vec<Variant>`. The targeted test anchors are:

- `cargo test -p oxvba-vm2 --test linearize_roundtrip redim`
- `cargo test -p oxvba-runtime safe_array::tests::safe_array_set_variant_element --lib`
- `cargo test -p oxvba-runtime array_variant_set_element_preserves_owned_safearray_pointer --lib`
- `cargo test -p oxvba-runtime record_variant_clone_deep_copies_record_payload --lib`
- `cargo test -p oxvba-runtime safe_array_variant_element_reads --lib`

Remaining exact-layout work includes UDT/record SAFEARRAY element storage,
expanded pointer-helper addressability tests, and COM/HAL call paths that still
clone through `variant_elements()` for non-temporary places.
