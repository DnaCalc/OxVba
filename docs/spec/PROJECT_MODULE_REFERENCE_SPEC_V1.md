# Project Module Reference Spec v1

Status: `working-draft`
Date: 2026-03-02
Scope: OxVba compiler/host project graph semantics (Project, Module, Reference)

## 1. Purpose

Define a formal, implementation-ready contract for VBA project/module/reference behavior in OxVba, aligned to:

- `CHARTER.md` priority order: robustness > compatibility > performance.
- Foundation source doctrine (`docs/FOUNDATION_SPEC_REFERENCE.md`).
- Current OxVba validation approach (clause catalogs, conformance lanes, deferred oracle gates).

This spec is intentionally precise about:

- state model,
- preconditions/postconditions,
- invariants,
- deterministic failure modes,
- implementation-defined boundaries,
- HAL interaction boundaries.

## 2. Normative Source Basis

Primary source set:

- MS-VBAL extracted set:
  - `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/conformance_items.jsonl`
  - `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/docs/discovered-ms-vbal-250520-f945507e/spec_items.jsonl`
- MS-OAUT extracted set:
  - `../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/conformance_items.jsonl`
- MS-OVBA extracted set:
  - `../Foundation/reference/runs/20260301-ms-ovba-pass01/outputs/spec_items.jsonl`
  - `../Foundation/reference/runs/20260301-ms-ovba-pass01/outputs/run_manifest.json`

Key source-quality note:

- MS-VBAL and MS-OAUT runs contain large extracted conformance sets and are suitable for clause mapping.
- Current MS-OVBA run is under-extracted (6 spec items, 0 conformance candidates). Sections 1.7/2 are marked normative, but section-level obligation extraction is missing and tracked as a hard requirement in this spec.

## 3. Formal State Model

## 3.1 Core Entities

```text
ProjectGraph
  projects: Map<ProjectId, ProjectNode>
  active_project: ProjectId

ProjectNode
  project_name: Identifier
  project_kind: {Source, Host, Library}
  module_order: Vec<ModuleId>
  modules: Map<ModuleId, ModuleNode>
  references: Vec<ProjectReference>
  conditional_constants: Map<Identifier, ConstValue>

ModuleNode
  module_name: Identifier
  module_kind: {Procedural, Class, Document, Form, Extension}
  header_attributes: ModuleAttributes
  declaration_ast: ModuleDeclAst
  code_ast: ModuleCodeAst

ProjectReference
  referenced_project_name: Identifier
  precedence_index: u32
  reference_kind: {Project, TypeLibrary, HostInjected}
  binding_state: {Unbound, Bound, Failed}

ModuleAttributes
  vb_name: Identifier
  vb_global_namespace: bool
  vb_creatable: bool
  vb_predeclared_id: bool
  vb_exposed: bool
  extras: Map<String, String>
```

## 3.2 Invariants

- INV-PMR-001: Every project name is a valid VBA identifier (`CONF-...-0035`).
- INV-PMR-002: Within a project, module names are unique (`CONF-...-0041`).
- INV-PMR-003: Reference list order is preserved and semantically significant (`SPEC-...-01230`).
- INV-PMR-004: Referenced project names in one project are pairwise distinct (`CONF-...-0038`).
- INV-PMR-005: For source projects, `VB_GlobalNamespace == False` and `VB_Creatable == False` (`CONF-...-0042`).
- INV-PMR-006: `Option Private Module` only applies to procedural modules (`SPEC-...-01366..01369`).
- INV-PMR-007: Procedural module variable declarations cannot include `WithEvents` (`CONF-...-0056`).
- INV-PMR-008: Implements clauses in class modules satisfy interface coverage constraints (`CONF-...-0095..0098`).
- INV-PMR-009: Public entity names that collide with project/module names require explicit qualification (`CONF-...-0053`, `...-0106`).

## 4. Operation Contracts

## 4.1 `create_project(project_name, project_kind)`

Preconditions:

- `project_name` parses as `<IDENTIFIER>`.
- no existing project with identical name in active environment.

Postconditions:

- new `ProjectNode` exists with empty module set and empty references.
- deterministic insertion order is established.

Failures:

