# Windows VBA 7.1 x64 VARIANT and SAFEARRAY Fact Pack

Date: 2026-04-20
Owner: Codex
Status: published
Workset: `WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md`
Bead: `bd-t8rr.2.3` / `vmm-b2`

## Scope

This note records the current evidence-backed fact pack for:

- Windows/VBA-facing `VARIANT` representation facts
- Windows/VBA-facing `SAFEARRAY` representation facts
- current checked-in OxVba baseline behavior for the same families

Normative precedence remains:

1. actual Windows/VBA observable behavior where we can establish it
2. published Microsoft specifications and API documentation
3. current OxVba behavior only as baseline evidence

## Primary Source Set

Microsoft primary sources used here:

- `VARIANT structure (oaidl.h)`:
  https://learn.microsoft.com/en-us/windows/win32/api/oaidl/ns-oaidl-variant
- `SAFEARRAY structure (oaidl.h)`:
  https://learn.microsoft.com/en-us/windows/win32/api/oaidl/ns-oaidl-safearray
- `SAFEARRAYBOUND structure (oaidl.h)`:
  https://learn.microsoft.com/en-us/windows/win32/api/oaidl/ns-oaidl-safearraybound
- `Array Manipulation Functions`:
  https://learn.microsoft.com/en-us/previous-versions/windows/desktop/automat/array-manipulation-functions
- `[MS-OAUT] 2.2.29.1 _wireVARIANT`:
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/4e2e9bff-2ac5-4bab-8308-1806b256833e

Checked-in OxVba source/test evidence used here:

