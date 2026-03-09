# Review Followup Triage — 2026-03-09

Triaged from `docs/REVIEW_20260309.md` using `docs/REVIEW_20260309_TRIAGE_PLAN.md`.

These are the few items that need an explicit project decision before clean execution can continue.

## Resolved Followup

## [F-01] Future of `oxvba-com`

- Status: resolved
- Source: `docs/REVIEW_20260309.md` sections `H6`, `CB-1`, `CB-2`, `CB-13`, `Architecture and Crate Structure`
- Additional sources: user decision on 2026-03-09
- Summary: The project decision is to repurpose `oxvba-com` into the Windows-first bidirectional COM bridge rather than delete it or preserve it as a tiny scaffold crate.
- Why it matters: maintainability | delivery
- Decision: resolved in favor of repurpose/extraction.
- Rationale: COM has grown into a transport/integration domain that fits poorly inside generic HAL traits. `oxvba-com` will become the home for COM state, client/server bridge logic, and COM-specific fixtures; `oxvba-hal` will contract back toward host capability/profile/bootstrap responsibilities.
- Duplicates merged: `H6`, `CB-1`, `CB-2`, `CB-13`, architecture concerns
- Next step: execute the staged plan in `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`.

## [F-02] Lock the Host-Bridge Boundary for Object Values and Event Ingress

- Status: resolved
- Source: `docs/REVIEW_20260309.md` sections `PR-1`, `PR-10`, `PR-11`, `PR-12`
- Additional sources: general section `8`
- Summary: The host/tooling proposal needed one explicit contract for object-typed values, collection/default-member dispatch, host-to-engine event ingress, and host error mapping posture. That contract is now locked.
- Why it matters: compatibility | delivery
- Decision: resolved in favor of a single `Variant` boundary plus explicit engine-side event ingress.
- Rationale: This keeps the bridge pragmatic, aligns with the existing proposal shape, and avoids hiding event-dispatch behavior inside unrelated bridge operations.
- Duplicates merged: `PR-1` suggestions 2-5, `PR-10`, `PR-11`, `PR-12`, general item `8`
- Next step: execute against `docs/worksets/WORKSET_2026-03-09_HOST_BRIDGE_OBJECT_VALUE_AND_EVENT_INGRESS_CONTRACT.md` and the updated host/tooling proposal.

## [F-03] Choose the DNA VbCalc Pathfinder Host Shell Baseline

- Status: resolved
- Source: `docs/REVIEW_20260309.md` section `PR-2`
- Additional sources: `PR-5`, `PR-11`
- Summary: The pathfinder proposal needed one practical host-shell baseline decision. That baseline is now locked as a Tauri desktop shell with a Rust backend and web UI frontend, opening `oxvba.toml` projects and presenting a debug/immediate-style shell as the first user-facing surface.
- Why it matters: delivery
- Decision: resolved in favor of a debug-centric Tauri baseline, recorded as a preparatory note for a future separate `DnaVbCalc` repository rather than as an OxVba workset.
- Rationale: This provides a concrete first scope without polluting the OxVba workset queue with implementation planning for a separate repository.
- Duplicates merged: `PR-2` suggestions 1-3, `PR-5` P5-critical-path note, `PR-11` risk 2
- Next step: use `docs/DNAVBCALC_HOST_SHELL_BASELINE_PREPARATION_2026-03-09.md` as the seed document when the separate `DnaVbCalc` repository is created.