- invalid identifier -> compile-time diagnostic `PMR-E-PROJECT-NAME-INVALID`.
- duplicate name -> compile-time diagnostic `PMR-E-PROJECT-NAME-DUPLICATE`.

## 4.2 `add_module(project, module)`

Preconditions:

- project exists.
- module header includes required attributes for module kind.

Postconditions:

- module inserted at specified deterministic order index.
- `module_order` and `modules` map remain consistent.

Failures:

- duplicate module name -> `PMR-E-MODULE-NAME-DUPLICATE`.
- malformed header/attribute grammar -> `PMR-E-MODULE-HEADER-INVALID`.

## 4.3 `add_reference(project, reference)`

Preconditions:

- referenced project name is syntactically valid.
- reference name does not duplicate existing reference target name.

Postconditions:

- reference appended with explicit precedence index.

Failures:

- duplicate reference target name -> `PMR-E-REFERENCE-DUPLICATE-TARGET`.

## 4.4 `resolve_qualified_name(project, module, name_expr)`

Preconditions:

- project and module are bound.
- module AST + symbol tables are available.

Postconditions:

- deterministic classification result:
  - local module symbol,
  - enclosing project symbol,
  - referenced project symbol,
  - unresolved.

Failures:

- unresolved ambiguous name -> `PMR-E-NAME-RESOLUTION-AMBIGUOUS`.
- unresolved missing name -> `PMR-E-NAME-RESOLUTION-NOT-FOUND`.
- unqualified access where qualification is required by collision rules -> `PMR-E-NAME-QUALIFICATION-REQUIRED`.

## 4.5 `validate_module_visibility(project, module, entity)`

Preconditions:

- module directives parsed (`Option Private Module` where present).

Postconditions:

- visibility classification is deterministic:
  - project-local only,
  - project+referencing projects,
  - class public interface constraints.

Failures:

- forbidden cross-project access from private module -> `PMR-E-VISIBILITY-DENIED`.

## 4.6 `materialize_default_instance(class_module)`

Preconditions:

- class module attributes available.

Postconditions:

- if `VB_PredeclaredId=True` or `VB_GlobalNamespace=True`, default instance metadata exists.
- default instance naming follows VBAL rules (named or unnamed expressible path).

Failures:

- contradictory class-instancing metadata -> `PMR-E-CLASS-INSTANCING-CONFLICT`.

## 5. Static Semantics Rules

The implementation SHALL enforce at minimum:

- project/module naming and uniqueness (`CONF-...-0035`, `...-0041`).
- module-kind legality and grammar conformance (`CONF-...-0039`).
- source-project class-attribute constraints (`CONF-...-0042`).
- qualification requirements for collision cases (`CONF-...-0053`, `...-0106`).
- `WithEvents` legality by module kind (`CONF-...-0056`, `...-0140`).
- Implements legality and interface coverage (`CONF-...-0095..0098`, `...-0143`).
- module-level declaration collision constraints (`CONF-...-0131`, `...-0132`, `...-0136`).

## 6. Dynamic and Runtime Semantics

Runtime-facing behaviors constrained by this spec:

- class-module event dispatch and `RaiseEvent` legality (`CONF-...-0176`, `...-0177`).
- default-instance exposure semantics from class attributes (source anchors `SPEC-...-01266`, `...-01267` and sentence anchors around class-module semantics).
- project reference precedence affecting runtime bind target selection (`SPEC-...-01230`).

### 6.1 Class Semantic Contract (A1 scope)

The class semantic contract is locked at language/runtime level even when full COM ABI wiring is staged:

- `Class_Initialize` executes before `Main` body effects become observable.
- `Class_Terminate` executes after `Main` path completion for deterministic teardown paths.
- `Property Let/Set` assignment routes to callable property procedures and preserves ByRef write route expectations.
- deterministic diagnostics are required for class-project features not yet executable (`WithEvents`, `Implements`, `RaiseEvent`) and must use stable PMR diagnostic codes.

Current executable evidence lives in host/compiler tests and is tracked in:

- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`
- `docs/spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md`

## 7. Reference and Binding Semantics

## 7.1 Project Reference Ordering

- The reference list is ordered and semantically relevant (`SPEC-...-01230`).
- Binder MUST treat lower index as higher precedence unless an explicit language rule overrides this.

## 7.2 Project Categories

OxVba model must explicitly support:

- source project,
- host project,
- library project,

per source anchors (`SPEC-...-01234`, `...-01236`, `...-01237`).

## 7.3 Cross-project Entity Access

- A project reference grants access to public entities in referenced projects (`SPEC-...-01232`).
- Mechanisms for physically identifying referenced projects are implementation-defined (`SPEC-...-01233`) and must be explicitly documented in the implementation-defined register.

## 7.4 OAUT-facing Constraints for Reference-backed Automation Calls

For calls routed through OLE Automation surfaces, OxVba must preserve OAUT rules, including:

- `GetIDsOfNames` contract + case-insensitivity (`CONF-...-0575`, `...-0599`).
- `Invoke` packing and output obligations (`CONF-...-0614..0623`, `...-0627..0631`).
- automation-compatible type constraints (`CONF-...-0468`, `...-0469`, `...-0483`, `...-0484`, `...-0530`).

### 7.5 Semantic vs Adapter Responsibilities (A3 boundary)

Semantic (language/runtime) obligations:

- class lifecycle ordering, property routing, deterministic project diagnostics.

Adapter/HAL obligations:

- actual COM activation/dispatch ABI behavior, policy gates, and host error projection.

Claim rule:

- class semantic compatibility can be `implemented-verified` without implying full COM ABI parity.
- COM-boundary claims must remain `implemented-partial` or `specified-pending` until bridge conformance lanes close.

## 8. Interaction with Existing OxVba Pipeline

Required compiler-host integration shape:

1. Input layer:
- host/CLI provides project manifest (project metadata + module set + references + conditional constants).

2. Parse layer:
- parse each module independently with preserved header attributes.

3. Project bind layer:
- build project graph,
- validate invariants,
- construct cross-module and cross-project symbol tables,
- resolve qualified/unqualified names with deterministic precedence.

4. Lowering layer:
- preserve enough metadata for runtime-class features (`WithEvents`, default instances, Implements dispatch tags).

5. Runtime/host layer:
- instantiate class default-instance metadata,
- enforce project-level visibility at invocation boundaries,
- route host-project and reference-backed entities through HAL or host integration contracts.

## 9. HAL Boundary and Responsibilities

Project/module/reference semantics are language-level first; however these interactions are HAL-adjacent and are tracked for HAL formalization:

- host project discovery/injection,
- reference graph materialization from host environment,
- open host project and extension-module attachment,
- persistent storage import/export (MS-OVBA),
- type library/importlib resolution where required by references.

Detailed HAL planning is defined in:

- `docs/spec/PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md`.
- `docs/spec/PROJECT_MODULE_REFERENCE_TYPELIB_IMPORTLIB_HAL_DRAFT_V1.md`.

## 10. Error Model

All project-model failures MUST be deterministic and reproducible.

Error classes:

- syntax/header errors: parser diagnostics.
- static semantic violations: binder/type checker diagnostics.
- host/reference materialization failures: host/HAL structured error mapped to compile-time or runtime phase per policy.

No silent fallback is allowed for violated MUST constraints.

## 11. Verification Model

Clause catalog:

- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv`

Conformance suite plan:

- `docs/spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md`
- `docs/spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md` (class semantics now, full COM interop mechanics staged/deferred)

Coverage/backlog tracker:

- `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`

## 12. Uncertainty and Implementation-defined Areas

Explicitly implementation-defined from extracted source set:

- project physical representation and storage mechanism (`SPEC-...-01231`).
- mechanism used to identify referenced projects (`SPEC-...-01233`).
- open host project module extension mechanism (`SPEC-...-01241`, `...-01299`).

These MUST be tracked in implementation-defined and deferred-oracle artifacts before compatibility claims are raised to full parity.

## 13. Immediate Next Steps

1. Implement parser retention for module header attributes (`VB_Name`, `VB_PredeclaredId`, `VB_GlobalNamespace`, `VB_Creatable`, `VB_Exposed`).
2. Introduce `ProjectGraph` binding stage and deterministic diagnostics for naming/qualification constraints.
3. Add executable conformance fixtures for two-module and three-module reference precedence paths.
4. Add OAUT-backed dispatch packaging checks at project-reference call boundary.
5. Close MS-OVBA extraction gap in Foundation source runs and map section 2 obligations into clause IDs.
