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
- pointer to UTF-16 string data suitable for Win32-style wide-string native
  calls

V1 design:
- support `StrPtr` for string expressions
- materialize a NUL-terminated UTF-16 backing buffer owned by the pointer-helper
  substrate
- guarantee validity for the duration of the supported native-interop use
- do not promise globally stable pointers across arbitrary later statements

This is required because OxVba strings are Rust-owned values, not native VBA
BSTR pointers.

### `VarPtr`

Intended meaning:
- pointer to variable storage / element storage

V1 design:
- support the ByRef/native-buffer scenarios actually needed by external native
  code
- start with variable and array-element shapes used in real interop probes
- keep writeback/aliasing rules explicit
- avoid promising arbitrary pointer arithmetic semantics as part of V1

### `ObjPtr`

Intended meaning:
- pointer identity for object references

V1 design:
- support only object categories for which OxVba can expose an honest native
  pointer or stable object identity compatible with the intended interop lane
- likely first support target is COM/host-backed object references where a real
  interface pointer is available
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

## Current Execution Order

1. land the shared substrate and native marshaling-lane expansion
2. move `StrPtr` through the SQLite boundary
3. land `VarPtr` support for the later SQLite/native-buffer lane
4. land `ObjPtr` support or explicit supported-boundary diagnostics
