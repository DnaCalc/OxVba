# Descriptor-Backed UDT Semantic Model Path

Date: 2026-05-01
Bead: `bd-9xmu.3.7` / `value-clean-006`
Workset: `WORKSET_2026-04-30_VALUE_SUBSTRATE_NUMERIC_UDT_CLEANUP.md`

## Current baseline

Current UDT support is a bounded non-boundary subset:

- compiler parsing collects UDT definitions in `UdtDefMap`;
- nested fields are expanded by compiler-side flattening;
- field access/assignment uses flattened field aliases;
- whole-UDT copy lowering is emitted only when source and target share declared
  UDT identity;
- same-shape/same-field cross-type assignment is rejected.

This is not a native layout model and not a general UDT-byref ABI model.

## Descriptor model v1

The first descriptor-backed semantic model should introduce stable descriptor
identity without changing retained `Variant` field storage immediately.

### Identifiers

- `UdtTypeId(u32)`: stable within a compiled project/bundle; identifies a
  declared `Type ... End Type` by project/module/name.
- `UdtFieldId(u32)`: stable within a `UdtTypeId`; identifies declared field
  order before any lowered storage expansion.

### Type descriptor

`UdtTypeDescriptor` should contain:

- `id: UdtTypeId`
- `project_name: Option<String>`
- `module_name: String`
- `declared_name: String`
- `fields: Vec<UdtFieldDescriptor>` in declaration order
- case-insensitive field lookup map
- copy/init policy (`zero_init`, `copy_by_field`, future `blocked` variants)

### Field descriptor

`UdtFieldDescriptor` should contain:

- `id: UdtFieldId`
- `declared_name: String`
- declared type (`Variant`, scalar tag, `String`, object, nested UDT, or array)
- declaration order
- optional fixed array bounds
- optional nested `UdtTypeId`
- lowered field-slot span for the current compiler/runtime implementation

### Runtime storage v1

- A UDT variable is a descriptor-backed logical aggregate over retained
  `Variant` field slots.
- No native struct overlay or platform packing is implied.
- Nested UDT fields retain nested `UdtTypeId` identity even if storage remains
  flattened into field slots.
- Whole-UDT copy uses descriptor identity and declared field descriptors, not
  suffix/name heuristics alone.
- Field initialization is descriptor-driven zero/empty initialization per field
  declared type.

## Implementation path

1. Promote `UdtFieldDef` / `UdtDefMap` in `crates/oxvba-compiler/src/resolve.rs`
   into public/internal descriptor structs or a dedicated `udt_descriptor`
   module.
2. Assign deterministic `UdtTypeId` and `UdtFieldId` during project/module
   resolution.
3. Carry descriptors into bound statements and bytecode metadata, initially as
   metadata only while preserving existing lowered field slots.
4. Replace same-type whole-copy inference with descriptor identity checks.
5. Route field initialization/copy through descriptors and add regression tests
   for nested UDT identity, same-shape cross-type rejection, and fixed-array
   field descriptor bounds.
6. Defer aggregate `Variant` UDT carrier or native ABI materialization until a
   later workset explicitly takes that storage/ABI scope.

## Non-goals

- Native memory layout parity.
- Arbitrary UDT-byref `Declare` marshaling.
- Struct overlay / packing equivalence.
- Arrays of UDT as native ABI payloads.

Those are classified separately by `bd-9xmu.3.8`.

## Verification

Documentation/support-only bead. Validation command:

```text
cargo check --workspace
```

Result: passed.
