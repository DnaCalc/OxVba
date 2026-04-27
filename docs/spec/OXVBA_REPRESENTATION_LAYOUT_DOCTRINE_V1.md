# OxVba Representation and Layout Doctrine V1

Date: 2026-04-27
Status: accepted for V0.2
Owner: Codex
Workset: `bd-bqm8.5`

## Decision

OxVba keeps semantic OxVba runtime values as the canonical internal execution
model. Raw VBA 7.1 / OLE Automation wire layouts are boundary representations,
not the core compiler, bytecode, VM, JIT, or host value model.

The permanent doctrine is boundary translation with targeted layout convergence
only where correctness requires an honest native boundary object.

This means:

- `oxvba-runtime` owns semantic carriers such as `Variant`, `RuntimeValue`,
  `BStr`, `SafeArray`, and `ObjectRef`.
- `oxvba-com` owns translation to and from COM/OLE Automation wire forms such
  as `VARIANT`, `BSTR`, `SAFEARRAY`, `IUnknown`, `IDispatch`, `DISPPARAMS`,
  `EXCEPINFO`, and connection-point event payloads.
- `oxvba-hal` owns capability gating, profile selection, and delegation seams;
  it must not become the owner of COM wire-format semantics.
- VM/JIT bytecode and execution slots must not treat raw external wire structs
  as canonical execution truth.

## Type-Level Doctrine

### Strings

The internal string carrier is `BStr`, an owned BSTR-shaped UTF-16 payload.
This is targeted layout convergence because VBA/native interop requires honest
BSTR payload pointers for `StrPtr`, `VarPtr(String)`, COM, and native calls.

The runtime still does not promise arbitrary pointer stability across unrelated
later statements. Pointer helper lifetimes remain bounded to the supported
native-interop window.

### Variant / VARIANT

`Variant` is the canonical semantic container. It carries a VBA-visible
`VarType` plus owned payload state and conversion helpers.

Native `VARIANT` cells are materialized only at boundaries that require them:
COM marshaling, native pointer-helper cells, and supported external call
surfaces. The in-memory Rust `Variant` API is not a promise that every VM/JIT
carrier is byte-identical to a Windows `VARIANT`.

### Date

`Date` is represented semantically as an OLE Automation serial `f64` with a
Date subtype. Boundary code must preserve `VT_DATE` where the external contract
requires it. The core runtime should not reintroduce packed date integers as
execution truth; packed date compatibility remains an explicit adapter concern.

### Object / Interface Identity

`ObjectRef` is the internal object identity carrier. For supported object
categories it retains an honest runtime `IUnknown`-shaped pointer identity.

COM-backed objects are adapted by `oxvba-com`/HAL standard adapters into the
same internal object protocol used by other OxVba objects. VM/JIT code should
operate on semantic object identity and member intent, not raw COM vtables or
`IDispatch` calls.

### Arrays / SAFEARRAY

`SafeArray` is the canonical array carrier for the current runtime value model.
Boundary code may materialize real `SAFEARRAY` values where COM, native calls,
or pointer helpers require them. Multi-dimensional bounds and element types
remain semantic runtime metadata until a boundary materialization is required.

### Structures and Event Payloads

Structure and event payloads use semantic OxVba values internally. COM event
callbacks and dispatch payloads cross through `oxvba-com`/HAL adapters, which
translate to or from `DISPPARAMS`, `VARIANT`, connection-point metadata, and
callback tokens.

Raw event wire payloads must not become the canonical VM or compiler payload
format.

## Migration Path

The V0.2 migration path is:

1. Preserve the completed compat-slot excision rule: legacy slot projection
   remains an explicit adapter, never core execution truth.
2. Keep targeted boundary cells where they are already required for correctness:
   BSTR payloads, VARIANT cells, SAFEARRAY cells, and retained object pointers.
3. Route new COM and native-boundary features through the owning boundary crate
   rather than threading raw wire structs into VM/JIT APIs.
4. Classify remaining representation risks as boundary risks, not as a mandate
   to rewrite the core runtime into raw OLE Automation layouts.
5. Use downstream hardening, COM corpus, and native-compilation beads to add
   evidence for the boundary materialization paths they depend on.

## Consequences

- `bd-bqm8.6` should harden malformed or unsupported boundary materialization
  paths without reopening the canonical runtime value model.
- `bd-bqm8.7` should expand Excel/Access/JET COM evidence through `oxvba-com`
  and HAL delegation, not by adding COM-specific execution rules to the VM.
- `bd-bqm8.10` should treat native compilation ABI obligations as wrapper or
  boundary obligations. Native compilation must preserve the semantic value
  model internally unless a declared external ABI requires materialization.

## Non-Goals

- No V0.2 rewrite to make every internal carrier byte-identical to Windows
  Automation structs.
- No raw `VARIANT`, `DISPPARAMS`, `SAFEARRAY`, `BSTR`, `IUnknown`, or
  `IDispatch` threading through bytecode as the primary value model.
- No broad promise that pointer-helper materialized cells are globally stable
  beyond the documented supported interop window.
