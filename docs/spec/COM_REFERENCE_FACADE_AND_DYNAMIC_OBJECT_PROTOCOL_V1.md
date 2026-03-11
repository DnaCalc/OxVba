# COM Reference Facade and Dynamic Object Protocol V1

Status: `working-draft`  
Date: 2026-03-11

## Goal

Define the architectural target that lets COM integrate into OxVba without becoming a special execution model inside the compiler, VM, or runtime:

1. at compile time, COM type libraries should look like referenced VBA-visible libraries as far as practical,
2. at runtime, COM-backed objects should adapt into the same internal late-bound object protocol used for OxVba/VBA objects,
3. `oxvba-com` should own the boundary adaptation to and from Windows COM wire semantics.

## Core decision

Binding rules:
1. COM libraries are not a second-class ad hoc name-resolution system.
2. Imported COM type information should present itself to the compiler as synthetic reference/project metadata wherever the VBA model permits that.
3. COM-backed objects are not a separate dynamic-object universe inside the VM.
4. OxVba should converge on one internal late-bound object protocol shaped by VBA semantics.
5. `oxvba-com` implements that protocol for COM-backed objects by adapting it to `IDispatch`/Automation behavior.

## 1. Compile-time facade

### 1.1 Desired effect

When a type library is referenced, OxVba should expose a synthetic reference facade that resembles a normal VBA-visible referenced library:
1. namespaces/type names participate in normal reference precedence,
2. imported classes/interfaces/enums/constants appear in the binder/type system as reference-owned symbols,
3. early-bound member lookup uses metadata from that facade rather than ad hoc runtime probing,
4. default members, events, invoke kind, and signature metadata are attached to those imported symbols.

### 1.2 Consequence

This means:
1. early-bound COM should increasingly look like "resolve a referenced external library symbol and lower it",
2. not like "special COM syntax that bypasses the normal reference/project model".

## 2. Runtime dynamic-object protocol

### 2.1 Desired effect

OxVba should converge on one internal late-bound object protocol for all dynamic object calls, regardless of backing store:
1. native OxVba/VBA objects,
2. COM-backed objects,
3. future non-COM host-backed objects.

### 2.2 Protocol shape

The protocol should be expressed in VBA semantic terms, not COM wire terms:
1. object identity/handle,
2. member identity or member-resolution request,
3. call kind:
   - method
   - property get
   - property let
   - property set
4. named arguments,
5. omitted arguments,
6. default-member intent,
7. object release/lifetime hooks,
8. event subscription/callback identities.

### 2.3 Value boundary

Arguments/results crossing that protocol should use canonical OxVba semantic values:
1. scalar values,
2. strings,
3. null/error states,
4. arrays,
5. object handles.

The protocol must not make raw COM wire structs (`VARIANT`, `DISPPARAMS`, `BSTR`, `SAFEARRAY`) the canonical VM/compiler value model.

## 3. `oxvba-com` responsibility

`oxvba-com` should own:
1. projection of type-library metadata into the synthetic reference facade,
2. binding imported COM members/types/events into compiler-visible metadata,
3. adaptation from the internal dynamic-object protocol to COM:
   - `GetIDsOfNames`
   - `Invoke`
   - `DISPPARAMS`
   - `VARIANT`
   - `EXCEPINFO`
   - connection points/source interfaces
4. adaptation from COM callback/result/object wire forms back into OxVba semantic values and object handles.

## 4. What this avoids

This architecture avoids:
1. raw COM wire types leaking into bytecode/VM as the canonical value representation,
2. a second late-bound call mechanism just for COM,
3. early-bound COM bypassing the normal reference/project model,
4. `oxvba-hal` becoming the permanent home of COM dispatch semantics.

## 5. What it does not remove

This architecture does not remove the need for:
1. a runtime external-object handle model,
2. a runtime dynamic-call carrier,
3. object lifetime/release semantics,
4. callback/event subscription tokens,
5. deterministic error/result translation.

It changes where those things are defined and what semantics shape them.

## 6. Immediate implementation implications

Next design/implementation steps should be:
1. define the internal late-bound object protocol in OxVba semantic terms,
2. define the canonical external-call value carrier used by that protocol,
3. make `oxvba-com` adapt that protocol for COM-backed objects,
4. reshape early-bound COM lowering to consume synthetic reference-facade metadata,
5. contract HAL toward bootstrap/delegation once the above protocol is authoritative.

## 7. Relationship to existing work

This note sharpens, rather than replaces:
1. `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
2. `docs/worksets/WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md`
3. `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`
4. `docs/spec/COM_CLIENT_LATEBOUND_BRIDGE_V1.md`

Use this note as the design constraint when refining those worksets and specs.
