# Neutral Callable Descriptor Model And VbaHost API Contract

Date: 2026-05-24
Bead: `bd-hjys.2`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`
Depends on: `docs/evidence/host_callable/BOUNDARY_AUDIT.md`

## Purpose

This document freezes the first-pass contract for replacing the old host-UDF
shape with a neutral host/project callable reflection and invocation model.

The contract is intentionally compatibility-free. Deprecated `HostUdf*` APIs,
UDF-named options, shims, bridges, and docs are removal targets, not adapters.
Git history is the archeology path.

## Ownership boundaries

| Layer | Owns | Must not own |
| --- | --- | --- |
| `oxvba-compiler` | VBA project/module/procedure/signature facts; source fingerprints; procedure runtime metadata; explicit source/project annotations. | UDF admission, worksheet-visible policy, volatility, dependency policy, side-effect policy, thread-safety claims, host registry identity, XLL/Excel registration. |
| `oxvba-runtime` / `oxvba-vm` | Prepared callable execution primitives, call frames, generic call context carrier, typed/variant conversion validation, execution diagnostics. | A worksheet-UDF concept; Excel-specific caller semantics; host registry semantics; build-target selection. |
| `oxvba-host` | `VbaHost` facade, project loading, reflection projection, session preparation, neutral callable invocation APIs, phase diagnostics. | Formula binding/name precedence; OxFunc registry mutation; XLL registration; compiler-owned metadata fabrication. |
| `oxvba-project` | Project file parsing/generation for source modules, build profiles, explicit native export declarations, explicit annotations. | Runtime execution policy or host UDF policy. |
| `oxvba-build` | Wrapper-generation plans and artifact generation for EXE/DLL/COM/future XLL profiles. | Compiler/runtime callable truth; generic UDF semantics. |
| Embedding host / DNA Calc consumer | Callable admission policy, UDF interpretation, registry requests, formula context, cancellation policy, host-specific diagnostics. | Mutating compiler/runtime facts or keeping a second hidden callable truth source. |
| OxFunc/OxFml | OxFunc owns UDF registration snapshots/change sets; OxFml owns formula binding/name precedence. | OxVba project reflection or runtime execution internals. |

## Crate/API placement

The new neutral API should be exposed from `oxvba-host` because it is the
in-process host facade over compiler/runtime/build-independent behavior. Compiler
and bundle crates provide the facts consumed by this facade.

Recommended modules/names:

| Proposed item | Crate/module | Notes |
| --- | --- | --- |
| `VbaHost` | `oxvba-host::host` or `oxvba-host::project_host` | Top-level in-process host object. |
| `VbaHostOptions` | `oxvba-host` | Runtime profile, policy preset, callback/services configuration, diagnostics options. No UDF policy. |
| `ProjectSource` | `oxvba-host` or `oxvba-project` | Text modules, file paths, bundle bytes, or caller-provided blob set. |
| `LoadedVbaProject` | `oxvba-host` | Owns compiled project or bundle-backed project plus reflection. |
| `PreparedVbaProject` | `oxvba-host` | Runtime session ready for calls. |
| `ProjectReflection` | `oxvba-host` projection over compiler/bundle facts | Host-facing neutral reflection graph. |
| `BundleCallableDescriptor` | `oxvba-compiler::bundle` | Serialized neutral callable descriptor inventory. |
| `WrapperGenerationPlan` | `oxvba-build` | Build-time wrapper plan over reflection. |

## Descriptor model

### `ProjectIdentity`

```rust
pub struct ProjectIdentity {
    pub project_name: String,
    pub project_id: ProjectId,
    pub source_fingerprint: SourceFingerprint,
    pub load_fingerprint: LoadFingerprint,
    pub bundle_identity: Option<BundleIdentity>,
}
```

Rules:

- `project_id` is stable for descriptor lookup within a loaded project.
- `source_fingerprint` changes when source/module/signature truth changes.
- `load_fingerprint` may include host load inputs such as file path set or blob
  set but must not include host UDF admission policy.
- `bundle_identity` is present for bundle-loaded projects and records bundle
  format/version/fingerprint.

### `ModuleDescriptor`

```rust
pub struct ModuleDescriptor {
    pub module_id: ModuleId,
    pub project_id: ProjectId,
    pub name: String,
    pub kind: ModuleKind,
    pub visibility: ModuleVisibility,
    pub source_fingerprint: SourceFingerprint,
    pub source_span: Option<SourceSpan>,
}

