<!-- generated: scripts/generate-pmr-event-diagnostic-snippets.ps1 -->
# PMR Event Diagnostic IDs (Generated)

Canonical source: `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv`.

| Diagnostic ID | Phase | Status | Description |
|---|---|---|---|
| `PMR-E-EVENT-MODULE-KIND` | compile-time | implemented | Event declarations are valid only in object modules |
| `PMR-E-EVENT-OPTIONAL-ARGUMENT` | compile-time | implemented | Event declarations cannot use Optional arguments |
| `PMR-E-EVENT-PARAMARRAY-ARGUMENT` | compile-time | implemented | Event declarations cannot use ParamArray arguments |
| `PMR-E-IMPLEMENTS-INTERFACE-NOT-FOUND` | compile-time | implemented | Implemented interface must resolve to a known project/type symbol |
| `PMR-E-IMPLEMENTS-MEMBER-MISSING` | compile-time | implemented | Class must provide required prefixed member coverage for each implemented interface |
| `PMR-E-IMPLEMENTS-MODULE-KIND` | compile-time | implemented | Implements directives are valid only in class modules |
| `PMR-E-RAISEEVENT-ARGUMENT-SHAPE` | compile-time | implemented | RaiseEvent arguments must use VBA event argument-list syntax |
| `PMR-E-RAISEEVENT-ARITY` | compile-time | implemented | RaiseEvent argument count must match the declared event signature |
| `PMR-E-RAISEEVENT-MODULE-KIND` | compile-time | implemented | RaiseEvent statements are valid only in class modules |
| `PMR-E-RAISEEVENT-UNDECLARED` | compile-time | implemented | RaiseEvent target must match a declared event in the class module |
| `PMR-E-WITHEVENTS-AS-NEW` | compile-time | implemented | WithEvents fields cannot use As New |
| `PMR-E-WITHEVENTS-MODULE-KIND` | compile-time | implemented | WithEvents declarations are valid only in class/document/form modules |
| `PMR-E-WITHEVENTS-SOURCE-TYPE` | compile-time | implemented | WithEvents fields must declare an object or event source type |
