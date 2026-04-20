# Windows VBA 7.1 x64 BSTR and String-Pointer Fact Pack

Date: 2026-04-20
Owner: Codex
Status: published
Workset: `WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md`
Bead: `bd-t8rr.2.2` / `vmm-b1`

## Scope

This note records the current evidence-backed fact pack for:

- Windows/VBA-facing `BSTR` representation facts relevant to OxVba's internal
  value-model migration
- current checked-in OxVba string and string-pointer behavior at the same
  boundary

The target posture is not "what seems convenient to implement". The target
posture is:

1. actual Windows/VBA observable behavior where we can establish it
2. published Microsoft specifications and API documentation
3. current OxVba behavior only as baseline evidence, not as normative truth

## Primary Source Set

Microsoft primary sources used here:

- `BSTR`:
  https://learn.microsoft.com/en-us/previous-versions/windows/desktop/automat/bstr
- `String Manipulation Functions`:
  https://learn.microsoft.com/en-us/previous-versions/windows/desktop/automat/string-manipulation-functions
- `SysStringLen`:
  https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-sysstringlen
- `SysAllocStringLen`:
  https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-sysallocstringlen
- `SysAllocStringByteLen`:
  https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-sysallocstringbytelen
- `[MS-OAUT] 2.2.23.2 BSTR Type Definition`:
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/1c9d2cfc-cf7d-4f4b-95bf-584be5defd81

Checked-in OxVba source/test evidence used here:

