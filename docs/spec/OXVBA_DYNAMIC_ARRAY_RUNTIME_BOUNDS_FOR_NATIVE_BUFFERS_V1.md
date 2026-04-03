# OxVba Dynamic Array Runtime Bounds For Native Buffers V1

Date: 2026-04-05
Status: draft
Owner: Codex

## Purpose

Record the bounded delivery shape needed to move the SQLiteForExcel lane past the
current runtime-sized `ReDim` boundary without pretending OxVba already has a
general dynamic-array model.

Current triggering shape:

```vb
Dim buf() As Byte
bSize = WideCharToMultiByte(...)
ReDim buf(bSize)
RetVal = WideCharToMultiByte(..., VarPtr(buf(0)), bSize, ...)
StringToUtf8Bytes = buf
```

## Current State

Today OxVba supports:

- static array bounds at declaration time
- static `ReDim` / `ReDim Preserve` over compile-time-known bounds
- `VarPtr(buf(0))` for zero-based static byte buffers
- runtime `ArrayIntent` values in the VM/runtime

Today OxVba does not yet support:

- runtime expression bounds in `ReDim`
- dynamic element indexing over runtime-sized arrays
- `VarPtr(buf(0))` over a runtime-resized array slot

The current lowering strategy expands arrays into compile-time alias slots such
as `buf_0`, `buf_1`, etc. That is why `ReDim buf(bSize)` cannot currently be
lowered honestly.

## Bounded V1 Goal

Deliver the smallest honest substrate that unblocks native-buffer helper shapes
like SQLiteForExcel `StringToUtf8Bytes`, without claiming a complete general VBA
dynamic-array implementation.

Bounded target:

- one-dimensional dynamic arrays
- non-`Preserve` `ReDim`
- runtime upper-bound expressions
- byte-compatible arrays first (`Byte`, then possibly `Variant`)
- `VarPtr(buf(0))` over the resized runtime buffer
- assignment/return of the resulting array value

Explicitly out of scope for the first slice:

- multidimensional runtime-sized arrays
- `ReDim Preserve` over runtime-sized arrays
- arbitrary dynamic array element reads/writes at non-constant indices
- full `LBound` / `UBound` semantics over dynamic arrays unless required by the
  bounded fixture lane

## Recommended Delivery Ladder

1. Add a typed dynamic-array `ReDim` substrate in compiler/bytecode/VM
   - a real instruction or instruction family for runtime array allocation
   - store the runtime result in the base array slot
   - keep existing static alias-slot lowering unchanged

2. Add a bounded native-buffer bridge
   - permit `VarPtr(buf(0))` when `buf` is a runtime dynamic array in the base
     slot
   - materialize a byte buffer from the runtime array payload rather than from
     compile-time alias slots

3. Add bounded return/assignment coherence
   - ensure `StringToUtf8Bytes = buf` remains a proper array/variant return
   - preserve existing static-array behavior

4. Then rerun the SQLite matrix
   - raw demo
   - normalized core
   - normalized demo
   - VM and JIT where applicable

## Preferred Internal Model

For this slice, the safest direction is a dual model:

- static arrays continue to use alias-slot lowering
- runtime-sized arrays use the base array slot with `RuntimeValue::ArrayIntent`

That avoids destabilizing existing static-array behavior while creating a clean
bounded lane for native-buffer helpers.

## Acceptance Criteria

- the compiler no longer emits the generic unsupported `ReDim` boundary for the
  SQLite UTF-8 helper lane
- `StringToUtf8Bytes` compiles in both raw and normalized SQLite fixtures
- `VarPtr(buf(0))` works against the runtime-sized buffer in VM and JIT, or the
  supported boundary is narrowed separately and explicitly
- existing static-array regressions continue to pass

## Non-Goals

- do not silently rewrite SQLite source
- do not special-case `StringToUtf8Bytes` by name
- do not claim full VBA dynamic-array parity from this bounded delivery
