# OxVba Pointer Helpers Contract V1

Date: 2026-04-03
Status: accepted design

## Covered Helpers

- `StrPtr(expr)`
- `VarPtr(expr)`
- `ObjPtr(expr)`

## Motivation

These helpers are historically undocumented, but they are widely used in real
VB6/VBA native interop code. OxVba therefore needs an explicit, honest contract
for them.

## Contract Goals

1. Support the common interop scenarios that depend on these helpers.
2. Preserve honest behavior boundaries where OxVba cannot mimic native VBA
   runtime internals exactly.
3. Keep the contract coherent across compiler, VM, JIT, and host/native FFI.

## V1 Design

### Shared Model

- pointer helpers produce an opaque pointer-like runtime value represented as a
  `LongPtr`-compatible quantity
- the value is primarily intended for native interop use
- the runtime owns any temporary backing storage needed to make the pointer
  usable
- helper evaluation flows through one shared compiler/runtime/native-interop
  substrate rather than three ad hoc special cases
- OxVba will not claim that these values are raw leaked Rust addresses or native
  VBA ABI internals
- the contract is defined in terms of the VBA-visible boundary representation
  denoted by the helper call, not in terms of OxVba's internal Rust storage
  layout

### Core Decision

OxVba will support these helpers through an OxVba-owned pointer-helper substrate
with explicit lifetime rules.

That means:

- `StrPtr`, `VarPtr`, and `ObjPtr` are real language/runtime surfaces
- helper results are meaningful first-class interop values
- but their backing storage and validity are defined by OxVba’s contract, not by
  undocumented assumptions about native VBA memory layout

### HAL / Native FFI Consequence

The current HAL contract only accepts:

- `m0-deterministic`
- `m1-native-ffi`

Pointer-helper support therefore requires explicit native marshaling-lane
expansion as part of the shared substrate. The existing pointer-lane rejection
tests remain useful because they prove the current HAL boundary is too small for
this lane.

### `StrPtr`

Intended meaning:
- pointer to the character payload of a valid `BSTR`

V1 design:
- support `StrPtr` for string expressions
- materialize a real `BSTR` allocation owned by the pointer-helper substrate
- return the character-data pointer corresponding to that `BSTR`
- guarantee validity for the duration of the supported native-interop use
- do not promise globally stable pointers across arbitrary later statements

This is required because OxVba strings are Rust-owned semantic values, not
native VBA `BSTR` pointers. The contract does not require OxVba's internal
string representation to be a `BSTR`; it requires `StrPtr` to expose a real
`BSTR` boundary shape when the helper is used.

### `VarPtr`

Intended meaning:
- pointer to variable storage / element storage
- for container values, the pointer targets the container representation rather
  than the payload denoted by helper-specific alternatives such as `StrPtr`

V1 design:
- support the ByRef/native-buffer scenarios actually needed by external native
  code
- start with variable and array-element shapes used in real interop probes
- support canonical zero-based byte-buffer shapes such as `VarPtr(buf(0))`
  through OxVba-owned backing storage suitable for native read/dereference
- for string variables, prefer a native boundary cell whose contents represent a
  `BSTR` reference/value rather than collapsing `VarPtr(s)` into `StrPtr(s)`
- for `Variant` values, prefer a native `VARIANT`-compatible boundary cell for
  the supported interop window, so `VarPtr(v)` points to the container while
  `StrPtr(v)` may still target the current string payload when `v` is
  string-valued
- keep writeback/aliasing rules explicit
- avoid promising arbitrary pointer arithmetic semantics as part of V1

Current bounded V1 support:
- scalar variable pointer production
- canonical zero-based byte-buffer dereference in VM and JIT
- `VarPtr(s As String)` through a native boundary cell whose contents carry a
  real `BSTR` payload pointer
- `VarPtr(v As Variant)` through a native `VARIANT` container cell for the
  supported scalar and string payload lanes
- writable native pointer sync for the current supported `StrPtr(varString)`
  and `VarPtr(buf(0))` shapes is driven by the VBA source expression and the
  materialized boundary kind, not by a special-case list of Windows API names
- no broader native writeback guarantee yet
- object-, array-, and decimal-valued `Variant` container materialization
  remains explicit unsupported territory until OxVba can expose an honest
  boundary shape for those lanes

### `ObjPtr`

Intended meaning:
- pointer identity for object references

V1 design:
- support only object categories for which OxVba can expose an honest native
  pointer or stable object identity compatible with the intended interop lane
- likely first support target is COM/host-backed object references where a real
  interface pointer is available
- reject non-object shapes explicitly, including string-valued `Variant` cases
  whose current payload is not an object reference
- reject or narrow unsupported object categories explicitly

## Non-Goals For V1

- arbitrary pointer arithmetic as a language guarantee
- exposing raw internal Rust addresses as if they were VBA ABI guarantees
- pretending that every OxVba object has a meaningful COM-compatible `ObjPtr`
- claiming that OxVba's internal string or variant storage is itself the native
  ABI representation just because the helper boundary materializes one

## Validation Requirements

- compiler recognition/type tests
- VM/JIT native-call tests for helper-backed flows
- SQLiteForExcel integration rerun
- explicit evidence for supported and unsupported `ObjPtr` cases

## Current Execution Order

1. land the shared substrate and native marshaling-lane expansion
2. move `StrPtr` through the SQLite boundary
3. land `VarPtr` support for the later SQLite/native-buffer lane
4. land `ObjPtr` support or explicit supported-boundary diagnostics
5. tighten the boundary representation so:
   - `StrPtr` exposes a real `BSTR` payload pointer,
   - `VarPtr` points at the supported native storage/container cell,
   - and `ObjPtr` remains restricted to honest object-reference categories

Current status on step 5:
- `StrPtr` now materializes a real `BSTR` and returns its character payload
  pointer on Windows
- `VarPtr(s As String)` now returns the address of a native boundary cell whose
  contents are the `BSTR` payload pointer, preserving the distinction from
  `StrPtr`
- `VarPtr(v As Variant)` now returns the address of a native `VARIANT`
  container cell for supported scalar and string current values
- `Variant` cases carrying object references still reject explicitly because the
  runtime does not yet have an honest COM-interface-pointer boundary to expose