pub enum ModuleKind {
    Procedural,
    Class,
    Document,
    Form,
}

pub struct ModuleVisibility {
    pub option_private_module: bool,
    pub vb_exposed: bool,
    pub vb_creatable: bool,
}
```

Rules:

- Module visibility is source/project fact only.
- `vb_exposed` and `vb_creatable` do not imply host callable admission.

### `ProcedureDescriptor`

```rust
pub struct ProcedureDescriptor {
    pub callable_id: CallableId,
    pub project_id: ProjectId,
    pub module_id: ModuleId,
    pub module_name: String,
    pub procedure_name: String,
    pub kind: ProcedureKind,
    pub visibility: ProcedureVisibility,
    pub signature: ProcedureSignature,
    pub runtime_route: Option<RuntimeProcedureRoute>,
    pub source_span: Option<SourceSpan>,
    pub descriptor_fingerprint: DescriptorFingerprint,
    pub annotations: Vec<ProcedureAnnotation>,
}

pub enum ProcedureKind {
    Sub,
    Function,
    PropertyGet,
    PropertyLet,
    PropertySet,
    Event,
}

pub struct ProcedureVisibility {
    pub is_public: bool,
    pub is_option_private: bool,
    pub is_class_member: bool,
}
```

Rules:

- Public procedural functions are just facts. A host may admit them as UDFs, but
  the descriptor does not.
- Public Subs remain descriptors. They may be commands or startup entries for a
  host, but not UDFs unless a host invents such a policy outside OxVba core.
- Class procedures remain class procedures; they are not standalone host
  callables unless a wrapper plan explicitly binds an object/instance model.
- `descriptor_fingerprint` includes project/module/procedure identity, procedure
  kind, parameter names/types, return type, source fingerprint, and explicit
  annotations that affect the callable surface. It excludes host policy overlays.

### `ProcedureSignature`

```rust
pub struct ProcedureSignature {
    pub parameters: Vec<ProcedureParameterDescriptor>,
    pub return_type: Option<VbaTypeDescriptor>,
    pub calling_shape: CallingShape,
}

pub struct ProcedureParameterDescriptor {
    pub name: Option<String>,
    pub passing_mode: PassingMode,
    pub optional: bool,
    pub param_array: bool,
    pub default_value: Option<LiteralValue>,
    pub value_type: Option<VbaTypeDescriptor>,
    pub source_type_text: Option<String>,
}

pub enum PassingMode {
    ByVal,
    ByRef,
    Unknown,
}

pub struct VbaTypeDescriptor {
    pub normalized: VbaType,
    pub source_text: Option<String>,
}

pub enum VbaType {
    Variant,
    Boolean,
    Byte,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Single,
    Double,
    Currency,
    Date,
    String,
    Object,
    Array,
    UserDefined(String),
    Any,
    Unknown,
}

pub enum CallingShape {
    Procedure,
    PropertyAccessor,
    EventHandler,
}
```

Rules:

- The signature is neutral. It does not contain Excel type strings, XLL type
  strings, COM `VARTYPE`, C ABI types, or formula registry type names.
- Boundary-specific type projections are conversion lanes owned by wrappers or
  hosts.
- If a source type cannot be normalized, keep `source_type_text` and mark
  `normalized = Unknown` rather than guessing.

### `RuntimeProcedureRoute`

```rust
pub struct RuntimeProcedureRoute {
    pub entry_pc: usize,
    pub param_slots: Vec<usize>,
    pub return_slot: Option<usize>,
    pub conversion_lanes: Vec<ConversionLaneId>,
}
```

Rules:

- This is runtime execution data, not host policy.
- Bundle descriptor inventory may include it when a bundle can support direct
  invocation.
- `conversion_lanes` names what the runtime can currently validate/convert, not
  what a host is willing to expose.

### `CallableCapability`

```rust
pub struct CallableCapability {
    pub callable_id: CallableId,
    pub invocable_in_prepared_session: bool,
    pub supported_invocation_lanes: Vec<InvocationLane>,
    pub unsupported_reasons: Vec<UnsupportedReason>,
}

