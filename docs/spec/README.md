# OxVba Spec Drafts

This directory contains early-stage OxVba internal design drafts.

Normative external specification sources are maintained in `../Foundation/reference`
(see `docs/FOUNDATION_SPEC_REFERENCE.md`).

Status model:
- `design-draft`: directional, incomplete, expected to change quickly.
- `working-draft`: structured and testable, still open for significant revision.
- `stable-draft`: implementation-linked and evidence-backed; still not final normative text.

Current draft set:
- [`HAL_DESIGN_DRAFT.md`](HAL_DESIGN_DRAFT.md) (`design-draft`): scope, principles, profile targets, and staged design plan for the Host Abstraction Layer.
- [`HAL_INTERFACE_DRAFT.md`](HAL_INTERFACE_DRAFT.md) (`design-draft`): proposed HAL contracts, capability schema, and maturity model.
- [`HAL_CONFORMANCE_DRAFT.md`](HAL_CONFORMANCE_DRAFT.md) (`design-draft`): proposed conformance classes, test obligations, and evidence model.
- [`HAL_SPEC_WORKING_DRAFT.md`](HAL_SPEC_WORKING_DRAFT.md) (`working-draft`): implementation-linked HAL contract, deterministic error model, unsupported-mode semantics, and Windows-only COM decision.
- [`HAL_SPEC_CROSSWALK.md`](HAL_SPEC_CROSSWALK.md) (`working-draft`): capability/intrinsic to Foundation anchor mapping plus known extraction gaps.
- [`HAL_CONFORMANCE_SUITE.md`](HAL_CONFORMANCE_SUITE.md) (`working-draft`): runnable HAL harness layers, commands, artifact schema, and expectations.
- [`HAL_FORMALIZATION_PROGRAM.md`](HAL_FORMALIZATION_PROGRAM.md) (`working-draft`): charter-driven HAL formalization program with 5-step execution ladder and H1/H2/H3 tracks.
- [`HAL_CONTRACT_CLAUSE_CATALOG_V1.md`](HAL_CONTRACT_CLAUSE_CATALOG_V1.md) (`working-draft`): explicit clause ID catalog with pre/postconditions, failure obligations, and verification mapping.

These files intentionally optimize for design velocity and clarity of open decisions rather than immediate lock-in.
