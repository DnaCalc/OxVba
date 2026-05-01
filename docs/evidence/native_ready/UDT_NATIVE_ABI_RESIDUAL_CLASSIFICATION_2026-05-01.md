# UDT Native ABI Residual Classification

Date: 2026-05-01
Bead: `bd-9xmu.3.8` / `value-clean-007`
Workset: `WORKSET_2026-04-30_VALUE_SUBSTRATE_NUMERIC_UDT_CLEANUP.md`

## Boundary statement

Internal UDT semantics and native ABI materialization are separate layers.

The current/native-ready value substrate may use descriptor-backed logical UDT
semantics over retained `Variant` field slots. That does not imply platform
struct layout, packing, or unconstrained UDT-byref `Declare` parity.

## Classification table

| Surface | Classification | Owner/follow-up | Notes |
|---|---|---|---|
| UDT declaration parsing | accepted current subset | compiler/value substrate | Existing parser/resolver handles bounded `Type ... End Type` declarations. |
| Flattened UDT field access/assignment | accepted current subset | compiler/value substrate | Preserved until descriptor-backed field-slot metadata replaces suffix heuristics. |
| Same-declared-type whole UDT copy | accepted current subset | compiler/value substrate | Current lowering is allowed only for matching declared identity. |
| Same-shape cross-type UDT assignment | accepted rejection | compiler/value substrate | Rejection is required; shape alone is not identity. |
| Nested UDT field expansion | accepted current subset | compiler/value substrate | Descriptor model must preserve nested `UdtTypeId` even if storage remains flattened. |
| Descriptor-backed semantic storage | planned phase path | value substrate | See `UDT_DESCRIPTOR_MODEL_PATH_2026-05-01.md`. |
| Arrays of UDT as internal semantic values | deferred | value substrate follow-up | Needs descriptor-backed aggregate/array policy before native ABI. |
| Native `Declare` UDT ByRef/ByVal marshaling | deferred/blocking for native ABI | future native ABI workset | Requires platform layout facts, packing/alignment rules, and writeback semantics. |
| Struct overlay / arbitrary memory reinterpretation | out of scope | future native ABI workset only if explicitly accepted | Not implied by semantic UDT support. |
| COM record/UDT transport | deferred | COM/native boundary follow-up | Requires COM record metadata and VARIANT/SAFEARRAY record handling beyond current scope. |
| Platform packing/alignment parity | deferred/blocking for native ABI | future native ABI workset | Must be handled by ABI materializer, not internal value substrate. |
| Direct native PE/ELF emission relying on UDT layout | blocked until ABI materializer exists | future native compiler/linker workset | Native compiler cannot assume internal UDT field-slot storage is ABI layout. |

## Guardrails

- Descriptor-backed UDT semantics may proceed without native layout parity.
- Native ABI materialization must consume descriptors and an ABI layout policy;
  it must not infer layout from compiler flattened field aliases.
- Any future native ABI workset must record platform-specific facts for packing,
  alignment, string/object fields, fixed arrays, nested UDTs, and writeback.

## Verification

Documentation/support-only bead. Validation command:

```text
cargo check --workspace
```

Result: passed.
