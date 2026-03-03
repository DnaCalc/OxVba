# PMR Project-Model Oracle Run

- Timestamp (UTC): 2026-03-03T07:41:34Z
- Excel version: 16.0
- Excel process id: 34852
- Dialog guardian enabled: True
- Dialog guardian log: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\pmr_project_model_20260303T074118Z\excel_dialog_guardian.log
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\pmr_project_model_20260303T074118Z\results.csv
- Total cases: 8
- Match count: 6
- Mismatch count: 2

## Topic Summary
- CCT-037: 3/3 matched
- CCT-038: 2/2 matched
- CCT-039: 1/1 matched
- CCT-040: 0/1 matched
- CCT-041: 0/1 matched

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-037 | CCT-037-A | ok: 101 | ok: 101 | true | OxVba anchor: active_project_resolution_uses_reference_precedence_order_for_shadowing |
| CCT-037 | CCT-037-B | ok: 202 | ok: 202 | true | OxVba anchor: reference precedence ordering |
| CCT-037 | CCT-037-C | ok: 303 | ok: 303 | true | OxVba anchor: active_project_resolution_prefers_local_symbols_before_references |
| CCT-038 | CCT-038-A | ok: 11 | ok: 11 | true | OxVba anchor: host export registry includes non-private procedural members |
| CCT-038 | CCT-038-B | ok: 22 | ok: 22 | true | OxVba host-export registry now retains Option Private procedures for host-direct invocation lanes |
| CCT-039 | CCT-039-A | ok: Attribute VB_Name = "Widget";Attribute VB_GlobalNameSpace = False;Attribute VB_Creatable = False;Attribute VB_PredeclaredId = False;Attribute VB_Exposed = False | ok: Attribute VB_Name = "Widget";Attribute VB_GlobalNameSpace = False;Attribute VB_Creatable = False;Attribute VB_PredeclaredId = False;Attribute VB_Exposed = False | true | OxVba anchor: module_unit_from_source defaults + source class attribute constraints |
| CCT-040 | CCT-040-A | ok: 1 | error: PMR-E-IMPLEMENTS-PROJECTGRAPH-REQUIRED | false | Known divergence: class interface coverage semantics pending |
| CCT-041 | CCT-041-A | ok: 1,3, | error: PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED | false | Known divergence: event model not yet implemented in OxVba |
