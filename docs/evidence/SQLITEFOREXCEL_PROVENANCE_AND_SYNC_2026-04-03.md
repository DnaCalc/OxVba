# SQLiteForExcel Provenance And Sync Note

Date: 2026-04-03

## Public Upstream

The public upstream used for this integration lane is:
- repo: `govert/SQLiteForExcel`
- remote: `https://github.com/govert/SQLiteForExcel.git`

Fresh local Git clone created for this lane:
- path: `C:\Work\SqliteForExcel`
- branch: `master`
- HEAD: `8aae8bd5c69a9083a67a295fcbcfde838c755f4f`
- commit date: `2022-08-05 10:11:14 +0200`
- subject: `Update README.md`

At clone time, local HEAD matched remote HEAD exactly.

## Existing Local Archive

Pre-existing local material also exists at:
- `C:\Work\SQLiteForExcelArchive`

Observations:
- this is not a Git checkout
- it contains `_FOSSIL_`, so it appears to be a Fossil-style archive/worktree
- `fossil` is not installed on this machine, so direct Fossil remote-sync verification was not possible

That means:
- the archive is useful as a local historical input,
- but exact remote-sync proof cannot be claimed from that checkout itself

## Sync Posture Conclusion

The local archive should be treated as stale for this lane.

For the exact declaration and DLL artifacts relevant to the OxVba integration probe, the archive does not match the fresh GitHub clone.

Because of that mismatch, the fresh Git clone at `C:\Work\SqliteForExcel` is the canonical source for fixture import in this workset.

## Key Artifact Comparison

Compared files:

| File | Fresh Clone Length | Archive Length | Match |
| --- | ---: | ---: | --- |
| `Source\SQLite3VBAModules\Sqlite3.bas` | 28720 | 28321 | no |
| `Source\SQLite3VBAModules\Sqlite3Demo.bas` | 40757 | 41527 | no |
| `Source\SQLite3VBAModules\Sqlite3_64.bas` | 40701 | 40299 | no |
| `Source\SQLite3VBAModules\Sqlite3Demo_64.bas` | 42996 | 43752 | no |
| `Distribution\SQLite3_StdCall.dll` | 75264 | 60416 | no |
| `Distribution\sqlite3.dll` | 824119 | 599419 | no |
| `Distribution\x64\sqlite3.dll` | 1672704 | 1176064 | no |

## Canonical Imported Fixture Set

The integration lane now uses the exact files copied from the fresh clone into:
- `.external/sqliteforexcel/upstream/`

That imported fixture set is the controlled OxVba-side dependency source for the next setup beads.

## Additional Machine-Wide SQLite Discovery

Under `C:\Programs`, the current machine sweep found:
- `C:\Programs\SQLite\sqlite3.exe`
- `C:\Programs\SQLite\Old\sqlite3.exe`

The sweep did not find:
- `sqlite3.dll`

So the controlled fixture lane should use the imported upstream DLLs, not `C:\Programs`, for the first integration runs.
