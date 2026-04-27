# V0.2 Representation/Layout Doctrine Rollout

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.5.1`
Parent: `bd-bqm8.5`
Status: complete

## Scope

`bd-bqm8.5` is an architecture decision lane. It must reconcile the completed
value-model and compat-slot work with the remaining V0.2 question: whether
OxVba should make raw VBA 7.1 / OLE Automation wire layouts the core runtime
representation, or keep OxVba semantic carriers canonical and translate at
boundary crates.

This rollout does not close the parent lane. It creates the executable path for
the decision, evidence scan, and closure checklist.

## Child Beads

- `bd-bqm8.5.1`: audit and roll out representation/layout doctrine child beads.
- `bd-bqm8.5.2`: publish the V0.2 representation/layout doctrine decision.
- `bd-bqm8.5.3`: scan boundary evidence and classify remaining representation
  risk surfaces.
- `bd-bqm8.5.4`: run the final representation/layout doctrine checklist and
  close `bd-bqm8.5` only if the decision, evidence, and downstream path are
  explicit.

## Initial Findings

- `OPERATIONS.md` already defines the binding external-boundary ownership
  doctrine: OxVba semantic values are canonical; external integration domains
  translate at boundary crates.
- `COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md` already rejects raw
  COM wire structs as canonical VM/compiler values.
- `OXVBA_POINTER_HELPERS_CONTRACT_V1.md` records targeted layout convergence
  where it is needed for native boundary correctness: BSTR payloads, VARIANT
  container cells, SAFEARRAY boundary cells, and retained object pointers.
- `bd-bqm8.2` closed the compat-slot projection seam as an external adapter
  boundary rather than core execution truth.

## Ready Path

The next ready bead is `bd-bqm8.5.2`, which will publish the doctrine decision
and migration path consumed by `bd-bqm8.6`, `bd-bqm8.7`, and `bd-bqm8.10`.
