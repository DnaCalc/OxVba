# COM TestEventServer Unresolved Typelib Oracle Run

- Run ID: 20260326T174738Z
- Generated UTC: 2026-03-26T17:47:44Z
- Missing TypeLib path: C:\Work\DnaCalc\OxVba\temp\missing\NoSuchTypeLib.tlb
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_unresolved_typelib_oracle_20260326T174738Z\results.csv
- Modal inspection note: both Excel probes returned promptly under hidden automation with DisplayAlerts = false; no modal popup was observed in this bounded lane.

- Total cases: 2
- Match count: 2
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-LIBID-001 | error: Object library not registered | ok: PMR-E-TYPELIB-LIBID-UNRESOLVED | true | Excel classification=unresolved-libid; ref_count=4; modal_observed=false; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_reports_unresolved_typelib_libid_identity -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_unresolved_typelib_oracle_20260326T174738Z\CCT-043-TES-LIBID-001.log.txt |
| CCT-043 | CCT-043-TES-IMPORTLIB-001 | error: Error in loading DLL | ok: PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED | true | Excel classification=unresolved-importlib; ref_count=4; modal_observed=false; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_reports_unresolved_typelib_importlib_identity -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_unresolved_typelib_oracle_20260326T174738Z\CCT-043-TES-IMPORTLIB-001.log.txt |
