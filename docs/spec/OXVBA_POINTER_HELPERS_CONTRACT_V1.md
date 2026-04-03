# OxVba Pointer Helpers Contract V1

Date: 2026-04-03
Status: draft

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

## V1 Direction

### Shared Model

- pointer helpers produce an opaque pointer-like runtime value represented as a
  `LongPtr`-compatible quantity
- the value is primarily intended for native interop use
- the runtime owns any temporary backing storage needed to make the pointer
  usable

### `StrPtr`

Intended meaning:
- pointer to UTF-16 string data suitable for Win32-style wide-string native
  calls

V1 direction:
- support `StrPtr` for string expressions
- ensure the pointer remains valid for the duration of the relevant native call
- do not promise general long-lived pointer stability outside that contract

### `VarPtr`

Intended meaning:
- pointer to variable storage / element storage

V1 direction:
- support the ByRef/native-buffer scenarios actually needed by external native
  code
- start with variable and array-element shapes used in real interop probes
- keep writeback/aliasing rules explicit

### `ObjPtr`

Intended meaning:
- pointer identity for object references

V1 direction:
- support only object categories for which OxVba can expose an honest native
  pointer or stable object identity compatible with the intended interop lane
- reject or narrow unsupported object categories explicitly

## Non-Goals For V1

- arbitrary pointer arithmetic as a language guarantee
- exposing raw internal Rust addresses as if they were VBA ABI guarantees
- pretending that every OxVba object has a meaningful COM-compatible `ObjPtr`

## Validation Requirements

- compiler recognition/type tests
- VM/JIT native-call tests for helper-backed flows
- SQLiteForExcel integration rerun
- explicit evidence for supported and unsupported `ObjPtr` cases
