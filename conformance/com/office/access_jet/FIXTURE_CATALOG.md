# Access/JET COM Fixture Catalog

| Row | Fixture | Purpose | V0.2 classification |
| --- | --- | --- | --- |
| `OFFICE-COM-011` | `access_application_activation_smoke.bas` | `Access.Application` activation and root property get/set shape. | environment-dependent |
| `OFFICE-COM-012` | `access_database_query_smoke.bas` | Access database/object interaction shape via late-bound dispatch. | environment-dependent |
| `OFFICE-COM-013` | `jet_adodb_provider_activation_smoke.bas` | ADODB plus ACE/JET OLE DB provider activation and open/close shape. | environment-dependent |
| `OFFICE-COM-012/013` | `dao_dbengine_create_query_smoke.bas` | DAO `DBEngine` activation plus create-database/create-table/insert/recordset-query interaction through the DAO object model (late-bound). | environment-dependent |
| `OFFICE-COM-014` | `access_jet_provider_boundary.bas` | Explicit provider/platform boundary for absent Access/JET/ACE/DAO installations. | unsupported-v02 when unavailable |

These fixtures stay late-bound and `DispatchInvoke`-based so they remain
compiler-accepted on machines without Office database providers. Live execution
evidence is refreshed separately under `bd-bqm8.7.5`.

