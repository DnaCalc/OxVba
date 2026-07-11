# Project Module Reference Spec v1

Status: current VBA semantic reference; implementation/evidence tracked separately
Date: 2026-03-02; authority alignment 2026-07-11
Scope: OxVba project, module, provider and reference-closure semantics
System clauses: `AUTH-SPEC-001`, `PROJ-REF-001`, `LS-WORKSPACE-001`
Higher authority: [`../../CHARTER.md`](../../CHARTER.md),
[`../../OPERATIONS.md`](../../OPERATIONS.md),
[`OXVBA_SYSTEM_CONTRACT_V1.md`](OXVBA_SYSTEM_CONTRACT_V1.md)
Current realization: [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
Compiler contract:
[`OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md`](OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md)

## 1. Purpose

Define the project/module/reference semantics that refine the OxVba system and
compiler contracts. Public specifications and reproducible Excel/VBA
observations decide VBA behavior. Repository code, tests, extracted source
indexes and historical plans are implementation or provenance evidence; none is
an independent semantic authority.

This document owns the conceptual project/reference model and its VBA-visible
rules. The compiler contract owns the public `AnalysisResultV1` boundary. The
Windows interop contract owns COM metadata and wire behavior. The language
service consumes compiler facts and does not reconstruct this model.

This spec is intentionally precise about:

- state model,
- preconditions/postconditions,
- invariants,
- deterministic failure modes,
- implementation-defined boundaries,
- compiler, host, Windows-interoperability and language-service boundaries.

## 2. Authority And Source Basis

The clean-room authority set is:

- the public Microsoft specifications `[MS-VBAL]`, `[MS-OVBA]` and
  `[MS-OAUT]`, plus public Microsoft VBA documentation;
- reproducible black-box observation of Excel/VBA for behavior the public
  sources leave unclear or where the product and specification appear to
  differ;
- published research where relevant.

The Foundation source map at
[`../FOUNDATION_SPEC_REFERENCE.md`](../FOUNDATION_SPEC_REFERENCE.md) and the
following extracted runs are searchable, pinned indexes into public material:

- `../../../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/`;
- `../../../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/`;
- `../../../Foundation/reference/runs/20260301-ms-ovba-pass01/outputs/`.

An extraction count or omission is evidence-pipeline status, not a VBA rule.
Missing section-level extraction remains an evidence backlog item and must be
resolved from public sources or an oracle observation before the affected row
is verified; it cannot be filled from current OxVba behavior.

## 3. Conceptual State Model

The following is a logical contract, not a second compiler API or a required
Rust storage layout. `AnalysisResultV1` publishes the corresponding immutable
identities and facts.

## 3.1 Core Entities

```text
ProjectClosure
  closure_id: ProjectClosureId
  closure_digest: Digest
  target: TargetIdentity
  active_project: ProjectId
  projects: OrderedMap<ProjectId, ProjectNode>
  providers: OrderedMap<ProviderId, ProviderSurface>

ProjectNode
  project_id: ProjectId
  version_or_digest: VersionOrDigest
  project_name: Identifier
  project_kind: {Source, Host, Library}
  module_order: Vec<ModuleId>
  modules: Map<ModuleId, ModuleNode>
  references: Vec<ReferenceEdge>
  local_providers: Vec<ProviderId>
  conditional_constants: Map<Identifier, ConstValue>
  provenance: ProjectProvenance

ModuleNode
  module_id: ModuleId
  document_id: DocumentId
  version_or_digest: VersionOrDigest
  module_name: Identifier
  module_kind: {Procedural, Class, Document, Form, Extension}
  header_attributes: ModuleAttributes
  active_view: ActiveViewIdentity
  syntax_identity: SyntaxIdentity
  provenance: SourceProvenance

ReferenceEdge
  reference_id: ReferenceId
  precedence_index: u32
  alias: Optional<Identifier>
  reference_kind: {
    SourceProject,
    VerifiedOxImage,
    VbaLibrary,
    HostProvider,
    ComTypeLibrary
  }
  target_provider: ProviderId
  target_identity: ReferenceTargetIdentity
  version_or_digest: VersionOrDigest
  visibility: ReferenceVisibility
  binding_state: {Resolved, Broken, Ambiguous}
  provenance: ReferenceProvenance

ProviderSurface
  provider_id: ProviderId
  version_or_digest: VersionOrDigest
  origin: ProviderOrigin
  public_symbols: StableSymbolSurface
  provenance: ProviderProvenance

ModuleAttributes
  vb_name: Identifier
  vb_global_namespace: bool
  vb_creatable: bool
  vb_predeclared_id: bool
  vb_exposed: bool
  extras: Map<String, String>
```

Source projects, verified OxImage references, the VBA library, host references,
COM type libraries and source `Declare` declarations publish through one stable
symbol/signature vocabulary. Source declarations, including `Declare`, are
local providers rather than fabricated entries in the ordered external
reference list. Provider origin changes visibility, mutability and navigation
policy, not the binding algorithm.

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
- INV-PMR-010: Project, module, document, provider and reference identities are
  collision-checked and distinguish changed inputs by version or digest.
- INV-PMR-011: Broken and ambiguous references diagnose deterministically; no
  provider is selected by filesystem, registry or load-order accident.
- INV-PMR-012: Referenced source projects and verified OxImage references expose
  equivalent VBA-visible public callable, class and data surfaces.
- INV-PMR-013: `Option Private Module`, visibility and reference precedence are
  enforced before a symbol becomes a bindable cross-project candidate.
- INV-PMR-014: The language service consumes these compiler-owned identities and
  facts; it does not build a second project graph or provider model.

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

- the reference has an explicit kind, target identity and provenance;
- any source-visible project or alias name is syntactically valid;
- the reference does not duplicate an existing target identity in a way VBA
  forbids.

Postconditions:

- the reference is appended with an explicit precedence index;
- the target provider is resolved or the edge retains an explicit
  `Broken`/`Ambiguous` state and diagnostic.

Failures:

- duplicate reference target -> `PMR-E-REFERENCE-DUPLICATE-TARGET`;
- unresolved target -> stable broken-reference diagnostic;
- ambiguous identity/version/alias selection -> stable ambiguity diagnostic.

## 4.4 `resolve_qualified_name(project, module, name_expr)`

Preconditions:

- project and module are bound.
- the module's lossless syntax and compiler-owned provider environment are
  available.

Postconditions:

- deterministic classification result:
  - local module symbol,
  - enclosing project symbol,
  - referenced source or verified-image symbol,
  - VBA library symbol,
  - host or COM metadata symbol,
  - source `Declare` symbol,
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

### 6.1 Class Semantic Contract

Class semantics are language/runtime obligations independent of whether the
object later crosses a COM boundary:

- `Class_Initialize` runs as part of instance initialization after storage is
  allocated and before the creating operation returns a reference to the new
  instance. This does not prohibit the initializer itself from publishing or
  passing `Me`. Its ordering and failure behavior are relative to the creation
  operation, not unconditionally to a procedure named `Main`.
- `Class_Terminate` is invoked before object destruction. Invocation may occur
  when the object becomes provably inaccessible or at a later implementation
  point permitted by VBA. The handler may make the object accessible again,
  but it runs at most once even if the resurrected object later becomes
  inaccessible again. Its inherited error policy is the default policy, so the
  handler must handle errors internally. Normal return, error unwinding,
  project reset/disposal, reentrancy, resurrection and reference cycles require
  explicit lifecycle evidence.
- `Property Let` and `Property Set` assignment invoke the resolved property
  procedure. The property value parameter has VBA's property-assignment ByVal
  semantics; ordinary indexed/property parameters retain their separately
  resolved ByVal/ByRef behavior.
- `WithEvents`, `Implements` and `RaiseEvent` legality is decided at compile
  time from project and class facts, with source-located diagnostics.
- Event binding preserves source identity, handler-signature compatibility,
  subscription/fan-out order, rebinding and teardown order. Synchronous ByRef
  event arguments are written back before the originating call returns.

VM3, JIT and Windows event adapters must observe the same project-owned class
facts. Passing a source-only event test does not by itself verify native COM
connection-point behavior.

## 7. Reference and Binding Semantics

## 7.1 Project Reference Ordering

- The reference list is ordered and semantically relevant (`SPEC-...-01230`).
- Binder MUST treat lower index as higher precedence unless an explicit language rule overrides this.
- Case-insensitive ambiguity at the same effective precedence is diagnosed; a
  provider must not win because it happened to load first.

## 7.2 Project Categories

OxVba model must explicitly support:

- source project,
- host project,
- library project,

per source anchors (`SPEC-...-01234`, `...-01236`, `...-01237`).

Those VBA project categories are distinct from provider/reference kinds.
Referenced source projects, verified OxImage exports, the VBA library, host
metadata, COM type libraries and `Declare` declarations all participate in the
closure through their explicit provider identity and provenance.

## 7.3 Cross-project Entity Access

- A project reference grants access to public entities in referenced projects (`SPEC-...-01232`).
- `Option Private Module` and entity visibility are applied before exporting a
  source or verified-image public surface.
- A compiled reference must preserve public data, callable signatures,
  classes/interfaces and other VBA-visible metadata; it cannot degrade to a
  name-only facade.
- Mechanisms for physically identifying referenced projects are implementation-defined (`SPEC-...-01233`) and must be explicitly documented in the implementation-defined register.

## 7.4 OAUT-facing Constraints for Reference-backed Automation Calls

For calls routed through OLE Automation surfaces, OxVba must preserve OAUT rules, including:

- `GetIDsOfNames` contract + case-insensitivity (`CONF-...-0575`, `...-0599`).
- `Invoke` packing and output obligations (`CONF-...-0614..0623`, `...-0627..0631`).
- automation-compatible type constraints (`CONF-...-0468`, `...-0469`, `...-0483`, `...-0484`, `...-0530`).

### 7.5 Semantic vs Adapter Responsibilities

Semantic (language/runtime) obligations:

- class lifecycle ordering, property routing, deterministic project diagnostics.

Windows-adapter obligations:

- authoritative type-library selection and stable metadata identity;
- COM activation, dispatch/vtable transport, marshalling, cleanup and error
  projection through a verified interop plan.

HAL obligations:

- capability discovery, host policy and delegation. HAL does not own name
  resolution, COM dispatch semantics or canonical carrier layout.

Claim rule:

- a required semantic row may become `verified` independently when its complete
  observable and authority evidence are present;
- that result does not verify COM transport or a profile aggregate;
- COM rows retain `planned`, `in-progress`, `implemented-subset` or
  `implemented-full` truth until their own required evidence permits
  `verified`. Profile completion requires every required row and no accepted
  residual.

## 8. Compiler, Artifact And Runtime Pipeline

The required production shape is:

```text
decoded project inputs and reference declarations
  -> deterministic ProjectClosure and provider surfaces
  -> shared lossless syntax, declarations and typed binding
  -> immutable AnalysisResultV1
       -> optional CoreProgram for valid Strict analysis
  -> Core IR
  -> OxIR project programs
  -> OxImage project closure
  -> bounded decode and sealed verification
  -> VerifiedOxImage
       -> VM3 or JIT backend-neutral project session
```

Binding completes before Core IR. `AnalysisResultV1` retains stable project,
module, document, provider, declaration/use-site, type/call, diagnostic and
provenance facts. Core IR and OxIR preserve reference/import/export identities
needed without source. OxImage records the ordered project closure and explicit
entry project; raw or merely deserialized programs never reach product
execution.

VM3 and JIT consume the same verified project/reference and class metadata.
Runtime and host layers do not repeat name resolution or reconstruct a public
surface from source text.

## 9. Boundary Ownership

The compiler/project layer owns:

- project discovery from supplied inputs, stable identity and deterministic
  closure construction;
- source and verified-image public surfaces, reference precedence, visibility,
  binding and diagnostics;
- compiler facts consumed by runtime and language-service clients.

Host/HAL owns:

- host-project and host-provider injection;
- capability/profile policy and delegation;
- persistent-storage and open-project access where the host supplies it.

Windows interop owns:

- registered/file-backed type-library selection, GUID/version/LCID/platform
  identity, aliases and broken-reference metadata;
- COM activation/invocation/event/serving and wire behavior;
- native loader and callback services under host policy.

The language service consumes `AnalysisResultV1` and immutable snapshots. It
may index reference facts and expose read-only virtual metadata, but it does not
parse, rebind or create an editor-only project/reference model.

These boundaries are refined by
[`OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`](OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md)
and
[`OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md`](OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md).
Older PMR/HAL and class/COM planning documents are provenance only where they
conflict with these contracts.

## 10. Error Model

All project-model failures MUST be deterministic and reproducible.

Error classes:

- syntax/header errors: parser diagnostics.
- static semantic violations: binder/type checker diagnostics.
- broken, ambiguous or inaccessible compiler references: project/reference
  diagnostics with source or metadata provenance.
- unavailable runtime targets after a legal declaration, such as a missing DLL
  export: runtime diagnostics rather than invented compile errors.
- host materialization or policy failures: stable host outcomes mapped to the
  VBA phase and error required for that operation.

No silent fallback is allowed for violated MUST constraints.

## 11. Verification And Current Truth

This semantic contract is refined into independently closable rows. Current
implementation and evidence state is owned by the active canonical matrices,
not by narrative in this file:

- [`../validation/CORE_COMPILER_VM_JIT_READINESS_MATRIX_V1.csv`](../validation/CORE_COMPILER_VM_JIT_READINESS_MATRIX_V1.csv)
  for compiler analysis, typed binding and project/reference facts;
- [`../validation/OXIMAGE_PACKAGE_CONTRACT_MATRIX_V1.csv`](../validation/OXIMAGE_PACKAGE_CONTRACT_MATRIX_V1.csv)
  for verified artifact exports and references;
- [`../validation/WINDOWS_ABI_CARRIER_MATRIX_V1.csv`](../validation/WINDOWS_ABI_CARRIER_MATRIX_V1.csv)
  and the Windows COM matrices for authoritative type-library and interop rows;
- [`../validation/LANGUAGE_SERVICE_REFERENCE_KIND_MATRIX_V1.csv`](../validation/LANGUAGE_SERVICE_REFERENCE_KIND_MATRIX_V1.csv)
  for IDE reference-kind parity;
- [`../validation/CURRENT_STACK_EXCEL_ORACLE_MATRIX_V1.csv`](../validation/CURRENT_STACK_EXCEL_ORACLE_MATRIX_V1.csv)
  for current Excel/VBA evidence.

The PMR clause catalog and older conformance plans remain supporting source
indexes and fixture provenance. They cannot override this contract or establish
current completion by themselves.

Required evidence includes:

- source and verified-image public-surface equivalence, including public data;
- reference order, collision, visibility, `Option Private` and broken-reference
  diagnostics;
- stable identity/provenance across source, providers, artifacts and snapshots;
- compile-time class/property/event legality and VM3/JIT structural runtime
  observables;
- Windows transport evidence for COM-specific rows, kept distinct from portable
  project/class semantics.

## 12. Uncertainty and Implementation-defined Areas

Explicitly implementation-defined by the cited public source rules:

- project physical representation and storage mechanism (`SPEC-...-01231`).
- mechanism used to identify referenced projects (`SPEC-...-01233`).
- open host project module extension mechanism (`SPEC-...-01241`, `...-01299`).

These MUST be tracked in implementation-defined and oracle/evidence artifacts
before the affected row is verified. An uncertainty does not authorize current
OxVba behavior as the target and does not silently narrow the accepted VBA
scope.

## 13. Status Ownership

[`../ARCHITECTURE.md`](../ARCHITECTURE.md) records current realization and
gaps. Active worksets, beads, canonical matrices and evidence artifacts own the
delivery sequence and status. Historical implementation plans may explain why
a rule was introduced, but they do not change the rule or prove it complete.
