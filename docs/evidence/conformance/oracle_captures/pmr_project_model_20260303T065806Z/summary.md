# PMR Project-Model Oracle Run

- Timestamp (UTC): 2026-03-03T06:58:21Z
- Excel version: 16.0
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\pmr_project_model_20260303T065806Z\results.csv
- Total cases: 9
- Match count: 4
- Mismatch count: 5

## Topic Summary
- CCT-037: 3/3 matched
- CCT-038: 1/2 matched
- CCT-039: 0/2 matched
- CCT-040: 0/1 matched
- CCT-041: 0/1 matched

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-037 | CCT-037-A | ok: 101 | ok: 101 | true | OxVba anchor: active_project_resolution_uses_reference_precedence_order_for_shadowing |
| CCT-037 | CCT-037-B | ok: 202 | ok: 202 | true | OxVba anchor: reference precedence ordering |
| CCT-037 | CCT-037-C | ok: 303 | ok: 303 | true | OxVba anchor: active_project_resolution_prefers_local_symbols_before_references |
| CCT-038 | CCT-038-A | ok: 11 | ok: 11 | true | OxVba anchor: host export registry includes non-private procedural members |
| CCT-038 | CCT-038-B | ok: 22 | error: PMR-E-VISIBILITY-DENIED-equivalent (host export omitted) | false | Current OxVba enforces this at export visibility boundary |
| CCT-039 | CCT-039-A | ok: VB_Name=Widget;Predeclared=<unsupported:VB_PredeclaredId>;Exposed=<unsupported:VB_Exposed>;GlobalNamespace=<unsupported:VB_GlobalNamespace>;Creatable=<unsupported:VB_Creatable> | ok: defaults retained (vb_name/module name; booleans false unless set) | false | OxVba anchor: module_unit_from_source defaults + source class attribute constraints |
| CCT-039 | CCT-039-B | ok: Predeclared=<unsupported:VB_PredeclaredId>;Exposed=<unsupported:VB_Exposed> | ok: attribute parsing preserves explicit values | false | OxVba anchor: ModuleAttributes(vb_predeclared_id, vb_exposed) |
| CCT-040 | CCT-040-A | ok: 1 | error: PMR-E-IMPLEMENTS-PROJECTGRAPH-REQUIRED | false | Known divergence: class interface coverage semantics pending |
| CCT-041 | CCT-041-A | ok: 1,3, | error: PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED | false | Known divergence: event model not yet implemented in OxVba |
