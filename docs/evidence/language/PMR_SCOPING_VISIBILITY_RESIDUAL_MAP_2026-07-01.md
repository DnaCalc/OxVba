# PMR Scoping Visibility Residual Map

Date: 2026-07-01
Owner bead: `bd-4ktq.38.1`

## Purpose

This map reconciles the closed vm3 multi-module scoping batches
(`bd-4ktq.9` and `bd-4ktq.36`) with the older PMR module/project
requirements and conformance surfaces.

The key distinction is scope: the vm3 scoping evidence proves a focused
multi-module and referenced-project subset, while several PMR rows remain
broader because they also cover host-project catalogs, external type libraries,
storage, full event lifecycle, or wider project/model semantics.

## Evidence Anchors

- `docs/evidence/conformance/vm3_scoping_visibility_oracle_20260701T0945Z/`
- `docs/evidence/conformance/vm3_scoping_followup_oracle_20260701T1655Z/`
- `crates/oxvba-differential/tests/scoping_visibility_vm3.rs`
- `docs/evidence/conformance/PMR_PROJECT_MODEL_FIXTURE_MATRIX_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md`

## Residual Map

| PMR surface | Rows | Current scoped truth | Residual owner |
|---|---|---|---|
| Project references and qualifiers | `MODPROJ-005`, `MODPROJ-016`, `PMR-NAME-003`, `PMR-REF-001`, `CCT-037`, `ODG-035` | The vm3 subset is proved for active-project shadowing, first-reference precedence, referenced module/project qualification, wrong-project rejection, duplicate referenced globals, and ambiguous-reference fallback blocking. Evidence: `SCOPING-XREF-BASELINE`, `SCOPING-XREF-MODULE-QUALIFIED`, `SCOPING-XREF-PROJECT-QUALIFIED`, `SCOPING-XREF-PRECEDENCE`, and the matching `scoping_visibility_vm3` tests. | `bd-4ktq.38.2` reconciled PMR row wording. Broader external/type-library and broken-reference behavior stays outside this scoping subset under `ODG-041` and related COM/reference work. |
| Public namespace collision diagnostics | `MODPROJ-018`, `MODPROJ-019`, `MODPROJ-021`, `PMR-VIS-002`, `PMR-VIS-003`, `PMR-NAME-001`, `PMR-NAME-002` | The vm3 subset is proved for duplicate public procedures, module-name/public-member collisions, Public Const/Public variable ambiguity, Public UDT/Public Enum ambiguity, and legal module/project-qualified access. Evidence: `SCOPING-DUP-PUBLIC`, `SCOPING-MODULE-MEMBER-COLLISION`, `SCOPING-CONST-VAR-COLLISION`, `SCOPING-UDT-ENUM-COLLISION`, and the matching `scoping_visibility_vm3` tests. | `bd-4ktq.38.3` reconciled PMR row wording. Broader project/module/library namespace edges remain partial. |
| `Option Private Module` reference and host boundaries | `MODPROJ-017`, `MODPROJ-039`, `PMR-VIS-001`, `PMR-VIS-004`, `CCT-038`, `ODG-036` | The vm3 subset is proved for hiding referenced `Option Private Module` members from external projects, preserving same-project access, preserving normal public referenced modules, and distinguishing that from the host-direct invocation/export contract. Evidence: `SCOPING-OPTION-PRIVATE-XREF`, `option_private_module_*` tests, and the earlier `pmr_project_model_20260303T070427Z` host-direct oracle. | `bd-4ktq.38.4` reconciled PMR row wording. Broader host catalog and host/HAL project-public-entity visibility remains outside this scoped vm3 batch. |
| `WithEvents` source visibility and handler-prefix binding | `MODPROJ-022`, `MODPROJ-023`, `PMR-CLS-001`, `PMR-CLS-002`, `CCT-041`, `ODG-039`, `DIV-0004` | The vm3 subset is proved for procedural-module `WithEvents` rejection, scalar/explicit-Variant/implicit-Variant/array `WithEvents` source-type rejection, active-project and referenced-project event source visibility, handler-prefix routing, mismatch non-routing, and private/non-exposed referenced source rejection. Evidence: `SCOPING-WITHEVENTS-ACTIVE`, `scanner_rejects_withevents_in_standard_modules`, `scanner_rejects_non_object_withevents_fields`, `withevents_scalar_field_type_is_bind_error`, and the matching `scoping_visibility_vm3` tests. | `bd-4ktq.38.5` reconciled PMR row wording. `MODPROJ-023` is now `partial` for the handler-prefix/source-visibility subset; the 2026-07-06 FE-8.5.f slice adds a compile-time source-type guard without claiming full event-source verification for every accepted object type. Full event ordering, lifecycle cleanup, COM event parity, and broader reassignment semantics remain outside this scoping subset under `DIV-0004` and event/COM work. |

