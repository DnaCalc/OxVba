# WrappedComServer host UDF descriptors PH-0011 evidence

Date: 2026-05-09
Bead: `bd-wcs1.9.1`
Matrix row: `PH-0011`

## Scope

This evidence covers the first host-UDF bridge metadata slice. It proves that
host-callable project procedures are persisted into `descriptor_inventory`
with stable identities, argument and return metadata, and conservative
Excel-informed UDF policy fields suitable for a later DnaOneCalc/OxIde catalog
API.

This is not host formula invocation evidence.

## Commands

```powershell
cargo test -p oxvba-compiler from_compiled_project_persists_descriptor_inventory --quiet
```

## Verified behavior

- `OxBundle::from_compiled_project` persists host-call descriptors in
  `descriptor_inventory.host_calls`.
- Host-call descriptors include stable project/module/procedure identity,
  procedure kind, entry point, parameter slots, return slot, parameter types,
  and return type.
- Host-call descriptors now carry UDF catalog policy metadata:
  `selection_policy`, optional category/description, per-argument description
  slots, volatility, dependency policy, side-effect policy, thread-safety
  policy, and allowed host contexts.
- The covered conservative defaults are:
  `selection_policy=public-procedural-functions`,
  `volatile=false`, `dependency_policy=explicit-arguments-only`,
  `side_effect_policy=no-host-side-effects`,
  `thread_safety_policy=single-threaded-vba-compatible`, and allowed contexts
  `worksheet-cell` plus `host-formula-evaluator`.
- Argument description slots are derived from runtime parameter slot metadata
  when names are available.
- The focused bundle test proves that a public procedural `HostAdd(a As Long,
  b As Long) As Long` descriptor carries two parameter slots, parameter names,
  parameter types, return type, and the policy fields above.
- The same test serializes and deserializes the `.oxb` bundle and verifies the
  host-call descriptor policy fields and argument metadata survive roundtrip.

## Residual

`PH-0011` remains `in-progress`. The current slice publishes descriptor metadata
only. Typed host UDF catalog/invoke APIs, caller context, volatile/dependency
sinks, scalar/array/error result mapping, and DnaOneCalc/OxIde host harness
evidence are still owned by later `bd-wcs1.9` beads.
