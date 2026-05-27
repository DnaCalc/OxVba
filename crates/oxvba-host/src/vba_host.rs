use std::{collections::BTreeMap, path::PathBuf};

use oxvba_compiler::{
    ModuleKind, OxBundle, ProjectKind, ProjectManifest, ProjectReflection, VbaType,
    compile_project, module_unit_from_source,
};
use oxvba_runtime::Variant;

use crate::engine::{Engine, HostConfig, PhaseDiagnostic, ProjectRuntimeSession};

#[derive(Debug, Clone, Default)]
pub struct VbaHostOptions {
    pub host_config: HostConfig,
}

pub struct VbaHost {
    options: VbaHostOptions,
}

#[derive(Debug, Clone)]
pub enum ProjectSource {
    ModuleTexts(Vec<ProjectModuleText>),
    FileSet(ProjectFileSet),
    BundleBytes(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct ProjectModuleText {
    pub name_hint: Option<String>,
    pub kind_hint: Option<ModuleKind>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ProjectFileSet {
    pub project_name: String,
    pub files: Vec<ProjectFile>,
}

#[derive(Debug, Clone)]
pub struct ProjectFile {
    pub path: PathBuf,
    pub module_name: Option<String>,
    pub module_kind: Option<ModuleKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostDiagnostic {
    pub phase: HostDiagnosticPhase,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostDiagnosticPhase {
    Load,
    Compile,
    Prepare,
    ValidateCall,
    Runtime,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostCallContext {
    pub caller: Option<HostCaller>,
    pub locale_id: Option<u32>,
    pub metadata: BTreeMap<String, HostContextValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostCaller {
    pub source_system: String,
    pub display_text: Option<String>,
    pub stable_id: Option<String>,
    pub metadata: BTreeMap<String, HostContextValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostContextValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    StringList(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedValue {
    Empty,
    Integer(i16),
    Long(i32),
    LongLong(i64),
    Single(f32),
    Double(f64),
    Boolean(bool),
    String(String),
    Variant(Variant),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostContextObservations {
    pub caller_source_system: Option<String>,
    pub locale_id: Option<u32>,
    pub metadata_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InvocationResult {
    pub value: Variant,
    pub conversion_lane: String,
    pub context_observations: HostContextObservations,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedInvocationResult {
    pub value: TypedValue,
    pub conversion_lane: String,
    pub context_observations: HostContextObservations,
}

pub struct LoadedVbaProject {
    source: LoadedProjectSource,
    reflection: ProjectReflection,
    diagnostics: Vec<HostDiagnostic>,
    options: VbaHostOptions,
}

enum LoadedProjectSource {
    Manifest(ProjectManifest),
    Bundle(Box<OxBundle>),
}

pub struct PreparedVbaProject {
    engine: Engine,
    session: ProjectRuntimeSession,
    last_context_observations: Option<HostContextObservations>,
}

impl VbaHost {
    pub fn new(options: VbaHostOptions) -> Self {
        Self { options }
    }

    pub fn load_project(&self, source: ProjectSource) -> Result<LoadedVbaProject, HostDiagnostic> {
        match source {
            ProjectSource::ModuleTexts(modules) => {
                self.load_module_texts("InMemoryProject", modules)
            }
            ProjectSource::FileSet(file_set) => self.load_file_set(file_set),
            ProjectSource::BundleBytes(bytes) => self.load_bundle_bytes(&bytes),
        }
    }

    pub fn load_bundle(&self, bytes: &[u8]) -> Result<LoadedVbaProject, HostDiagnostic> {
        self.load_bundle_bytes(bytes)
    }

    fn load_module_texts(
        &self,
        project_name: &str,
        modules: Vec<ProjectModuleText>,
    ) -> Result<LoadedVbaProject, HostDiagnostic> {
        let mut units = Vec::with_capacity(modules.len());
        for (index, module) in modules.into_iter().enumerate() {
            let name = module
                .name_hint
                .unwrap_or_else(|| format!("Module{}", index + 1));
            let kind = module.kind_hint.unwrap_or(ModuleKind::Procedural);
            let unit = module_unit_from_source(name, kind, module.text).map_err(|err| {
                HostDiagnostic::new(
                    HostDiagnosticPhase::Load,
                    "HOST-LOAD-MODULE".to_string(),
                    err.to_string(),
                )
            })?;
            units.push(unit);
        }
        let manifest = ProjectManifest {
            project_name: project_name.to_string(),
            project_kind: ProjectKind::Source,
            modules: units,
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        };
        self.load_manifest(manifest)
    }

    fn load_file_set(&self, file_set: ProjectFileSet) -> Result<LoadedVbaProject, HostDiagnostic> {
        let modules = file_set
            .files
            .into_iter()
            .map(|file| {
                let text = std::fs::read_to_string(&file.path).map_err(|err| {
                    HostDiagnostic::new(
                        HostDiagnosticPhase::Load,
                        "HOST-LOAD-FILE".to_string(),
                        format!("{}: {err}", file.path.display()),
                    )
                })?;
                let name_hint = file.module_name.or_else(|| {
                    file.path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(ToString::to_string)
                });
                Ok(ProjectModuleText {
                    name_hint,
                    kind_hint: file.module_kind,
                    text,
                })
            })
            .collect::<Result<Vec<_>, HostDiagnostic>>()?;
        self.load_module_texts(&file_set.project_name, modules)
    }

    fn load_manifest(&self, manifest: ProjectManifest) -> Result<LoadedVbaProject, HostDiagnostic> {
        let compiled = compile_project(&manifest).map_err(|err| {
            HostDiagnostic::new(
                HostDiagnosticPhase::Compile,
                err.code().to_string(),
                err.to_string(),
            )
        })?;
        Ok(LoadedVbaProject {
            source: LoadedProjectSource::Manifest(manifest),
            reflection: compiled.project_reflection,
            diagnostics: Vec::new(),
            options: self.options.clone(),
        })
    }

    fn load_bundle_bytes(&self, bytes: &[u8]) -> Result<LoadedVbaProject, HostDiagnostic> {
        let bundle = OxBundle::deserialize_from_bytes(bytes).map_err(|err| {
            HostDiagnostic::new(
                HostDiagnosticPhase::Load,
                "HOST-LOAD-BUNDLE".to_string(),
                err,
            )
        })?;
        let reflection = bundle.project_reflection().map_err(|err| {
            HostDiagnostic::new(
                HostDiagnosticPhase::Load,
                "HOST-BUNDLE-DESCRIPTOR-INVENTORY".to_string(),
                format!("bundle descriptor inventory unavailable: {err:?}"),
            )
        })?;
        Ok(LoadedVbaProject {
            source: LoadedProjectSource::Bundle(Box::new(bundle)),
            reflection,
            diagnostics: Vec::new(),
            options: self.options.clone(),
        })
    }
}

impl Default for VbaHost {
    fn default() -> Self {
        Self::new(VbaHostOptions::default())
    }
}

impl LoadedVbaProject {
    pub fn reflection(&self) -> &ProjectReflection {
        &self.reflection
    }

    pub fn diagnostics(&self) -> &[HostDiagnostic] {
        &self.diagnostics
    }

    pub fn prepare(&self) -> Result<PreparedVbaProject, HostDiagnostic> {
        let engine = Engine::new(self.options.host_config.clone());
        let session = match &self.source {
            LoadedProjectSource::Manifest(manifest) => engine.compile_and_prepare_session(manifest),
            LoadedProjectSource::Bundle(bundle) => {
                engine.compile_and_prepare_session_from_bundle(bundle)
            }
        }
        .map_err(HostDiagnostic::from_phase_diagnostic)?;
        Ok(PreparedVbaProject {
            engine,
            session,
            last_context_observations: None,
        })
    }
}

impl PreparedVbaProject {
    pub fn reflection(&self) -> &ProjectReflection {
        self.session.project_reflection()
    }

    pub fn last_context_observations(&self) -> Option<&HostContextObservations> {
        self.last_context_observations.as_ref()
    }

    pub fn invoke_callable_variant(
        &mut self,
        callable_id: &str,
        context: HostCallContext,
        args: &[Variant],
    ) -> Result<InvocationResult, HostDiagnostic> {
        let (module_name, procedure_name, expected_arity) = self.resolve_callable(callable_id)?;
        if args.len() != expected_arity {
            return Err(HostDiagnostic::new(
                HostDiagnosticPhase::ValidateCall,
                "HOST-CALL-ARITY".to_string(),
                format!(
                    "callable `{callable_id}` expects {expected_arity} arguments, got {}",
                    args.len()
                ),
            ));
        }
        let observations = observe_context(&context);
        self.last_context_observations = Some(observations.clone());
        let value = self.invoke_by_name_variant(&module_name, &procedure_name, args)?;
        Ok(InvocationResult {
            value,
            conversion_lane: "VariantPositional".to_string(),
            context_observations: observations,
        })
    }

    pub fn invoke_callable_typed(
        &mut self,
        callable_id: &str,
        context: HostCallContext,
        args: &[TypedValue],
    ) -> Result<TypedInvocationResult, HostDiagnostic> {
        let procedure = self
            .reflection()
            .procedures
            .iter()
            .find(|procedure| procedure.callable_id == callable_id)
            .cloned()
            .ok_or_else(|| {
                HostDiagnostic::new(
                    HostDiagnosticPhase::ValidateCall,
                    "HOST-CALL-NOT-FOUND".to_string(),
                    format!("callable not found: {callable_id}"),
                )
            })?;
        if args.len() != procedure.signature.parameters.len() {
            return Err(HostDiagnostic::new(
                HostDiagnosticPhase::ValidateCall,
                "HOST-CALL-ARITY".to_string(),
                format!(
                    "callable `{callable_id}` expects {} arguments, got {}",
                    procedure.signature.parameters.len(),
                    args.len()
                ),
            ));
        }
        let mut variants = Vec::with_capacity(args.len());
        for (index, (arg, param)) in args
            .iter()
            .zip(procedure.signature.parameters.iter())
            .enumerate()
        {
            variants.push(typed_value_to_variant(
                arg,
                param.value_type.as_ref().map(|ty| &ty.normalized),
                callable_id,
                index,
            )?);
        }
        let invocation = self.invoke_callable_variant(callable_id, context, &variants)?;
        let return_type = procedure
            .signature
            .return_type
            .as_ref()
            .map(|ty| &ty.normalized);
        let value = variant_to_typed_value(invocation.value, return_type, callable_id)?;
        Ok(TypedInvocationResult {
            value,
            conversion_lane: "TypedScalarFirstTier".to_string(),
            context_observations: invocation.context_observations,
        })
    }

    fn resolve_callable(
        &self,
        callable_id: &str,
    ) -> Result<(String, String, usize), HostDiagnostic> {
        self.reflection()
            .procedures
            .iter()
            .find(|procedure| procedure.callable_id == callable_id)
            .map(|procedure| {
                (
                    procedure.module_name.clone(),
                    procedure.procedure_name.clone(),
                    procedure.signature.parameters.len(),
                )
            })
            .ok_or_else(|| {
                HostDiagnostic::new(
                    HostDiagnosticPhase::ValidateCall,
                    "HOST-CALL-NOT-FOUND".to_string(),
                    format!("callable not found: {callable_id}"),
                )
            })
    }

    pub fn invoke_by_name_variant(
        &mut self,
        module_name: &str,
        procedure_name: &str,
        args: &[Variant],
    ) -> Result<Variant, HostDiagnostic> {
        self.engine
            .invoke_procedure_with_variants(&mut self.session, module_name, procedure_name, args)
            .map_err(HostDiagnostic::from_phase_diagnostic)
    }
}

fn observe_context(context: &HostCallContext) -> HostContextObservations {
    HostContextObservations {
        caller_source_system: context
            .caller
            .as_ref()
            .map(|caller| caller.source_system.clone()),
        locale_id: context.locale_id,
        metadata_keys: context.metadata.keys().cloned().collect(),
    }
}

fn typed_value_to_variant(
    value: &TypedValue,
    expected: Option<&VbaType>,
    callable_id: &str,
    index: usize,
) -> Result<Variant, HostDiagnostic> {
    match (value, expected) {
        (TypedValue::Variant(value), _) => Ok(value.clone()),
        (TypedValue::Long(value), Some(VbaType::Long) | None) => Ok(Variant::from_i32(*value)),
        (TypedValue::Integer(value), Some(VbaType::Integer) | None) => {
            Ok(Variant::from_i16(*value))
        }
        (TypedValue::LongLong(value), Some(VbaType::LongLong) | None) => {
            Ok(Variant::from_i64(*value))
        }
        (TypedValue::Single(value), Some(VbaType::Single) | None) => Ok(Variant::from_f32(*value)),
        (TypedValue::Double(value), Some(VbaType::Double) | None) => Ok(Variant::from_f64(*value)),
        (TypedValue::Boolean(value), Some(VbaType::Boolean) | None) => {
            Ok(Variant::from_bool(*value))
        }
        (TypedValue::String(value), Some(VbaType::String) | None) => {
            Ok(Variant::from_string(value.clone()))
        }
        (TypedValue::Empty, _) => Ok(Variant::empty()),
        (_, Some(expected)) => Err(HostDiagnostic::new(
            HostDiagnosticPhase::ValidateCall,
            "HOST-CALL-TYPE".to_string(),
            format!(
                "callable `{callable_id}` argument {index} is not admitted by TypedScalarFirstTier for expected type {expected:?}"
            ),
        )),
    }
}

fn variant_to_typed_value(
    value: Variant,
    expected: Option<&VbaType>,
    callable_id: &str,
) -> Result<TypedValue, HostDiagnostic> {
    match expected {
        Some(VbaType::Long) => value.as_i32().map(TypedValue::Long),
        Some(VbaType::Integer) => value.as_i16().map(TypedValue::Integer),
        Some(VbaType::LongLong) => value.as_i64().map(TypedValue::LongLong),
        Some(VbaType::Single) => value.as_f32().map(TypedValue::Single),
        Some(VbaType::Double) => value.as_f64().map(TypedValue::Double),
        Some(VbaType::Boolean) => value.as_bool().map(TypedValue::Boolean),
        Some(VbaType::String) => value.as_bstr().map(|value| TypedValue::String(value.to_string())),
        Some(VbaType::Variant) | None => Some(TypedValue::Variant(value)),
        Some(other) => {
            return Err(HostDiagnostic::new(
                HostDiagnosticPhase::ValidateCall,
                "HOST-CALL-RETURN-TYPE".to_string(),
                format!(
                    "callable `{callable_id}` return type {other:?} is not admitted by TypedScalarFirstTier"
                ),
            ));
        }
    }
    .ok_or_else(|| {
        HostDiagnostic::new(
            HostDiagnosticPhase::Runtime,
            "HOST-CALL-RETURN-CONVERSION".to_string(),
            format!("callable `{callable_id}` return value did not match declared type"),
        )
    })
}

impl HostDiagnostic {
    fn new(phase: HostDiagnosticPhase, code: String, message: String) -> Self {
        Self {
            phase,
            code,
            message,
        }
    }

    fn from_phase_diagnostic(diagnostic: PhaseDiagnostic) -> Self {
        let phase = match diagnostic.phase() {
            crate::engine::DiagnosticPhase::CompileTime => HostDiagnosticPhase::Compile,
            crate::engine::DiagnosticPhase::Runtime => HostDiagnosticPhase::Runtime,
        };
        Self {
            phase,
            code: "HOST-PHASE-DIAGNOSTIC".to_string(),
            message: diagnostic.message().to_string(),
        }
    }
}
