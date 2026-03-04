# Project Module Reference Source Crosswalk v1

Status: `working-draft`
Date: 2026-03-02

## 1. Purpose

Provide explicit source-to-clause traceability for Project/Module/Reference semantics.

This crosswalk is the evidence companion for:

- `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`

## 2. MS-VBAL Anchors (Core PMR)

| Anchor ID | Statement summary | Primary PMR clauses |
|---|---|---|
| `CONF-discovered-ms-vbal-250520-f945507e-0035` | Project name must be a valid identifier. | `PMR-PROJ-001` |
| `CONF-discovered-ms-vbal-250520-f945507e-0038` | Project references must identify distinct project names. | `PMR-PROJ-002` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01230` | Reference list order defines precedence. | `PMR-PROJ-003`, `PMR-NAME-003` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01234` | Project kinds: source/host/library. | `PMR-PROJ-004` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01239` | Host project public entities accessible to source projects. | `PMR-PROJ-005` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01240` | Open host project concept. | `PMR-PROJ-006`, `PMR-HAL-001` |
| `CONF-discovered-ms-vbal-250520-f945507e-0039` | Procedural and class module kinds. | `PMR-MOD-001` |
| `CONF-discovered-ms-vbal-250520-f945507e-0041` | Module names are distinct in a project. | `PMR-MOD-002` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01285` | Module name is `VB_Name` attribute value. | `PMR-MOD-003` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01286` | Module name max length 31. | `PMR-MOD-004` |
| `CONF-discovered-ms-vbal-250520-f945507e-0042` | Source project modules require `VB_GlobalNamespace=False` and `VB_Creatable=False`. | `PMR-MOD-005` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01266` | `VB_PredeclaredId` class attribute. | `PMR-MOD-006` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01267` | `VB_Exposed` class attribute. | `PMR-MOD-006` |
| `CONF-discovered-ms-vbal-250520-f945507e-0043` | Extension module name must match extensible module name. | `PMR-MOD-007` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01366` | `Option Private Module` grammar. | `PMR-VIS-001` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01368` | Private module accessibility restriction. | `PMR-VIS-001`, `PMR-VIS-004` |
| `SPEC-discovered-ms-vbal-250520-f945507e-01369` | Public module accessibility to referencing projects. | `PMR-VIS-001` |
| `CONF-discovered-ms-vbal-250520-f945507e-0053` | Public variable collision requires module qualification. | `PMR-VIS-002` |
| `CONF-discovered-ms-vbal-250520-f945507e-0106` | Public procedure collision requires qualification. | `PMR-VIS-003` |
| `CONF-discovered-ms-vbal-250520-f945507e-0131` | Sub/function declaration-space collision rule. | `PMR-NAME-001` |
| `CONF-discovered-ms-vbal-250520-f945507e-0132` | Property declaration-space collision rule. | `PMR-NAME-001` |
| `CONF-discovered-ms-vbal-250520-f945507e-0136` | Property signature equivalence requirement. | `PMR-NAME-001` |
| `CONF-discovered-ms-vbal-250520-f945507e-0078` | Public UDT cross-module naming conflict rule. | `PMR-NAME-002` |
| `CONF-discovered-ms-vbal-250520-f945507e-0083` | Public enum cross-module naming conflict rule. | `PMR-NAME-002` |
| `CONF-discovered-ms-vbal-250520-f945507e-0056` | WithEvents declarations forbidden in procedural module declaration list. | `PMR-CLS-001` |
| `CONF-discovered-ms-vbal-250520-f945507e-0140` | Event handler naming must use WithEvents prefix. | `PMR-CLS-002` |
| `CONF-discovered-ms-vbal-250520-f945507e-0095` | Implements cannot occur in extension module. | `PMR-CLS-003` |
| `CONF-discovered-ms-vbal-250520-f945507e-0096` | Implements interface class names must be distinct. | `PMR-CLS-004` |
| `CONF-discovered-ms-vbal-250520-f945507e-0097` | Implements requires method coverage. | `PMR-CLS-005` |
| `CONF-discovered-ms-vbal-250520-f945507e-0098` | Implements requires variable/member coverage. | `PMR-CLS-005` |
| `CONF-discovered-ms-vbal-250520-f945507e-0143` | Implemented procedure naming must use interface prefix. | `PMR-CLS-006` |
| `CONF-discovered-ms-vbal-250520-f945507e-0176` | RaiseEvent only inside class-module procedures. | `PMR-CLS-007` |
| `CONF-discovered-ms-vbal-250520-f945507e-0177` | Raised identifier must be declared class event. | `PMR-CLS-007` |
| `CONF-discovered-ms-vbal-250520-f945507e-0065` | Class instancing restriction on `as-auto-object` by project relation. | `PMR-CLS-008` |

## 3. MS-OAUT Anchors (Reference-bound Automation)

| Anchor ID | Statement summary | Primary PMR clauses |
|---|---|---|
| `CONF-discovered-ms-oaut-240423-b76f9b41-0561` | `importlib` string must allow locating referenced type definitions. | `PMR-REF-002` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0575` | `GetIDsOfNames` maps names to DISPIDs for `Invoke`. | `PMR-REF-003` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0599` | `GetIDsOfNames` must be case-insensitive. | `PMR-REF-003` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0614` | `Invoke` requires `DISPPARAMS`. | `PMR-REF-004` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0615` | `Invoke` args in `rgvarg` are reverse-ordered. | `PMR-REF-004` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0617` | `Invoke` `pVarResult` must be a `VARIANT` output. | `PMR-REF-004` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0618` | `Invoke` exception path fills `EXCEPINFO`. | `PMR-REF-004` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0620` | `pArgErr` index obligations for type/parameter errors. | `PMR-REF-004` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0622` | `cVarRef` obligations for byref args. | `PMR-REF-004` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0623` | `rgVarRefIdx` obligations for byref arg indices. | `PMR-REF-004` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0627` | `EXCEPINFO` error-code fill obligations. | `PMR-REF-004` |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0629` | `pArgErr` index return obligations. | `PMR-REF-004` |

## 4. MS-OVBA Anchors and Gap

Current extracted anchors:

- `SPEC-ms-ovba-...-00005`: sections 1.7 and 2 are normative.

Current gap:

- Run `20260301-ms-ovba-pass01` produced no section-level conformance candidates.
- PMR storage clauses remain `specified-pending` until section-level extraction is available.

## 5. Crosswalk Governance

- Any PMR clause promoted to `implemented-verified` must have at least one direct source anchor listed here.
- If source ambiguity remains implementation-defined, add matching records to:
  - `docs/evidence/conformance/IMPLEMENTATION_DEFINED.md`
  - `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
