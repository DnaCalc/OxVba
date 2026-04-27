# V0.2 Representation/Layout Doctrine Decision

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.5.2`
Parent: `bd-bqm8.5`
Status: complete

## Decision Artifact

Published:

- [OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md)

Decision:

- OxVba semantic runtime values remain canonical internally.
- OLE Automation / VBA 7.1 wire layouts are boundary representations.
- Targeted layout convergence is allowed where native boundary correctness
  requires an honest BSTR, VARIANT cell, SAFEARRAY, or object/interface pointer.
- Boundary crates own translation: `oxvba-com` for COM/OLE Automation,
  `oxvba-hal` for capability/policy/delegation, and `oxvba-runtime` for
  semantic carriers.

## Covered Type Families

- Strings: canonical `BStr` owned payload with bounded pointer-helper lifetime
  guarantees.
- Variant / VARIANT: semantic `Variant` internally, materialized native
  `VARIANT` cells only at required boundaries.
- Date: Date-subtyped OLE Automation serial `f64`, with `VT_DATE` preservation
  at external boundaries.
- Object/interface identity: `ObjectRef` internally, COM pointer/interface
  translation at the boundary.
- Arrays / SAFEARRAY: semantic `SafeArray` internally, real `SAFEARRAY`
  materialization where boundary contracts require it.
- Structures/events: semantic payloads internally, COM event and dispatch wire
  payloads translated by boundary adapters.

## Migration Path

Downstream lanes consume this decision as follows:

- `bd-bqm8.6`: harden malformed/unsupported boundary materialization paths.
- `bd-bqm8.7`: expand Excel and Access/JET COM evidence through `oxvba-com`
  and HAL delegation.
- `bd-bqm8.10`: treat native compilation ABI obligations as wrapper/boundary
  materialization obligations, not a replacement for semantic internal values.

## Verification

Decision sources checked:

- `OPERATIONS.md` external-boundary ownership doctrine
- `COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md`
- `OXVBA_POINTER_HELPERS_CONTRACT_V1.md`
- `V02_COMPAT_SLOT_FINAL_CHECKLIST_2026-04-27.md`
- `V02_COM_HAL_COMPAT_BRIDGE_PROGRESS_2026-04-27.md`

The next ready bead is `bd-bqm8.5.3`, which will scan evidence and classify
remaining representation risk surfaces against this accepted doctrine.
