# Workset: Pointer Helpers (`StrPtr` / `VarPtr` / `ObjPtr`) Execution

Date: 2026-04-03
Owner: Codex
Status: in-progress

## Purpose

Plan and execute first-class OxVba support for the three widely used undocumented
VBA pointer helpers:

- `StrPtr`
- `VarPtr`
- `ObjPtr`

This lane exists because realistic native interop code relies on these helpers,
and the SQLiteForExcel integration probe has now reached `StrPtr` as the next
real native-boundary blocker.

## Why This Exists

The current OxVba runtime and compiler support:

- declared native calls,
- `LongPtr`,
- host-backed dynamic linking,
- native FFI marshaling for scalar and string lanes,
- and increasingly realistic VBA/Win32 integration probes.

But OxVba does not yet model the pointer helper functions at all. This is now a
real compatibility gap, not a hypothetical one:

- SQLiteForExcel currently stops at `call to unknown procedure: strptr`
- the same module later uses `VarPtr(buf(0))`
- external interop-heavy VBA code widely treats these helpers as standard tools
  even though Microsoft never documented them as part of the ordinary VBA
  surface

## Reference Posture

This workset is grounded by external historical and practical references, not by
official Microsoft VBA documentation:

- classicvb.net “Unofficial Documentation for VarPtr, StrPtr, and ObjPtr”
- Bruce McKinney / classic VB literature that treats these helpers as real,
  widely used pointer-entry surfaces
- community explanation and usage references such as Stack Overflow posts on
  `StrPtr` / `VarPtr` / `ObjPtr`

The governing consequence is:

- OxVba should support these helpers because they are de facto part of serious
  VBA/VB6 interop practice
- but the contract must still be explicit, because OxVba’s runtime
  representation is not native VB’s runtime representation

## Current Starting Facts

- `StrPtr`, `VarPtr`, and `ObjPtr` are not implemented anywhere in OxVba today.
- current native FFI marshaling supports deterministic scalar lanes and a native
  string lane, but not pointer-helper lanes
- HAL conformance already treats pointer-string marshaling as unsupported today
- the runtime string model is Rust-owned string data, not a native VBA BSTR
  pointer
- the object model uses OxVba runtime handles and host-backed object seams,
  which are not automatically equivalent to COM interface pointers

## Governing Design Direction

1. The helpers should be modeled as real language/runtime surfaces, not as fake
   user-library functions.
2. The contract must be explicit about pointer lifetime and scope.
3. The first implementation priority is real native interop usefulness, not
   pointer arithmetic as a general-purpose programming model.
4. `StrPtr`, `VarPtr`, and `ObjPtr` should share one coherent pointer-helper
   substrate instead of being added as unrelated one-offs.
5. If `ObjPtr` cannot honestly expose a stable native pointer for a given object
   class, the runtime must define that boundary explicitly rather than inventing
   a misleading number.

## Scope

This workset covers:

- pointer-helper research capture and design contract
- compiler recognition and typing for `StrPtr` / `VarPtr` / `ObjPtr`
- runtime/native-call substrate needed to make the helpers useful
- validation against targeted synthetic probes and the SQLiteForExcel fixture
- explicit support boundaries for unsupported host/object cases

This workset does not yet cover:

- arbitrary pointer arithmetic as a full language extension
- general memory-peeking/poking APIs
- undocumented VB helper variants such as `VarPtrArray`

## Required Outcomes

1. OxVba publishes an explicit contract for `StrPtr`, `VarPtr`, and `ObjPtr`.
2. SQLiteForExcel moves past the current `StrPtr` frontier.
3. `VarPtr` is supported enough for the later SQLite buffer lane.
4. `ObjPtr` support or boundary is explicit and tested.
5. Pointer lifetime and host/runtime ownership rules are documented honestly.
6. Validation includes unit coverage and real integration evidence.

## Planned Execution Slices

1. publish the pointer-helper contract and execution lane
2. define the shared runtime/native pointer-helper substrate
3. implement `StrPtr` through the native interop path
4. implement `VarPtr` through the native interop path
5. implement `ObjPtr` through the supported object/native interop path
6. validate synthetic probes and SQLiteForExcel movement
7. publish exact support boundaries and evidence

## Exit Condition

This workset is complete only when:

- the three helpers have an explicit public contract,
- supported execution paths are implemented and tested,
- SQLiteForExcel has been rerun against the moved boundary,
- and any unsupported `ObjPtr` / pointer-lifetime cases are documented as exact
  boundaries rather than implicit gaps.
