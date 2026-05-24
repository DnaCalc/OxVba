use oxvba_compiler::{ProcedureDescriptor, ProcedureKind, ProjectReflection, VbaType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReflectionInput {
    pub project_name: String,
    pub reflection: ProjectReflection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperGenerationPlan {
    pub plan_id: String,
    pub input: ProjectReflectionInput,
    pub output_kind: WrapperOutputKind,
    pub callable_selection: CallableSelectionPlan,
    pub conversion_lanes: Vec<WrapperConversionLane>,
    pub diagnostics_policy: WrapperDiagnosticsPolicy,
    pub argument_parser: Option<ArgumentParserPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapperOutputKind {
    CliExe,
    IntrospectionExe,
    NativeLibrary,
    ComServer,
    FutureXll,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallableSelectionPlan {
    ExplicitCallableIds(Vec<String>),
    PublicProceduralFunctions,
    HostPolicyNamed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperConversionLane {
    pub lane_id: String,
    pub argument_lane: String,
    pub result_lane: String,
    pub supported_types: Vec<VbaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperDiagnosticsPolicy {
    pub lane: String,
    pub include_descriptor_identity: bool,
    pub fail_on_unsupported_callable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentParserPlan {
    pub parser_kind: ArgumentParserKind,
    pub accepts_named_arguments: bool,
    pub emits_host_context: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentParserKind {
    PositionalCli,
    JsonStdin,
    NativeAbi,
    ComDispatch,
    XllOper,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedWrapperArtifact {
    pub artifact_kind: WrapperArtifactKind,
    pub path_hint: String,
    pub content: Option<String>,
    pub diagnostics: Vec<WrapperPlanDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapperArtifactKind {
    RustSource,
    Manifest,
    DefFile,
    MetadataJson,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperPlanDiagnostic {
    pub code: String,
    pub message: String,
    pub callable_id: Option<String>,
}

impl WrapperGenerationPlan {
    pub fn selected_callables(&self) -> (Vec<&ProcedureDescriptor>, Vec<WrapperPlanDiagnostic>) {
        let selected = match &self.callable_selection {
            CallableSelectionPlan::ExplicitCallableIds(ids) => ids
                .iter()
                .filter_map(|id| {
                    self.input
                        .reflection
                        .procedures
                        .iter()
                        .find(|procedure| &procedure.callable_id == id)
                })
                .collect::<Vec<_>>(),
            CallableSelectionPlan::PublicProceduralFunctions => self
                .input
                .reflection
                .procedures
                .iter()
                .filter(|procedure| {
                    procedure.kind == ProcedureKind::Function
                        && procedure.visibility.is_public
                        && !procedure.visibility.is_class_member
                })
                .collect::<Vec<_>>(),
            CallableSelectionPlan::HostPolicyNamed(_) => Vec::new(),
        };

        let diagnostics = selected
            .iter()
            .flat_map(|procedure| self.diagnostics_for_callable(procedure))
            .collect::<Vec<_>>();
        (selected, diagnostics)
    }

    fn diagnostics_for_callable(
        &self,
        procedure: &ProcedureDescriptor,
    ) -> Vec<WrapperPlanDiagnostic> {
        if self.conversion_lanes.is_empty() {
            return vec![WrapperPlanDiagnostic {
                code: "WRAPPER-NO-CONVERSION-LANE".to_string(),
                message: "wrapper plan has no conversion lanes".to_string(),
                callable_id: Some(procedure.callable_id.clone()),
            }];
        }
        Vec::new()
    }
}

impl WrapperConversionLane {
    pub fn typed_scalar_first_tier() -> Self {
        Self {
            lane_id: "TypedScalarFirstTier".to_string(),
            argument_lane: "typed-scalar-arguments".to_string(),
            result_lane: "typed-scalar-result".to_string(),
            supported_types: vec![
                VbaType::Long,
                VbaType::Double,
                VbaType::String,
                VbaType::Boolean,
            ],
        }
    }

    pub fn variant_positional() -> Self {
        Self {
            lane_id: "VariantPositional".to_string(),
            argument_lane: "variant-positional-arguments".to_string(),
            result_lane: "variant-result".to_string(),
            supported_types: vec![VbaType::Variant],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_compiler::{
        ModuleKind, ProjectKind, ProjectManifest, compile_project, module_unit_from_source,
    };

    fn reflection() -> ProjectReflection {
        let manifest = ProjectManifest {
            project_name: "WrapProj".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source(
                    "Main",
                    ModuleKind::Procedural,
                    "Public Function Add(a As Long) As Long\nAdd = a\nEnd Function\nPublic Sub Helper()\nEnd Sub",
                )
                .unwrap(),
                module_unit_from_source(
                    "Widget",
                    ModuleKind::Class,
                    "Public Function ClassAdd(a As Long) As Long\nClassAdd = a\nEnd Function",
                )
                .unwrap(),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        };
        compile_project(&manifest).unwrap().project_reflection
    }

    #[test]
    fn wrapper_plan_selects_by_explicit_identity() {
        let reflection = reflection();
        let callable_id = reflection
            .procedures
            .iter()
            .find(|procedure| procedure.procedure_name == "add")
            .unwrap()
            .callable_id
            .clone();
        let plan = WrapperGenerationPlan {
            plan_id: "explicit".to_string(),
            input: ProjectReflectionInput {
                project_name: "WrapProj".to_string(),
                reflection,
            },
            output_kind: WrapperOutputKind::CliExe,
            callable_selection: CallableSelectionPlan::ExplicitCallableIds(vec![callable_id]),
            conversion_lanes: vec![WrapperConversionLane::typed_scalar_first_tier()],
            diagnostics_policy: WrapperDiagnosticsPolicy {
                lane: "text".to_string(),
                include_descriptor_identity: true,
                fail_on_unsupported_callable: true,
            },
            argument_parser: Some(ArgumentParserPlan {
                parser_kind: ArgumentParserKind::PositionalCli,
                accepts_named_arguments: false,
                emits_host_context: true,
            }),
        };
        let (selected, diagnostics) = plan.selected_callables();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].procedure_name, "add");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn wrapper_plan_represents_cli_native_com_and_future_xll_profiles() {
        let kinds = [
            WrapperOutputKind::CliExe,
            WrapperOutputKind::IntrospectionExe,
            WrapperOutputKind::NativeLibrary,
            WrapperOutputKind::ComServer,
            WrapperOutputKind::FutureXll,
        ];
        assert_eq!(kinds.len(), 5);
        let parsers = [
            ArgumentParserKind::PositionalCli,
            ArgumentParserKind::JsonStdin,
            ArgumentParserKind::NativeAbi,
            ArgumentParserKind::ComDispatch,
            ArgumentParserKind::XllOper,
        ];
        assert_eq!(parsers.len(), 5);
    }

    #[test]
    fn wrapper_plan_public_function_selection_is_host_build_policy() {
        let plan = WrapperGenerationPlan {
            plan_id: "public-functions".to_string(),
            input: ProjectReflectionInput {
                project_name: "WrapProj".to_string(),
                reflection: reflection(),
            },
            output_kind: WrapperOutputKind::NativeLibrary,
            callable_selection: CallableSelectionPlan::PublicProceduralFunctions,
            conversion_lanes: Vec::new(),
            diagnostics_policy: WrapperDiagnosticsPolicy {
                lane: "structured".to_string(),
                include_descriptor_identity: true,
                fail_on_unsupported_callable: false,
            },
            argument_parser: None,
        };
        let (selected, diagnostics) = plan.selected_callables();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].procedure_name, "add");
        assert_eq!(diagnostics[0].code, "WRAPPER-NO-CONVERSION-LANE");
    }
}