pub enum InvocationLane {
    VariantPositional,
    TypedScalarFirstTier,
    HostContextAware,
}
```

Rules:

- Capability states describe the current OxVba implementation ability.
- They do not describe worksheet safety, volatility, dependency graph semantics,
  side-effect policy, or thread safety. Those are host/wrapper policy.

## Reflection graph API

```rust
pub struct ProjectReflection {
    pub identity: ProjectIdentity,
    pub modules: Vec<ModuleDescriptor>,
    pub procedures: Vec<ProcedureDescriptor>,
    pub capabilities: Vec<CallableCapability>,
}

impl ProjectReflection {
    pub fn modules(&self) -> &[ModuleDescriptor];
    pub fn procedures(&self) -> &[ProcedureDescriptor];
    pub fn public_procedures(&self) -> impl Iterator<Item = &ProcedureDescriptor>;
    pub fn public_functions(&self) -> impl Iterator<Item = &ProcedureDescriptor>;
    pub fn find_callable(&self, id: &CallableId) -> Option<&ProcedureDescriptor>;
    pub fn find_procedure(&self, module: &str, procedure: &str) -> Vec<&ProcedureDescriptor>;
}
```

Rules:

- Convenience filters such as `public_functions()` are neutral filters, not UDF
  admission.
- Any host-specific admission output must live outside `ProjectReflection`.

## VbaHost lifecycle contract

```rust
pub struct VbaHost {
    options: VbaHostOptions,
}

impl VbaHost {
    pub fn new(options: VbaHostOptions) -> Self;
    pub fn load_project(&self, source: ProjectSource) -> Result<LoadedVbaProject, HostDiagnostic>;
    pub fn load_bundle(&self, bytes: &[u8]) -> Result<LoadedVbaProject, HostDiagnostic>;
}

pub struct LoadedVbaProject { /* private */ }

impl LoadedVbaProject {
    pub fn identity(&self) -> &ProjectIdentity;
    pub fn reflection(&self) -> &ProjectReflection;
    pub fn prepare(&self) -> Result<PreparedVbaProject, HostDiagnostic>;
    pub fn diagnostics(&self) -> &[HostDiagnostic];
}

pub struct PreparedVbaProject { /* private */ }

impl PreparedVbaProject {
    pub fn reflection(&self) -> &ProjectReflection;
    pub fn invoke_variant(
        &mut self,
        callable_id: &CallableId,
        context: HostCallContext,
        args: &[Variant],
    ) -> Result<InvocationResult, HostDiagnostic>;

    pub fn invoke_typed(
        &mut self,
        callable_id: &CallableId,
        context: HostCallContext,
        args: &[TypedValue],
    ) -> Result<TypedInvocationResult, HostDiagnostic>;
}
```

Rules:

- `VbaHost` is the embedding/process-facing root. `Engine` may remain internal or
  lower-level, but new host examples should use `VbaHost`.
- `LoadedVbaProject::reflection()` is available before runtime preparation.
- `PreparedVbaProject` owns mutable execution/session state.
- Multiple loaded/prepared projects must not share callable/session identity.

## Project source contract

```rust
pub enum ProjectSource {
    ModuleTexts(Vec<ProjectModuleText>),
    FileSet(ProjectFileSet),
    BundleBytes(Vec<u8>),
}