## Non-Scoping Rows

The following PMR rows appeared during the audit but are not delivery scope for
`bd-4ktq.38`: host project public-entity visibility (`MODPROJ-006`), open host
project extensibility (`MODPROJ-007`), broad module-kind runtime semantics
(`MODPROJ-008`), source-flattening retirement (`MODPROJ-015`), Implements
edge cases (`MODPROJ-025`), class instancing (`MODPROJ-026`), type-library and
OAUT reference binding (`MODPROJ-032`, `MODPROJ-033`, `ODG-041`), conditional
compilation (`MODPROJ-034`), storage/roundtrip (`MODPROJ-035`, `MODPROJ-036`),
circular dependencies (`MODPROJ-037`), and startup/project configuration
oracle work (`ODG-043`).

## Row-Status Decisions

- No unresolved status/evidence candidate remains for `bd-4ktq.38.2` through
  `bd-4ktq.38.5`; the concrete reconciliation decisions are listed below.
- Reconciled in `bd-4ktq.38.2`: `MODPROJ-005` and `MODPROJ-016` now point to
  the scoped vm3 referenced-project and qualifier fixtures while staying
  `partial` for external type-library, broken-reference, library/type-space,
  and broader reference-boundary edges.
- Reconciled in `bd-4ktq.38.3`: `MODPROJ-018`, `MODPROJ-019`,
  `PMR-VIS-003`, and the current qualified-name anchors now point to live vm3
  and symbol fixtures for duplicate public procedures, module-name/member
  collisions, Public Const/Public variable ambiguity, UDT/Enum ambiguity, and
  legal qualified access. They remain `partial` where their row scope reaches
  broader project/module/library namespace behavior.
- Reconciled in `bd-4ktq.38.4`: `MODPROJ-017`, `MODPROJ-039`, and
  `PMR-VIS-001` now point to live vm3/symbol fixtures for the referenced-project
  `Option Private Module` boundary and to historical CCT-038 oracle evidence
  for the host-direct invocation distinction. Rows stay `partial` where their
  scope reaches broader host catalog and host/HAL project-public-entity
  behavior.
- Reconciled in `bd-4ktq.38.5`: `MODPROJ-022` and `PMR-CLS-001` now point to
  live symbol/vm3 fixtures for procedural-module `WithEvents` rejection, and
  `MODPROJ-023` moved from `planned` to `partial` for active-project and
  referenced-project source visibility plus handler-prefix routing/non-routing.
  Full event lifecycle and reassignment parity stays under `DIV-0004`.
- No status downgrade indicated: `MODPROJ-021`, `PMR-VIS-004`, `PMR-CLS-002`,
  `CCT-037`, `CCT-038`, `ODG-035`, `ODG-036`, and `ODG-039` remain consistent
  when interpreted with the scoped residual boundaries above.
- Terminal reconciliation in `bd-4ktq.38.6` refreshed the remaining active PMR
  class/event anchors that still pointed at the removed `oxvba-compiler` crate.
  `PMR-GEN-002`, `PMR-CLS-003`, `PMR-CLS-005`, `PMR-CLS-006`,
  `PMR-CLS-007`, `MODPROJ-024`, `MODPROJ-025`, and `MODPROJ-038` now point to
  live symbol/binder/vm3 anchors where they are used by this truth surface.
- No new delivery fix was exposed outside the existing child beads.

## Audit Result

No new untracked delivery lane was exposed by this audit. The accepted
reconciliation work is represented by closed child beads `bd-4ktq.38.2` through
`bd-4ktq.38.6`; broader residuals stay with their named non-scoping owners.