- [variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/variant.rs)
- [safe_array.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/safe_array.rs)
- [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
- [windows_variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_variant.rs)
- [com_client_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_client_end_to_end.rs)

Local x64 ABI-size probe used here:

- a one-off PowerShell/C# layout probe derived from the documented field layout
  reported:
  - `VARIANT_COMPAT size=16`
  - `SAFEARRAY size=32`
  - `IntPtr size=8`

That probe is supporting evidence for x64 size consequences. It is not a
Microsoft-published size table.

## Confirmed Windows/Automation Facts

### `VARIANT-F1`: Memory layout skeleton

- the documented `VARIANT` layout is:
  - `VARTYPE vt`
  - `WORD wReserved1`
  - `WORD wReserved2`
  - `WORD wReserved3`
  - a union carrying scalar, pointer, byref, array, and interface payloads
  - a `DECIMAL decVal` overlay at the outer union level
- the fixed header before the payload union is therefore 8 bytes

Migration implication:

- a Windows-aligned internal `Variant` carrier must preserve the fixed
  `vt + reserved words + payload` shape, not just the same logical cases
- `Decimal` is special because it overlays the whole outer union, not merely one
  of the ordinary union payload arms

### `VARIANT-F2`: x64 size posture

- the Windows docs give the field layout but not an explicit x64 byte size
- the local x64 ABI probe derived from that documented layout reported 16 bytes
- the checked-in OxVba runtime test
  [variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/variant.rs)
  also asserts `size_of::<Variant>() == 16` for the retained runtime carrier

Migration implication:

- the target value migration can plausibly retain a 16-byte core `VARIANT`
  container size on x64
- but size parity alone is not enough, because the current OxVba carrier still
  omits major payload families

### `VARIANT-F3`: Payload families

- the documented `VARIANT` payload union includes scalar lanes and pointer lanes
  for:
  - `BSTR`
  - `IUnknown*`
  - `IDispatch*`
  - `SAFEARRAY*`
  - `VARIANT*`
  - generic `PVOID byref`
  - `DECIMAL*` and record metadata forms

Migration implication:

- the migrated canonical carrier must account for string, array, and interface
  ownership inside the core variant representation rather than leaving them as
  wholly external side cases

### `VARIANT-F4`: `VARIANT_BOOL` semantics

- Microsoft documents `VARIANT_BOOL` as a 16-bit Boolean
- `0xFFFF` means true
- `0x0000` means false
- no other values are valid

Migration implication:

- any canonical bool lane that claims Windows `VARIANT` compatibility must
  preserve this exact representation at the observable boundary

### `VARIANT-F5`: Byref restriction

- Microsoft documents that when a variant uses `VT_VARIANT | VT_BYREF`, the
  referenced variant cannot itself also be `VT_VARIANT | VT_BYREF`

Migration implication:

- any future canonical byref support must preserve this restriction instead of
  allowing recursive byref-to-byref shapes

### `VARIANT-F6`: Wire marshalling truth

- `[MS-OAUT] _wireVARIANT` defines:
  - `clSize`
  - `rpcReserved`
  - `vt`
  - three reserved words
  - a `switch_is(vt)` payload union
- the spec says:
  - fields other than the union are marshalled little-endian
  - `VT_ARRAY`, `VT_BSTR`, `VT_UNKNOWN`, `VT_DISPATCH`, and `VT_RECORD` payloads
    marshal through their own specific rules

Migration implication:

- the migration must distinguish presented in-memory `VARIANT` truth from
  transmitted `_wireVARIANT` truth
- boundary code can become thinner after migration, but it still cannot pretend
  that in-memory and wire forms are identical

### `SAFEARRAY-F1`: Descriptor layout

- Microsoft documents `SAFEARRAY` as:
  - `USHORT cDims`
  - `USHORT fFeatures`
  - `ULONG cbElements`
  - `ULONG cLocks`
  - `PVOID pvData`
  - `SAFEARRAYBOUND rgsabound[1]`
- `SAFEARRAYBOUND` is:
  - `ULONG cElements`
  - `LONG lLbound`

Migration implication:

- a Windows-aligned internal array carrier must preserve explicit rank, element
  size, lower-bound metadata, and backing-data pointer semantics

### `SAFEARRAY-F2`: x64 size posture

- Microsoft documents the field layout but not an explicit x64 descriptor size
- the local x64 ABI probe derived from that layout reported:
  - `SAFEARRAY` with one bound entry: 32 bytes
  - `IntPtr`: 8 bytes

Migration implication:

- descriptor overhead for array-backed value transport is materially larger than
  the current semantic-only `SafeArray` wrapper
- performance and memory reporting must include descriptor cost, not only
  element-payload cost

### `SAFEARRAY-F3`: Dimension ordering and storage order

- Microsoft documents that:
  - `rgsabound[0]` holds the left-most dimension
  - data is stored in column-major order
  - this matches Visual Basic and FORTRAN, not C

Migration implication:

- OxVba's canonical array ordering for the migrated lane must remain column
  major
- any array benchmark or correctness comparison must include rank-2 and lower
  bound cases, not just 0-based vectors

### `SAFEARRAY-F4`: Feature flags matter for release and typing

- the docs explicitly tie `fFeatures` to both element-type information and
  release behavior
- relevant flags include:
  - `FADF_FIXEDSIZE`
  - `FADF_HAVEVARTYPE`
  - `FADF_BSTR`
  - `FADF_UNKNOWN`
  - `FADF_DISPATCH`
  - `FADF_VARIANT`
  - `FADF_RECORD`
  - `FADF_HAVEIID`

Migration implication:

- a true Windows-aligned array substrate cannot treat descriptor flags as
  optional decoration
- string/interface/object cleanup semantics depend on the descriptor truth

### `SAFEARRAY-F5`: `SafeArrayCreateVector` is fixed-size

- Microsoft documents that a `SAFEARRAY` created by `SafeArrayCreateVector`
  always has `FADF_FIXEDSIZE`

Migration implication:

- one-dimensional vector helper lanes and benchmarks should distinguish
  fixed-size Automation arrays from resizable higher-level OxVba array
  abstractions

## Current OxVba Baseline Findings

### `OLD-VARIANT-1`: The checked-in runtime `Variant` is a bounded retained carrier with compatibility-slot adapters

- [variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/variant.rs)
  defines a 16-byte `Variant` with:
  - `VarType`
  - three reserved words
  - an 8-byte `VariantData` union
- its supported `VarType` set is narrow and OxVba-owned:
  `Empty`, `Null`, `Integer`, `Long`, `Single`, `Double`, `Currency`, `Date`,
  `String`, `Object`, `Error`, `Boolean`, `Decimal`, `Byte`, `LongLong`

Migration implication:

- the old runtime already mirrors the basic fixed-size container shape
- it does not yet own the full Windows payload surface or lifetime rules

### `OLD-VARIANT-2`: Current runtime `Variant` support is explicitly partial

- the checked-in bridge
  `Variant::from_runtime_value(...)` rejects:
  - `RuntimeValue::String`
  - `RuntimeValue::ArrayIntent`
  - `RuntimeValue::ObjectHandle`
  - `RuntimeValue::BindingHandle`
- the current runtime test suite pins those rejections

Migration implication:

- the value migration is not about chasing size parity
- it is mainly about making the canonical carrier own payload families that are
  still externalized today

### `OLD-VARIANT-3`: `VarPtr(Variant)` currently projects a real Windows-style container cell on demand

- [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
  materializes `OwnedVariant`
- supported scalar/string/decimal cases are projected into a Windows-compatible
  `VARIANT` cell
- string-valued runtime variants are projected as `VT_BSTR`
- object and array cases still reject explicitly

Migration implication:

- the current old implementation already knows which observable `VARIANT` cell
  shapes it wants to expose
- the migration should make those shapes fall out of the canonical runtime
  carrier instead of rebuilding them ad hoc

### `OLD-SAFEARRAY-1`: The checked-in runtime `SafeArray` is semantic, not a raw descriptor

- [safe_array.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/safe_array.rs)
  stores:
  - `dimensions`
  - `len`
  - optional per-dimension `bounds`
  - optional owned `elements`
- it does not store:
  - `fFeatures`
  - `cbElements`
  - `cLocks`
  - `pvData`
  - a native descriptor header

Migration implication:

- array migration is a real representation shift, not just a rename
- the current semantic wrapper is useful, but it is not equivalent to the
  Windows `SAFEARRAY` descriptor

### `OLD-SAFEARRAY-2`: Current OxVba semantic ordering already matches the Windows column-major rule

- [safe_array.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/safe_array.rs)
  explicitly documents `from_values_nd(...)` as column-major with first
  dimension varying fastest

Migration implication:

- the existing semantic ordering choice is already aligned with Windows/VBA and
  should be preserved

### `OLD-COM-BRIDGE-1`: `oxvba-com` owns the actual Windows `VARIANT`/`SAFEARRAY` seam

- [windows_variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_variant.rs)
  performs the real translation between semantic `ComValue` and Windows
  `VARIANT` / `SAFEARRAY`
- it:
  - allocates `BSTR` payloads
  - converts scalar and typed-array variants
  - collects `SAFEARRAY` bounds
  - reconstructs semantic arrays in column-major order
  - uses `VariantClear` for cleanup

Migration implication:

- the old architecture already isolates the seam correctly
- after migration, this crate should reconcile and validate the canonical
  Windows-style carrier, not remain the place where major value-shape truth is
  invented

### `OLD-EVIDENCE-1`: Current host coverage already pins broad array/result behavior

- [com_client_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_client_end_to_end.rs)
  includes passing end-to-end coverage for:
  - typed `SAFEARRAY` results including `VT_I2`, `VT_UI1`, `VT_I1`, `VT_INT`,
    `VT_UINT`, `VT_I8`, `VT_UI8`, `VT_I4`, `VT_UI4`, `VT_BOOL`, and `VT_BSTR`
  - scalar `VT_BOOL` and `VT_BSTR` results
  - multidimensional typed-array and variant-array result shapes

Migration implication:

- the baseline is more capable than some older workset text suggests
- the migration plan should treat typed-array and rank-2 result preservation as
  an already checked-in compatibility constraint

## Observable Old/New Compatibility Requirements

The migrated implementation must preserve or intentionally re-document at least
the following externally observable truths:

1. scalar `VARIANT` results preserve correct payload typing for the currently
   supported bool, string, numeric, date, currency, decimal, empty, and null
   lanes
2. typed one-dimensional `SAFEARRAY` results preserve element typing and
   semantic order, including `VT_BSTR`
3. multidimensional `SAFEARRAY` and variant-array results preserve rank and
   bounds metadata
4. `VarPtr(v As Variant)` still exposes a true container cell rather than
   collapsing to the payload pointer
5. unsupported payload families remain explicit and deterministic until they are
   fully migrated

## Initial Discretionary-Decision Seeds

These are not resolved by this bead, but this fact pack establishes the input
to later decisions:

1. whether canonical OxVba `Variant` should remain a strict 16-byte raw-compatible
   container everywhere or wrap auxiliary ownership state around that shape
2. whether canonical OxVba arrays should become real `SAFEARRAY` descriptors or
   a descriptor-backed semantic wrapper
3. how much `VT_BYREF` truth should become canonical rather than boundary-only
4. how much of `SAFEARRAY` flag and lock-count behavior should be preserved as
   directly observable runtime truth

## Evidence Commands Run

The following focused checks were run against the current old implementation on
2026-04-20 and passed:

```text
cargo test -p oxvba-runtime com_variant_layout_shape -- --nocapture
cargo test -p oxvba-runtime safe_array_from_values_nd_preserves_multi_dimensional_shape -- --nocapture
cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_accepts_typed_safe_array_variant_results -- --nocapture
cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_multidim_variant_array_results_preserve_two_dimensional_shape -- --nocapture
```

Local x64 ABI size probe run:

```text
PowerShell Add-Type probe derived from documented field layouts:
VARIANT_COMPAT size=16
SAFEARRAY size=32
IntPtr size=8
```
