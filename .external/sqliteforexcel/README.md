# SQLiteForExcel External Fixture Set

This directory holds the controlled external fixture set for the OxVba SQLiteForExcel integration lane.

Canonical upstream source:
- repo: `https://github.com/govert/SQLiteForExcel.git`
- imported from fresh clone at:
  - `C:\Work\SqliteForExcel`
  - commit `8aae8bd5c69a9083a67a295fcbcfde838c755f4f`

Imported on:
- 2026-04-03

Why this exists:
- the pre-existing local archive at `C:\Work\SQLiteForExcelArchive` is not a Git checkout and does not match the fresh GitHub clone for the specific source/DLL artifacts under test
- this repo therefore controls the minimal fixture set needed for reproducible declare/native integration probes

Imported artifact set:
- `upstream/Source/SQLite3VBAModules/Sqlite3.bas`
- `upstream/Source/SQLite3VBAModules/Sqlite3Demo.bas`
- `upstream/Source/SQLite3VBAModules/Sqlite3_64.bas`
- `upstream/Source/SQLite3VBAModules/Sqlite3Demo_64.bas`
- `upstream/Distribution/SQLite3_StdCall.dll`
- `upstream/Distribution/sqlite3.dll`
- `upstream/Distribution/x64/sqlite3.dll`

Canonical integrity values from the fresh clone:

| File | Length | SHA256 |
| --- | ---: | --- |
| `Source/SQLite3VBAModules/Sqlite3.bas` | 28720 | `57931CF024C5ADE740362F9662731FCBFF0A843622EB4D4C2AEC4AF38BD8E36F` |
| `Source/SQLite3VBAModules/Sqlite3Demo.bas` | 40757 | `B21D48F132C90176364674702DC10854F6FE69B1B4859A71E13DA7A47B2BD2C9` |
| `Source/SQLite3VBAModules/Sqlite3_64.bas` | 40701 | `A0D5B16A1BEAA3B44FDA82B3A1EE6505BCB66CD85A878D0135CDA12F2D5D5ADF` |
| `Source/SQLite3VBAModules/Sqlite3Demo_64.bas` | 42996 | `1A5A8A0FAF224D55D78F558A6619C2EECC3B00F050F47B577A86453EE086C963` |
| `Distribution/SQLite3_StdCall.dll` | 75264 | `8BF6811D898677C9D46CB80EB71138C0E8A1C7F4CFBD0ED6966B119AE6E3C00E` |
| `Distribution/sqlite3.dll` | 824119 | `6F39BC231354F1A0F49B1B94458CA3CC35FA653B0703745B0032CD37FAC35265` |
| `Distribution/x64/sqlite3.dll` | 1672704 | `D067BAE9F3D72F06BFAAB7BCC6E7DF935389C038CB17069D9E123438C99E38BD` |

This directory is for controlled integration fixtures, not for general upstream mirroring.
