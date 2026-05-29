# Access/JET COM Corpus Fixtures

Status: `environment-dependent-fixture-pack` (`v02.7`)

These fixtures define the V0.2 Access/JET side of the Office COM corpus without
requiring Microsoft Access, DAO, ACE, or JET providers in default CI. The default
test path validates durable fixture presence and compiler acceptance. Live
provider execution belongs to the Office-enabled evidence lane.

Current fixtures:

- `access_application_activation_smoke.bas`: late-bound
  `CreateObject("Access.Application")` activation and root property shape.
- `access_database_query_smoke.bas`: Access application database/query-style
  interaction through late-bound dispatch.
- `jet_adodb_provider_activation_smoke.bas`: provider activation shape for
  ADODB plus ACE/JET OLE DB connection string flow.
- `dao_dbengine_create_query_smoke.bas`: late-bound DAO `DBEngine` create-database,
  create-table, insert, and recordset query shape through the Access Database Engine
  (ACE) DAO object model. Early-bound DAO variants (typed `DAO.Database`/`Recordset`/
  `Field`) are covered live by the Office-enabled showcase and the
  `*_access_jet_dao_database_subset` integration tests.
- `access_jet_provider_boundary.bas`: explicit V0.2 environment/provider
  boundary row for absent Access, ACE, DAO, or JET installations.

Execution rule:

- Default CI validates fixture presence and syntax only.
- Live automation must classify absent Access/provider dependencies as
  environment skips, not failures.
- Provider-specific evidence must record the exact ProgID/provider string used.

