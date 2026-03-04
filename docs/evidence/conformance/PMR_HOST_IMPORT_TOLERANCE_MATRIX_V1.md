# PMR Host-Import Tolerance Matrix v1

Date: 2026-03-03  
Scope: `PMR-FUP-004` follow-through (`header parsing strictness + host-edge tolerance`)

## Intent

Define explicit "tolerate vs reject" behavior for module header import lines so host-side
module ingestion remains deterministic and non-silent.

## Matrix

| Case ID | Input shape | Expected behavior | Evidence anchor |
|---|---|---|---|
| `PMR-TOL-001` | `Attribute VB_Name = "..."` with mixed/lowercase keyword casing | `tolerate` | `project::tests::module_unit_tolerates_lowercase_attribute_keyword_and_option_private_spacing` |
| `PMR-TOL-002` | `Option Private Module` with surrounding whitespace | `tolerate` | `project::tests::module_unit_tolerates_lowercase_attribute_keyword_and_option_private_spacing` |
| `PMR-TOL-003` | Unknown attribute key with valid assignment form | `tolerate` | `project::tests::module_unit_tolerates_unknown_header_attributes` |
| `PMR-TOL-004` | Unknown attribute key inside referenced/imported project modules | `tolerate` | `project::tests::compile_project_tolerates_unknown_reference_module_attributes` |
| `PMR-TOL-005` | Known boolean attribute with non-boolean payload (e.g. `VB_Exposed = 1`) | `reject` with stable diagnostic | `project::tests::module_unit_rejects_non_boolean_known_header_attribute` |
| `PMR-TOL-006` | Malformed attribute line missing `=` | `reject` with stable diagnostic | `project::tests::module_unit_rejects_malformed_attribute_line` |

## Deterministic Diagnostics

Rejecting cases in this matrix must surface:
- `PMR-E-MODULE-HEADER-INVALID`

This keeps import behavior deterministic and auditable while still tolerating safe host-edge
variance for unknown or non-semantic header lines.
