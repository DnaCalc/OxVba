# V0.2 Access/JET COM Corpus Fixtures

Date: 2026-04-27

Bead: `bd-bqm8.7.4`

## Scope

This bead adds the Access/JET half of the Office COM corpus fixture pack
selected by `V02_OFFICE_COM_CORPUS_MATRIX_2026-04-27.md`. The fixtures are
durable source artifacts and compiler-accepted active tests. They do not make
Microsoft Access, DAO, ACE, ADODB, or JET provider availability a default CI
requirement.

## Added Fixtures

| Row | Fixture | Coverage |
| --- | --- | --- |
| `OFFICE-COM-011` | `conformance/com/office/access_jet/access_application_activation_smoke.bas` | Late-bound `Access.Application` activation plus root property shape. |
| `OFFICE-COM-012` | `conformance/com/office/access_jet/access_database_query_smoke.bas` | Access database/query object interaction shape via late-bound dispatch. |
| `OFFICE-COM-013` | `conformance/com/office/access_jet/jet_adodb_provider_activation_smoke.bas` | `ADODB.Connection` activation plus ACE/JET OLE DB connection-string flow. |
| `OFFICE-COM-014` | `conformance/com/office/access_jet/access_jet_provider_boundary.bas` | Explicit V0.2 provider/platform boundary for absent Access/JET/ACE/DAO installations. |

Catalog/docs:

- `conformance/com/office/access_jet/README.md`
- `conformance/com/office/access_jet/FIXTURE_CATALOG.md`

## Active Test

Added host formal test:

- `formal_v02_7_access_jet_com_fixture_pack_exists_and_compiles`

The test verifies the Access/JET fixture catalog files exist and compiles each
`.bas` fixture with `oxvba_compiler::compile`. Live provider execution remains an
environment-gated evidence lane.

## Checks Run

- `cargo test -p oxvba-host formal_v02_7_access_jet_com_fixture_pack_exists_and_compiles -- --nocapture`

## Result

`bd-bqm8.7.4` is complete for Access/JET corpus fixture delivery. The Office COM
corpus lane remains in-progress pending refreshed VM/JIT/host evidence and the
final checklist.
