//! oxvba-compiler: semantic analysis and bytecode emission scaffolding.

pub mod bytecode;
pub mod emit;
pub mod lower_to_hir;
pub mod optimize;
pub mod resolve;
pub mod typecheck;

use thiserror::Error;

pub use bytecode::{Bytecode, Instruction};

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("empty source")]
    EmptySource,
    #[error("type error: {0}")]
    TypeError(String),
}

pub fn compile(source: &str) -> Result<Bytecode, CompileError> {
    if source.trim().is_empty() {
        return Err(CompileError::EmptySource);
    }

    let bound = resolve::resolve_symbols(source);
    let checked = typecheck::check_types(bound).map_err(CompileError::TypeError)?;
    let optimized = if std::env::var("OXVBA_DISABLE_OPT").ok().as_deref() == Some("1") {
        checked
    } else {
        optimize::optimize_module(checked)
    };
    let _hir = lower_to_hir::lower_to_hir(&optimized);
    Ok(emit::emit_bytecode(&optimized))
}

#[cfg(test)]
mod tests {
    use super::{Instruction, compile};
    use crate::bytecode::StringCompareMode;

    #[test]
    fn compile_simple_module() {
        let out = compile("Sub Main()\nEnd Sub").expect("compile should succeed");
        assert_eq!(out.instructions, vec![Instruction::Halt]);
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
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::AddConstI32 { slot: 0, value: 5 },
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
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::SubConstI32 { slot: 0, value: 3 },
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
                Instruction::LoadConstI32 { slot: 0, value: 1 },
                Instruction::AddConstI32 { slot: 0, value: 2 },
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
    fn compile_conditional_compilation_if_else_branch() {
        let source = "#Const ENABLE = True\nSub Main()\nDim x\n#If ENABLE Then\nx = 7\n#Else\nx = 1\n#End If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::LoadConstI32 { slot: 0, value: 7 },
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
    fn late_bound_object_default_member_call_is_classified_with_explicit_diagnostic() {
        let source = "Sub Main()\nDim obj As Object\nCall obj(1)\nEnd Sub";
        let err = compile(source).expect_err("late-bound target is classified but not executable");
        assert!(
            err.to_string()
                .contains("late-bound default-member call is not yet executable")
        );
    }

    #[test]
    fn late_bound_named_argument_call_is_classified_with_explicit_diagnostic() {
        let source = "Sub Main()\nDim obj As Object\nCall obj(x:=1)\nEnd Sub";
        let err = compile(source)
            .expect_err("late-bound named-arg target is classified but not executable");
        assert!(
            err.to_string()
                .contains("late-bound default-member call is not yet executable")
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
                .any(|i| matches!(i, Instruction::IncSlot { .. }))
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
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value, .. } if *value < 0))
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
}
