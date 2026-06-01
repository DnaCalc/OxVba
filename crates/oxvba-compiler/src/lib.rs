//! oxvba-compiler: semantic analysis and bytecode emission scaffolding.

pub mod bundle;
pub mod bytecode;
pub mod descriptor_identity;
pub mod emit;
pub mod frontend_assignment_semantics;
pub mod frontend_class_semantics;
pub mod frontend_diagnostics;
pub mod frontend_diff;
pub mod frontend_event_semantics;
pub mod frontend_external_references;
pub mod frontend_hir;
pub mod frontend_lowering_contract;
pub mod frontend_member_dispatch;
pub mod frontend_operator_normalization;
pub mod frontend_project_symbols;
pub mod frontend_query;
pub mod frontend_retirement_inventory;
pub mod frontend_route_policy;
pub mod frontend_semantic_model;
pub mod frontend_structural_intrinsics;
pub mod frontend_symbols;
pub mod frontend_type_hooks;
pub mod lsp_support;
pub mod optimize;
pub mod project;
pub mod resolve;
pub mod syntax_bridge;
pub mod typecheck;

use thiserror::Error;

pub use bundle::{
    BundleCallableDescriptor, BundleComClassDescriptor, BundleComEventDescriptor,
    BundleComInterfaceDescriptor, BundleComMemberDescriptor, BundleComParamDescriptor,
    BundleCompileContext, BundleConditionalConstantFact, BundleDefaultTypeFamilyFact,
    BundleDescriptorInventoryError, BundleHostCapabilityRequirement, BundleModuleSourceMap,
    BundleNativeLibraryFact, BundlePackageDiagnostic, BundlePackageGapClassification,
    BundleProcedureAnnotation, BundleProcedureParameterDescriptor, BundleProcedureSignature,
    BundleProjectContext, BundleProjectModuleFact, BundleProjectReferenceFact,
    BundleReferencedProjectFact, BundleSourceLineMapping, BundleVbaTypeDescriptor,
    ComClassExportEntry, DescriptorInventory, OxBundle,
};
pub use bytecode::{Bytecode, DeclareParamType, Instruction};
pub use descriptor_identity::{
    DescriptorFamily, DescriptorIdentity, VbaTypeRegistryEntry, canonical_descriptor_id,
    descriptor_digest_debug, descriptor_digest_from_fields, descriptor_identity_debug,
    vba_type_registry,
};
pub use emit::{
    ArgumentBindingDescriptor, ArgumentBindingKindDescriptor, ArgumentExpressionKindDescriptor,
    ArgumentSourceKindDescriptor, ArgumentWritebackDescriptor, ArrayBoundDescriptor,
    ArrayShapeDescriptor, ArrayStorageKind, CallDiagnosticKindDescriptor,
    CallDiagnosticOwnerDescriptor, CallDiagnosticPolicyDescriptor, CallInvocationSyntaxDescriptor,
    CallReturnDescriptor, CallSiteDescriptor, CallTargetKindDescriptor, CarrierLayoutDescriptor,
    CarrierLayoutKind, CoercionDescriptor, CoercionKindDescriptor, CoercionStaticStatusDescriptor,
    DefaultMemberPolicyDescriptor, EvaluationOrderDescriptor, ExpressionClassificationDescriptor,
    ExpressionSemanticsDescriptor, ExpressionSourceContextDescriptor,
    ImplicitCurrentObjectDescriptor, MemberDispatchKindDescriptor, NameBindingDescriptor,
    NameBindingKindDescriptor, NameBindingPrecedenceDescriptor, ObjectActivationDescriptor,
    ObjectDefaultMemberDescriptor, ObjectDescriptorSupport, ObjectEventBindingDescriptor,
    ObjectInstanceDescriptor, ObjectMemberBindingDescriptor, ObjectMemberKindDescriptor,
    ObjectTypeDescriptor, ObjectTypeDescriptorKind, OperatorCompareModeDescriptor,
    OperatorFamilyDescriptor, OperatorSemanticsDescriptor, OptionalDefaultValue,
    OptionalMissingStatePolicy, OptionalParameterDescriptor, ParamArrayBindingDescriptor,
    ParamArrayDescriptor, ParameterDescriptor, ParameterPassingMode, ParameterRole,
    ProcedureKindDescriptor, ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind,
    ProcedureRuntimeSlotMetadata, ProcedureSignatureDescriptor, ResolvedParameterMechanism,
    RuntimeCarrierKind, RuntimeFailurePolicyDescriptor, SlotInitialState, SlotRole,
    SlotTypeDescriptor, SourceParameterMechanism, UdtCleanupDescriptor, UdtCopySemanticsDescriptor,
    UdtFieldAliasDescriptor, UdtFieldDescriptor, UdtInstanceDescriptor, UdtStorageKind,
    UdtTypeDescriptor, ValueStateDescriptor, ValueStateKind, ValueStateSource,
    VbaOperatorDescriptor, VbaTypeId, bound_type_to_declare_param_type,
};
pub use project::{
    CallableCapability, CallingShape, CompiledProject, CompilerLineMapping,
    CompilerModuleSourceMap, CompilerSourceLineKind, CompilerSourceMap, ExportKind,
    HostProcedureExport, InvocationLane, ModuleAttributes, ModuleDescriptor, ModuleKind,
    ModuleUnit, ModuleVisibility, PassingMode, ProcedureAnnotation, ProcedureDescriptor,
    ProcedureKind, ProcedureParameterDescriptor, ProcedureSignature, ProcedureVisibility,
    ProjectComWithEventsRoute, ProjectCompileError, ProjectDynamicMemberKind,
    ProjectDynamicMemberRoute, ProjectDynamicObjectRoute, ProjectDynamicParamRoute,
    ProjectEventDispatchBinding, ProjectIdentity, ProjectKind, ProjectManifest, ProjectReference,
    ProjectReflection, ReferenceKind, ReferencedProjectManifest, RuntimeProcedureRoute, SourceSpan,
    UnsupportedReason, VbaType, VbaTypeDescriptor, compile_project, module_unit_from_source,
    reflect_project,
};

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("empty source")]
    EmptySource,
    #[error("resolve error: {0}")]
    ResolveError(String),
    #[error("type error: {0}")]
    TypeError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompileOptions {
    pub frontend_v2: bool,
}

pub fn compile(source: &str) -> Result<Bytecode, CompileError> {
    compile_with_runtime_metadata(source).map(|(bytecode, _)| bytecode)
}

pub fn compile_with_options(
    source: &str,
    options: CompileOptions,
) -> Result<Bytecode, CompileError> {
    if options.frontend_v2 {
        return syntax_bridge::compile_source_via_syntax_bridge(source)
            .map_err(|err| CompileError::ResolveError(format!("frontend_v2 bridge error: {err}")));
    }
    compile(source)
}

