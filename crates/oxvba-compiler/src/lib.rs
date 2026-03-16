//! oxvba-compiler: semantic analysis and bytecode emission scaffolding.

pub mod bytecode;
pub mod emit;
pub mod lower_to_hir;
pub mod optimize;
pub mod project;
pub mod resolve;
pub mod typecheck;

use thiserror::Error;

pub use bytecode::{Bytecode, Instruction};
pub use emit::ProcedureRuntimeMetadata;
pub use project::{
    CompiledProject, ExportKind, HostProcedureExport, ModuleAttributes, ModuleKind, ModuleUnit,
    ProjectCompileError, ProjectDynamicMemberKind, ProjectDynamicMemberRoute,
    ProjectDynamicObjectRoute, ProjectEventDispatchBinding, ProjectKind, ProjectManifest,
    ProjectReference, ReferenceKind, ReferencedProjectManifest, compile_project,
    module_unit_from_source,
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

pub fn compile(source: &str) -> Result<Bytecode, CompileError> {
    compile_with_runtime_metadata(source).map(|(bytecode, _)| bytecode)
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
    if source.trim().is_empty() {
        return Err(CompileError::EmptySource);
    }

    let bound = resolve::resolve_symbols(source);
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
    let _hir = lower_to_hir::lower_to_hir(&optimized);
    Ok(emit::emit_bytecode_with_runtime_metadata(&optimized))
}

#[cfg(test)]
mod tests {
    use super::{Instruction, compile, compile_with_runtime_metadata};
    use crate::bytecode::StringCompareMode;
    use oxvba_runtime::value_tags::ERROR_TAG_BASE;

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
    fn reject_empty_input() {
        assert!(compile(" \n ").is_err());
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
    fn declaration_collision_with_other_procedure_is_rejected() {
        let source = "Sub Main()\nDim helper\nhelper = 1\nEnd Sub\nSub Helper()\nEnd Sub";
        let err = compile(source).expect_err("declaration/procedure collision should fail");
        assert!(
            err.to_string()
                .contains("name collision between variable and procedure")
        );
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
        let source = "Sub Main()\nDim v As Variant\nSet v = CreateObject(4)\nEnd Sub";
        compile(source).expect("Set should allow object-producing call result into Variant target");
    }

    #[test]
    fn set_keyword_accepts_object_target_for_object_call_result() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = CreateObject(4)\nEnd Sub";
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
    fn set_keyword_rejects_scalar_target_for_object_call_result() {
        let source = "Sub Main()\nDim n As Long\nSet n = CreateObject(4)\nEnd Sub";
        let err = compile(source)
            .expect_err("Set should reject object-producing call result for scalar target");
        assert!(
            err.to_string()
                .contains("Set requires Object or Variant target, got Long variable n")
        );
    }

    #[test]
    fn let_keyword_rejects_object_target_for_object_call_result() {
        let source = "Sub Main()\nDim obj As Object\nLet obj = CreateObject(4)\nEnd Sub";
        let err = compile(source)
            .expect_err("Let should reject object-producing call result on Object target");
        assert!(
            err.to_string()
                .contains("Let cannot assign to Object variable obj")
        );
    }

    #[test]
    fn let_keyword_accepts_variant_target_for_object_call_result() {
        let source = "Sub Main()\nDim v As Variant\nLet v = CreateObject(4)\nEnd Sub";
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
    fn let_keyword_rejects_scalar_target_for_object_call_result() {
        let source = "Sub Main()\nDim n As Long\nLet n = CreateObject(4)\nEnd Sub";
        let err = compile(source)
            .expect_err("Let should reject object-producing call result for scalar target");
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn implicit_assignment_accepts_variant_target_for_object_call_result() {
        let source = "Sub Main()\nDim v As Variant\nv = CreateObject(4)\nEnd Sub";
        compile(source).expect(
            "implicit assignment should allow object-producing call result into Variant target",
        );
    }

    #[test]
    fn implicit_assignment_accepts_object_target_for_object_call_result() {
        let source = "Sub Main()\nDim obj As Object\nobj = CreateObject(4)\nEnd Sub";
        compile(source).expect(
            "implicit assignment should allow object-producing call result into Object target",
        );
    }

    #[test]
    fn implicit_assignment_rejects_scalar_target_for_object_call_result() {
        let source = "Sub Main()\nDim n As Long\nn = CreateObject(4)\nEnd Sub";
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
    fn compile_cverr_emits_error_tag_encoding_sequence() {
        let source = "Sub Main()\nDim x\nx = CVErr(7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicAbsI32 { .. }))
        );
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::AddConstI32 { value, .. } if *value == ERROR_TAG_BASE
        )));
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
    fn reject_unsupported_statement() {
        let source = "Sub Main()\nDim x\nx = x * 2\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("unsupported statement"));
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
    fn compile_paramarray_named_args_are_rejected_for_current_subset() {
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
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(11), 2, 3)\nEnd Sub";
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
        let source =
            "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(4), 6, Array(1, 2, 3))\nEnd Sub";
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
    fn compile_createobject_with_progid_literal_maps_to_known_token() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"Scripting.Dictionary\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for known ProgID literal");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCreateObjectHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 4, .. }))
        );
    }

    #[test]
    fn compile_createobject_with_oxvba_test_dispatch_literal_maps_to_known_token() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for known controlled test ProgID");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 4, .. }))
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 1, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 3, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 7, .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 8, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 35, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 36, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 37, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 39, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 41, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 42, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 45, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 46, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 49, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 51, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 53, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 55, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 57, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 63, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 64, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 76, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 77, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 78, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 79, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 80, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 81, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 82, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 83, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 84, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 85, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 86, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 65, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 66, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 67, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 68, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 69, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 20, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 23, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 25, .. }))
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 26, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 27, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 28, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 29, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 31, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 32, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 33, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 34, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 38, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 40, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 43, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 44, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 47, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 48, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 50, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 52, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 54, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 56, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 58, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 59, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 60, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 61, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 62, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 70, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 71, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 72, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 73, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 30, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 74, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 75, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 11, .. }))
        );
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 10, .. }))
        );
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
    fn compile_createobject_with_unknown_progid_literal_is_rejected() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"Unknown.Component\")\nEnd Sub";
        let err =
            compile(source).expect_err("unknown ProgID literal should fail in current subset");
        assert!(!err.to_string().trim().is_empty());
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
                .any(|i| matches!(i, Instruction::LoadErrNumber { .. }))
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
    fn compile_declare_with_multiple_arguments_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long, ByVal y As Long) As Long\nSub Main()\nDim z\nz = HostPing(3, 4)\nEnd Sub";
        let err = compile(source).expect_err("multiple declare args should be rejected");
        assert!(err.to_string().contains("only one argument is supported"));
    }

    #[test]
    fn compile_declare_with_non_long_parameter_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As String) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source).expect_err("non-Long declare param should be rejected");
        assert!(err.to_string().contains("only `Long` parameter type"));
    }

    #[test]
    fn compile_declare_with_variant_parameter_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Variant) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source).expect_err("Variant declare param should be rejected in M0 lane");
        assert!(err.to_string().contains("only `Long` parameter type"));
    }

    #[test]
    fn compile_declare_with_array_parameter_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x() As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source)
            .expect_err("array declare param should be rejected in dynamic-link subset");
        assert!(
            err.to_string()
                .contains("external procedure declaration rejected")
        );
    }

    #[test]
    fn compile_declare_with_non_long_return_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As String\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source).expect_err("non-Long declare return should be rejected");
        assert!(err.to_string().contains("only `Long` return type"));
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