- [bstr.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/bstr.rs)
- [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
- [windows_variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_variant.rs)
- [pointer_helpers_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/pointer_helpers_end_to_end.rs)
- [native_declare_string_marshalling_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs)
- [OXVBA_POINTER_HELPERS_CONTRACT_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/OXVBA_POINTER_HELPERS_CONTRACT_V1.md)
- [POINTER_HELPERS_STATUS_2026-04-04.md](/C:/Work/DnaCalc/OxVba/docs/evidence/POINTER_HELPERS_STATUS_2026-04-04.md)

## Confirmed Windows/Automation Facts

### `BSTR-F1`: Nominal type and pointer target

- `BSTR` is declared as `OLECHAR*` / `WCHAR*`.
- The `BSTR` value points to the first character of the data string, not to the
  length prefix.
- On Windows x64, the pointer value itself is therefore 8 bytes. This last
  point is an ABI inference from Win64 pointer size, not a special `BSTR`
  clause.

Migration implication:

- a Windows VBA 7.1 x64-aligned internal string carrier should expose a payload
  pointer shape that is compatible with `WCHAR*`/`OLECHAR*` expectations
- helper surfaces that target payload should continue to target the character
  payload, not an OxVba-private header address

### `BSTR-F2`: Composite layout

- `BSTR` is a composite data type consisting of:
  - a 4-byte length prefix
  - a Unicode data string
  - a terminating `WCHAR(0)`
- the length prefix is stored immediately before the first character
- the length prefix counts bytes in the data string and does not include the
  terminator

Migration implication:

- the migrated string carrier should be designed around a 4-byte byte-length
  prefix plus UTF-16 payload plus 2-byte terminator
- any "nearly native" design that changes the prefix width, removes the
  terminator, or stores a character count instead of a byte count would still
  require compensating translation and would not actually match the Windows
  substrate

### `BSTR-F3`: Length semantics and embedded nulls

- `BSTR` may contain embedded null characters
- `SysStringLen` returns the number of characters, not including the terminator
- `SysStringLen` returns the character count specified when the string was
  allocated with `SysAllocStringLen`, even if the payload contains embedded
  nulls
- `SysAllocStringLen` copies the specified character count and appends a null
  terminator

Migration implication:

- OxVba must preserve explicit length and must not let observable length depend
  on scanning for the first null
- internal string operations and benchmark lanes need embedded-null coverage,
  not just ordinary text coverage

### `BSTR-F4`: Null and empty semantics

- the Automation string-manipulation guidance says a null `BSTR` pointer is a
  valid value and is conventionally treated the same as a pointer to a zero
  character `BSTR`
- the MS-OAUT wire definition distinguishes transmitted null from transmitted
  empty:
  - transmitted null uses a `FLAGGED_WORD_BLOB` null marker
  - transmitted empty uses zero `cBytes`, zero `clSize`, and no payload

Migration implication:

- internal design must treat "null versus empty" deliberately
- the minimum requirement is correct boundary behavior for both cases
- whether the canonical in-process OxVba carrier preserves the distinction
  internally is a discretionary design choice that must be recorded separately

### `BSTR-F5`: Allocation and ownership rules

- `BSTR` allocation and release are defined through the Automation allocation
  functions such as `SysAllocString`, `SysAllocStringLen`, `SysFreeString`, and
  related reallocation helpers
- Microsoft explicitly documents that Automation may cache freed `BSTR` blocks
  and that this can affect allocator-observation tooling

Migration implication:

- the migrated carrier should own `BSTR`-style allocation/free behavior as a
  runtime concern rather than only as a COM-boundary concern
- benchmark interpretation must not assume that raw allocator counts map
  one-to-one to logical string lifetimes on Windows

### `BSTR-F6`: Binary/byte-count edge lane

- `SysAllocStringByteLen` exists and allocates a `BSTR` by byte count
- Microsoft describes it as a binary-data edge function and explicitly warns
  against using it where ANSI/Unicode translation may occur

Migration implication:

- this is not the canonical VBA text-string construction path
- it remains relevant as an interop edge case and should stay in the migration
  risk register, especially if OxVba later exposes undocumented byte-oriented
  `BSTR` behavior

### `BSTR-F7`: Alignment posture

- the public sources used here do not state a stronger required alignment than
  "valid `OLECHAR*` / `WCHAR*` string data allocated by Automation"
- the docs do state the exact prefix size, payload element type, and terminator
  type

Migration implication:

- the migration should preserve layout facts that are actually documented
- stronger heap-alignment assumptions should be treated as undocumented and
  must not become correctness dependencies unless a stronger source or direct
  VBA evidence is added

## Current OxVba Baseline Findings

### `OLD-BSTR-1`: Canonical string carrier is still semantic-first Rust `String`

- [bstr.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/bstr.rs) defines:
  `pub struct BStr(pub String);`
- the checked-in old runtime therefore stores canonical strings as Rust-owned
  UTF-8 `String`, not as Windows-style UTF-16 `BSTR`

Migration implication:

- the migration is a real substrate replacement, not a minor boundary cleanup

### `OLD-BSTR-2`: COM translation still allocates real `BSTR` values at the boundary

- [windows_variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_variant.rs)
  allocates a `BSTR` with `SysAllocString` when projecting semantic text to COM
- that same boundary reconstructs semantic text from `BSTR` using `SysStringLen`

Migration implication:

- the current implementation already exercises real Automation allocation and
  length behavior on the COM seam
- after migration, that seam should collapse toward ownership transfer and
  compatibility validation rather than re-encoding the canonical payload

### `OLD-BSTR-3`: `StrPtr` on Windows already exposes a real BSTR payload pointer

- [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
  builds `OwnedBstr` with `SysAllocString`
- its helper pointer is the character-payload pointer, not a pointer to the
  prefix
- host evidence in
  [pointer_helpers_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/pointer_helpers_end_to_end.rs)
  shows `wcslen(StrPtr(":memory:")) = 8` in both VM and JIT

Migration implication:

- the migrated runtime can turn a currently synthesized Windows-observable truth
  into the canonical internal truth, but it must preserve the same helper
  boundary behavior

### `OLD-BSTR-4`: `VarPtr(String)` currently exposes a BSTR cell, not the payload pointer

- [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
  materializes `OwnedBstrCell`
- the cell stores the `BSTR` payload pointer value
- host evidence in
  [pointer_helpers_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/pointer_helpers_end_to_end.rs)
  asserts that dereferencing the cell yields a `BSTR` whose `SysStringLen` is 5
  for `"alpha"`

Migration implication:

- the migration must preserve the distinction between `StrPtr(s)` and
  `VarPtr(s)`
- this is one of the main reasons a true Windows-style internal carrier is
  attractive: the helper can stop fabricating a container shape that the runtime
  itself does not own

### `OLD-BSTR-5`: `VarPtr(Variant)` with string current value currently exposes a `VT_BSTR` container

- [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
  materializes `OwnedVariant`
- string-valued runtime variants are projected as `VT_BSTR`
- host evidence in
  [pointer_helpers_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/pointer_helpers_end_to_end.rs)
  asserts both `vt = VT_BSTR` and `SysStringLen(payload) = 5`

Migration implication:

- the new string carrier must integrate cleanly with the later `VARIANT`
  migration lane and preserve this observable container truth

### `OLD-BSTR-6`: Native writeback through `StrPtr(varString)` is already supported in the checked-in Windows lane

- host evidence in
  [native_declare_string_marshalling_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs)
  shows `MultiByteToWideChar(..., StrPtr(resultText), ...)` updating
  `resultText` to `"alpha"` in both VM and JIT
- that same test file also records that the writeback behavior is driven by the
  source-expression shape, not by special casing the declared Windows API name

Migration implication:

- the migrated carrier must keep a writable wide-string target lane for
  supported native interop calls
- writeback semantics need to be benchmarked as part of the string migration,
  not only correctness-tested

### `OLD-BSTR-7`: Non-Windows helper fallback is still a compatibility shim, not Windows truth

- [pointer_helpers.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-runtime/src/pointer_helpers.rs)
  uses a temporary UTF-16 buffer on non-Windows builds instead of a real `BSTR`

Migration implication:

- the migration target is intentionally stronger than the current cross-platform
  fallback
- all-platform convergence onto the Windows VBA 7.1 x64 substrate is a real
  behavioral change in representation, even where current helper behavior is
  already sufficient for limited interop

## Observable Old/New Compatibility Requirements

The migrated implementation must preserve or intentionally re-document at least
the following externally observable truths:

1. `StrPtr(s)` targets the string payload, not a prefix or OxVba-private object.
2. `VarPtr(s As String)` targets a container cell whose contents denote a
   `BSTR` reference/value rather than collapsing to `StrPtr(s)`.
3. `VarPtr(v As Variant)` with current string value exposes a `VT_BSTR`
   container shape.
4. embedded-null-aware length semantics remain correct at the Automation
   boundary.
5. writable native wide-string targets continue to synchronize back into the
   owning OxVba string variable for the supported interop lane.

## Initial Discretionary-Decision Seeds

These are not resolved by this bead, but this fact pack establishes the input
to later decisions:

1. whether canonical OxVba string state will distinguish internal null and empty
   string representations or normalize them internally while preserving boundary
   truth
2. whether OxVba will model any observable `BSTR` caching behavior beyond what
   falls out of the host allocator/runtime implementation
3. whether byte-oriented `BSTR` construction edge lanes such as
   `SysAllocStringByteLen` need first-class migration tests or can remain
   secondary interop cases
4. whether any stronger-than-documented alignment assumption is required by real
   VBA/Excel probes

## Evidence Commands Run

The following focused host tests were run against the current old
implementation on 2026-04-20 and passed:

```text
cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::strptr_supports_wide_native_call_in_vm_and_jit -- --exact --nocapture
cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::varptr_string_variable_exposes_bstr_container_cell_in_vm_and_jit -- --exact --nocapture
cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::varptr_variant_variable_exposes_variant_container_in_vm_and_jit -- --exact --nocapture
cargo test -p oxvba-host --test native_declare_string_marshalling_end_to_end windows_native_declare_string_e2e::multibytetowidechar_strptr_target_writes_back_string_slot_in_vm_and_jit -- --exact --nocapture
```
