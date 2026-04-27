# V0.2 Office COM Corpus Matrix

Date: 2026-04-27

Bead: `bd-bqm8.7.2`

## Scope Rule

This matrix defines the V0.2 Office COM corpus rows for Excel and Access/JET
without treating machine-local Office availability as a universal CI
requirement. Controlled OxVba COM servers remain the always-runnable active
baseline. Real Office rows are environment-dependent and must skip explicitly
when the required Office application or provider is absent.

## Environment Prerequisites

- Windows COM host support is required for all live Office automation rows.
- Excel rows require Microsoft Excel with VBOM/macro automation allowed for
  oracle probes that enter VBE/project-model surfaces.
- Access/JET rows require Microsoft Access, ACE, DAO, or an installed JET/ACE
  OLE DB provider matching the fixture row.
- Hidden automation probes must use the existing bounded dialog helpers:
  `scripts/excel-dialog-guardian.ps1` and
  `scripts/excel-vbe-dialog-handler.ps1`.
- Default CI must execute controlled OxVba COM rows and classify missing
  Office/provider rows as environment skips, not failures.

## Corpus Matrix

| Row | Area | Scenario | V0.2 status | Evidence / command anchor | Owner bead |
| --- | --- | --- | --- | --- | --- |
| `OFFICE-COM-001` | Controlled COM | Late-bound `OxVba.TestDispatch` activation and invoke baseline. | supported-active | `conformance/com/client/c2-latebound/*.bas`; `scripts/run-com-conformance.ps1` | `bd-bqm8.7.5` |
| `OFFICE-COM-002` | Controlled COM | Default-member and named-argument late-bound dispatch rows. | supported-active | `conformance/tests/late_bound_default_member_exec.bas`; `conformance/tests/late_bound_named_argument_exec.bas` | `bd-bqm8.7.5` |
| `OFFICE-COM-003` | Controlled typelib COM | `OxVba.TestEventServer` typelib import, early-bound `Ping`, and `WithEvents` payload. | controlled-supported | `scripts/run-com-testeventserver-oracle.ps1`; `scripts/run-com-testeventserver-typelib-probe.ps1` | `bd-bqm8.7.5` |
| `OFFICE-COM-004` | Controlled marshaling | TestEventServer scalar, array, object, and dispatch-element marshaling. | controlled-supported | `scripts/run-com-testeventserver-marshaling-oracle.ps1` | `bd-bqm8.7.5` |
| `OFFICE-COM-005` | Excel project model | Excel PMR project/reference behavior for `CCT-037..CCT-041`. | environment-dependent | `scripts/run-pmr-project-model-oracle.ps1`; `docs/CONFORMANCE.md` | `bd-bqm8.7.5` |
| `OFFICE-COM-006` | Excel references | Excel reference order, broken-reference, and versioned typelib behavior for `CCT-043` / `CCT-048`. | environment-dependent | `scripts/run-com-testeventserver-*-oracle.ps1`; oracle captures under `docs/evidence/conformance/oracle_captures/` | `bd-bqm8.7.5` |
| `OFFICE-COM-007` | Excel automation | `CreateObject("Excel.Application")` activation and root property smoke. | environment-dependent fixture target | `bd-bqm8.7.3` Excel corpus fixture | `bd-bqm8.7.3` |
| `OFFICE-COM-008` | Excel object model | Workbook/worksheet/range property get/set and default-member access. | environment-dependent fixture target | `bd-bqm8.7.3` Excel corpus fixture | `bd-bqm8.7.3` |
| `OFFICE-COM-009` | Excel dispatch metadata | Named-argument method/property path where typelib metadata is authoritative. | environment-dependent fixture target | `bd-bqm8.7.3` Excel corpus fixture | `bd-bqm8.7.3` |
| `OFFICE-COM-010` | Excel events | Real `Excel.Application` event sink coverage beyond controlled TestEventServer events. | unsupported-v02 | Residual row for final checklist; no capability claim in V0.2. | `bd-bqm8.7.6` |
| `OFFICE-COM-011` | Access automation | `CreateObject("Access.Application")` activation and root property smoke. | environment-dependent fixture target | `bd-bqm8.7.4` Access/JET corpus fixture | `bd-bqm8.7.4` |
| `OFFICE-COM-012` | Access/JET data | Database open/query/table interaction through Access application or DAO/ACE/JET provider. | environment-dependent fixture target | `bd-bqm8.7.4` Access/JET corpus fixture | `bd-bqm8.7.4` |
| `OFFICE-COM-013` | Provider activation | Provider object activation such as `ADODB.Connection`, DAO DBEngine, or ACE OLE DB when installed. | environment-dependent fixture target | `bd-bqm8.7.4` Access/JET corpus fixture | `bd-bqm8.7.4` |
| `OFFICE-COM-014` | Platform boundary | Non-Windows Office COM parity. | unsupported-v02 | Explicit platform residual; Windows COM is the scoped live host boundary. | `bd-bqm8.7.6` |
| `OFFICE-COM-015` | Office forms designer | Full forms designer / Office UI designer behavior. | out-of-scope-v02 | Explicit non-scope row; do not count support documentation as capability closure. | `bd-bqm8.7.6` |

## Evidence Inventory

- Controlled COM fixtures already exist under `conformance/com/client`,
  `conformance/com/early`, and `conformance/tests`.
- Controlled server and Excel oracle runners are listed in `scripts/README.md`
  and referenced by `docs/CONFORMANCE.md`.
- Existing oracle captures under
  `docs/evidence/conformance/oracle_captures/` cover TestEventServer,
  versioned typelib, broken reference, dual-interface, marshaling, and PMR
  Excel project-model lanes.
- The `bd-bqm8.7.3` and `bd-bqm8.7.4` delivery beads must add durable fixture
  artifacts for the environment-dependent Excel and Access/JET rows rather
  than relying on this matrix alone.

## Checks Run

- `rg --files conformance docs/evidence crates/oxvba-host crates/oxvba-com crates/oxvba-hal | rg "(?i)(com|dispatch|excel|access|jet|office|typelib|conformance)"`
- `rg -n "bd-bqm8\.7\.|Office COM|Excel|Access|JET|run-pmr-project-model-oracle|run-com-conformance" docs/worksets docs/CONFORMANCE.md scripts/README.md .beads/issues.jsonl`

## Result

`bd-bqm8.7.2` is complete as a support/delivery matrix bead. The capability
lane remains in-progress until the Excel fixtures, Access/JET fixtures, refreshed
evidence, and final checklist beads complete.
