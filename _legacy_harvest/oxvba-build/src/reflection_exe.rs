use oxvba_compiler::{ProcedureDescriptor, ProjectReflection, VbaType};
use oxvba_host::{
    HostCallContext, HostDiagnostic, HostDiagnosticPhase, PreparedVbaProject, ProjectSource,
    TypedValue, VbaHost,
};

use crate::wrapper_plan::{
    ArgumentParserKind, ArgumentParserPlan, CallableSelectionPlan, ProjectReflectionInput,
    WrapperConversionLane, WrapperDiagnosticsPolicy, WrapperGenerationPlan, WrapperOutputKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionExeGeneration {
    pub plan: WrapperGenerationPlan,
    pub rust_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionExeOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct ReflectionExeWrapper {
    reflection: ProjectReflection,
    prepared: PreparedVbaProject,
}

pub fn generate_reflection_exe_wrapper(
    project_name: impl Into<String>,
    reflection: ProjectReflection,
) -> ReflectionExeGeneration {
    let project_name = project_name.into();
    let plan = WrapperGenerationPlan {
        plan_id: format!("reflection-exe:{project_name}"),
        input: ProjectReflectionInput {
            project_name: project_name.clone(),
            reflection,
        },
        output_kind: WrapperOutputKind::IntrospectionExe,
        callable_selection: CallableSelectionPlan::PublicProceduralFunctions,
        conversion_lanes: vec![WrapperConversionLane::typed_scalar_first_tier()],
        diagnostics_policy: WrapperDiagnosticsPolicy {
            lane: "stderr-text".to_string(),
            include_descriptor_identity: true,
            fail_on_unsupported_callable: true,
        },
        argument_parser: Some(ArgumentParserPlan {
            parser_kind: ArgumentParserKind::PositionalCli,
            accepts_named_arguments: false,
            emits_host_context: true,
        }),
    };
    let rust_source = format!(
        r#"// generated reflection executable wrapper for {project_name}
// commands: list | describe Module.Proc | call Module.Proc [typed args...]
fn main() {{
    // generated glue loads an OxBundle through VbaHost, prints descriptor-driven
    // reflection, parses typed positional arguments, and invokes callable IDs via
    // PreparedVbaProject::invoke_callable_typed.
}}
"#
    );
    ReflectionExeGeneration { plan, rust_source }
}

impl ReflectionExeWrapper {
    pub fn from_bundle_bytes(bytes: &[u8]) -> Result<Self, HostDiagnostic> {
        let host = VbaHost::default();
        let loaded = host.load_project(ProjectSource::BundleBytes(bytes.to_vec()))?;
        let reflection = loaded.reflection().clone();
        let prepared = loaded.prepare()?;
        Ok(Self {
            reflection,
            prepared,
        })
    }

    pub fn run(&mut self, args: &[&str]) -> ReflectionExeOutput {
        match self.try_run(args) {
            Ok(stdout) => ReflectionExeOutput {
                status: 0,
                stdout,
                stderr: String::new(),
            },
            Err(err) => ReflectionExeOutput {
                status: 1,
                stdout: String::new(),
                stderr: format!("{}: {}", err.code, err.message),
            },
        }
    }

    fn try_run(&mut self, args: &[&str]) -> Result<String, HostDiagnostic> {
        match args {
            ["list"] => Ok(self.list()),
            ["describe", target] => self.describe(target),
            ["call", target, values @ ..] => self.call(target, values),
            _ => Err(cli_error(
                "WRAPPER-CLI-USAGE",
                "usage: list | describe Module.Proc | call Module.Proc [args...]",
            )),
        }
    }

    fn list(&self) -> String {
        let mut rows = self
            .reflection
            .procedures
            .iter()
            .map(|procedure| format!("{}.{}", procedure.module_name, procedure.procedure_name))
            .collect::<Vec<_>>();
        rows.sort();
        rows.join("\n")
    }

    fn describe(&self, target: &str) -> Result<String, HostDiagnostic> {
        let procedure = self.find_target(target)?;
        let params = procedure
            .signature
            .parameters
            .iter()
            .map(|param| {
                format!(
                    "{}:{}",
                    param.name.as_deref().unwrap_or("_"),
                    param
                        .value_type
                        .as_ref()
                        .map(|ty| vba_type_name(&ty.normalized))
                        .unwrap_or("Unknown")
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let return_type = procedure
            .signature
            .return_type
            .as_ref()
            .map(|ty| vba_type_name(&ty.normalized))
            .unwrap_or("Void");
        Ok(format!(
            "callable={} module={} procedure={} params=[{}] return={}",
            procedure.callable_id,
            procedure.module_name,
            procedure.procedure_name,
            params,
            return_type
        ))
    }

    fn call(&mut self, target: &str, raw_args: &[&str]) -> Result<String, HostDiagnostic> {
        let procedure = self.find_target(target)?.clone();
        if raw_args.len() != procedure.signature.parameters.len() {
            return Err(cli_error(
                "WRAPPER-CLI-ARITY",
                format!(
                    "{} expects {} arguments, got {}",
                    target,
                    procedure.signature.parameters.len(),
                    raw_args.len()
                ),
            ));
        }
        let mut typed_args = Vec::with_capacity(raw_args.len());
        for (idx, (raw, param)) in raw_args
            .iter()
            .zip(procedure.signature.parameters.iter())
            .enumerate()
        {
            let Some(value_type) = param.value_type.as_ref().map(|ty| &ty.normalized) else {
                return Err(cli_error(
                    "WRAPPER-CLI-UNSUPPORTED-TYPE",
                    format!("argument {idx} has no supported type descriptor"),
                ));
            };
            typed_args.push(parse_typed_arg(raw, value_type, idx)?);
        }
        let result = self.prepared.invoke_callable_typed(
            &procedure.callable_id,
            HostCallContext::default(),
            &typed_args,
        )?;
        Ok(format_typed_value(&result.value))
    }

    fn find_target(&self, target: &str) -> Result<&ProcedureDescriptor, HostDiagnostic> {
        let Some((module, procedure)) = target.split_once('.') else {
            return Err(cli_error(
                "WRAPPER-CLI-TARGET",
                "target must be Module.Procedure",
            ));
        };
        self.reflection
            .procedures
            .iter()
            .find(|candidate| {
                candidate.module_name.eq_ignore_ascii_case(module)
                    && candidate.procedure_name.eq_ignore_ascii_case(procedure)
            })
            .ok_or_else(|| {
                cli_error(
                    "WRAPPER-CLI-NOT-FOUND",
                    format!("unknown procedure {target}"),
                )
            })
    }
}

fn parse_typed_arg(raw: &str, ty: &VbaType, idx: usize) -> Result<TypedValue, HostDiagnostic> {
    match ty {
        VbaType::Long => raw.parse::<i32>().map(TypedValue::Long).map_err(|err| {
            cli_error(
                "WRAPPER-CLI-PARSE",
                format!("argument {idx} Long parse failed: {err}"),
            )
        }),
        VbaType::Integer => raw.parse::<i16>().map(TypedValue::Integer).map_err(|err| {
            cli_error(
                "WRAPPER-CLI-PARSE",
                format!("argument {idx} Integer parse failed: {err}"),
            )
        }),
        VbaType::LongLong => raw.parse::<i64>().map(TypedValue::LongLong).map_err(|err| {
            cli_error(
                "WRAPPER-CLI-PARSE",
                format!("argument {idx} LongLong parse failed: {err}"),
            )
        }),
        VbaType::Single => raw.parse::<f32>().map(TypedValue::Single).map_err(|err| {
            cli_error(
                "WRAPPER-CLI-PARSE",
                format!("argument {idx} Single parse failed: {err}"),
            )
        }),
        VbaType::Double => raw.parse::<f64>().map(TypedValue::Double).map_err(|err| {
            cli_error(
                "WRAPPER-CLI-PARSE",
                format!("argument {idx} Double parse failed: {err}"),
            )
        }),
        VbaType::Boolean => raw.parse::<bool>().map(TypedValue::Boolean).map_err(|err| {
            cli_error(
                "WRAPPER-CLI-PARSE",
                format!("argument {idx} Boolean parse failed: {err}"),
            )
        }),
        VbaType::String => Ok(TypedValue::String(raw.to_string())),
        other => Err(cli_error(
            "WRAPPER-CLI-UNSUPPORTED-TYPE",
            format!("argument {idx} type {other:?} is not supported by generated CLI parser"),
        )),
    }
}

fn format_typed_value(value: &TypedValue) -> String {
    match value {
        TypedValue::Empty => String::new(),
        TypedValue::Integer(value) => value.to_string(),
        TypedValue::Long(value) => value.to_string(),
        TypedValue::LongLong(value) => value.to_string(),
        TypedValue::Single(value) => value.to_string(),
        TypedValue::Double(value) => value.to_string(),
        TypedValue::Boolean(value) => value.to_string(),
        TypedValue::String(value) => value.clone(),
        TypedValue::Variant(value) => format!("{value:?}"),
    }
}

fn vba_type_name(ty: &VbaType) -> &'static str {
    match ty {
        VbaType::Variant => "Variant",
        VbaType::Boolean => "Boolean",
        VbaType::Byte => "Byte",
        VbaType::Integer => "Integer",
        VbaType::Long => "Long",
        VbaType::LongLong => "LongLong",
        VbaType::LongPtr => "LongPtr",
        VbaType::Single => "Single",
        VbaType::Double => "Double",
        VbaType::Currency => "Currency",
        VbaType::Date => "Date",
        VbaType::String => "String",
        VbaType::Object => "Object",
        VbaType::Array => "Array",
        VbaType::UserDefined(_) => "UserDefined",
        VbaType::Any => "Any",
        VbaType::Unknown => "Unknown",
    }
}

fn cli_error(code: impl Into<String>, message: impl Into<String>) -> HostDiagnostic {
    HostDiagnostic {
        phase: HostDiagnosticPhase::ValidateCall,
        code: code.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_compiler::{
        ModuleKind, OxBundle, ProjectKind, ProjectManifest, compile_project,
        module_unit_from_source,
    };

    fn bundle_bytes(source: &str) -> Vec<u8> {
        let manifest = ProjectManifest {
            project_name: "CliWrap".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_unit_from_source("Main", ModuleKind::Procedural, source).unwrap()],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        };
        let compiled = compile_project(&manifest).unwrap();
        OxBundle::from_compiled_project(&compiled, &manifest.project_name)
            .serialize_to_bytes()
            .unwrap()
    }

    #[test]
    fn generated_exe_lists_describes_and_calls_from_descriptors() {
        let bytes = bundle_bytes(
            "Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function",
        );
        let mut wrapper = ReflectionExeWrapper::from_bundle_bytes(&bytes).unwrap();
        assert_eq!(wrapper.run(&["list"]).stdout, "Main.add");
        let describe = wrapper.run(&["describe", "Main.Add"]);
        assert!(describe.stdout.contains("params=[a:Long,b:Long]"));
        assert!(describe.stdout.contains("return=Long"));
        let call = wrapper.run(&["call", "Main.Add", "2", "5"]);
        assert_eq!(call.status, 0);
        assert_eq!(call.stdout, "7");
    }

    #[test]
    fn generated_exe_reports_cli_negative_cases() {
        let bytes = bundle_bytes(
            "Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function\nPublic Function Echo(v As Variant) As Variant\nEcho = v\nEnd Function",
        );
        let mut wrapper = ReflectionExeWrapper::from_bundle_bytes(&bytes).unwrap();
        assert!(
            wrapper
                .run(&["describe", "Main.Missing"])
                .stderr
                .contains("WRAPPER-CLI-NOT-FOUND")
        );
        assert!(
            wrapper
                .run(&["call", "Main.Add", "1"])
                .stderr
                .contains("WRAPPER-CLI-ARITY")
        );
        assert!(
            wrapper
                .run(&["call", "Main.Echo", "1"])
                .stderr
                .contains("WRAPPER-CLI-UNSUPPORTED-TYPE")
        );
        assert!(
            wrapper
                .run(&["call", "Main.Add", "nope", "1"])
                .stderr
                .contains("WRAPPER-CLI-PARSE")
        );
    }

    #[test]
    fn generated_exe_propagates_runtime_diagnostics() {
        let bytes =
            bundle_bytes("Public Function Fails() As Long\nErr.Raise 5\nFails = 1\nEnd Function");
        let mut wrapper = ReflectionExeWrapper::from_bundle_bytes(&bytes).unwrap();
        let result = wrapper.run(&["call", "Main.Fails"]);
        assert_eq!(result.status, 1);
        assert!(
            result.stderr.contains("HOST-PHASE-DIAGNOSTIC") || result.stderr.contains("runtime")
        );
    }

    #[test]
    fn generation_plan_and_source_document_commands() {
        let bytes = bundle_bytes("Public Function Add(a As Long) As Long\nAdd = a\nEnd Function");
        let wrapper = ReflectionExeWrapper::from_bundle_bytes(&bytes).unwrap();
        let generated = generate_reflection_exe_wrapper("CliWrap", wrapper.reflection.clone());
        assert_eq!(
            generated.plan.output_kind,
            WrapperOutputKind::IntrospectionExe
        );
        assert!(
            generated
                .rust_source
                .contains("list | describe Module.Proc | call Module.Proc")
        );
        assert_eq!(
            generated.plan.conversion_lanes[0].lane_id,
            "TypedScalarFirstTier"
        );
    }
}