pub struct ProjectModuleText {
    pub name_hint: Option<String>,
    pub kind_hint: Option<ModuleKind>,
    pub text: String,
}
```

Rules:

- The embedding process may load text/blob/path data however it wants. OxVba
  should not require file ownership for in-process hosting.
- File-set loading is convenience and provenance; text/blob loading is equally
  first-class.

## HostCallContext contract

```rust
pub struct HostCallContext {
    pub caller: Option<HostCaller>,
    pub locale_id: Option<u32>,
    pub cancellation: Option<HostCancellationToken>,
    pub metadata: BTreeMap<String, HostContextValue>,
}

pub struct HostCaller {
    pub source_system: String,
    pub display_text: Option<String>,
    pub stable_id: Option<String>,
    pub metadata: BTreeMap<String, HostContextValue>,
}

pub enum HostContextValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    StringList(Vec<String>),
}
```

Rules:

- `HostCallContext` is provenance/context, not UDF policy.
- Excel-like callers can set `source_system = "excel"` or host-specific metadata;
  OxVba does not interpret worksheet address precedence.
- Runtime delivery must be real: context must be available to the invocation
  path or to a documented host-service observation point. It must not be only an
  output echo.
- Cancellation may initially be unsupported, but the shape must not preclude it.

## Invocation contracts

### Variant invocation

```rust
pub struct InvocationResult {
    pub value: Variant,
    pub context_observations: HostContextObservations,
    pub diagnostics: Vec<HostDiagnostic>,
}
```

Rules:

- Resolves by `CallableId`.
- Validates callable existence and arity.
- Uses `RuntimeProcedureRoute` to invoke a prepared session.
- Preserves runtime diagnostics with phase and callable identity.

### Typed invocation

```rust
pub enum TypedValue {
    Empty,
    Boolean(bool),
    Byte(u8),
    Integer(i16),
    Long(i32),
    LongLong(i64),
    Single(f32),
    Double(f64),
    CurrencyScaled(i64),
    DateSerial(f64),
    String(String),
    Variant(Variant),
}

pub struct TypedInvocationResult {
    pub value: TypedValue,
    pub declared_type: Option<VbaTypeDescriptor>,
    pub diagnostics: Vec<HostDiagnostic>,
}
```

Rules:

- Typed invocation is a conversion lane, not a UDF feature.
- The first implementation may support only a documented scalar subset; unsupported
  types return structured diagnostics.
- Type mismatch and arity mismatch are validation diagnostics, not panics.
- `TypedValue::Variant` is a deliberate escape lane, not the default for all
  typed calls.

## Diagnostics contract

```rust
pub struct HostDiagnostic {
    pub phase: HostDiagnosticPhase,
    pub code: String,
    pub message: String,
    pub project_id: Option<ProjectId>,
    pub module_id: Option<ModuleId>,
    pub callable_id: Option<CallableId>,
    pub source_span: Option<SourceSpan>,
}

pub enum HostDiagnosticPhase {
    Load,
    Compile,
    Reflect,
    Prepare,
    ValidateCall,
    Runtime,
    WrapperGeneration,
}
```

Rules:

- Diagnostics carry enough identity for host UI and wrapper error reporting.
- Diagnostics do not imply formula error mapping; formula hosts map diagnostics
  to formula errors outside OxVba.

## Bundle descriptor contract

The bundle inventory should change from host-call descriptors to neutral callable
descriptors:

```rust
pub struct DescriptorInventory {
    pub callables: Vec<BundleCallableDescriptor>,
    pub com_classes: Vec<BundleComClassDescriptor>,
    pub com_events: Vec<BundleComEventDescriptor>,
}

