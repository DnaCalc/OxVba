# WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_PLANNING_V407_V416

## Scope

Establish the full design baseline for COM early binding and type library consumption before implementation-heavy phases.

Profiles covered: `v407..v416`
Terminal planning gate: `v416`

## Deliverables

1. Source-grounded scope/spec document:
   - `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`
2. Conformance/formal plan document:
   - `docs/spec/COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md`
3. Execution ladder for implementation through terminal gate:
   - `docs/worksets/PROFILE_LADDER_2026-03-05_MACH1000_V407_V466_COM_EARLY_BINDING_TYPELIB.md`
4. Explicit three-iteration cross-reference record integrated in scope doc.
5. Updated repository indexes and logs to make the new plan discoverable.

## Required design decisions at `v416`

1. PMR reference identity and binding-state model for typelibs.
2. HAL trait boundary for resolve/load/cache-invalidate operations.
3. Binder algorithm for early-bound type/member resolution.
4. Dual-interface dispatch strategy policy.
5. Error taxonomy and deterministic propagation model.
6. Formal and conformance lane structure (E0..E6).

## Exit criteria (`v416`)

1. All required decisions are explicit, non-contradictory, and cross-linked.
2. At least one source anchor family is mapped for each major subsystem (PMR, HAL, binder, runtime, conformance).
3. Workset ladder has no undefined intermediate steps between `v407` and `v466`.
4. Open ambiguities are explicitly listed as implementation-defined or deferred-oracle topics.

## Verification commands

- `./scripts/meta-check.ps1 -Fast`
- `rg -n "COM_EARLY_BINDING_TYPELIB" docs/README.md docs/IMPLEMENTATION_LOG.md`
- `rg -n "v407..v466|v416|v466" docs/worksets/PROFILE_LADDER_2026-03-05_MACH1000_V407_V466_COM_EARLY_BINDING_TYPELIB.md`
