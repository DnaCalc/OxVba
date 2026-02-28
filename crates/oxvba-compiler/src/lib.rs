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