pub struct BundleCallableDescriptor {
    pub callable_id: String,
    pub project_name: String,
    pub module_name: String,
    pub procedure_name: String,
    pub kind: ProcedureKind,
    pub visibility: ProcedureVisibility,
    pub signature: ProcedureSignature,
    pub runtime_route: Option<RuntimeProcedureRoute>,
    pub descriptor_fingerprint: String,
    pub annotations: Vec<ProcedureAnnotation>,
}
```

Rules:

- Remove `selection_policy`, synthesized `volatile`, synthesized dependency,
  side-effect, thread-safety, and `allowed_contexts` from compiler-owned bundle
  callable descriptors.
- Bundle-loaded projects use `callables` as packaged descriptor truth when
  present.
- Missing descriptor inventory is an explicit `DescriptorInventoryUnavailable`
  state, not permission to invent policy.

## Build wrapper contract

```rust
pub struct WrapperGenerationPlan {
    pub input: ProjectReflectionInput,
    pub output_kind: WrapperOutputKind,
    pub callable_selection: CallableSelectionPlan,
    pub conversion_lanes: Vec<WrapperConversionLane>,
    pub diagnostics_policy: WrapperDiagnosticsPolicy,
}
```

Rules:

- Wrapper plans consume `ProjectReflection`; they do not ask compiler/runtime to
  decide host-specific callable meaning.
- EXE introspection/call wrapper, wrapped native library, COM, and future XLL are
  profiles over this same plan shape.
- XLL registration metadata belongs only to an XLL wrapper plan.

## Host-owned UDF policy example contract

A UDF policy example is allowed only outside compiler/runtime neutral facts:

```rust
pub struct HostUdfAdmissionPolicy { /* example host code */ }

impl HostUdfAdmissionPolicy {
    pub fn admit(&self, reflection: &ProjectReflection) -> Vec<HostAdmittedUdf>;
}
```

Rules:

- This is example/host-owned policy, not core truth.
- It may project to OxFunc W093-shaped registration requests.
- It must not mutate `ProjectReflection` or `BundleCallableDescriptor`.
- It must not implement formula precedence or registry snapshots in OxVba.

## No-compatibility removal posture

The following old surfaces are deleted/replaced directly when their owning bead
lands:

| Old surface | Replacement |
| --- | --- |
| `HostUdfCatalog` | `ProjectReflection` / `CallableCatalog` |
| `HostUdfFunctionDescriptor` | `ProcedureDescriptor` |
| `HostUdfCallContext` | `HostCallContext` |
| `HostUdfTypedValue` | `TypedValue` |
| `HostUdfTypedSignature` | `CallableTypedSignature` |
| `HostUdfTypedInvokeResult` | `TypedInvocationResult` |
| `HostUdfInvokeResult` | `InvocationResult` |
| `HostUdfTypeMapEvidence` | generic conversion-lane evidence or delete if unused |
| `Engine::host_udf_catalog` | `LoadedVbaProject::reflection` / `PreparedVbaProject::reflection` |
| `Engine::invoke_host_udf_*` | `PreparedVbaProject::invoke_*` |
| `RuntimeCallSource::HostUdf` | neutral host-call source/provenance |
| `BundleHostCallDescriptor` | `BundleCallableDescriptor` |

No adapter layer should remain at the end of this workset.

## Implementation order dependencies

- `bd-hjys.3` implements descriptor structs/projections from compiler facts.
- `bd-hjys.4` persists/consumes `BundleCallableDescriptor`.
- `bd-hjys.5` introduces `VbaHost`, `LoadedVbaProject`, and
  `PreparedVbaProject`.
- `bd-hjys.6` implements `HostCallContext` delivery and neutral invocation.
- `bd-hjys.7` deletes old `HostUdf*` APIs and migrates tests.
- `bd-hjys.8` adds host-owned UDF policy/W093 projection example.
- `bd-hjys.9+` use this contract for wrapper-generation work.

## Fresh-eyes review notes

Read-through findings after drafting:

- The document separates neutral convenience filters such as `public_functions()`
  from host UDF admission, avoiding a hidden UDF policy in the reflection API.
- `HostCallContext` is explicitly provenance/context and must be delivered to the
  invocation path or a documented observation point, addressing the discarded
  frame problem from the audit.
- Bundle descriptors explicitly remove synthesized policy fields rather than
  renaming them.
- The no-compatibility deletion posture is repeated in a concrete old-to-new
  table so later implementation beads can use it as a search checklist.