pub fn compile_with_runtime_metadata(
    source: &str,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    CompileError,
> {
    compile_with_runtime_metadata_object_locals(source, &std::collections::BTreeMap::new())
}

/// Compile a single source snippet into a complete, strict executable semantic
/// package held in memory.
///
/// Wraps the snippet in a one-module project and runs the full project compile,
/// so the resulting `OxBundle` carries the manifest, project context, descriptor
/// inventory, and export inventory required by the strict package support gate —
/// unlike the lightweight `compile_with_runtime_metadata` + `OxBundle::new` path,
/// which produces an incomplete (non-strict) package. This is the in-memory
/// counterpart of a serialized bundle: same strict completeness, no serialization.
pub fn compile_source_to_bundle(source: &str) -> Result<OxBundle, ProjectCompileError> {
    let manifest = ProjectManifest {
        project_name: "InMemory".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![module_unit_from_source(
            "Main",
            ModuleKind::Procedural,
            source,
        )?],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };
    let compiled = compile_project(&manifest)?;
    Ok(OxBundle::from_compiled_project(
        &compiled,
        &manifest.project_name,
    ))
}

pub(crate) fn compile_with_runtime_metadata_object_locals(
    source: &str,
    forced_object_locals_by_proc: &std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    >,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    CompileError,
> {
    compile_with_runtime_metadata_object_locals_class(source, forced_object_locals_by_proc, false)
}

pub(crate) fn compile_with_runtime_metadata_object_locals_class(
    source: &str,
    forced_object_locals_by_proc: &std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    >,
    has_class_modules: bool,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    CompileError,
> {
    if source.trim().is_empty() {
        return Err(CompileError::EmptySource);
    }

    let mut bound = resolve::resolve_symbols(source);
    bound.is_class_module = has_class_modules;
    if !bound.resolution_diagnostics.is_empty() {
        return Err(CompileError::ResolveError(
            bound.resolution_diagnostics.join("; "),
        ));
    }
    let checked = typecheck::check_types(bound).map_err(CompileError::TypeError)?;
    let optimized = if std::env::var("OXVBA_DISABLE_OPT").ok().as_deref() == Some("1") {
        checked
    } else {
        optimize::optimize_module(checked)
    };
    let mut optimized = optimized;
    for proc in &mut optimized.procedures {
        let Some(vars) = forced_object_locals_by_proc.get(&proc.name) else {
            continue;
        };
        for var in vars {
            proc.declaration_types
                .insert(var.clone(), resolve::BoundType::Object);
        }
    }
    Ok(emit::emit_bytecode_with_runtime_metadata(&optimized))
}

#[cfg(test)]
mod tests {
    use super::{
        ArgumentBindingKindDescriptor, ArgumentSourceKindDescriptor, ArrayStorageKind, Bytecode,
        CallDiagnosticKindDescriptor, CallDiagnosticOwnerDescriptor,
        CallInvocationSyntaxDescriptor, CallTargetKindDescriptor, CoercionKindDescriptor,
        DefaultMemberPolicyDescriptor, EvaluationOrderDescriptor,
        ExpressionClassificationDescriptor, Instruction, NameBindingKindDescriptor,
        ObjectMemberKindDescriptor, OperatorCompareModeDescriptor, OperatorFamilyDescriptor,
        OptionalDefaultValue, OptionalMissingStatePolicy, OptionalParameterDescriptor,
        ParameterPassingMode, ParameterRole, ProcedureKindDescriptor, ProcedureRuntimeSlotKind,
        ResolvedParameterMechanism, RuntimeCarrierKind, SlotInitialState, SlotRole,
        SourceParameterMechanism, UdtCopySemanticsDescriptor, ValueStateKind,
        VbaOperatorDescriptor, VbaTypeId, compile, compile_with_runtime_metadata,
    };
    use crate::bytecode::{
        RuntimeAssignmentIntent, RuntimeAssignmentTargetKind, StringCompareMode,
    };
    use crate::{resolve, typecheck};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum DispatchInvokeMemberSelector {
        I32(i32),
        String(String),
    }

    fn dispatch_invoke_member_before(
        instructions: &[Instruction],
        invoke_index: usize,
        member_slot: usize,
    ) -> Option<DispatchInvokeMemberSelector> {
        for instruction in instructions[..invoke_index].iter().rev() {
            match instruction {
                Instruction::LoadConstI32 { slot, value } if *slot == member_slot => {
                    return Some(DispatchInvokeMemberSelector::I32(*value));
                }
                Instruction::LoadConstString { slot, value } if *slot == member_slot => {
                    return Some(DispatchInvokeMemberSelector::String(value.clone()));
                }
                _ => {}
            }
        }
        None
    }

    fn dispatch_invoke_members(out: &Bytecode) -> Vec<DispatchInvokeMemberSelector> {
        out.instructions
            .iter()
            .enumerate()
            .filter_map(|(index, instruction)| match instruction {
                Instruction::IntrinsicDispatchInvokeHost { member, .. } => {
                    dispatch_invoke_member_before(&out.instructions, index, *member)
                }
                _ => None,
            })
            .collect()
    }

    fn assert_has_dispatch_member(out: &Bytecode, expected_token: i32, expected_name: &str) {
        let members = dispatch_invoke_members(out);
        assert!(
            members.iter().any(|member| {
                matches!(member, DispatchInvokeMemberSelector::I32(value) if *value == expected_token)
                    || matches!(member, DispatchInvokeMemberSelector::String(value) if value.eq_ignore_ascii_case(expected_name))
            }),
            "expected dispatch selector token {expected_token} or name {expected_name:?}, got: {members:?}"
        );
    }

    fn dispatchinvoke_member_literal_from_source(source: &str) -> Option<&str> {
        for line in source.lines() {
            if !line.contains("DispatchInvoke(") {
                continue;
            }
            let mut literals = Vec::new();
            let mut start = None;
            for (index, ch) in line.char_indices() {
                match (ch, start) {
                    ('"', None) => start = Some(index + 1),
                    ('"', Some(begin)) => {
                        literals.push(&line[begin..index]);
                        start = None;
                    }
                    _ => {}
                }
            }
            if literals.len() >= 2 {
                return Some(literals[1]);
            }
        }
        None
    }

    fn assert_dispatchinvoke_source_member(out: &Bytecode, source: &str, expected_token: i32) {
        let expected_name = dispatchinvoke_member_literal_from_source(source)
            .expect("DispatchInvoke source should contain an explicit member literal");
        assert_has_dispatch_member(out, expected_token, expected_name);
    }

    #[test]
    fn compile_simple_module() {
        let out = compile("Sub Main()\nEnd Sub").expect("compile should succeed");
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn compile_options_default_keeps_legacy_path() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let legacy = compile(source).expect("legacy compile should succeed");
        let defaulted = super::compile_with_options(source, super::CompileOptions::default())
            .expect("default compile");
        assert_eq!(
            format!("{:?}", legacy.instructions),
            format!("{:?}", defaulted.instructions),
            "default compile options must not change bytecode route"
        );
    }

    #[test]
    fn compile_options_frontend_v2_is_opt_in_bridge_route() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let out = super::compile_with_options(source, super::CompileOptions { frontend_v2: true })
            .expect("frontend_v2 bridge compile should succeed");
        assert!(!out.instructions.is_empty());
    }

    #[test]
    fn compile_options_frontend_v2_rejects_syntax_before_legacy_lowering() {
        let err = super::compile_with_options(
            "Sub Main()\n    x = (1 + 2\nEnd Sub\n",
            super::CompileOptions { frontend_v2: true },
        )
        .expect_err("frontend_v2 bridge should reject syntax parse errors first");
        assert!(
            err.to_string().contains("frontend_v2 bridge error"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn reject_empty_input() {
        assert!(compile(" \n ").is_err());
    }

    #[test]
    fn resolve_and_typecheck_dynamic_byte_array_function_return_call() {
        let source = "Private Function MakeBuf() As Byte()\nDim buf() As Byte\nReDim buf(2)\nbuf(0) = 90\nbuf(1) = 91\nbuf(2) = 92\nMakeBuf = buf\nEnd Function\n\nSub Main()\nDim result() As Byte\nresult = MakeBuf()\nEnd Sub";
        let bound = resolve::resolve_symbols(source);
        let procedure_names = bound
            .procedures
            .iter()
            .map(|procedure| procedure.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            procedure_names,
            vec!["makebuf".to_string(), "main".to_string()]
        );
        typecheck::check_types(bound)
            .expect("same-module dynamic byte-array function call should typecheck");
    }

    #[test]
    fn compile_dynamic_byte_array_function_return_emits_all_index_reads() {
        let source = "Private Function MakeBuf() As Byte()\nDim buf() As Byte\nReDim buf(2)\nbuf(0) = 90\nbuf(1) = 91\nbuf(2) = 92\nMakeBuf = buf\nEnd Function\n\nSub Main()\nDim result() As Byte\nDim x0 As Long\nDim x1 As Long\nDim x2 As Long\nresult = MakeBuf()\nx0 = result(0)\nx1 = result(1)\nx2 = result(2)\nEnd Sub";
        let (bytecode, metadata) = compile_with_runtime_metadata(source)
            .expect("dynamic byte-array return assignment should compile");
        let get_count = bytecode
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::IntrinsicArrayGet { .. }))
            .count();
        assert_eq!(
            get_count, 3,
            "expected one array read per indexed assignment"
        );
        let main = metadata.get("main").expect("main metadata should exist");
        let main_slot_names = main
            .slots
            .iter()
            .map(|slot| slot.name.clone())
            .collect::<Vec<_>>();
        assert!(
            main_slot_names.contains(&"result".to_string())
                && main_slot_names.contains(&"x0".to_string())
                && main_slot_names.contains(&"x1".to_string())
                && main_slot_names.contains(&"x2".to_string()),
            "main slot metadata should preserve all locals, got {main_slot_names:?}"
        );
    }

    #[test]
    fn compile_dim_assign_and_add() {
        let source = "Sub Main()\nDim x\nx = 10\nx = x + 5\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::AddConstI32 { slot: 0, value: 5 },
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn compile_dim_assign_and_subtract() {
        let source = "Sub Main()\nDim x\nx = 10\nx = x - 3\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::SubConstI32 { slot: 0, value: 3 },
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn compile_line_continuation_expression() {
        let source = "Sub Main()\nDim x\nx = 1\nx = x + _\n2\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::LoadConstI32 { slot: 0, value: 1 },
                Instruction::AddConstI32 { slot: 0, value: 2 },
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn compile_with_runtime_metadata_projects_named_slot_kinds() {
        let source = "Function AddOne(ByVal value As Long) As Long\n\
                      Dim localValue As Long\n\
                      localValue = value + 1\n\
                      AddOne = localValue\n\
                      End Function";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let add_one = metadata
            .get("addone")
            .expect("function metadata should be present");
        assert!(add_one.slots.iter().any(|slot| {
            slot.name.eq_ignore_ascii_case("value")
                && slot.kind == ProcedureRuntimeSlotKind::Parameter
        }));
        assert!(add_one.slots.iter().any(|slot| {
            slot.name.eq_ignore_ascii_case("localvalue")
                && slot.kind == ProcedureRuntimeSlotKind::Local
        }));
        assert!(add_one.slots.iter().any(|slot| {
            slot.name.eq_ignore_ascii_case("addone")
                && slot.kind == ProcedureRuntimeSlotKind::ReturnValue
        }));
    }

    #[test]
    fn procedure_runtime_metadata_projects_first_slot_type_descriptor_view() {
        let source = "Function Combine(ByVal amount As Double, suffix As String) As Variant\n\
                      Dim localValue As Long\n\
                      Dim values(0 To 1) As Long\n\
                      localValue = 7\n\
                      values(0) = localValue\n\
                      Combine = CStr(amount + localValue) & suffix\n\
                      End Function";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let combine = metadata
            .get("combine")
            .expect("function metadata should be present");
        let descriptors = combine.slot_type_descriptors();

        let amount = descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("amount"))
            })
            .expect("amount parameter descriptor should be present");
        assert_eq!(amount.role, SlotRole::Parameter);
        assert_eq!(amount.declared_type, VbaTypeId::Double);
        assert_eq!(amount.initial_state, SlotInitialState::CallerProvided);
        assert_eq!(amount.carrier, RuntimeCarrierKind::F64);

        let suffix = descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("suffix"))
            })
            .expect("suffix parameter descriptor should be present");
        assert_eq!(suffix.role, SlotRole::Parameter);
        assert_eq!(suffix.declared_type, VbaTypeId::String);
        assert_eq!(suffix.initial_state, SlotInitialState::CallerProvided);
        assert_eq!(suffix.carrier, RuntimeCarrierKind::BStr);

        let return_value = descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("Combine"))
            })
            .expect("function return descriptor should be present");
        assert_eq!(return_value.role, SlotRole::ReturnValue);
        assert_eq!(return_value.declared_type, VbaTypeId::Variant);
        assert_eq!(return_value.initial_state, SlotInitialState::Empty);
        assert_eq!(return_value.carrier, RuntimeCarrierKind::Variant);

        let local = descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("localValue"))
            })
            .expect("local descriptor should be present");
        assert_eq!(local.role, SlotRole::Local);
        assert_eq!(local.declared_type, VbaTypeId::Long);
        assert_eq!(local.initial_state, SlotInitialState::ScalarZero);
        assert_eq!(local.carrier, RuntimeCarrierKind::I32);

        let generated = descriptors
            .iter()
            .find(|descriptor| {
                descriptor.role == SlotRole::CompilerGenerated
                    && descriptor
                        .name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case("values_0"))
            })
            .expect("fixed-array element descriptor should be compiler-generated");
        assert_eq!(generated.declared_type, VbaTypeId::Long);
        assert_eq!(generated.initial_state, SlotInitialState::ScalarZero);
        assert_eq!(generated.carrier, RuntimeCarrierKind::I32);

        let temporary = descriptors
            .iter()
            .find(|descriptor| descriptor.role == SlotRole::Temporary)
            .expect("expression temporary descriptor should be present");
        assert_eq!(
            temporary.declared_type,
            VbaTypeId::Unknown,
            "temporary declared types are intentionally unknown until expression typing is preserved"
        );
        assert_eq!(temporary.initial_state, SlotInitialState::CompilerDefined);
        assert_eq!(temporary.carrier, RuntimeCarrierKind::Unknown);
    }

    #[test]
    fn procedure_runtime_metadata_carries_typed_carriers_and_value_states() {
        let source = "Type Point\n\
                      X As Long\n\
                      Label As String\n\
                      End Type\n\
                      Sub Main()\n\
                      End Sub\n\
                      Sub Probe(Optional ByVal marker As Variant, Optional ByVal amount As Long = 7)\n\
                      Dim l As Long\n\
                      Dim d As Double\n\
                      Dim b As Boolean\n\
                      Dim s As String\n\
                      Dim v As Variant\n\
                      Dim o As Object\n\
                      Dim p As Point\n\
                      Dim dec As Decimal\n\
                      Debug.Print Empty\n\
                      Debug.Print Null\n\
                      Debug.Print CVErr(7)\n\
                      Debug.Print vbNullString\n\
                      dec = CDec(1)\n\
                      Debug.Print dec\n\
                      End Sub";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let probe = metadata.get("probe").unwrap_or_else(|| {
            panic!(
                "Probe metadata keys={:?}",
                metadata.keys().cloned().collect::<Vec<_>>()
            )
        });

        for carrier in [
            RuntimeCarrierKind::I32,
            RuntimeCarrierKind::F64,
            RuntimeCarrierKind::Boolean,
            RuntimeCarrierKind::BStr,
            RuntimeCarrierKind::Variant,
            RuntimeCarrierKind::ObjectRef,
            RuntimeCarrierKind::Decimal96VariantSubtype,
        ] {
            assert!(
                probe
                    .carrier_layouts
                    .iter()
                    .any(|layout| layout.carrier == carrier),
                "missing carrier layout {carrier:?}"
            );
        }
        assert!(
            probe
                .carrier_layouts
                .iter()
                .any(|layout| matches!(layout.carrier, RuntimeCarrierKind::UdtFields { .. })),
            "UDT aggregate carrier layout should be present"
        );
        assert!(
            probe.carrier_layouts.iter().any(|layout| layout.carrier
                == RuntimeCarrierKind::Decimal96VariantSubtype
                && layout
                    .notes
                    .iter()
                    .any(|note| note == "declared-decimal-extension=variant-subtype")),
            "declared Decimal should be classified as the Decimal96 Variant subtype extension"
        );

        for state in [
            ValueStateKind::Empty,
            ValueStateKind::Null,
            ValueStateKind::Error,
            ValueStateKind::MissingArgument,
            ValueStateKind::OmittedDefault,
            ValueStateKind::Nothing,
            ValueStateKind::VbNullString,
            ValueStateKind::DecimalVariantSubtype,
        ] {
            assert!(
                probe
                    .value_states
                    .iter()
                    .any(|descriptor| descriptor.state == state),
                "missing value-state descriptor {state:?}"
            );
        }
    }

    #[test]
    fn procedure_runtime_metadata_carries_array_udt_enum_aggregate_facts() {
        let source = "Option Base 1\n\
                      Enum Mode\n\
                      Fast = 3\n\
                      Safe\n\
                      End Enum\n\
                      Type Inner\n\
                      X As Long\n\
                      End Type\n\
                      Type Record\n\
                      Name As String * 5\n\
                      Scores(1 To 2) As Long\n\
                      Inner As Inner\n\
                      End Type\n\
                      Sub Main()\n\
                      Dim fixed(3) As Long\n\
                      Dim explicit(0 To 2) As Long\n\
                      Dim matrix(1 To 2, 3 To 4) As Long\n\
                      Dim dyn() As Long\n\
                      Dim r As Record\n\
                      Dim modeValue As Long\n\
                      modeValue = Safe\n\
                      ReDim dyn(2 To 4)\n\
                      End Sub";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");

        let fixed = main
            .array_shapes
            .iter()
            .find(|descriptor| descriptor.name.eq_ignore_ascii_case("fixed"))
            .expect("fixed array shape should be present");
        assert_eq!(fixed.storage, ArrayStorageKind::StaticFixed);
        assert_eq!(fixed.option_base, 1);
        assert_eq!(fixed.bounds[0].lower_bound, 1);
        assert_eq!(fixed.bounds[0].upper_bound, 3);

        let matrix = main
            .array_shapes
            .iter()
            .find(|descriptor| descriptor.name.eq_ignore_ascii_case("matrix"))
            .expect("multi-rank array shape should be present");
        assert_eq!(matrix.rank, 2);
        assert_eq!(matrix.bounds[0].lower_bound, 1);
        assert_eq!(matrix.bounds[0].upper_bound, 2);
        assert_eq!(matrix.bounds[1].lower_bound, 3);
        assert_eq!(matrix.bounds[1].upper_bound, 4);

        let dyn_array = main
            .array_shapes
            .iter()
            .find(|descriptor| descriptor.name.eq_ignore_ascii_case("dyn"))
            .expect("dynamic array shape should be present");
        assert_eq!(dyn_array.storage, ArrayStorageKind::Dynamic);
        assert!(dyn_array.bounds.is_empty());

        let record = main
            .udt_types
            .iter()
            .find(|descriptor| descriptor.type_name.eq_ignore_ascii_case("Record"))
            .expect("Record UDT descriptor should be present");
        assert_eq!(
            record.copy_semantics,
            UdtCopySemanticsDescriptor::FieldWiseCopy
        );
        assert!(record.cleanup.owns_bstr);
        let name_field = record
            .fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case("Name"))
            .expect("fixed string field should be present");
        assert_eq!(name_field.fixed_string_len, Some(5));
        let scores_field = record
            .fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case("Scores"))
            .expect("fixed array field should be present");
        assert_eq!(scores_field.array_bounds[0].lower_bound, 1);
        assert_eq!(scores_field.array_bounds[0].upper_bound, 2);
        let inner_field = record
            .fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case("Inner"))
            .expect("nested UDT field should be present");
        assert_eq!(inner_field.nested_udt_name.as_deref(), Some("inner"));

        assert!(main.name_bindings.iter().any(|binding| {
            binding.binding_id == "NAME-BINDING-ENUM-TYPE"
                && binding.target.as_deref() == Some("enum:mode")
        }));
        assert!(main.name_bindings.iter().any(|binding| {
            binding.binding_id == "NAME-BINDING-ENUM-MEMBER"
                && binding.target.as_deref() == Some("enum:mode:safe=4")
        }));
    }

    #[test]
    fn procedure_runtime_metadata_projects_first_signature_descriptor_view() {
        let source = "Sub Main()\n\
                      End Sub\n\
                      Sub Capture(target As Long, ByRef alias As Long, Optional ByVal value As Long = 7)\n\
                      End Sub\n\
                      Sub Maybe(Optional ByVal arg As Variant)\n\
                      End Sub\n\
                      Sub Pack(ByRef target As Variant, ParamArray items() As Variant)\n\
                      End Sub\n\
                      Function Echo(ByVal text As String) As String\n\
                      Echo = text\n\
                      End Function\n\
                      Property Get Value() As Long\n\
                      Value = 1\n\
                      End Property\n\
                      Property Let Value(ByRef newValue As Long)\n\
                      End Property";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");

        let capture = metadata
            .get("capture")
            .expect("Capture metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(capture.kind, ProcedureKindDescriptor::Sub);
        assert_eq!(capture.return_type, None);
        assert_eq!(capture.parameters.len(), 3);
        assert_eq!(capture.parameters[0].name, "target");
        assert_eq!(capture.parameters[0].role, ParameterRole::Positional);
        assert_eq!(
            capture.parameters[0].passing_mode,
            ParameterPassingMode::ByRef
        );
        assert_eq!(
            capture.parameters[0].source_mechanism,
            SourceParameterMechanism::Omitted
        );
        assert_eq!(
            capture.parameters[0].resolved_mechanism,
            ResolvedParameterMechanism::ByRef
        );
        assert_eq!(capture.parameters[0].declared_type, VbaTypeId::Long);
        assert_eq!(capture.parameters[1].name, "alias");
        assert_eq!(
            capture.parameters[1].source_mechanism,
            SourceParameterMechanism::ExplicitByRef
        );
        assert_eq!(
            capture.parameters[1].passing_mode,
            ParameterPassingMode::ByRef
        );
        assert_eq!(capture.parameters[2].name, "value");
        assert_eq!(capture.parameters[2].role, ParameterRole::Optional);
        assert_eq!(
            capture.parameters[2].source_mechanism,
            SourceParameterMechanism::ExplicitByVal
        );
        assert_eq!(
            capture.parameters[2].passing_mode,
            ParameterPassingMode::ByVal
        );
        assert_eq!(capture.parameters[2].default_value, Some(7));
        assert_eq!(
            capture.parameters[2].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitI32(7),
                missing_state: OptionalMissingStatePolicy::AssignDefaultLocal,
            }
        );

        let maybe = metadata
            .get("maybe")
            .expect("Maybe metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(
            maybe.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::VariantMissingError448,
                missing_state: OptionalMissingStatePolicy::PreserveMissingArgumentState,
            }
        );

        let pack = metadata
            .get("pack")
            .expect("Pack metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(pack.parameters[1].name, "items");
        assert_eq!(pack.parameters[1].role, ParameterRole::ParamArray);
        assert_eq!(pack.parameters[1].passing_mode, ParameterPassingMode::ByVal);
        assert_eq!(pack.parameters[1].declared_type, VbaTypeId::Array);
        assert_eq!(
            pack.parameters[1]
                .param_array_descriptor
                .as_ref()
                .map(|descriptor| (descriptor.element_type, descriptor.array_lower_bound)),
            Some((VbaTypeId::Variant, 0))
        );

        let echo = metadata
            .get("echo")
            .expect("Echo metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(echo.kind, ProcedureKindDescriptor::Function);
        assert_eq!(echo.return_type, Some(VbaTypeId::String));
        assert_eq!(echo.return_slot, metadata["echo"].return_slot);
        assert_eq!(echo.parameters[0].passing_mode, ParameterPassingMode::ByVal);
        assert_eq!(echo.parameters[0].declared_type, VbaTypeId::String);

        let property_get = metadata
            .get("property_get_value")
            .expect("Property Get metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(property_get.kind, ProcedureKindDescriptor::PropertyGet);
        assert_eq!(property_get.return_type, Some(VbaTypeId::Long));
        assert_eq!(property_get.property_group.as_deref(), Some("value"));

        let property_let = metadata
            .get("property_let_value")
            .expect("Property Let metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(property_let.kind, ProcedureKindDescriptor::PropertyLet);
        assert_eq!(property_let.return_type, None);
        assert_eq!(property_let.property_group.as_deref(), Some("value"));
        assert_eq!(
            property_let.parameters[0].role,
            ParameterRole::PropertyValue
        );
        assert_eq!(
            property_let.parameters[0].passing_mode,
            ParameterPassingMode::ByRef,
            "legacy passing_mode still reports parsed source mechanism"
        );
        assert_eq!(
            property_let.parameters[0].source_mechanism,
            SourceParameterMechanism::ExplicitByRef
        );
        assert_eq!(
            property_let.parameters[0].resolved_mechanism,
            ResolvedParameterMechanism::PropertyValueByVal
        );
    }

    #[test]
    fn procedure_runtime_metadata_projects_first_call_site_descriptor_view() {
        let source = "Sub Main()\n\
                      Dim x As Long\n\
                      Dim y As Long\n\
                      Dim observed As Long\n\
                      x = 1\n\
                      Call Touch(x)\n\
                      Call Fill(target := y)\n\
                      observed = Echo(x)\n\
                      Call Capture(y, 5, 7)\n\
                      End Sub\n\
                      Sub Touch(ByRef value As Long)\n\
                      value = value + 1\n\
                      End Sub\n\
                      Sub Fill(ByRef target As Long, Optional ByVal value As Long = 7)\n\
                      target = value\n\
                      End Sub\n\
                      Function Echo(ByVal value As Long) As Long\n\
                      Echo = value\n\
                      End Function\n\
                      Sub Capture(ByRef target As Long, ParamArray items() As Variant)\n\
                      target = UBound(items)\n\
                      End Sub";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");
        assert_eq!(main.call_sites.len(), 4);

        let touch = main
            .call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Touch"))
            .expect("Touch call descriptor should be present");
        assert!(touch.call_site_id.starts_with("callsite:main@pc:"));
        assert_eq!(touch.target_kind, CallTargetKindDescriptor::Procedure);
        assert!(touch.target_entry_pc.is_some());
        assert_eq!(
            touch.default_member_policy,
            DefaultMemberPolicyDescriptor::NotApplicable
        );
        assert_eq!(
            touch.arguments[0].binding_kind,
            ArgumentBindingKindDescriptor::ByRefAlias
        );
        assert!(
            touch.arguments[0]
                .writeback
                .as_ref()
                .is_some_and(|w| w.required)
        );

        let fill = main
            .call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Fill"))
            .expect("Fill call descriptor should be present");
        assert_eq!(fill.arguments[0].source_name.as_deref(), Some("target"));
        assert_eq!(
            fill.arguments[0].source_kind,
            ArgumentSourceKindDescriptor::Named
        );
        assert_eq!(
            fill.arguments[1].source_kind,
            ArgumentSourceKindDescriptor::Omitted
        );
        assert_eq!(
            fill.arguments[1].binding_kind,
            ArgumentBindingKindDescriptor::OptionalDefault
        );
        assert_eq!(
            fill.arguments[1].optional_default,
            Some(OptionalDefaultValue::ExplicitI32(7))
        );

        let echo = main
            .call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Echo"))
            .expect("Echo call descriptor should be present");
        assert_eq!(echo.target_kind, CallTargetKindDescriptor::Function);
        assert!(
            echo.return_value
                .as_ref()
                .is_some_and(|return_value| return_value.copyout_required)
        );
        assert_eq!(
            echo.arguments[0].binding_kind,
            ArgumentBindingKindDescriptor::ByValCopy
        );

        let capture = main
            .call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Capture"))
            .expect("Capture call descriptor should be present");
        assert_eq!(
            capture.arguments[1].source_kind,
            ArgumentSourceKindDescriptor::ParamArrayPack
        );
        assert_eq!(
            capture.arguments[1].binding_kind,
            ArgumentBindingKindDescriptor::ParamArrayPack
        );
        assert_eq!(
            capture.arguments[1]
                .param_array
                .as_ref()
                .map(|param_array| param_array.element_count),
            Some(2)
        );
    }

    #[test]
    fn call_site_descriptors_preserve_invocation_syntax_and_diagnostic_policy() {
        let source = "Sub Main()\n\
                      Dim x As Long\n\
                      Dim y As Long\n\
                      Touch x\n\
                      Call Touch(y)\n\
                      Touch (x)\n\
                      y = Echo(x)\n\
                      End Sub\n\
                      Sub Touch(ByRef value As Long)\n\
                      value = value + 1\n\
                      End Sub\n\
                      Function Echo(ByVal value As Long) As Long\n\
                      Echo = value\n\
                      End Function";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");
        assert!(
            main.call_sites.iter().any(|call| call.invocation_syntax
                == CallInvocationSyntaxDescriptor::StatementNoCall
                && call.arguments[0].binding_kind == ArgumentBindingKindDescriptor::ByRefAlias
                && call.argument_evaluation_order == vec![0]),
            "statement calls without Call should preserve source order and alias policy"
        );
        assert!(
            main.call_sites.iter().any(|call| call.invocation_syntax
                == CallInvocationSyntaxDescriptor::StatementCallKeyword
                && call.arguments[0].binding_kind == ArgumentBindingKindDescriptor::ByRefAlias),
            "Call keyword invocation should be package-visible"
        );
        assert!(
            main.call_sites.iter().any(|call| call.invocation_syntax
                == CallInvocationSyntaxDescriptor::StatementNoCall
                && call.arguments[0].force_byval
                && call.arguments[0].binding_kind
                    == ArgumentBindingKindDescriptor::ByRefExpressionTemp
                && call
                    .diagnostic_policies
                    .iter()
                    .any(|policy| policy.diagnostic
                        == CallDiagnosticKindDescriptor::ByRefExpressionNoWriteback
                        && policy.owner == CallDiagnosticOwnerDescriptor::OracleNeeded)),
            "parenthesized statement-level argument should record no-writeback temp policy"
        );
        assert!(
            main.call_sites.iter().any(|call| call.invocation_syntax
                == CallInvocationSyntaxDescriptor::ExpressionCall
                && call.target_name.eq_ignore_ascii_case("Echo")
                && call
                    .return_value
                    .as_ref()
                    .is_some_and(|return_value| return_value.copyout_required)),
            "assignment from function should record expression-call syntax and return copyout"
        );
        assert!(
            main.call_sites.iter().any(|call| call
                .diagnostic_policies
                .iter()
                .any(|policy| policy.diagnostic
                    == CallDiagnosticKindDescriptor::MissingRequiredArgument
                    && policy.owner == CallDiagnosticOwnerDescriptor::CompilerCurrent)),
            "current invalid-call diagnostics should remain compiler-owned package policy"
        );
    }

    #[test]
    fn procedure_runtime_metadata_carries_expression_operator_and_coercion_descriptors() {
        let source = "Option Compare Text\n\
                      Sub Main()\n\
                      Dim x As Long\n\
                      Dim y As Double\n\
                      Dim s As String\n\
                      x = 1\n\
                      y = x\n\
                      y = y + x\n\
                      Dim v As Variant\n\
                      v = Null + 1\n\
                      v = Empty + 1\n\
                      v = CVErr(7) + 1\n\
                      s = \"a\" & CStr(x)\n\
                      If Null = 0 Or Empty = \"\" Then\n\
                      s = s\n\
                      End If\n\
                      If x > 0 And y > 0 Then\n\
                      s = s & vbNullString\n\
                      End If\n\
                      If x Then\n\
                      s = s\n\
                      End If\n\
                      Call TakeDouble(x)\n\
                      End Sub\n\
                      Sub TakeDouble(ByVal value As Double)\n\
                      End Sub";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");

        assert!(main.expression_semantics.iter().any(|descriptor| {
            descriptor.classification == ExpressionClassificationDescriptor::Variable
        }));
        assert!(main.expression_semantics.iter().any(|descriptor| {
            descriptor.classification == ExpressionClassificationDescriptor::OperatorResult
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-CONCAT-AMPERSAND"
                && descriptor.operator == VbaOperatorDescriptor::Concatenate
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-BOOL-NOT-AND-OR"
                && descriptor.evaluation_order == EvaluationOrderDescriptor::LeftToRightEager
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-CMP-NULL"
                && descriptor
                    .result_value_states
                    .contains(&ValueStateKind::Null)
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-CMP-EMPTY-STRING"
                && descriptor
                    .result_value_states
                    .contains(&ValueStateKind::Empty)
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-ADD-I32-COMPAT"
                && descriptor
                    .result_value_states
                    .contains(&ValueStateKind::Error)
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-IIF-EAGER-DEFERRED"
                && descriptor.evaluation_order == EvaluationOrderDescriptor::UnsupportedDeferred
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.family == OperatorFamilyDescriptor::Comparison
                && descriptor.compare_mode == Some(OperatorCompareModeDescriptor::Text)
        }));
        assert!(main.coercions.iter().any(|descriptor| {
            descriptor.coercion_id == "COERCE-LET-NUMERIC-WIDEN"
                && descriptor.kind == CoercionKindDescriptor::Let
        }));
        assert!(main.coercions.iter().any(|descriptor| {
            descriptor.coercion_id == "COERCE-CALL-BYVAL-DECLARED-TARGET"
                && descriptor.kind == CoercionKindDescriptor::CallLet
                && descriptor.source_declared_type == VbaTypeId::Long
                && descriptor.target_declared_type == VbaTypeId::Double
        }));
        assert!(
            main.coercions
                .iter()
                .any(|descriptor| { descriptor.coercion_id == "COERCE-VM-TRUTHINESS" })
        );
    }

    #[test]
    fn procedure_runtime_metadata_carries_name_and_property_member_descriptors() {
        let source = "Sub Main()\n\
                      Dim x As Long\n\
                      x = 1\n\
                      Value = x\n\
                      End Sub\n\
                      Property Get Value() As Long\n\
                      Value = 1\n\
                      End Property\n\
                      Property Let Value(ByRef newValue As Long)\n\
                      End Property";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let all_metadata = metadata.values().collect::<Vec<_>>();
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");

        assert!(main.name_bindings.iter().any(|descriptor| {
            descriptor.binding_id == "NAME-BINDING-PROCEDURE-POLICY"
                && descriptor.binding_kind == NameBindingKindDescriptor::Policy
                && descriptor
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic == "with-context=deferred")
        }));
        assert!(all_metadata.iter().any(|metadata| {
            metadata.name_bindings.iter().any(|descriptor| {
                descriptor.binding_id == "NAME-BINDING-PROPERTY-ACCESSOR"
                    && descriptor.name == "value"
            })
        }));
        assert!(all_metadata.iter().any(|metadata| {
            metadata.object_member_bindings.iter().any(|descriptor| {
                descriptor.binding_id == "BIND-PROPERTY-LET-VALUE"
                    && descriptor.member_kind == ObjectMemberKindDescriptor::PropertyLet
                    && descriptor.argument_binding_policy == "value-param-is-runtime-ByVal"
            })
        }));
        assert!(
            main.coercions
                .iter()
                .any(|descriptor| { descriptor.coercion_id == "COERCE-PROPERTY-VALUE-BYVAL" })
        );
    }

    #[test]
    fn compile_with_block_member_assignments() {
        let source = "Sub Main()\nDim x\nWith x\n.Value = 1\n.Value = .Value + 2\nx = .Value\nEnd With\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 1, .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddConstI32 { value: 2, .. }))
        );
    }

    #[test]
    fn compile_with_block_direct_member_target_assignments() {
        let source = "Sub Main()\nDim x\nWith x.inner\n.Value = 4\n.Value = .Value + 3\nx = .Value\nEnd With\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 4, .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddConstI32 { value: 3, .. }))
        );
    }

    #[test]
    fn compile_conditional_compilation_if_else_branch() {
        let source = "#Const ENABLE = True\nSub Main()\nDim x\n#If ENABLE Then\nx = 7\n#Else\nx = 1\n#End If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::LoadConstI32 { slot: 0, value: 7 },
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn undeclared_variable_with_option_explicit_is_rejected() {
        let source = "Option Explicit\nSub Main()\nx = 1\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("undeclared variable"));
    }

    #[test]
    fn duplicate_dim_declaration_is_rejected() {
        let source = "Sub Main()\nDim x\nDim x\nx = 1\nEnd Sub";
        let err = compile(source).expect_err("duplicate declarations should fail");
        assert!(err.to_string().contains("duplicate declaration"));
    }

    #[test]
    fn duplicate_label_declaration_is_rejected() {
        let source = "Sub Main()\nDim x\nGoSub mark\nIf Err.Number = -1 Then\nmark:\nx = 1\nReturn\nmark:\nx = 2\nReturn\nEnd If\nEnd Sub";
        let err = compile(source).expect_err("duplicate labels should fail");
        assert!(err.to_string().contains("duplicate label declaration"));
    }

    #[test]
    fn declaration_same_name_as_other_procedure_is_allowed() {
        // VBA allows a local variable to have the same name as another procedure.
        let source = "Sub Main()\nDim helper\nhelper = 1\nEnd Sub\nSub Helper()\nEnd Sub";
        compile(source).expect("variable sharing name with another procedure should compile");
    }

    #[test]
    fn defobj_applies_to_implicit_declarations() {
        let source = "DefObj A-Z\nSub Main()\na = 1\nEnd Sub";
        let err = compile(source).expect_err("DefObj should type implicit a as Object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn type_char_overrides_def_type_for_dim() {
        let source = "DefObj A-Z\nSub Main()\nDim a%\na = 1\nEnd Sub";
        compile(source).expect("type character should override DefObj for Dim");
    }

    #[test]
    fn explicit_as_overrides_type_char_for_dim() {
        let source = "Sub Main()\nDim a% As Object\na = 1\nEnd Sub";
        let err = compile(source).expect_err("explicit As should override type character");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn def_type_applies_to_untyped_params() {
        let source = "DefObj A-Z\nSub Main()\nCall Use(1)\nEnd Sub\nSub Use(ByVal alpha)\nEnd Sub";
        let err = compile(source).expect_err("DefObj should type alpha as Object");
        assert!(err.to_string().contains("argument type mismatch"));
    }

    #[test]
    fn type_char_overrides_def_type_for_params() {
        let source = "DefObj A-Z\nSub Main()\nCall Use(1)\nEnd Sub\nSub Use(ByVal alpha%)\nEnd Sub";
        compile(source).expect("type character should override DefObj for parameters");
    }

    #[test]
    fn explicit_as_overrides_type_char_for_params() {
        let source = "Sub Main()\nCall Use(1)\nEnd Sub\nSub Use(ByVal alpha% As Object)\nEnd Sub";
        let err = compile(source).expect_err("explicit As should override parameter type char");
        assert!(err.to_string().contains("argument type mismatch"));
    }

    #[test]
    fn function_return_typechar_overrides_def_type() {
        let source = "DefObj A-Z\nFunction alpha%()\nalpha = 1\nEnd Function\nSub Main()\nEnd Sub";
        compile(source).expect("function return type character should override DefObj");
    }

    #[test]
    fn function_return_explicit_as_overrides_typechar() {
        let source = "Function alpha%() As Object\nalpha = 1\nEnd Function\nSub Main()\nEnd Sub";
        let err = compile(source).expect_err("explicit As should control function return type");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn byref_typed_exact_match_is_required() {
        let source = "Sub Main()\nDim x As Integer\nx = 1\nCall Touch(x)\nEnd Sub\nSub Touch(ByRef target As Long)\ntarget = target + 1\nEnd Sub";
        let err = compile(source).expect_err("typed ByRef mismatch should be rejected");
        assert!(err.to_string().contains("ByRef parameter"));
    }

    #[test]
    fn byref_typed_exact_match_accepts_same_type() {
        let source = "Sub Main()\nDim x As Long\nx = 1\nCall Touch(x)\nEnd Sub\nSub Touch(ByRef target As Long)\ntarget = target + 1\nEnd Sub";
        compile(source).expect("typed ByRef with exact type should compile");
    }

    #[test]
    fn late_bound_object_default_member_call_is_executable_subset() {
        let source = "Sub Main()\nDim obj As Object\nCall obj(1)\nEnd Sub";
        let out = compile(source).expect("late-bound target should compile in executable subset");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
    }

    #[test]
    fn late_bound_named_argument_call_preserves_dispatch_lowering() {
        let source = "Sub Main()\nDim obj As Object\nCall obj(x:=1)\nEnd Sub";
        let out = compile(source).expect(
            "late-bound named-arg target should lower into the default-member dispatch path",
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { args, .. } if args.len() == 1 && args[0].name.as_deref() == Some("x")))
        );
    }

    #[test]
    fn mixed_call_coercion_variant_to_long_is_allowed() {
        let source = "Sub Main()\nDim v As Variant\nv = 5\nCall Use(v)\nEnd Sub\nSub Use(ByVal x As Long)\nEnd Sub";
        compile(source).expect("mixed-mode variant argument should coerce to long parameter");
    }

    #[test]
    fn coercion_assignment_variant_to_long_is_allowed() {
        let source = "Sub Main()\nDim v As Variant\nDim x As Long\nx = v\nEnd Sub";
        compile(source).expect("variant should be assignable to typed target in current matrix");
    }

    #[test]
    fn coercion_assignment_object_to_long_is_rejected() {
        let source = "Sub Main()\nDim o As Object\nDim x As Long\nx = o\nEnd Sub";
        let err = compile(source).expect_err("object should not coerce to long");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn coercion_argument_object_to_long_is_rejected() {
        let source =
            "Sub Main()\nDim o As Object\nCall Use(o)\nEnd Sub\nSub Use(ByVal x As Long)\nEnd Sub";
        let err = compile(source).expect_err("object argument should not coerce to long");
        assert!(err.to_string().contains("argument type mismatch"));
    }

    #[test]
    fn conversion_intrinsic_cint_to_object_assignment_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = CInt(5)\nEnd Sub";
        let err = compile(source).expect_err("typed conversion result should not assign to object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn set_keyword_accepts_variant_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim v As Variant\nSet v = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        compile(source).expect("Set should allow object-producing call result into Variant target");
    }

    #[test]
    fn set_keyword_accepts_object_target_for_object_call_result() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        compile(source).expect("Set should allow object-producing call result into Object target");
    }

    #[test]
    fn set_keyword_rejects_variant_target_for_scalar_source() {
        let source = "Sub Main()\nDim v As Variant\nSet v = 7\nEnd Sub";
        let err = compile(source).expect_err("Set should reject scalar source for Variant target");
        assert!(
            err.to_string()
                .contains("Set requires object value for variable v")
        );
    }

    #[test]
    fn set_keyword_rejects_object_target_for_scalar_source() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = 7\nEnd Sub";
        let err = compile(source).expect_err("Set should reject scalar source for Object target");
        assert!(
            err.to_string()
                .contains("Set requires object value for variable obj")
        );
    }

    #[test]
    fn set_keyword_rejects_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nSet n = 7\nEnd Sub";
        let err = compile(source).expect_err("Set should reject scalar source for scalar target");
        assert!(
            err.to_string()
                .contains("Set requires Object or Variant target, got Long variable n")
        );
    }

    #[test]
    fn set_keyword_rejects_scalar_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim n As Long\nSet n = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source)
            .expect_err("Set should reject object-producing call result for scalar target");
        assert!(
            err.to_string()
                .contains("Set requires Object or Variant target, got Long variable n")
        );
    }

    #[test]
    fn let_keyword_rejects_object_target_for_object_call_result() {
        let source = "Sub Main()\nDim obj As Object\nLet obj = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source)
            .expect_err("Let should reject object-producing call result on Object target");
        assert!(
            err.to_string()
                .contains("Let cannot assign to Object variable obj")
        );
    }

    #[test]
    fn let_keyword_accepts_variant_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim v As Variant\nLet v = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        compile(source).expect("Let should allow object-producing call result into Variant target");
    }

    #[test]
    fn let_keyword_rejects_object_target_for_scalar_source() {
        let source = "Sub Main()\nDim obj As Object\nLet obj = 7\nEnd Sub";
        let err = compile(source).expect_err("Let should reject scalar source on Object target");
        assert!(
            err.to_string()
                .contains("Let cannot assign to Object variable obj")
        );
    }

    #[test]
    fn let_keyword_accepts_variant_target_for_scalar_source() {
        let source = "Sub Main()\nDim v As Variant\nLet v = 7\nEnd Sub";
        compile(source).expect("Let should allow scalar source on Variant target");
    }

    #[test]
    fn let_keyword_accepts_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nLet n = 7\nEnd Sub";
        compile(source).expect("Let should allow scalar source on scalar target");
    }

    #[test]
    fn let_keyword_rejects_scalar_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim n As Long\nLet n = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source)
            .expect_err("Let should reject object-producing call result for scalar target");
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn implicit_assignment_accepts_variant_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim v As Variant\nv = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        compile(source).expect(
            "implicit assignment should allow object-producing call result into Variant target",
        );
    }

    #[test]
    fn implicit_assignment_rejects_object_target_for_object_call_result_without_set() {
        let source =
            "Sub Main()\nDim obj As Object\nobj = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source)
            .expect_err("implicit assignment should require Set for object call result");
        assert!(
            err.to_string()
                .contains("Set required for Object variable obj")
        );
    }

    #[test]
    fn implicit_assignment_rejects_scalar_target_for_object_call_result() {
        let source = "Sub Main()\nDim n As Long\nn = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source).expect_err(
            "implicit assignment should reject object-producing call result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn implicit_assignment_rejects_object_target_for_scalar_source() {
        let source = "Sub Main()\nDim obj As Object\nobj = 7\nEnd Sub";
        let err = compile(source)
            .expect_err("implicit assignment should reject scalar source on Object target");
        assert!(
            err.to_string()
                .contains("cannot assign Long to Object variable obj")
        );
    }

    #[test]
    fn implicit_assignment_accepts_variant_target_for_scalar_source() {
        let source = "Sub Main()\nDim v As Variant\nv = 7\nEnd Sub";
        compile(source).expect("implicit assignment should allow scalar source on Variant target");
    }

    #[test]
    fn implicit_assignment_accepts_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nn = 7\nEnd Sub";
        compile(source).expect("implicit assignment should allow scalar source on scalar target");
    }

    #[test]
    fn object_source_assignment_accepts_set_targets_and_variant_let_implicit() {
        let cases = [
            (
                "Set Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet v = src\nEnd Sub",
            ),
            (
                "Set Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet dst = src\nEnd Sub",
            ),
            (
                "Let Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet v = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nv = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            compile(source).unwrap_or_else(|err| panic!("{label} should compile, got {err}"));
        }
    }

    #[test]
    fn object_source_assignment_rejects_object_and_scalar_mismatch_lanes() {
        let cases = [
            (
                "Let Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet dst = src\nEnd Sub",
                "Let cannot assign to Object variable dst",
            ),
            (
                "implicit Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\ndst = src\nEnd Sub",
                "Set required for Object variable dst",
            ),
            (
                "Set scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet n = src\nEnd Sub",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet n = src\nEnd Sub",
                "cannot assign Object to Long variable n",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nn = src\nEnd Sub",
                "cannot assign Object to Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match compile(source) {
                Ok(_) => panic!("{label} should reject"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(message.contains(expected), "{label}: {message}");
        }
    }

    #[test]
    fn variant_source_scalar_payload_assignment_accepts_runtime_checked_lanes() {
        let cases = [
            (
                "Set Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\nSet dst = src\nEnd Sub",
            ),
            (
                "Let Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nLet v = src\nEnd Sub",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nLet n = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nv = src\nEnd Sub",
            ),
            (
                "implicit Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\ndst = src\nEnd Sub",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nn = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            compile(source).unwrap_or_else(|err| panic!("{label} should compile, got {err}"));
        }
    }

    #[test]
    fn variant_source_scalar_payload_assignment_rejects_compile_time_mismatch_lanes() {
        let cases = [
            (
                "Let Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\nLet dst = src\nEnd Sub",
                "Let cannot assign to Object variable dst",
            ),
            (
                "Set scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nSet n = src\nEnd Sub",
                "Set requires Object or Variant target, got Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match compile(source) {
                Ok(_) => panic!("{label} should reject"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(message.contains(expected), "{label}: {message}");
        }
    }

    #[test]
    fn variant_source_object_payload_assignment_accepts_runtime_checked_lanes() {
        let cases = [
            (
                "Set Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet v = src\nEnd Sub",
            ),
            (
                "Set Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet dst = src\nEnd Sub",
            ),
            (
                "Let Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet v = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nv = src\nEnd Sub",
            ),
            (
                "implicit Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\ndst = src\nEnd Sub",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet n = src\nEnd Sub",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nn = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            compile(source).unwrap_or_else(|err| panic!("{label} should compile, got {err}"));
        }
    }

    #[test]
    fn variant_source_object_payload_assignment_rejects_compile_time_mismatch_lanes() {
        let cases = [
            (
                "Let Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet dst = src\nEnd Sub",
                "Let cannot assign to Object variable dst",
            ),
            (
                "Set scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet n = src\nEnd Sub",
                "Set requires Object or Variant target, got Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match compile(source) {
                Ok(_) => panic!("{label} should reject"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(message.contains(expected), "{label}: {message}");
        }
    }

    #[test]
    fn variant_source_set_assignment_emits_runtime_validation() {
        let out = compile(
            "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nSet v = src\nEnd Sub",
        )
        .expect("compile should succeed");
        assert!(out.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::ValidateRuntimeAssignment {
                    intent: RuntimeAssignmentIntent::Set,
                    target_kind: RuntimeAssignmentTargetKind::Variant,
                    target_name,
                    ..
                } if target_name == "v"
            )
        }));
    }

    #[test]
    fn variant_source_implicit_object_assignment_emits_runtime_validation() {
        let out = compile(
            "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\ndst = src\nEnd Sub",
        )
        .expect("compile should succeed");
        assert!(out.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::ValidateRuntimeAssignment {
                    intent: RuntimeAssignmentIntent::Implicit,
                    target_kind: RuntimeAssignmentTargetKind::Object,
                    target_name,
                    ..
                } if target_name == "dst"
            )
        }));
    }

    #[test]
    fn conversion_intrinsic_cint_to_long_assignment_is_allowed() {
        let source = "Sub Main()\nDim x As Long\nx = CInt(5)\nEnd Sub";
        compile(source).expect("typed conversion result should assign to widening numeric target");
    }

    #[test]
    fn conversion_intrinsic_str_to_object_assignment_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = Str(5)\nEnd Sub";
        let err = compile(source).expect_err("Str result should not assign to object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn instrrev_result_to_object_assignment_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = InStrRev(123231, 23)\nEnd Sub";
        let err = compile(source).expect_err("InStrRev result should be typed as Long");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn mid_statement_object_target_is_rejected() {
        let source = "Sub Main()\nDim o As Object\nMid(o, 2, 2) = 99\nEnd Sub";
        let err = compile(source).expect_err("Mid statement should reject object targets");
        assert!(
            err.to_string()
                .contains("type mismatch in Mid assignment target")
        );
    }

    #[test]
    fn vbnullstring_assigns_to_string() {
        let source = "Sub Main()\nDim s As String\ns = vbNullString\nEnd Sub";
        compile(source).expect("vbNullString should be treated as string-typed intrinsic constant");
    }

    #[test]
    fn vbnullstring_assignment_to_object_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = vbNullString\nEnd Sub";
        let err = compile(source).expect_err("vbNullString should not assign to object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn vbnullstring_assignment_to_long_is_rejected() {
        let source = "Sub Main()\nDim x As Long\nx = vbNullString\nEnd Sub";
        let err = compile(source).expect_err("vbNullString should not assign to numeric target");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn vbnullstring_argument_to_long_param_is_rejected() {
        let source =
            "Sub Main()\nCall Use(vbNullString)\nEnd Sub\nSub Use(ByVal x As Long)\nEnd Sub";
        let err = compile(source).expect_err("vbNullString should not pass to numeric parameter");
        assert!(err.to_string().contains("argument type mismatch"));
    }

    #[test]
    fn compile_cverr_emits_retained_error_instruction() {
        let source = "Sub Main()\nDim x\nx = CVErr(7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCVErr { .. }))
        );
        assert!(
            !out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddConstI32 { .. }))
        );
    }

    #[test]
    fn compile_financial_intrinsics_emit_algorithmic_ops() {
        let source = "Sub Main()\nDim c\nDim d\nDim e\nDim f\nDim g\nc = NPV(1, 10, 20, 30)\nd = IRR(50)\ne = MIRR(70, 1, 2)\nf = Rate(10, 2, 99)\ng = NPer(1, 2, 88, 3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicNpvI32 { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicIrrI32 { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicMirrI32 { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicRateI32 { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicNPerI32 { .. }))
        );
    }

    #[test]
    fn arithmetic_object_plus_const_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = o + 1\nEnd Sub";
        let err = compile(source).expect_err("object arithmetic should be rejected");
        assert!(
            err.to_string()
                .contains("type mismatch in arithmetic expression")
        );
    }

    #[test]
    fn comparison_object_long_is_rejected() {
        let source = "Sub Main()\nDim o As Object\nIf o = 1 Then\nEnd If\nEnd Sub";
        let err = compile(source).expect_err("object vs long comparison should be rejected");
        assert!(err.to_string().contains("type mismatch in comparison"));
    }

    #[test]
    fn comparison_variant_long_is_allowed() {
        let source = "Sub Main()\nDim v As Variant\nIf v = 1 Then\nv = 2\nEnd If\nEnd Sub";
        compile(source).expect("variant comparison with long should compile");
    }

    #[test]
    fn compile_mul_expression() {
        let source = "Sub Main()\nDim x\nx = x * 2\nEnd Sub";
        let out = compile(source).expect("multiply expression should compile");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::MulSlots { .. }))
        );
    }

    #[test]
    fn undeclared_variable_without_option_explicit_is_accepted() {
        let source = "Sub Main()\nx = 1\nx = x + 1\nEnd Sub";
        let out = compile(source).expect("implicit declaration should compile");
        assert_eq!(out.slot_count, 1);
    }

    #[test]
    fn compile_if_statement_emits_branch_instructions() {
        let source = "Sub Main()\nDim x\nx = 1\nIf x = 1 Then\nx = x + 2\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpEqSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
    }

    #[test]
    fn compile_if_statement_with_relational_and_boolean_ops() {
        let source =
            "Sub Main()\nDim x\nx = 1\nIf Not x <> 1 Or x < 2 Then\nx = x + 1\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpNeSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLtSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolNot { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolOr { .. }))
        );
    }

    #[test]
    fn compile_single_line_if_statement_emits_branch_instructions() {
        let source = "Sub Main()\nDim x\nx = 1\nIf x = 1 Then x = x + 2\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpEqSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
    }

    #[test]
    fn compile_for_statement_emits_loop_instructions() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLeSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
    }

    #[test]
    fn compile_if_else_if_else_emits_chain_jumps() {
        let source = "Sub Main()\nDim x\nx = 2\nIf x = 1 Then\nx = 10\nElseIf x = 2 Then\nx = 20\nElse\nx = 30\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let jump_if_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::JumpIfZero { .. }))
            .count();
        let jump_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::Jump { .. }))
            .count();
        assert!(jump_if_count >= 2);
        assert!(jump_count >= 2);
    }

    #[test]
    fn compile_do_while_emits_loop_jumps() {
        let source = "Sub Main()\nDim x\nDo While x < 3\nx = x + 1\nLoop\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
    }

    #[test]
    fn compile_for_step_emits_addslots() {
        let source =
            "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 5 To 1 Step -2\nx = x + 1\nNext i\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddSlots { .. }))
        );
    }

    #[test]
    fn compile_for_step_zero_is_diagnostic_error() {
        let source = "Sub Main()\nDim i\nFor i = 1 To 5 Step 0\ni = i + 1\nNext i\nEnd Sub";
        let err = compile(source).expect_err("compile should fail for zero step");
        assert!(matches!(
            err,
            super::CompileError::TypeError(msg) if msg.contains("for loop step cannot be zero")
        ));
    }

    #[test]
    fn compile_select_case_is_range_emits_clause_dispatch() {
        let source = "Sub Main()\nDim x\nSelect Case x\nCase Is < 0\nx = 1\nCase 1 To 3\nx = 2\nEnd Select\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLtSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolAnd { .. }))
        );
    }

    #[test]
    fn compile_goto_label_binds_target() {
        let source = "Sub Main()\nDim x\nx = 1\nGoTo done\nx = 99\ndone:\nx = x + 1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
    }

    #[test]
    fn compile_resume_label_emits_resume_label_instruction() {
        let source =
            "Sub Main()\nOn Error GoTo handler\nError 5\nhandler:\nResume done\ndone:\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::ResumeLabel { .. }))
        );
    }

    #[test]
    fn compile_err_clear_emits_clear_err_instruction() {
        let source = "Sub Main()\nOn Error Resume Next\nError 7\nErr.Clear\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::ClearErr))
        );
    }

    #[test]
    fn compile_select_case_emits_case_dispatch() {
        let source = "Sub Main()\nDim x\nSelect Case x\nCase 1\nx = 10\nCase 2, 3\nx = 20\nCase Else\nx = 30\nEnd Select\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolOr { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
    }

    #[test]
    fn compile_named_sub_call_emits_callproc() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall Foo\nEnd Sub\nSub Foo()\nDim y\ny = 2\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Return))
        );
    }

    #[test]
    fn compile_with_runtime_metadata_reports_entry_points_for_named_procedures() {
        let source = "Sub Main()\nDim x\nx = 1\nCall Foo(x)\nEnd Sub\nSub Foo(ByVal n)\nDim y\ny = n\nEnd Sub";
        let (bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("entry procedure metadata should exist");
        let foo = metadata
            .get("foo")
            .expect("named procedure metadata should exist");
        assert!(main.entry_pc < bytecode.instructions.len());
        assert!(foo.entry_pc < bytecode.instructions.len());
        assert_eq!(foo.param_slots.len(), 1);
        assert_eq!(main.procedure_name, "main");
        assert_eq!(main.source_line_start, 1);
        assert_eq!(main.source_line_end, 5);
        assert_eq!(main.statement_line_numbers, vec![2, 3, 4]);
        assert_eq!(main.statement_entry_pcs.len(), 2);
        assert_eq!(main.statement_entry_pcs[0], main.entry_pc + 1);
        assert_eq!(foo.procedure_name, "foo");
        assert_eq!(foo.source_line_start, 6);
        assert_eq!(foo.source_line_end, 9);
        assert_eq!(foo.statement_line_numbers, vec![7, 8]);
        assert_eq!(foo.statement_entry_pcs.len(), 1);
        assert_eq!(foo.statement_entry_pcs[0], foo.entry_pc + 1);
    }

    #[test]
    fn compile_optional_param_call_accepts_omitted_arg() {
        let source = "Sub Main()\nDim x\nCall Fill(x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_optional_param_call_rejects_missing_required_arg() {
        let source = "Sub Main()\nCall Fill\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("missing required argument"));
    }

    #[test]
    fn compile_named_args_call_is_supported() {
        let source = "Sub Main()\nDim x\nCall Fill(target := x, value := 9)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_rejects_positional_argument_after_named_argument() {
        let source = "Sub Main()\nDim x\nCall Fill(value := 9, x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(
            err.to_string()
                .contains("positional argument cannot follow named argument")
        );
    }

    #[test]
    fn compile_paramarray_call_accepts_trailing_positional_args() {
        let source = "Sub Main()\nDim x\nCall Capture(x, 5, 7, 9)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicUBoundArray { .. }))
        );
        assert!(out.instructions.iter().any(
            |i| matches!(i, Instruction::IntrinsicArrayLiteral { values, .. } if values.len() == 3)
        ));
    }

    #[test]
    fn compile_paramarray_named_fixed_arg_with_positional_pack_compiles() {
        let source = "Sub Main()\nDim x\nCall Capture(target := x, 5, 7)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(out.instructions.iter().any(
            |i| matches!(i, Instruction::IntrinsicArrayLiteral { values, .. } if values.len() == 2)
        ));
    }

    #[test]
    fn compile_paramarray_named_param_still_rejects_named_pack_target() {
        let source = "Sub Main()\nDim x\nCall Capture(target := x, items := 5)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("ParamArray"));
    }

    #[test]
    fn compile_gosub_and_return_subset() {
        let source = "Sub Main()\nDim x\nx = 1\nGoSub add_two\nx = x + 1\nIf Err.Number = -1 Then\nadd_two:\nx = x + 2\nReturn\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let call_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::CallProc { .. }))
            .count();
        let return_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::Return))
            .count();
        assert!(call_count >= 1);
        assert!(return_count >= 1);
    }

    #[test]
    fn compile_gosub_rejects_missing_label() {
        let source = "Sub Main()\nGoSub nope\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("gosub target label not found"));
    }

    #[test]
    fn compile_redim_preserve_subset() {
        let source =
            "Sub Main()\nDim a(1)\nDim x\na(0) = 7\nReDim Preserve a(3)\nx = a(0)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 7, .. }))
        );
    }

    #[test]
    fn compile_redim_shrink_bounds_violation_is_rejected() {
        let source = "Sub Main()\nDim a(3)\nReDim a(1)\na(2) = 9\nEnd Sub";
        let err = compile(source).expect_err("compile should fail");
        let message = err.to_string();
        assert!(
            message.contains("a(2)") || message.contains("a_2") || message.contains("unsupported"),
            "expected bounds rejection diagnostic, got: {message}"
        );
    }

    #[test]
    fn compile_runtime_redim_expression_bounds_on_dynamic_array_emits_resize_instruction() {
        let source = "Sub Main()\nDim length As Long\nDim buf() As Byte\nlength = 3\nReDim buf(length - 1)\nEnd Sub";
        assert!(
            compile(source)
                .expect("compile should succeed")
                .instructions
                .iter()
                .any(|i| matches!(
                    i,
                    Instruction::IntrinsicArrayResize {
                        lower_bounds,
                        element_type: crate::bytecode::RuntimeArrayElementType::Byte,
                        ..
                    } if lower_bounds == &vec![0]
                )),
            "expected runtime array resize instruction for dynamic ReDim"
        );
    }

    #[test]
    fn compile_runtime_redim_preserve_expression_bounds_emits_runtime_preserve_resize() {
        let source = "Sub Main()\nDim length As Long\nDim buf() As Byte\nlength = 3\nReDim Preserve buf(length - 1)\nEnd Sub";
        assert!(
            compile(source)
                .expect("compile should succeed")
                .instructions
                .iter()
                .any(|i| matches!(
                    i,
                    Instruction::IntrinsicArrayResizePreserve {
                        lower_bounds,
                        element_type: crate::bytecode::RuntimeArrayElementType::Byte,
                        ..
                    } if lower_bounds == &vec![0]
                )),
            "expected runtime preserve array resize instruction for dynamic ReDim Preserve"
        );
    }

    #[test]
    fn compile_option_base_one_array_indexing_subset() {
        let source =
            "Option Base 1\nSub Main()\nDim a(3)\nDim x\na(1) = 4\na(3) = 9\nx = a(3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 9, .. }))
        );
    }

    #[test]
    fn compile_multidim_array_reference_subset() {
        let source = "Sub Main()\nDim m(1 To 2, 1 To 3)\nDim x\nm(2, 3) = 17\nx = m(2, 3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 17, .. }))
        );
    }

    #[test]
    fn compile_redim_preserve_non_last_dimension_change_is_rejected() {
        let source = "Sub Main()\nDim m(1 To 2, 1 To 2)\nReDim Preserve m(1 To 3, 1 To 2)\nEnd Sub";
        let err = compile(source).expect_err("compile should fail");
        assert!(
            err.to_string()
                .contains("redim preserve only supports resizing")
        );
    }

    #[test]
    fn compile_module_const_usage_is_supported() {
        let source = "Const BASE = 5\nSub Main()\nDim x\nx = BASE + 2\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 5, .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddConstI32 { value: 2, .. }))
        );
    }

    #[test]
    fn compile_enum_member_usage_is_supported() {
        let source =
            "Enum Mode\nFast = 3\nSafe\nEnd Enum\nSub Main()\nDim x\nx = Safe + 1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 4, .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddConstI32 { value: 1, .. }))
        );
    }

    #[test]
    fn compile_udt_declaration_block_is_accepted() {
        let source =
            "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim x\nx = 9\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 9, .. }))
        );
    }

    #[test]
    fn compile_intrinsic_conversion_subset_is_accepted() {
        let source = "Sub Main()\nDim x\nx = CLng(CInt(7))\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 7, .. }))
        );
    }

    #[test]
    fn compile_len_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = Len(1234)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicLenDigits { .. }))
        );
    }

    #[test]
    fn compile_mid_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = Mid(12345, 2, 2)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicMidDigits { .. }))
        );
    }

    #[test]
    fn compile_mid_statement_emits_mutation_instruction() {
        let source = "Sub Main()\nDim x\nx = 12345\nMid(x, 2, 2) = 99\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicMidStmtDigits { .. }))
        );
    }

    #[test]
    fn compile_replace_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = Replace(12345, 23, 67)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicReplaceDigits { .. }))
        );
    }

    #[test]
    fn compile_strcomp_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = StrComp(12, 123)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicStrCompDigits { .. }))
        );
    }

    #[test]
    fn compile_instrrev_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = InStrRev(123231, 23)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicInStrRevDigits { .. }))
        );
    }

    #[test]
    fn compile_option_compare_text_emits_text_compare_mode_intrinsics() {
        let source = "Option Compare Text\nSub Main()\nDim x\nx = StrComp(12, 12)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::IntrinsicStrCompDigits {
                mode: StringCompareMode::Text,
                ..
            }
        )));
    }

    #[test]
    fn compile_like_condition_emits_like_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nDim y\ny = 12\nIf y Like 12 Then\nx = 1\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicLikeDigits { .. }))
        );
    }

    #[test]
    fn compile_like_value_expr_emits_like_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = \"ABC\" Like \"abc\"\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicLikeDigits { .. }))
        );
    }

    #[test]
    fn compile_dateserial_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = DateSerial(2026, 2, 28)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDateSerialDigits { .. }))
        );
    }

    #[test]
    fn compile_math_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = Abs(-7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicAbsI32 { .. }))
        );
    }

    #[test]
    fn compile_array_introspection_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = UBound(Array(1, 2, 3))\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicUBoundArray { .. }))
        );
    }

    #[test]
    fn compile_array_span_expr_preserves_lbound_ubound_and_binary_shape() {
        let source = concat!(
            "Private Sub MeasureBounds(ByRef value() As Byte, ByRef spanValue As Long)\n",
            "    spanValue = UBound(value) - LBound(value) + 1\n",
            "End Sub\n",
            "Sub Main()\n",
            "    Dim buf(2) As Byte\n",
            "    Dim spanValue As Long\n",
            "    MeasureBounds buf, spanValue\n",
            "End Sub",
        );
        let out = compile(source).expect("compile should succeed");
        assert_eq!(
            out.instructions
                .iter()
                .filter(|i| matches!(i, Instruction::IntrinsicLBoundArray { .. }))
                .count(),
            1
        );
        assert_eq!(
            out.instructions
                .iter()
                .filter(|i| matches!(i, Instruction::IntrinsicUBoundArray { .. }))
                .count(),
            1
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::SubSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddSlots { .. }))
        );
    }

    #[test]
    fn compile_host_sensitive_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = Environ(77)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicEnvironHost { .. }))
        );
    }

    #[test]
    fn compile_time_locale_intrinsics_emit_host_instructions() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = Date()\nb = Time()\nc = Now()\nd = Timer()\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDateNowHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicTimeNowHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicNowHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicTimerHost { .. }))
        );
    }

    #[test]
    fn compile_freefile_intrinsic_emits_host_instruction() {
        let source = "Sub Main()\nDim a\nDim b\na = FreeFile()\nb = FreeFile(1)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::IntrinsicFreeFileHost {
                range_selector: None,
                ..
            }
        )));
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::IntrinsicFreeFileHost {
                range_selector: Some(_),
                ..
            }
        )));
    }

    #[test]
    fn compile_file_position_intrinsics_emit_host_instructions() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = EOF(1)\nb = LOF(1)\nc = Seek(1)\nd = Loc(1)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileEofHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileLofHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileSeekHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileLocHost { .. }))
        );
    }

    #[test]
    fn compile_file_write_statement_emits_host_instruction() {
        let source =
            "Sub Main()\nOpen \"x\" For Output As #1\nWrite #1, \"hello,world\"\nClose #1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileWriteHost { .. }))
        );
    }

    #[test]
    fn compile_file_kill_statement_emits_host_instruction() {
        let source = "Sub Main()\nDim path As String\npath = \"x\"\nKill path\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileKillHost { .. }))
        );
    }

    #[test]
    fn compile_multi_field_file_write_statement_emits_multiple_host_instructions() {
        let source = "Sub Main()\nOpen \"x\" For Output As #1\nWrite #1, 42, True, \"hello,world\"\nClose #1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::IntrinsicFileWriteHost { .. }))
            .count();
        assert_eq!(count, 3, "expected one host write per Write# field");
    }

    #[test]
    fn compile_console_io_statements_emit_console_host_instructions() {
        let source = "Sub Main()\nDim a\nDim b\nPrint \"hello\"\nInput a\nLine Input b\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicConsolePrintHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicConsoleInputHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicConsoleLineInputHost { .. }))
        );
    }

    #[test]
    fn compile_beep_statement_emits_host_instruction() {
        let source = "Sub Main()\nBeep\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicBeepHost { .. }))
        );
    }

    #[test]
    fn compile_direct_top_level_mainline_with_option_private_module_succeeds() {
        let source = "Option Private Module\nvalueOut = 41\nCall Bump(valueOut)\nSub Bump(ByRef value)\nvalue = value + 1\nEnd Sub";
        compile(source)
            .expect("direct top-level mainline with Option Private Module should compile");
    }

    #[test]
    fn compile_direct_top_level_mainline_with_defobj_preserves_module_scope_defaults() {
        let source = "DefObj A-Z\na = 1\n";
        let err =
            compile(source).expect_err("DefObj should still type implicit top-level `a` as Object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn compile_direct_top_level_mainline_with_module_const_directive_succeeds() {
        let source =
            "#Const ENABLE = True\n#If ENABLE Then\nvalueOut = 41\n#Else\nvalueOut = 0\n#End If\n";
        compile(source).expect("top-level mainline with module #Const should compile");
    }

    #[test]
    fn compile_direct_top_level_mainline_with_mixed_module_scope_declarations_succeeds() {
        let source = concat!(
            "Option Explicit\n",
            "Option Private Module\n",
            "Rem module comment\n",
            "#Const ENABLE = True\n",
            "DefLng A-Z\n",
            "Public valueOut As Long\n",
            "Public sharedCount As Long\n",
            "Private counter As Long\n",
            "Global totalCount As Long\n",
            "Static stickyCount As Long\n",
            "Private Type CounterState\n",
            "    Value As Long\n",
            "End Type\n",
            "Public Enum CounterMode\n",
            "    CounterModeDefault = 1\n",
            "End Enum\n",
            "counter = 41\n",
            "valueOut = counter\n",
            "Call Bump(valueOut)\n",
            "Public Sub Bump(ByRef value)\n",
            "    value = value + 1\n",
            "End Sub\n",
        );
        compile(source).expect("mixed module declarations plus top-level mainline should compile");
    }

    #[test]
    fn compile_debug_print_emits_diagnostics_host_instruction() {
        let source = "Sub Main()\nDebug.Print \"hello\"\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDebugPrintHost { .. }))
        );
    }

    #[test]
    fn compile_debug_print_multiple_exprs_emits_diagnostics_host_instruction() {
        let source = "Sub Main()\nDebug.Print \"hello\", Err.LastDllError\nEnd Sub";
        let out = compile(source).expect("multi-expr Debug.Print compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDebugPrintHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::ConcatSlots { .. })),
            "expected concatenation for multi-expr Debug.Print in {:?}",
            out.instructions
        );
    }

    #[test]
    fn compile_exit_function_is_accepted() {
        let source = "Function F()\nF = 1\nExit Function\nF = 2\nEnd Function\nSub Main()\nDim y\ny = F()\nEnd Sub";
        compile(source).expect("Exit Function compile should succeed");
    }

    #[test]
    fn compile_boolean_literal_emits_bool_const_instruction() {
        let source = "Sub Main()\nDim x\nx = True\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstBool { value: true, .. }))
        );
    }

    #[test]
    fn compile_ui_event_intrinsics_emit_host_instructions() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\na = MsgBox(7, 3)\nb = InputBox(9, 4)\nc = DoEvents()\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicMsgBoxHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicInputBoxHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDoEventsHost { .. }))
        );
    }

    #[test]
    fn compile_collection_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = CollectionCount(CollectionAdd(0, 9))\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCollectionAdd { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCollectionCount { .. }))
        );
    }

    #[test]
    fn compile_dispatch_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), 2, 3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCreateObjectHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
    }

    #[test]
    fn compile_dispatch_intrinsic_with_array_argument_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), 6, Array(1, 2, 3))\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert!(out.instructions.iter().any(
            |i| matches!(i, Instruction::IntrinsicArrayLiteral { values, .. } if values.len() == 3)
        ));
    }

    #[test]
    fn compile_createobject_with_progid_literal_preserves_string_literal() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"Scripting.Dictionary\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for ProgID literal");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCreateObjectHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstString { value, .. } if value == "Scripting.Dictionary"))
        );
    }

    #[test]
    fn compile_createobject_with_oxvba_test_dispatch_literal_preserves_string_literal() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for controlled test ProgID");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstString { value, .. } if value == "OxVba.TestDispatch"))
        );
    }

    #[test]
    fn compile_dispatchinvoke_with_member_literal_maps_to_known_member_token() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"Scripting.Dictionary\"), \"Count\", 0)\nEnd Sub";
        let out = compile(source).expect("compile should succeed for known member literal");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 1);
    }

    #[test]
    fn compile_dispatchinvoke_with_firechanged_literal_maps_to_member_token_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"FireChanged\", 7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed for controlled event member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 3);
    }

    #[test]
    fn compile_dispatchinvoke_with_property_set_literals_maps_to_member_tokens() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetValue\", 7)\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetValueRef\", 7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed for controlled setter members");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_has_dispatch_member(&out, 7, "SetValue");
        assert_has_dispatch_member(&out, 8, "SetValueRef");
    }

    #[test]
    fn compile_dispatchinvoke_with_long_result_literal_maps_to_member_token_thirty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnLong\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for long result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 35);
    }

    #[test]
    fn compile_dispatchinvoke_with_unsigned_long_result_literal_maps_to_member_token_thirty_six() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnUnsignedLong\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for unsigned-long result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 36);
    }

    #[test]
    fn compile_dispatchinvoke_with_byte_result_literal_maps_to_member_token_thirty_seven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnByte\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for byte result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 37);
    }

    #[test]
    fn compile_dispatchinvoke_with_signed_byte_result_literal_maps_to_member_token_thirty_nine() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSignedByte\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for signed-byte result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 39);
    }

    #[test]
    fn compile_dispatchinvoke_with_platform_int_result_literal_maps_to_member_token_forty_one() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlatformInt\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for platform-int result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 41);
    }

    #[test]
    fn compile_dispatchinvoke_with_platform_uint_result_literal_maps_to_member_token_forty_two() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlatformUInt\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for platform-uint result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 42);
    }
    #[test]
    fn compile_dispatchinvoke_with_hyper_result_literal_maps_to_member_token_forty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnHyper\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for hyper result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 45);
    }

    #[test]
    fn compile_dispatchinvoke_with_unsigned_hyper_result_literal_maps_to_member_token_forty_six() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnUnsignedHyper\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for unsigned-hyper result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 46);
    }
    #[test]
    fn compile_dispatchinvoke_with_double_result_literal_maps_to_member_token_forty_nine() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDouble\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for double result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 49);
    }

    #[test]
    fn compile_dispatchinvoke_with_single_result_literal_maps_to_member_token_fifty_one() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSingle\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for single result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 51);
    }

    #[test]
    fn compile_dispatchinvoke_with_date_result_literal_maps_to_member_token_fifty_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDate\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for date result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 53);
    }

    #[test]
    fn compile_dispatchinvoke_with_currency_result_literal_maps_to_member_token_fifty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnCurrency\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for currency result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 55);
    }

    #[test]
    fn compile_dispatchinvoke_with_decimal_result_literal_maps_to_member_token_fifty_seven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDecimal\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for decimal result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 57);
    }

    #[test]
    fn compile_dispatchinvoke_with_bool_result_literal_maps_to_member_token_sixty_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnBool\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for bool result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 63);
    }

    #[test]
    fn compile_dispatchinvoke_with_string_result_literal_maps_to_member_token_sixty_four() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnString\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for string result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 64);
    }

    #[test]
    fn compile_dispatchinvoke_with_missing_member_name_result_literal_maps_to_member_token_seventy_six()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnMissingMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for missing-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 76);
    }

    #[test]
    fn compile_dispatchinvoke_with_ping_member_name_result_literal_maps_to_member_token_seventy_seven()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPingMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for ping-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 77);
    }

    #[test]
    fn compile_dispatchinvoke_with_lookup_member_name_result_literal_maps_to_member_token_seventy_eight()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnLookupMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for lookup-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 78);
    }

    #[test]
    fn compile_dispatchinvoke_with_sum_pair_member_name_result_literal_maps_to_member_token_seventy_nine()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSumPairMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for sum-pair-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 79);
    }

    #[test]
    fn compile_dispatchinvoke_with_lookup_pair_member_name_result_literal_maps_to_member_token_eighty()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnLookupPairMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for lookup-pair-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 80);
    }

    #[test]
    fn compile_dispatchinvoke_with_set_value_member_name_result_literal_maps_to_member_token_eighty_one()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSetValueMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for set-value-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 81);
    }

    #[test]
    fn compile_dispatchinvoke_with_set_value_ref_member_name_result_literal_maps_to_member_token_eighty_two()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSetValueRefMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for set-value-ref-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 82);
    }

    #[test]
    fn compile_dispatchinvoke_with_set_indexed_value_member_name_result_literal_maps_to_member_token_eighty_three()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSetIndexedValueMemberName\")\nEnd Sub";
        let out = compile(source).expect(
            "compile should succeed for set-indexed-value-member-name result fixture member",
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 83);
    }

    #[test]
    fn compile_dispatchinvoke_with_set_indexed_value_ref_member_name_result_literal_maps_to_member_token_eighty_four()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSetIndexedValueRefMemberName\")\nEnd Sub";
        let out = compile(source).expect(
            "compile should succeed for set-indexed-value-ref-member-name result fixture member",
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 84);
    }

    #[test]
    fn compile_dispatchinvoke_with_value_member_name_result_literal_maps_to_member_token_eighty_five()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnValueMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for value-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 85);
    }

    #[test]
    fn compile_dispatchinvoke_with_default_member_name_result_literal_maps_to_member_token_eighty_six()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDefaultMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for default-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 86);
    }

    #[test]
    fn compile_dispatchinvoke_with_empty_result_literal_maps_to_member_token_sixty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnEmpty\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for empty result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 65);
    }

    #[test]
    fn compile_dispatchinvoke_with_null_result_literal_maps_to_member_token_sixty_six() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnNull\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for null result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 66);
    }

    #[test]
    fn compile_dispatchinvoke_with_error_result_literal_maps_to_member_token_sixty_seven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnError\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for error result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 67);
    }

    #[test]
    fn compile_dispatchinvoke_with_byref_long_result_literal_maps_to_member_token_sixty_eight() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnByRefLong\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for byref-long result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 68);
    }

    #[test]
    fn compile_dispatchinvoke_with_byref_long_array_result_literal_maps_to_member_token_sixty_nine()
    {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnByRefLongArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for byref-long-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 69);
    }

    #[test]
    fn compile_dispatchinvoke_with_typed_array_result_literal_maps_to_member_token_twenty() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSmallIntArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for typed SAFEARRAY fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 20);
    }

    #[test]
    fn compile_dispatchinvoke_with_object_result_literal_maps_to_member_token_twenty_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSelfDispatch\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for object-result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 23);
    }

    #[test]
    fn compile_dispatchinvoke_with_variant_classifier_literal_maps_to_member_token_twenty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ClassifyVariantArg\", 7)\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for variant classifier fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 25);
    }

    #[test]
    fn compile_dispatchinvoke_with_unknown_member_literal_preserves_string_selector() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestEventServer\"), \"IsSelf\", CreateObject(\"OxVba.TestEventServer\"))\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for external string member selector");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert!(
            out.instructions.iter().any(
                |i| matches!(i, Instruction::LoadConstString { value, .. } if value == "IsSelf")
            )
        );
    }

    #[test]
    fn compile_dispatchinvoke_with_external_colliding_member_literal_preserves_string_selector() {
        let source = "Sub Main()\nDim obj\nDim x\nobj = CreateObject(\"OxVba.TestEventServer\")\nx = DispatchInvoke(obj, \"SumPair\", 3, 14)\nEnd Sub";
        let out = compile(source)
            .expect("compile should preserve external colliding string member selector");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert!(out.instructions.iter().any(
            |i| matches!(i, Instruction::LoadConstString { value, .. } if value == "SumPair")
        ));
        assert!(
            !out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 12, .. })),
            "external colliding member literal must not lower to deterministic TestDispatch token"
        );
    }

    #[test]
    fn compile_dispatchinvoke_with_variant_array_classifier_literal_maps_to_member_token_twenty_six()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ClassifyVariantArrayFirstElementArg\", Array(1))\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for variant-array classifier fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 26);
    }

    #[test]
    fn compile_dispatchinvoke_with_dispatch_array_result_literal_maps_to_member_token_twenty_seven()
    {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSelfDispatchArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for dispatch-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 27);
    }

    #[test]
    fn compile_dispatchinvoke_with_typed_dispatch_array_result_literal_maps_to_member_token_twenty_eight()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSelfTypedDispatchArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for typed dispatch-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 28);
    }

    #[test]
    fn compile_dispatchinvoke_with_typed_unknown_array_result_literal_maps_to_member_token_twenty_nine()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSelfTypedUnknownArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for typed unknown-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 29);
    }

    #[test]
    fn compile_dispatchinvoke_with_plain_unknown_result_literal_maps_to_member_token_thirty_one() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlainUnknown\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for plain-unknown result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 31);
    }
    #[test]
    fn compile_dispatchinvoke_with_plain_unknown_array_result_literal_maps_to_member_token_thirty_two()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlainUnknownArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for plain-unknown-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 32);
    }
    #[test]
    fn compile_dispatchinvoke_with_long_array_result_literal_maps_to_member_token_thirty_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnLongArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for long-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 33);
    }

    #[test]
    fn compile_dispatchinvoke_with_unsigned_long_array_result_literal_maps_to_member_token_thirty_four()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnUnsignedLongArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for unsigned-long-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 34);
    }

    #[test]
    fn compile_dispatchinvoke_with_byte_array_result_literal_maps_to_member_token_thirty_eight() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnByteArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for byte-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 38);
    }

    #[test]
    fn compile_dispatchinvoke_with_signed_byte_array_result_literal_maps_to_member_token_forty() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSignedByteArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for signed-byte-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 40);
    }

    #[test]
    fn compile_dispatchinvoke_with_platform_int_array_result_literal_maps_to_member_token_forty_three()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlatformIntArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for platform-int-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 43);
    }

    #[test]
    fn compile_dispatchinvoke_with_platform_uint_array_result_literal_maps_to_member_token_forty_four()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlatformUIntArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for platform-uint-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 44);
    }
    #[test]
    fn compile_dispatchinvoke_with_hyper_array_result_literal_maps_to_member_token_forty_seven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnHyperArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for hyper-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 47);
    }

    #[test]
    fn compile_dispatchinvoke_with_unsigned_hyper_array_result_literal_maps_to_member_token_forty_eight()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnUnsignedHyperArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for unsigned-hyper-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 48);
    }
    #[test]
    fn compile_dispatchinvoke_with_double_array_result_literal_maps_to_member_token_fifty() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDoubleArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for double-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 50);
    }

    #[test]
    fn compile_dispatchinvoke_with_single_array_result_literal_maps_to_member_token_fifty_two() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSingleArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for single-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 52);
    }

    #[test]
    fn compile_dispatchinvoke_with_date_array_result_literal_maps_to_member_token_fifty_four() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDateArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for date-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 54);
    }

    #[test]
    fn compile_dispatchinvoke_with_currency_array_result_literal_maps_to_member_token_fifty_six() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnCurrencyArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for currency-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 56);
    }

    #[test]
    fn compile_dispatchinvoke_with_decimal_array_result_literal_maps_to_member_token_fifty_eight() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDecimalArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for decimal-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 58);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_unsigned_long_result_literal_maps_to_member_token_fifty_nine()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideUnsignedLong\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide unsigned-long result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 59);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_unsigned_long_array_result_literal_maps_to_member_token_sixty()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideUnsignedLongArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide unsigned-long-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 60);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_platform_uint_result_literal_maps_to_member_token_sixty_one()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWidePlatformUInt\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide platform-uint result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 61);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_platform_uint_array_result_literal_maps_to_member_token_sixty_two()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWidePlatformUIntArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide platform-uint-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 62);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_hyper_result_literal_maps_to_member_token_seventy() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideHyper\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for wide hyper result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 70);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_hyper_array_result_literal_maps_to_member_token_seventy_one()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideHyperArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide hyper-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 71);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_unsigned_hyper_result_literal_maps_to_member_token_seventy_two()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideUnsignedHyper\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide unsigned-hyper result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 72);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_unsigned_hyper_array_result_literal_maps_to_member_token_seventy_three()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideUnsignedHyperArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide unsigned-hyper-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 73);
    }

    #[test]
    fn compile_dispatchinvoke_with_smallint_matrix_result_literal_maps_to_member_token_thirty() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSmallIntMatrix\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for smallint-matrix result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 30);
    }

    #[test]
    fn compile_dispatchinvoke_with_variant_matrix_result_literal_maps_to_member_token_seventy_four()
    {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnVariantMatrix\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for variant-matrix result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 74);
    }

    #[test]
    fn compile_dispatchinvoke_with_plain_unknown_variant_array_result_literal_maps_to_member_token_seventy_five()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlainUnknownVariantArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for plain-unknown-variant-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 75);
    }

    #[test]
    fn compile_dispatchinvoke_with_source_interface_event_literal_maps_to_member_token_eleven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"FireChangedSourceInterface\", 7)\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for source-interface trigger member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 11);
    }

    #[test]
    fn compile_dispatchinvoke_with_quit_literal_maps_to_member_token_ten() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"Quit\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for quit member literal");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 10);
    }

    #[test]
    fn compile_dispatchinvoke_accepts_two_arg_property_get_form() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"Scripting.Dictionary\"), \"Count\")\nEnd Sub";
        let out = compile(source).expect("two-arg DispatchInvoke should compile");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
    }

    #[test]
    fn compile_dispatchinvoke_accepts_multi_arg_form() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SumPair\", 3, 14)\nEnd Sub";
        let out = compile(source).expect("multi-arg DispatchInvoke should compile");
        let invoke = out
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::IntrinsicDispatchInvokeHost { args, .. } => Some(args.clone()),
                _ => None,
            })
            .expect("dispatch invoke instruction should be present");
        assert_eq!(invoke.len(), 2);
        assert!(invoke.iter().all(|arg| arg.name.is_none()));
    }

    #[test]
    fn compile_dispatchinvoke_accepts_named_args_in_assignment_form() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetIndexedValue\", value := 11, lhs := 7)\nEnd Sub";
        let out = compile(source).expect("named DispatchInvoke assignment form should compile");
        let invoke = out
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::IntrinsicDispatchInvokeHost { args, .. } => Some(args.clone()),
                _ => None,
            })
            .expect("dispatch invoke instruction should be present");
        assert_eq!(invoke.len(), 2);
        assert_eq!(invoke[0].name.as_deref(), Some("value"));
        assert_eq!(invoke[1].name.as_deref(), Some("lhs"));
    }

    #[test]
    fn compile_createobject_with_unknown_progid_literal_is_allowed() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"Unknown.Component\")\nEnd Sub";
        let out = compile(source).expect("unknown ProgID literal should remain a runtime concern");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstString { value, .. } if value == "Unknown.Component"))
        );
    }

    #[test]
    fn compile_err_raise_statement_is_supported() {
        let source = "Sub Main()\nOn Error Resume Next\nErr.Raise 7\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::RaiseError { code: 7 }))
        );
    }

    #[test]
    fn compile_property_let_assignment_routes_to_call() {
        let source = "Sub Main()\nDim x\nx = 1\nValue = x\nEnd Sub\nProperty Let Value(ByRef target)\ntarget = target + 2\nEnd Property";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_property_set_assignment_routes_to_call() {
        let source = "Sub Main()\nDim x\nx = 2\nObj = x\nEnd Sub\nProperty Set Obj(ByRef target)\ntarget = target + 5\nEnd Property";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_property_get_procedure_is_accepted() {
        let source =
            "Sub Main()\nDim x\nx = 4\nEnd Sub\nProperty Get Value()\nDim y\ny = 1\nEnd Property";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 4, .. }))
        );
    }

    #[test]
    fn compile_on_error_resume_next_emits_error_state_ops() {
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::SetOnErrorResumeNext))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::RaiseError { code: 5 }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadErrNumber { .. }))
        );
    }

    #[test]
    fn compile_err_member_aliases_are_accepted_under_option_explicit() {
        let source = "Option Explicit\nSub Main()\nDim a\nDim b\nDim c\na = Err.Description\nb = Err.Source\nc = Err.HelpContext\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadErrDescription { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadErrSource { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 0, .. }))
        );
    }

    #[test]
    fn compile_on_error_goto_zero_and_resume_next_emits_ops() {
        let source =
            "Sub Main()\nOn Error Resume Next\nOn Error GoTo 0\nResume Next\nError 3\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::SetOnErrorGoto0))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::ResumeNext))
        );
    }

    #[test]
    fn compile_procedure_boundaries_insert_clearerr_guards() {
        let source = "Sub Main()\nCall Worker\nEnd Sub\nSub Worker()\nDim x\nx = 1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let clear_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::ClearErr))
            .count();
        assert!(
            clear_count >= 4,
            "expected at least main/worker entry+exit clear guards, found {clear_count}"
        );
    }

    #[test]
    fn compile_for_each_subset_is_accepted() {
        let source = "Sub Main()\nDim x\nDim v\nFor Each v In Array(1, 2, 3)\nx = v\nNext\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 3, .. }))
        );
    }

    #[test]
    fn compile_line_number_statement_form_is_supported() {
        let source = "Sub Main()\nDim x\nGoTo 200\n100 x = 1\n200 x = 5\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
    }

    #[test]
    fn compile_property_get_read_routes_to_assign_from_call_subset() {
        let source =
            "Sub Main()\nDim x\nx = Value\nEnd Sub\nProperty Get Value()\nValue = 9\nEnd Property";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_udt_field_access_subset_is_accepted() {
        let source = "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim p As Point\nDim x\np.X = 7\np.Y = p.X\nx = p.Y\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 7, .. }))
        );
    }

    #[test]
    fn compile_udt_whole_assignment_emits_field_copy_slots() {
        let source = "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim a As Point\nDim b As Point\na.X = 7\na.Y = 9\nb = a\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let copy_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::CopySlot { .. }))
            .count();
        assert!(
            copy_count >= 2,
            "expected field copy lowering for UDT assignment"
        );
    }

    #[test]
    fn compile_late_bound_assignment_emits_dispatchinvoke_subset() {
        let source = "Sub Main()\nDim obj As Object\nDim x\nx = obj(7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
    }

    #[test]
    fn compile_declare_function_stub_binding_subset_is_accepted() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicInvokeSymbolHost { .. }))
        );
        assert_eq!(out.external_call_descriptors.len(), 1);
        let descriptor = &out.external_call_descriptors[0];
        assert_eq!(descriptor.declared_name.to_ascii_lowercase(), "hostping");
        assert_eq!(descriptor.library, "host");
        assert_eq!(descriptor.alias, "ping");
        assert_eq!(descriptor.marshal_lane, "m0-deterministic");
        assert_eq!(descriptor.calling_convention, "platform-default");
        assert_eq!(descriptor.selection_policy, "case-insensitive-canonical");
    }

    #[test]
    fn compile_private_declare_function_stub_binding_subset_is_accepted() {
        let source = "Private Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("private declare compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicInvokeSymbolHost { .. }))
        );
        assert_eq!(out.external_call_descriptors.len(), 1);
        let descriptor = &out.external_call_descriptors[0];
        assert_eq!(descriptor.declared_name.to_ascii_lowercase(), "hostping");
        assert_eq!(descriptor.library, "host");
        assert_eq!(descriptor.alias, "ping");
    }

    #[test]
    fn compile_declare_descriptor_table_is_stable_for_identical_source() {
        let source = "Declare PtrSafe Function HostPing Lib \"HOST\" Alias \"PiNg\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out1 = compile(source).expect("first compile should succeed");
        let out2 = compile(source).expect("second compile should succeed");
        assert_eq!(
            out1.external_call_descriptors,
            out2.external_call_descriptors
        );
    }

    #[test]
    fn compile_declare_without_alias_preserves_source_symbol_case() {
        let source = "Declare PtrSafe Function MultiByteToWideChar Lib \"kernel32\" (ByVal x As Long) As Long\nSub Main()\nEnd Sub";
        let out = compile(source).expect("declare without alias should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
        let descriptor = &out.external_call_descriptors[0];
        assert_eq!(
            descriptor.declared_name.to_ascii_lowercase(),
            "multibytetowidechar"
        );
        assert_eq!(descriptor.alias, "MultiByteToWideChar");
    }

    #[test]
    fn compile_same_module_statement_call_without_parentheses_and_without_args_succeeds() {
        let source = "Public Sub Main()\nTestVersion\nEnd Sub\nPublic Sub TestVersion()\nEnd Sub";
        compile(source).expect("same-module no-paren no-arg call should compile");
    }

    #[test]
    fn compile_declare_without_ptrsafe_is_rejected() {
        let source = "Declare Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source).expect_err("non-ptrsafe declare should be rejected");
        assert!(err.to_string().contains("PtrSafe keyword is required"));
    }

    #[test]
    fn compile_declare_with_invalid_ordinal_alias_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"#12a\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source).expect_err("invalid ordinal alias should be rejected");
        assert!(err.to_string().contains("ordinal alias"));
    }

    #[test]
    fn compile_declare_with_ordinal_alias_uses_ordinal_selection_policy() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"#0007\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("ordinal alias subset declaration should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
        let descriptor = &out.external_call_descriptors[0];
        assert!(descriptor.ordinal_alias);
        assert_eq!(descriptor.alias, "#7");
        assert_eq!(descriptor.selection_policy, "ordinal-literal-canonical");
    }

    #[test]
    fn compile_declare_with_multiple_arguments_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long, ByVal y As Long) As Long\nSub Main()\nDim z\nz = HostPing(3, 4)\nEnd Sub";
        let out = compile(source).expect("multi-arg declare should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
        assert_eq!(out.external_call_descriptors[0].param_count, 2);
    }

    #[test]
    fn compile_declare_with_non_long_parameter_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As String) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("non-Long declare param should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
    }

    #[test]
    fn compile_declare_with_variant_parameter_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Variant) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("Variant declare param should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
    }

    #[test]
    fn compile_declare_with_array_parameter_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x() As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source)
            .expect_err("array declare param should be rejected in dynamic-link subset");
        let rendered = err.to_string().to_ascii_lowercase();
        assert!(
            rendered.contains("declare")
                || rendered.contains("external")
                || rendered.contains("array"),
            "unexpected declare-array rejection: {rendered}"
        );
    }

    #[test]
    fn compile_declare_with_non_long_return_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As String\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("non-Long declare return should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn compile_native_declare_uses_native_ffi_lane_on_native_targets() {
        let source = "Declare PtrSafe Function LstrlenW Lib \"kernel32\" Alias \"lstrlenW\" (ByVal lpString As LongPtr) As Long\nSub Main()\nDim y\ny = LstrlenW(0)\nEnd Sub";
        let out = compile(source).expect("native declare should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
        assert_eq!(
            out.external_call_descriptors[0].marshal_lane,
            "m1-native-ffi"
        );
    }

    #[test]
    fn compile_withevents_declaration_in_single_module_subset_succeeds() {
        let source = "Sub Main()\nDim WithEvents app As Object\nEnd Sub";
        compile(source).expect("WithEvents declaration should compile in deterministic subset");
    }

    #[test]
    fn compile_implements_directive_in_single_module_subset_succeeds() {
        let source = "Implements IFoo\nSub Main()\nEnd Sub";
        compile(source).expect("Implements directive should compile in deterministic subset");
    }

    #[test]
    fn compile_raiseevent_statement_in_single_module_subset_succeeds() {
        let source = "Sub Main()\nRaiseEvent Tick\nEnd Sub";
        compile(source).expect("RaiseEvent statement should compile in deterministic subset");
    }

    #[test]
    fn compile_withevents_runtime_binding_intrinsics_emit_deterministically() {
        let source = "Sub Main()\nDim x\nx = __oxvba_withevents_set(0, 7, 42)\nIf __oxvba_withevents_get(0, 7) = 42 Then\nx = 1\nEnd If\nEnd Sub";
        let out = compile(source).expect("WithEvents binding intrinsics should compile");
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsSet { .. })),
            "expected IntrinsicWithEventsSet emission"
        );
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsGet { .. })),
            "expected IntrinsicWithEventsGet emission"
        );
    }

    #[test]
    fn compile_withevents_clear_owner_intrinsic_emits_deterministically() {
        let source = "Sub Main()\nDim x\nx = __oxvba_withevents_clear_owner(11)\nEnd Sub";
        let out = compile(source).expect("WithEvents clear-owner intrinsic should compile");
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsClearOwner { .. })),
            "expected IntrinsicWithEventsClearOwner emission"
        );
    }

    #[test]
    fn compile_withevents_owner_iteration_intrinsics_emit_deterministically() {
        let source = "Sub Main()\nDim x\nx = __oxvba_withevents_first_owner(1, 7)\nx = __oxvba_withevents_next_owner()\nEnd Sub";
        let out = compile(source).expect("WithEvents owner iteration intrinsics should compile");
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsFirstOwner { .. })),
            "expected IntrinsicWithEventsFirstOwner emission"
        );
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsNextOwner { .. })),
            "expected IntrinsicWithEventsNextOwner emission"
        );
    }

    #[test]
    fn compile_com_event_subscription_intrinsics_emit_deterministically() {
        let source = "Sub Main()\nDim x\nDim y\nDim z\nDim w\nx = __oxvba_com_subscribe_event(20001, 1)\ny = __oxvba_com_callback_subscription(60001)\nz = __oxvba_com_callback_arg(60001, 0)\nw = __oxvba_com_release_callback(60001)\ny = __oxvba_com_unsubscribe_event(x)\nEnd Sub";
        let out = compile(source).expect("COM event subscription intrinsics should compile");
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicComSubscribeEventHost { .. })),
            "expected IntrinsicComSubscribeEventHost emission"
        );
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicComUnsubscribeEventHost { .. })),
            "expected IntrinsicComUnsubscribeEventHost emission"
        );
        assert!(
            out.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::IntrinsicComEventCallbackSubscriptionHost { .. }
            )),
            "expected IntrinsicComEventCallbackSubscriptionHost emission"
        );
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicComEventCallbackArgHost { .. })),
            "expected IntrinsicComEventCallbackArgHost emission"
        );
        assert!(
            out.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::IntrinsicComReleaseEventCallbackHost { .. }
            )),
            "expected IntrinsicComReleaseEventCallbackHost emission"
        );
    }
}
