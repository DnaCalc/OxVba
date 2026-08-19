//! Unit tests extracted from the former monolithic `lib.rs`.

use super::*;
use oxvba_bundle::{
    BundleImport, ExportToken, ProcedureKind, ProjectMemberKind, StringCompareMode,
};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_oxir::{
    ClassId, GlobalId, ImportId, LocalId, ObjClass, OxBlock, OxClass, OxClassField, OxGlobal,
    OxInst, OxLocal, OxParamInfo, RecordLayoutId, TempId, verify_program,
};

fn straight_line_program() -> OxProgram {
    let n = LocalId(0);
    let t0 = TempId(0);
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(n),
                value: OxOperand::Const(OxConst::I32(10)),
            },
            OxInst::Arith {
                dst: OxPlace::Temp(t0),
                op: ArithOp::Add,
                lhs: OxOperand::Use(OxPlace::Local(n)),
                rhs: OxOperand::Const(OxConst::I32(5)),
                mode: long,
            },
            OxInst::Arith {
                dst: OxPlace::Local(n),
                op: ArithOp::Mul,
                lhs: OxOperand::Use(OxPlace::Temp(t0)),
                rhs: OxOperand::Const(OxConst::I32(2)),
                mode: long,
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let fault = OxBlock {
        id: BlockId(1),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::FaultDispatch {
            resume: BlockId(0),
            resume_next: BlockId(2),
        },
    };
    let exit = OxBlock {
        id: BlockId(2),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![OxLocal {
            name: "n".to_string(),
            ty: OxTy::Long,
            array_element: None,
            param: None,
            escaped: false,
        }],
        temps: vec![OxTy::Long],
        param_count: 0,
        return_local: None,
        blocks: vec![entry, fault, exit],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn byte_arithmetic_program() -> OxProgram {
    let byte = NumericMode::Checked(NumericCoerceTarget::Byte);
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Coerce {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::I32(10)),
                target: OxCoerceTarget::Numeric(NumericCoerceTarget::Byte),
            },
            OxInst::Arith {
                dst: OxPlace::Temp(TempId(0)),
                op: ArithOp::Add,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::I32(5)),
                mode: byte,
            },
            OxInst::Arith {
                dst: OxPlace::Local(LocalId(0)),
                op: ArithOp::Mul,
                lhs: OxOperand::temp(TempId(0)),
                rhs: OxOperand::Const(OxConst::I32(2)),
                mode: byte,
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("n", OxTy::Byte, None)],
        temps: vec![OxTy::Byte],
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn byte_constant_arithmetic_program() -> OxProgram {
    let byte = NumericMode::Checked(NumericCoerceTarget::Byte);
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![OxInst::Arith {
            dst: OxPlace::Local(LocalId(0)),
            op: ArithOp::Sub,
            lhs: OxOperand::Const(OxConst::I32(300)),
            rhs: OxOperand::Const(OxConst::I32(100)),
            mode: byte,
        }],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("n", OxTy::Byte, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn double_arithmetic_program() -> OxProgram {
    let double = NumericMode::Checked(NumericCoerceTarget::Double);
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::F64(1.25f64.to_bits())),
            },
            OxInst::Arith {
                dst: OxPlace::Temp(TempId(0)),
                op: ArithOp::Add,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::F64(2.5f64.to_bits())),
                mode: double,
            },
            OxInst::Arith {
                dst: OxPlace::Local(LocalId(0)),
                op: ArithOp::Mul,
                lhs: OxOperand::temp(TempId(0)),
                rhs: OxOperand::Const(OxConst::F64(2.0f64.to_bits())),
                mode: double,
            },
            OxInst::Arith {
                dst: OxPlace::Local(LocalId(0)),
                op: ArithOp::Sub,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::F64(1.0f64.to_bits())),
                mode: double,
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("d", OxTy::Double, None)],
        temps: vec![OxTy::Double],
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn double_div_pow_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::F64(9.0f64.to_bits())),
            },
            OxInst::Div {
                dst: OxPlace::Global(GlobalId(0)),
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::F64(2.0f64.to_bits())),
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::F64(3.0f64.to_bits())),
            },
            OxInst::Pow {
                dst: OxPlace::Global(GlobalId(1)),
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::F64(4.0f64.to_bits())),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("d", OxTy::Double, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![
            OxGlobal {
                name: "g_div".to_string(),
                ty: OxTy::Double,
                array_element: None,
            },
            OxGlobal {
                name: "g_pow".to_string(),
                ty: OxTy::Double,
                array_element: None,
            },
        ],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn scalar_negation_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::I32(7)),
            },
            OxInst::Neg {
                dst: OxPlace::Global(GlobalId(0)),
                src: OxOperand::local(LocalId(0)),
                mode: NumericMode::Checked(NumericCoerceTarget::Long),
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(1)),
                value: OxOperand::Const(OxConst::I64(10)),
            },
            OxInst::Neg {
                dst: OxPlace::Global(GlobalId(1)),
                src: OxOperand::local(LocalId(1)),
                mode: NumericMode::Checked(NumericCoerceTarget::LongLong),
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(2)),
                value: OxOperand::Const(OxConst::Currency(1234)),
            },
            OxInst::Neg {
                dst: OxPlace::Global(GlobalId(2)),
                src: OxOperand::local(LocalId(2)),
                mode: NumericMode::Checked(NumericCoerceTarget::Currency),
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(3)),
                value: OxOperand::Const(OxConst::F32(1.25f32.to_bits())),
            },
            OxInst::Neg {
                dst: OxPlace::Global(GlobalId(3)),
                src: OxOperand::local(LocalId(3)),
                mode: NumericMode::Checked(NumericCoerceTarget::Single),
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(4)),
                value: OxOperand::Const(OxConst::F64(2.5f64.to_bits())),
            },
            OxInst::Neg {
                dst: OxPlace::Global(GlobalId(4)),
                src: OxOperand::local(LocalId(4)),
                mode: NumericMode::Checked(NumericCoerceTarget::Double),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("l", OxTy::Long, None),
            local("ll", OxTy::LongLong, None),
            local("c", OxTy::Currency, None),
            local("s", OxTy::Single, None),
            local("d", OxTy::Double, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![
            OxGlobal {
                name: "g_l".to_string(),
                ty: OxTy::Long,
                array_element: None,
            },
            OxGlobal {
                name: "g_ll".to_string(),
                ty: OxTy::LongLong,
                array_element: None,
            },
            OxGlobal {
                name: "g_c".to_string(),
                ty: OxTy::Currency,
                array_element: None,
            },
            OxGlobal {
                name: "g_s".to_string(),
                ty: OxTy::Single,
                array_element: None,
            },
            OxGlobal {
                name: "g_d".to_string(),
                ty: OxTy::Double,
                array_element: None,
            },
        ],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn integer_div_rem_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::I32(17)),
            },
            OxInst::Arith {
                dst: OxPlace::Global(GlobalId(0)),
                op: ArithOp::IntDiv,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::I32(5)),
                mode: NumericMode::Checked(NumericCoerceTarget::Long),
            },
            OxInst::Arith {
                dst: OxPlace::Global(GlobalId(1)),
                op: ArithOp::Mod,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::I32(5)),
                mode: NumericMode::Checked(NumericCoerceTarget::Long),
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(1)),
                value: OxOperand::Const(OxConst::I64(5_000_000_017)),
            },
            OxInst::Arith {
                dst: OxPlace::Global(GlobalId(2)),
                op: ArithOp::IntDiv,
                lhs: OxOperand::local(LocalId(1)),
                rhs: OxOperand::Const(OxConst::I64(5)),
                mode: NumericMode::Checked(NumericCoerceTarget::LongLong),
            },
            OxInst::Arith {
                dst: OxPlace::Global(GlobalId(3)),
                op: ArithOp::Mod,
                lhs: OxOperand::local(LocalId(1)),
                rhs: OxOperand::Const(OxConst::I64(5)),
                mode: NumericMode::Checked(NumericCoerceTarget::LongLong),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("l", OxTy::Long, None),
            local("ll", OxTy::LongLong, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![
            OxGlobal {
                name: "g_l_div".to_string(),
                ty: OxTy::Long,
                array_element: None,
            },
            OxGlobal {
                name: "g_l_rem".to_string(),
                ty: OxTy::Long,
                array_element: None,
            },
            OxGlobal {
                name: "g_ll_div".to_string(),
                ty: OxTy::LongLong,
                array_element: None,
            },
            OxGlobal {
                name: "g_ll_rem".to_string(),
                ty: OxTy::LongLong,
                array_element: None,
            },
        ],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_widening_double_arithmetic_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::F64(1.25f64.to_bits())),
            },
            OxInst::Arith {
                dst: OxPlace::Temp(TempId(0)),
                op: ArithOp::Add,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::F64(2.5f64.to_bits())),
                mode: NumericMode::Widening,
            },
            OxInst::Coerce {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::temp(TempId(0)),
                target: OxCoerceTarget::Numeric(NumericCoerceTarget::Double),
            },
            OxInst::Arith {
                dst: OxPlace::Temp(TempId(0)),
                op: ArithOp::Mul,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::F64(2.0f64.to_bits())),
                mode: NumericMode::Widening,
            },
            OxInst::Coerce {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::temp(TempId(0)),
                target: OxCoerceTarget::Numeric(NumericCoerceTarget::Double),
            },
            OxInst::Arith {
                dst: OxPlace::Temp(TempId(0)),
                op: ArithOp::Sub,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::F64(1.0f64.to_bits())),
                mode: NumericMode::Widening,
            },
            OxInst::Coerce {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::temp(TempId(0)),
                target: OxCoerceTarget::Numeric(NumericCoerceTarget::Double),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("d", OxTy::Double, None)],
        temps: vec![OxTy::Variant],
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_assignment_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::Empty),
            },
            OxInst::Assign {
                dst: OxPlace::Global(GlobalId(0)),
                value: OxOperand::local(LocalId(0)),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("v", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Variant,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_box_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![OxInst::Box {
            dst: OxPlace::Global(GlobalId(0)),
            src: OxOperand::Const(OxConst::I16(42)),
            from: OxTy::Integer,
        }],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Variant,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_unbox_long_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Box {
                dst: OxPlace::Temp(TempId(0)),
                src: OxOperand::Const(OxConst::I32(42)),
                from: OxTy::Long,
            },
            OxInst::Unbox {
                dst: OxPlace::Global(GlobalId(0)),
                src: OxOperand::temp(TempId(0)),
                to: OxTy::Long,
                checked: true,
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: vec![OxTy::Variant],
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_unbox_type_mismatch_program() -> OxProgram {
    let mut program = variant_unbox_long_program();
    if let OxInst::Box { src, from, .. } = &mut program.funcs[0].blocks[0].instrs[0] {
        *src = OxOperand::Const(OxConst::I16(42));
        *from = OxTy::Integer;
    }
    program
}

fn variant_logical_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::Null),
            },
            OxInst::Logical {
                dst: OxPlace::Temp(TempId(0)),
                op: LogicalOp::And,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::Bool(false)),
            },
            OxInst::Not {
                dst: OxPlace::Global(GlobalId(0)),
                src: OxOperand::temp(TempId(0)),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("v", OxTy::Variant, None)],
        temps: vec![OxTy::Variant],
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Variant,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_truthy_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::Null),
            },
            OxInst::Truthy {
                dst: OxPlace::Temp(TempId(0)),
                src: OxOperand::local(LocalId(0)),
            },
        ],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Branch {
            cond: OxOperand::temp(TempId(0)),
            then_blk: BlockId(1),
            else_blk: BlockId(2),
        },
    };
    let then_blk = OxBlock {
        id: BlockId(1),
        instrs: vec![OxInst::Assign {
            dst: OxPlace::Global(GlobalId(0)),
            value: OxOperand::Const(OxConst::I32(1)),
        }],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Return,
    };
    let else_blk = OxBlock {
        id: BlockId(2),
        instrs: vec![OxInst::Assign {
            dst: OxPlace::Global(GlobalId(0)),
            value: OxOperand::Const(OxConst::I32(2)),
        }],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Return,
    };
    let fault = OxBlock {
        id: BlockId(3),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::FaultDispatch {
            resume: BlockId(0),
            resume_next: BlockId(4),
        },
    };
    let exit = OxBlock {
        id: BlockId(4),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("v", OxTy::Variant, None)],
        temps: vec![OxTy::Bool],
        param_count: 0,
        return_local: None,
        blocks: vec![entry, then_blk, else_blk, fault, exit],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_compare_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::Null),
            },
            OxInst::Compare {
                dst: OxPlace::Global(GlobalId(0)),
                op: CmpOp::Eq,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::I32(1)),
                mode: StringCompareMode::Binary,
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("v", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Variant,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_changed_program() -> OxProgram {
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Box {
                dst: OxPlace::Temp(TempId(0)),
                src: OxOperand::Const(OxConst::I32(1)),
                from: OxTy::Long,
            },
            OxInst::Box {
                dst: OxPlace::Temp(TempId(1)),
                src: OxOperand::Const(OxConst::I32(2)),
                from: OxTy::Long,
            },
            OxInst::VariantChanged {
                dst: OxPlace::Global(GlobalId(0)),
                current: OxOperand::temp(TempId(0)),
                original: OxOperand::temp(TempId(1)),
            },
            OxInst::VariantChanged {
                dst: OxPlace::Global(GlobalId(1)),
                current: OxOperand::temp(TempId(0)),
                original: OxOperand::temp(TempId(0)),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: vec![OxTy::Variant, OxTy::Variant],
        param_count: 0,
        return_local: None,
        blocks: vec![
            entry,
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![
            OxGlobal {
                name: "g_changed".to_string(),
                ty: OxTy::Bool,
                array_element: None,
            },
            OxGlobal {
                name: "g_same".to_string(),
                ty: OxTy::Bool,
                array_element: None,
            },
        ],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn paramarray_no_alias_call_program() -> OxProgram {
    let packed_array_ty = OxTy::Array(Box::new(OxTy::Variant), ArrayShape::Dynamic);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: vec![packed_array_ty.clone()],
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::ArrayLiteral {
                        dst: OxPlace::Temp(TempId(0)),
                        values: vec![
                            OxOperand::Const(OxConst::I32(10)),
                            OxOperand::Const(OxConst::Str("alpha".to_string())),
                            OxOperand::Const(OxConst::Bool(true)),
                        ],
                        aliases: vec![None, None, None],
                        lower_bound: 0,
                    },
                    OxInst::CallProc {
                        dst: None,
                        proc: FuncId(1),
                        args: vec![OxArg::ByVal(OxOperand::temp(TempId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    let store_items = OxFunc {
        name: "StoreItems".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "items",
            packed_array_ty.clone(),
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: true,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Global(GlobalId(0)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, store_items],
        globals: vec![OxGlobal {
            name: "stored".to_string(),
            ty: packed_array_ty,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn paramarray_scalar_arg_decline_program() -> OxProgram {
    let mut program = paramarray_no_alias_call_program();
    program.funcs[0].temps.clear();
    program.funcs[0].blocks[0].instrs = vec![OxInst::CallProc {
        dst: None,
        proc: FuncId(1),
        args: vec![OxArg::ByVal(OxOperand::Const(OxConst::I32(10)))],
    }];
    program
}

fn paramarray_bounds_call_program(values: Vec<OxOperand>) -> OxProgram {
    let packed_array_ty = OxTy::Array(Box::new(OxTy::Variant), ArrayShape::Dynamic);
    let aliases = vec![None; values.len()];
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: vec![packed_array_ty.clone()],
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::ArrayLiteral {
                        dst: OxPlace::Temp(TempId(0)),
                        values,
                        aliases,
                        lower_bound: 0,
                    },
                    OxInst::CallProc {
                        dst: None,
                        proc: FuncId(1),
                        args: vec![OxArg::ByVal(OxOperand::temp(TempId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    let inspect_items = OxFunc {
        name: "InspectItems".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "items",
            packed_array_ty,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: true,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Bound {
                        dst: OxPlace::Global(GlobalId(0)),
                        which: BoundWhich::Lower,
                        array: OxOperand::local(LocalId(0)),
                        dimension: None,
                    },
                    OxInst::Bound {
                        dst: OxPlace::Global(GlobalId(1)),
                        which: BoundWhich::Upper,
                        array: OxOperand::local(LocalId(0)),
                        dimension: None,
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, inspect_items],
        globals: vec![
            OxGlobal {
                name: "lower".to_string(),
                ty: OxTy::Long,
                array_element: None,
            },
            OxGlobal {
                name: "upper".to_string(),
                ty: OxTy::Long,
                array_element: None,
            },
        ],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_array_bounds_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("items", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::ArrayLiteral {
                        dst: OxPlace::Local(LocalId(0)),
                        values: vec![
                            OxOperand::Const(OxConst::I32(10)),
                            OxOperand::Const(OxConst::Str("alpha".to_string())),
                            OxOperand::Const(OxConst::Bool(true)),
                        ],
                        aliases: vec![None, None, None],
                        lower_bound: 0,
                    },
                    OxInst::Bound {
                        dst: OxPlace::Global(GlobalId(0)),
                        which: BoundWhich::Lower,
                        array: OxOperand::local(LocalId(0)),
                        dimension: None,
                    },
                    OxInst::Bound {
                        dst: OxPlace::Global(GlobalId(1)),
                        which: BoundWhich::Upper,
                        array: OxOperand::local(LocalId(0)),
                        dimension: None,
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![
            OxGlobal {
                name: "lower".to_string(),
                ty: OxTy::Long,
                array_element: None,
            },
            OxGlobal {
                name: "upper".to_string(),
                ty: OxTy::Long,
                array_element: None,
            },
        ],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn fixed_variant_array_store_load_program() -> OxProgram {
    let array_ty = OxTy::Array(Box::new(OxTy::Variant), ArrayShape::Fixed { rank: 1 });
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("items", array_ty, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::ArrayRedim {
                        dst: OxPlace::Local(LocalId(0)),
                        upper_bounds: vec![OxOperand::Const(OxConst::I32(2))],
                        lower_bounds: Vec::new(),
                        element: ArrayElementType::Variant,
                        preserve: false,
                        fixed: true,
                    },
                    OxInst::ArraySet {
                        array: OxPlace::Local(LocalId(0)),
                        indices: vec![OxOperand::Const(OxConst::I32(0))],
                        value: OxOperand::Const(OxConst::I16(5)),
                    },
                    OxInst::ArraySet {
                        array: OxPlace::Local(LocalId(0)),
                        indices: vec![OxOperand::Const(OxConst::I32(1))],
                        value: OxOperand::Const(OxConst::I16(7)),
                    },
                    OxInst::ArrayGet {
                        dst: OxPlace::Global(GlobalId(0)),
                        array: OxOperand::local(LocalId(0)),
                        indices: vec![OxOperand::Const(OxConst::I32(1))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Variant,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn variant_bound_non_array_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("items", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Bound {
                    dst: OxPlace::Global(GlobalId(0)),
                    which: BoundWhich::Upper,
                    array: OxOperand::local(LocalId(0)),
                    dimension: None,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "upper".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn dynamic_variant_array_bound_unallocated_program() -> OxProgram {
    let array_ty = OxTy::Array(Box::new(OxTy::Variant), ArrayShape::Dynamic);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("items", array_ty, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Bound {
                    dst: OxPlace::Global(GlobalId(0)),
                    which: BoundWhich::Upper,
                    array: OxOperand::local(LocalId(0)),
                    dimension: None,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        globals: vec![OxGlobal {
            name: "upper".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

#[test]
fn jit_erl_seats_numeric_line_and_err_number_write() {
    let err_number = LocalId(0);
    let erl = LocalId(1);
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::SetErrorHandler(ErrorHandler::ResumeNext),
            OxInst::SetLineNumber { line: 10 },
            OxInst::Arith {
                dst: OxPlace::Local(err_number),
                op: ArithOp::IntDiv,
                lhs: OxOperand::Const(OxConst::I32(1)),
                rhs: OxOperand::Const(OxConst::I32(0)),
                mode: NumericMode::Checked(NumericCoerceTarget::Long),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Jump(BlockId(2)),
    };
    let fault = OxBlock {
        id: BlockId(1),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::FaultDispatch {
            resume: BlockId(0),
            resume_next: BlockId(2),
        },
    };
    let exit = OxBlock {
        id: BlockId(2),
        instrs: vec![
            OxInst::ErrFieldGet {
                dst: OxPlace::Local(err_number),
                field: ErrField::Number,
            },
            OxInst::ErlGet {
                dst: OxPlace::Local(erl),
            },
        ],
        fault_target: None,
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            OxLocal {
                name: "errNumber".to_string(),
                ty: OxTy::Long,
                array_element: None,
                param: None,
                escaped: false,
            },
            OxLocal {
                name: "erl".to_string(),
                ty: OxTy::Long,
                array_element: None,
                param: None,
                escaped: false,
            },
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![entry, fault, exit],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![main],
        globals: Vec::new(),
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };
    assert_eq!(verify_program(&program), Ok(()));
    let compiled = JitEngine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.err.number, 11);
    assert_eq!(
        outcome.values.first().and_then(Variant::as_i32),
        Some(11),
        "division-by-zero Err.Number"
    );
    assert_eq!(
        outcome.values.get(1).and_then(Variant::as_i32),
        Some(10),
        "Erl seats the active numeric line"
    );
}

#[test]
fn jit_compiles_and_runs_straight_line_long_arithmetic() {
    let program = straight_line_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(30));
}

#[test]
fn jit_compiles_and_runs_direct_byte_arithmetic() {
    let program = byte_arithmetic_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_u8), Some(30));
}

#[test]
fn jit_byte_arithmetic_narrows_result_after_operation() {
    let program = byte_constant_arithmetic_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_u8), Some(200));
}

#[test]
fn jit_lowers_direct_double_arithmetic() {
    let program = double_arithmetic_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_f64), Some(6.5));
}

#[test]
fn jit_lowers_direct_double_division_and_power() {
    let program = double_div_pow_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_f64), Some(4.5));
    assert_eq!(outcome.values.get(1).and_then(Variant::as_f64), Some(81.0));
}

#[test]
fn jit_lowers_direct_scalar_negation() {
    let program = scalar_negation_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(-7));
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i64), Some(-10));
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(Variant::as_currency_scaled_i64),
        Some(-1234)
    );
    assert_eq!(outcome.values.get(3).and_then(Variant::as_f32), Some(-1.25));
    assert_eq!(outcome.values.get(4).and_then(Variant::as_f64), Some(-2.5));
}

#[test]
fn jit_lowers_direct_integer_division_remainder() {
    let program = integer_div_rem_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(3));
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(2));
    assert_eq!(
        outcome.values.get(2).and_then(Variant::as_i64),
        Some(1_000_000_003)
    );
    assert_eq!(outcome.values.get(3).and_then(Variant::as_i64), Some(2));
}

#[test]
fn jit_lowers_variant_widening_double_arithmetic() {
    let program = variant_widening_double_arithmetic_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_f64), Some(6.5));
}

#[test]
fn jit_lowers_variant_assignment_to_slot() {
    let program = variant_assignment_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    let empty = Variant::empty();
    assert_eq!(outcome.values.first(), Some(&empty));
    assert_eq!(outcome.values.get(1), Some(&empty));
}

#[test]
fn jit_lowers_box_to_variant_slot() {
    let program = variant_box_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i16), Some(42));
}

#[test]
fn jit_lowers_checked_unbox_to_typed_slot() {
    let program = variant_unbox_long_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
}

#[test]
fn jit_checked_unbox_type_mismatch_seats_error_13() {
    let program = variant_unbox_type_mismatch_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(outcome.raised);
    assert_eq!(outcome.err.number, 13);
    assert_eq!(outcome.err.description, "Type mismatch");
}

#[test]
fn jit_lowers_variant_logical_and_not_to_slot() {
    let program = variant_logical_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(
        outcome.values.first().and_then(Variant::as_bool),
        Some(true)
    );
}

#[test]
fn jit_lowers_variant_truthy_to_bool_slot() {
    let program = variant_truthy_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(2));
}

#[test]
fn jit_lowers_variant_compare_to_slot() {
    let program = variant_compare_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(
        outcome.values.first().map(Variant::vtype),
        Some(VarType::Null)
    );
}

#[test]
fn jit_lowers_variant_changed_to_bool_slot() {
    let program = variant_changed_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(
        outcome.values.first().and_then(Variant::as_bool),
        Some(true)
    );
    assert_eq!(
        outcome.values.get(1).and_then(Variant::as_bool),
        Some(false)
    );
}

#[test]
fn jit_lowers_no_alias_paramarray_pack_and_static_call_copy_in() {
    let program = paramarray_no_alias_call_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    let stored = outcome.values.first().expect("stored ParamArray global");
    let (bounds, len) = stored
        .safearray_bounds_len()
        .expect("ParamArray should be stored as a Variant SAFEARRAY");
    assert_eq!(bounds, vec![SafeArrayBound { count: 3, lower: 0 }]);
    assert_eq!(len, 3);
    assert_eq!(
        stored.safearray_element(0).transpose().expect("element 0"),
        Some(Variant::from_i32(10))
    );
    assert_eq!(
        stored
            .safearray_element(1)
            .transpose()
            .expect("element 1")
            .and_then(|value| value.as_bstr().map(|text| text.as_str().to_owned())),
        Some("alpha".to_string())
    );
    assert_eq!(
        stored
            .safearray_element(2)
            .transpose()
            .expect("element 2")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
}

#[test]
fn jit_declines_paramarray_without_packed_array_carrier() {
    let program = paramarray_scalar_arg_decline_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let err = match engine.compile_image(&[&program]) {
        Ok(_) => panic!("compile should decline"),
        Err(err) => err,
    };
    assert!(
        err.unsupported_message()
            .is_some_and(|message| message.contains("packed dynamic Variant-array carrier")),
        "{err:?}"
    );
}

#[test]
fn jit_lowers_paramarray_bounds_after_static_call_copy_in() {
    let program = paramarray_bounds_call_program(vec![
        OxOperand::Const(OxConst::I32(10)),
        OxOperand::Const(OxConst::Str("alpha".to_string())),
        OxOperand::Const(OxConst::Bool(true)),
    ]);
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(0));
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(2));
}

#[test]
fn jit_lowers_empty_paramarray_bounds_after_static_call_copy_in() {
    let program = paramarray_bounds_call_program(Vec::new());
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(0));
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(-1));
}

#[test]
fn jit_lowers_variant_array_bounds() {
    let program = variant_array_bounds_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(0));
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(2));
}

#[test]
fn jit_lowers_fixed_variant_array_redim_get_set() {
    let program = fixed_variant_array_store_load_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.first().and_then(Variant::as_i16), Some(7));
}

#[test]
fn jit_variant_bound_non_array_raises_type_mismatch() {
    let program = variant_bound_non_array_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(outcome.raised);
    assert_eq!(outcome.err.number, 13);
    assert_eq!(outcome.err.description, "expected an array");
}

#[test]
fn jit_dynamic_variant_array_bound_unallocated_raises_subscript_error() {
    let program = dynamic_variant_array_bound_unallocated_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(outcome.raised);
    assert_eq!(outcome.err.number, 9);
    assert_eq!(outcome.err.description, "array has no bounds");
}

#[test]
fn jit_direct_byte_arithmetic_overflow_seats_error_6() {
    let mut program = byte_arithmetic_program();
    if let OxInst::Coerce { src, .. } = &mut program.funcs[0].blocks[0].instrs[0] {
        *src = OxOperand::Const(OxConst::I32(255));
    }
    program.funcs[0].blocks[0].instrs.truncate(2);
    if let OxInst::Arith { dst, rhs, .. } = &mut program.funcs[0].blocks[0].instrs[1] {
        *dst = OxPlace::Local(LocalId(0));
        *rhs = OxOperand::Const(OxConst::I32(1));
    }
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(outcome.raised);
    assert_eq!(outcome.err.number, 6);
}

#[test]
fn jit_overflow_raises_through_rt_abi_shim() {
    let mut program = straight_line_program();
    if let OxInst::Assign { value, .. } = &mut program.funcs[0].blocks[0].instrs[0] {
        *value = OxOperand::Const(OxConst::I32(i32::MAX));
    }
    if let OxInst::Arith { rhs, .. } = &mut program.funcs[0].blocks[0].instrs[1] {
        *rhs = OxOperand::Const(OxConst::I32(1));
    }
    program.funcs[0].blocks[0].instrs.truncate(2);
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(outcome.raised);
    assert_eq!(outcome.err.number, 6);
}

#[test]
fn jit_initializer_fault_status_is_observed() {
    let mut program = straight_line_program();
    program.global_initializer = Some(FuncId(0));
    program.entry = None;
    if let OxInst::Assign { value, .. } = &mut program.funcs[0].blocks[0].instrs[0] {
        *value = OxOperand::Const(OxConst::I32(i32::MAX));
    }
    if let OxInst::Arith { rhs, .. } = &mut program.funcs[0].blocks[0].instrs[1] {
        *rhs = OxOperand::Const(OxConst::I32(1));
    }
    program.funcs[0].blocks[0].instrs.truncate(2);
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(outcome.raised);
    assert_eq!(outcome.err.number, 6);
}

#[test]
fn jit_call_helper_seats_out_of_stack_at_vm3_frame_ceiling() {
    unsafe extern "C" fn unreachable_entry(_run: *mut JitRun, _state: *mut RawExecState) -> i32 {
        panic!("frame-depth guard must fire before invoking the callee")
    }

    let program = straight_line_program();
    let mut functions: Vec<JitEntryFn> = vec![unreachable_entry];
    let mut globals = Vec::new();
    let mut globals_table = vec![&mut globals as *mut Vec<Variant>];
    let program_images = [JitProgramImage {
        program: &program,
        functions: functions.as_mut_ptr(),
        function_count: functions.len(),
    }];
    let mut run = JitRun {
        globals: globals_table.as_mut_ptr(),
        global_count: globals_table.len(),
        frames: (0..MAX_JIT_FRAMES)
            .map(|_| new_jit_frame(&program, 0, &program.funcs[0]).expect("frame"))
            .collect(),
        explicit_refs: Vec::new(),
        for_each: HashMap::new(),
        as_new_slots: HashMap::new(),
        param_array_aliases: HashMap::new(),
        next_collection_instance_id: i32::MIN + 1,
        programs: program_images.as_ptr(),
        program_count: program_images.len(),
    };
    let host = NullHostServices::new(HostPolicy::default());
    let mut exec = ExecState::new(&host);
    exec.programs = vec![build_loaded(&program).expect("loaded")];
    let state = exec_state_as_raw(&mut exec);

    // SAFETY: `run` and `exec` are owned test locals; `state` was derived from
    // the unique live `exec`, and the zero-argument call permits a null args pointer.
    let status = unsafe { rt_jit_call_proc_i32(&mut run, state, 0, 0, std::ptr::null(), -1, -1) };
    assert_eq!(status, ST_FAULT);
    assert_eq!(exec.err_engine.err.number, 28);
    assert_eq!(exec.err_engine.err.description, "Out of stack space");
}

#[test]
fn jit_compiled_static_call_seats_out_of_stack_at_vm3_frame_ceiling() {
    let program = static_call_frame_guard_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let mut exec = ExecState::new(&host);
    exec.programs = vec![build_loaded(&program).expect("loaded")];
    let mut globals_table = vec![&mut exec.programs[0].globals as *mut Vec<Variant>];
    let functions = &compiled.functions[0];
    let program_images = [JitProgramImage {
        program: &program,
        functions: functions.as_ptr(),
        function_count: functions.len(),
    }];
    let mut run = JitRun {
        globals: globals_table.as_mut_ptr(),
        global_count: globals_table.len(),
        frames: (0..MAX_JIT_FRAMES)
            .map(|_| new_jit_frame(&program, 0, &program.funcs[0]).expect("frame"))
            .collect(),
        explicit_refs: Vec::new(),
        for_each: HashMap::new(),
        as_new_slots: HashMap::new(),
        param_array_aliases: HashMap::new(),
        next_collection_instance_id: i32::MIN + 1,
        programs: program_images.as_ptr(),
        program_count: program_images.len(),
    };
    let state = exec_state_as_raw(&mut exec);
    let entry = compiled.functions[0][0];

    // SAFETY: the entry pointer was compiled for `JitEntryFn`; owned `run` and
    // the uniquely borrowed execution state remain live for the synchronous call.
    let status = unsafe { entry(&mut run, state) };
    assert_eq!(status, ST_FAULT);
    assert_eq!(exec.err_engine.err.number, 28);
    assert_eq!(exec.err_engine.err.description, "Out of stack space");
    assert_eq!(exec.programs[0].globals[0].as_i32(), Some(0));
}

#[test]
fn jit_direct_one_i32_function_call_copies_return() {
    let program = direct_one_i32_function_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
}

#[test]
fn jit_direct_ignored_noarg_function_call_preserves_side_effect() {
    let program = direct_ignored_noarg_function_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(17));
}

#[test]
fn jit_direct_ignored_one_i32_function_call_preserves_side_effect() {
    let program = direct_ignored_one_i32_function_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
}

#[test]
fn jit_direct_one_i32_byref_sub_call_mutates_caller() {
    let program = direct_one_i32_byref_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
}

#[test]
fn jit_direct_one_i32_byref_function_call_mutates_and_copies_return() {
    let program = direct_one_i32_byref_function_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(41));
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
}

#[test]
fn jit_direct_ignored_one_i32_byref_function_call_mutates_caller() {
    let program = direct_ignored_one_i32_byref_function_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
}

#[test]
fn jit_direct_descriptor_string_byval_sub_call_preserves_side_effect() {
    let program = direct_descriptor_string_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(
        outcome
            .values
            .first()
            .and_then(|value| value.as_bstr().map(|text| text.as_str().to_owned())),
        Some("alpha".to_string())
    );
}

#[test]
fn jit_direct_two_i32_sub_call_preserves_side_effect() {
    let program = direct_two_i32_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
}

#[test]
fn jit_direct_two_i32_function_call_copies_return() {
    let program = direct_two_i32_function_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
}

#[test]
fn jit_direct_ignored_two_i32_function_call_preserves_side_effect() {
    let program = direct_ignored_two_i32_function_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
}

fn static_call_frame_guard_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProc {
                    dst: None,
                    proc: FuncId(1),
                    args: Vec::new(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    let callee = OxFunc {
        name: "ShouldNotRun".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Global(GlobalId(0)),
                    value: OxOperand::Const(OxConst::I32(99)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(1),
                },
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, callee],
        globals: vec![OxGlobal {
            name: "ran".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn direct_one_i32_function_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProc {
                    dst: Some(OxPlace::Global(GlobalId(0))),
                    proc: FuncId(1),
                    args: vec![OxArg::ByVal(OxOperand::Const(OxConst::I32(41)))],
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let callee = OxFunc {
        name: "AddOne".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "x",
                OxTy::Long,
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("AddOne", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Arith {
                    dst: OxPlace::Local(LocalId(1)),
                    op: ArithOp::Add,
                    lhs: OxOperand::local(LocalId(0)),
                    rhs: OxOperand::Const(OxConst::I32(1)),
                    mode: long,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, callee],
        globals: vec![OxGlobal {
            name: "result".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn direct_ignored_noarg_function_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProc {
                    dst: None,
                    proc: FuncId(1),
                    args: Vec::new(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let callee = OxFunc {
        name: "Touch".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("Touch", OxTy::Long, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Global(GlobalId(0)),
                        value: OxOperand::Const(OxConst::I32(17)),
                    },
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::I32(99)),
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, callee],
        globals: vec![OxGlobal {
            name: "result".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn direct_ignored_one_i32_function_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProc {
                    dst: None,
                    proc: FuncId(1),
                    args: vec![OxArg::ByVal(OxOperand::Const(OxConst::I32(41)))],
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let callee = OxFunc {
        name: "TouchArg".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "x",
                OxTy::Long,
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("TouchArg", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Arith {
                        dst: OxPlace::Global(GlobalId(0)),
                        op: ArithOp::Add,
                        lhs: OxOperand::local(LocalId(0)),
                        rhs: OxOperand::Const(OxConst::I32(1)),
                        mode: long,
                    },
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(1)),
                        value: OxOperand::Const(OxConst::I32(99)),
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, callee],
        globals: vec![OxGlobal {
            name: "result".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn direct_one_i32_byref_sub_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = direct_one_i32_byref_main(None);
    let callee = OxFunc {
        name: "Mutate".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![direct_one_i32_byref_param()],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: direct_one_i32_byref_blocks(vec![OxInst::Arith {
            dst: OxPlace::Local(LocalId(0)),
            op: ArithOp::Add,
            lhs: OxOperand::local(LocalId(0)),
            rhs: OxOperand::Const(OxConst::I32(2)),
            mode: long,
        }]),
        entry: BlockId(0),
    };
    direct_one_i32_byref_program(vec![long_global("value")], main, callee)
}

fn direct_one_i32_byref_function_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = direct_one_i32_byref_main(Some(OxPlace::Global(GlobalId(1))));
    let callee = OxFunc {
        name: "MutateAndReturn".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            direct_one_i32_byref_param(),
            local("MutateAndReturn", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: direct_one_i32_byref_blocks(vec![
            OxInst::Arith {
                dst: OxPlace::Local(LocalId(0)),
                op: ArithOp::Add,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::I32(1)),
                mode: long,
            },
            OxInst::Arith {
                dst: OxPlace::Local(LocalId(1)),
                op: ArithOp::Add,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::I32(1)),
                mode: long,
            },
        ]),
        entry: BlockId(0),
    };
    direct_one_i32_byref_program(
        vec![long_global("value"), long_global("result")],
        main,
        callee,
    )
}

fn direct_ignored_one_i32_byref_function_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = direct_one_i32_byref_main(None);
    let callee = OxFunc {
        name: "MutateIgnored".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            direct_one_i32_byref_param(),
            local("MutateIgnored", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: direct_one_i32_byref_blocks(vec![
            OxInst::Arith {
                dst: OxPlace::Local(LocalId(0)),
                op: ArithOp::Add,
                lhs: OxOperand::local(LocalId(0)),
                rhs: OxOperand::Const(OxConst::I32(2)),
                mode: long,
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(1)),
                value: OxOperand::Const(OxConst::I32(99)),
            },
        ]),
        entry: BlockId(0),
    };
    direct_one_i32_byref_program(vec![long_global("value")], main, callee)
}

fn direct_one_i32_byref_main(dst: Option<OxPlace>) -> OxFunc {
    OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Global(GlobalId(0)),
                        value: OxOperand::Const(OxConst::I32(40)),
                    },
                    OxInst::CallProc {
                        dst,
                        proc: FuncId(1),
                        args: vec![OxArg::ByRef(OxPlace::Global(GlobalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    }
}

fn direct_one_i32_byref_param() -> OxLocal {
    local(
        "x",
        OxTy::Long,
        Some(OxParamInfo {
            optional: false,
            by_ref: true,
            variadic: false,
        }),
    )
}

fn direct_one_i32_byref_blocks(instrs: Vec<OxInst>) -> Vec<OxBlock> {
    vec![
        OxBlock {
            id: BlockId(0),
            instrs,
            fault_target: Some(BlockId(1)),
            terminator: OxTerminator::Return,
        },
        OxBlock {
            id: BlockId(1),
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::FaultDispatch {
                resume: BlockId(0),
                resume_next: BlockId(2),
            },
        },
        OxBlock {
            id: BlockId(2),
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::Return,
        },
    ]
}

fn direct_one_i32_byref_program(globals: Vec<OxGlobal>, main: OxFunc, callee: OxFunc) -> OxProgram {
    OxProgram {
        funcs: vec![main, callee],
        globals,
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn direct_descriptor_string_byval_sub_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProc {
                    dst: None,
                    proc: FuncId(1),
                    args: vec![OxArg::ByVal(OxOperand::Const(OxConst::Str(
                        "alpha".to_string(),
                    )))],
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let callee = OxFunc {
        name: "Capture".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "text",
            OxTy::Str,
            Some(OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: direct_one_i32_byref_blocks(vec![OxInst::Assign {
            dst: OxPlace::Global(GlobalId(0)),
            value: OxOperand::local(LocalId(0)),
        }]),
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, callee],
        globals: vec![string_global("result")],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn long_global(name: &str) -> OxGlobal {
    OxGlobal {
        name: name.to_string(),
        ty: OxTy::Long,
        array_element: None,
    }
}

fn string_global(name: &str) -> OxGlobal {
    OxGlobal {
        name: name.to_string(),
        ty: OxTy::Str,
        array_element: None,
    }
}

fn direct_two_i32_sub_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = direct_two_i32_main(None);
    let callee = OxFunc {
        name: "Add".to_string(),
        kind: ProcedureKind::Sub,
        locals: direct_two_i32_params(),
        temps: Vec::new(),
        param_count: 2,
        return_local: None,
        blocks: direct_two_i32_body_blocks(OxPlace::Global(GlobalId(0)), long, false),
        entry: BlockId(0),
    };
    direct_two_i32_program(main, callee)
}

fn direct_two_i32_function_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = direct_two_i32_main(Some(OxPlace::Global(GlobalId(0))));
    let callee = OxFunc {
        name: "Add".to_string(),
        kind: ProcedureKind::Function,
        locals: {
            let mut locals = direct_two_i32_params();
            locals.push(local("Add", OxTy::Long, None));
            locals
        },
        temps: Vec::new(),
        param_count: 2,
        return_local: Some(LocalId(2)),
        blocks: direct_two_i32_body_blocks(OxPlace::Local(LocalId(2)), long, false),
        entry: BlockId(0),
    };
    direct_two_i32_program(main, callee)
}

fn direct_ignored_two_i32_function_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = direct_two_i32_main(None);
    let callee = OxFunc {
        name: "TouchAdd".to_string(),
        kind: ProcedureKind::Function,
        locals: {
            let mut locals = direct_two_i32_params();
            locals.push(local("TouchAdd", OxTy::Long, None));
            locals
        },
        temps: Vec::new(),
        param_count: 2,
        return_local: Some(LocalId(2)),
        blocks: direct_two_i32_body_blocks(OxPlace::Global(GlobalId(0)), long, true),
        entry: BlockId(0),
    };
    direct_two_i32_program(main, callee)
}

fn direct_two_i32_main(dst: Option<OxPlace>) -> OxFunc {
    OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProc {
                    dst,
                    proc: FuncId(1),
                    args: vec![
                        OxArg::ByVal(OxOperand::Const(OxConst::I32(19))),
                        OxArg::ByVal(OxOperand::Const(OxConst::I32(23))),
                    ],
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    }
}

fn direct_two_i32_params() -> Vec<OxLocal> {
    vec![
        local(
            "x",
            OxTy::Long,
            Some(OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
        local(
            "y",
            OxTy::Long,
            Some(OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
    ]
}

fn direct_two_i32_body_blocks(
    dst: OxPlace,
    mode: NumericMode,
    assign_return: bool,
) -> Vec<OxBlock> {
    let mut instrs = vec![OxInst::Arith {
        dst,
        op: ArithOp::Add,
        lhs: OxOperand::local(LocalId(0)),
        rhs: OxOperand::local(LocalId(1)),
        mode,
    }];
    if assign_return {
        instrs.push(OxInst::Assign {
            dst: OxPlace::Local(LocalId(2)),
            value: OxOperand::Const(OxConst::I32(99)),
        });
    }
    vec![
        OxBlock {
            id: BlockId(0),
            instrs,
            fault_target: Some(BlockId(1)),
            terminator: OxTerminator::Return,
        },
        OxBlock {
            id: BlockId(1),
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::FaultDispatch {
                resume: BlockId(0),
                resume_next: BlockId(2),
            },
        },
        OxBlock {
            id: BlockId(2),
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::Return,
        },
    ]
}

fn direct_two_i32_program(main: OxFunc, callee: OxFunc) -> OxProgram {
    OxProgram {
        funcs: vec![main, callee],
        globals: vec![OxGlobal {
            name: "result".to_string(),
            ty: OxTy::Long,
            array_element: None,
        }],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn branch_program() -> OxProgram {
    let n = LocalId(0);
    let b = LocalId(1);
    let cond = TempId(0);
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Compare {
                dst: OxPlace::Temp(cond),
                op: CmpOp::Lt,
                lhs: OxOperand::Const(OxConst::I32(1)),
                rhs: OxOperand::Const(OxConst::I32(2)),
                mode: StringCompareMode::Binary,
            },
            OxInst::Assign {
                dst: OxPlace::Local(b),
                value: OxOperand::Use(OxPlace::Temp(cond)),
            },
        ],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Branch {
            cond: OxOperand::Use(OxPlace::Local(b)),
            then_blk: BlockId(1),
            else_blk: BlockId(2),
        },
    };
    let then_blk = OxBlock {
        id: BlockId(1),
        instrs: vec![OxInst::Assign {
            dst: OxPlace::Local(n),
            value: OxOperand::Const(OxConst::I32(42)),
        }],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Return,
    };
    let else_blk = OxBlock {
        id: BlockId(2),
        instrs: vec![OxInst::Assign {
            dst: OxPlace::Local(n),
            value: OxOperand::Const(OxConst::I32(13)),
        }],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Return,
    };
    let fault = OxBlock {
        id: BlockId(3),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::FaultDispatch {
            resume: BlockId(0),
            resume_next: BlockId(4),
        },
    };
    let exit = OxBlock {
        id: BlockId(4),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            OxLocal {
                name: "n".to_string(),
                ty: OxTy::Long,
                array_element: None,
                param: None,
                escaped: false,
            },
            OxLocal {
                name: "b".to_string(),
                ty: OxTy::Bool,
                array_element: None,
                param: None,
                escaped: false,
            },
        ],
        temps: vec![OxTy::Bool],
        param_count: 0,
        return_local: None,
        blocks: vec![entry, then_blk, else_blk, fault, exit],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn bool_logical_program() -> OxProgram {
    let n = LocalId(0);
    let not_false = TempId(0);
    let cond = TempId(1);
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Not {
                dst: OxPlace::Temp(not_false),
                src: OxOperand::Const(OxConst::Bool(false)),
            },
            OxInst::Logical {
                dst: OxPlace::Temp(cond),
                op: LogicalOp::And,
                lhs: OxOperand::Const(OxConst::Bool(true)),
                rhs: OxOperand::Use(OxPlace::Temp(not_false)),
            },
        ],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Branch {
            cond: OxOperand::Use(OxPlace::Temp(cond)),
            then_blk: BlockId(1),
            else_blk: BlockId(2),
        },
    };
    let then_blk = OxBlock {
        id: BlockId(1),
        instrs: vec![OxInst::Assign {
            dst: OxPlace::Local(n),
            value: OxOperand::Const(OxConst::I32(1)),
        }],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Return,
    };
    let else_blk = OxBlock {
        id: BlockId(2),
        instrs: vec![OxInst::Assign {
            dst: OxPlace::Local(n),
            value: OxOperand::Const(OxConst::I32(2)),
        }],
        fault_target: Some(BlockId(3)),
        terminator: OxTerminator::Return,
    };
    let fault = OxBlock {
        id: BlockId(3),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::FaultDispatch {
            resume: BlockId(0),
            resume_next: BlockId(4),
        },
    };
    let exit = OxBlock {
        id: BlockId(4),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("n", OxTy::Long, None)],
        temps: vec![OxTy::Bool, OxTy::Bool],
        param_count: 0,
        return_local: None,
        blocks: vec![entry, then_blk, else_blk, fault, exit],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn numeric_logical_program() -> OxProgram {
    let n = LocalId(0);
    let anded = TempId(0);
    let entry = OxBlock {
        id: BlockId(0),
        instrs: vec![
            OxInst::Logical {
                dst: OxPlace::Temp(anded),
                op: LogicalOp::And,
                lhs: OxOperand::Const(OxConst::I32(6)),
                rhs: OxOperand::Const(OxConst::I32(3)),
            },
            OxInst::Logical {
                dst: OxPlace::Local(n),
                op: LogicalOp::Or,
                lhs: OxOperand::Use(OxPlace::Temp(anded)),
                rhs: OxOperand::Const(OxConst::I32(8)),
            },
        ],
        fault_target: Some(BlockId(1)),
        terminator: OxTerminator::Return,
    };
    let fault = OxBlock {
        id: BlockId(1),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::FaultDispatch {
            resume: BlockId(0),
            resume_next: BlockId(2),
        },
    };
    let exit = OxBlock {
        id: BlockId(2),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::Return,
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("n", OxTy::Long, None)],
        temps: vec![OxTy::Long],
        param_count: 0,
        return_local: None,
        blocks: vec![entry, fault, exit],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn local(name: &str, ty: OxTy, param: Option<oxvba_oxir::OxParamInfo>) -> OxLocal {
    OxLocal {
        name: name.to_string(),
        ty,
        array_element: None,
        param,
        escaped: false,
    }
}

fn escaped_local(name: &str, ty: OxTy, param: Option<oxvba_oxir::OxParamInfo>) -> OxLocal {
    let mut local = local(name, ty, param);
    local.escaped = true;
    local
}

fn run_jit_program(program: &OxProgram) -> JitOutcome {
    assert_eq!(verify_program(program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    outcome
}

fn return_block(id: usize) -> OxBlock {
    OxBlock {
        id: BlockId(id),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::Return,
    }
}

fn fault_block(id: usize, resume: usize, resume_next: usize) -> OxBlock {
    OxBlock {
        id: BlockId(id),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::FaultDispatch {
            resume: BlockId(resume),
            resume_next: BlockId(resume_next),
        },
    }
}

fn class_import() -> BundleImport {
    BundleImport {
        unit: "VBA".to_string(),
        token: ExportToken::Class {
            name: "ExternalWidget".to_string(),
        },
    }
}

fn unsupported_project_object_instruction_program(inst: OxInst) -> OxProgram {
    let imports = if matches!(
        inst,
        OxInst::NewExtern { .. }
            | OxInst::PredeclaredExtern { .. }
            | OxInst::PredeclaredExternSet { .. }
    ) {
        vec![class_import()]
    } else {
        Vec::new()
    };
    let object_local = local("obj", OxTy::Object(ObjClass::Class(ClassId(0))), None);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            object_local,
            local("other", OxTy::Object(ObjClass::Untyped), None),
            local("flag", OxTy::Bool, None),
            local("value", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![inst],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main],
        classes: vec![OxClass {
            name: "Widget".to_string(),
            predeclared: true,
            initialize: None,
            terminate: None,
            fields: vec![
                OxClassField {
                    name: "Value".to_string(),
                    token: 1,
                    ty: OxTy::Long,
                    array_element: None,
                },
                OxClassField {
                    name: "Items".to_string(),
                    token: 2,
                    ty: OxTy::Array(Box::new(OxTy::Variant), ArrayShape::Dynamic),
                    array_element: Some(ArrayElementType::Variant),
                },
            ],
            methods: Vec::new(),
            as_new_fields: Vec::new(),
            implements: vec!["IWidget".to_string()],
        }],
        imports,
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn assert_jit_declines_project_object_instruction(
    label: &str,
    inst: OxInst,
    expected_instruction: &str,
) {
    let program = unsupported_project_object_instruction_program(inst);
    assert_eq!(verify_program(&program), Ok(()), "{label}");
    let engine = JitEngine;
    let err = match engine.compile_image(&[&program]) {
        Ok(_) => panic!("{label} unexpectedly compiled"),
        Err(err) => err,
    };
    let message = err
        .unsupported_message()
        .unwrap_or_else(|| panic!("{label} should be an unsupported JIT boundary: {err:?}"));
    assert!(
        message.contains(expected_instruction)
            && message.contains("unsupported")
            && !message.contains("instruction not lowered"),
        "{label}: {message}"
    );
}

#[test]
fn jit_addref_release_refcount_effects_compile_without_unsupported_diagnostic() {
    for (label, inst) in [
        (
            "AddRef",
            OxInst::AddRef {
                object: OxOperand::local(LocalId(0)),
            },
        ),
        (
            "Release",
            OxInst::Release {
                object: OxOperand::local(LocalId(0)),
                may_terminate: true,
            },
        ),
    ] {
        let program = unsupported_project_object_instruction_program(inst);
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let outcome = run_jit_program(&program);
        assert!(
            !outcome.raised,
            "{label} should execute as a JIT refcount effect, got {:?}",
            outcome.err
        );
    }
}

#[test]
fn jit_defaults_supported_carrier_slots() {
    let record = OxTy::Record(RecordLayoutId(0));
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("dec", OxTy::Decimal, None),
            local("obj", OxTy::Object(ObjClass::Untyped), None),
            local("rec", record.clone(), None),
            local("fixed", OxTy::FixedStr(4), None),
            local("proc", OxTy::ProcRef, None),
            local(
                "decArray",
                OxTy::Array(Box::new(OxTy::Decimal), ArrayShape::Dynamic),
                None,
            ),
            local(
                "objArray",
                OxTy::Array(
                    Box::new(OxTy::Object(ObjClass::Untyped)),
                    ArrayShape::Dynamic,
                ),
                None,
            ),
            local(
                "recordArray",
                OxTy::Array(Box::new(record), ArrayShape::Dynamic),
                None,
            ),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![return_block(0)],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![main],
        record_layouts: vec![vec![ArrayElementType::Long, ArrayElementType::String]],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };

    let outcome = run_jit_program(&program);
    assert_eq!(outcome.values[0].as_decimal96(), Some(Decimal96::default()));
    assert_eq!(outcome.values[1].vtype(), VarType::Object);
    assert!(outcome.values[1].as_object_ref().is_none());
    assert_eq!(outcome.values[2].vtype(), VarType::Record);
    assert!(outcome.values[2].as_vba_record().is_some());
    assert_eq!(
        outcome.values[3]
            .as_bstr()
            .map(|text| text.as_str().to_string()),
        Some("    ".to_string())
    );
    assert_eq!(outcome.values[4].vtype(), VarType::Empty);
    assert_eq!(
        outcome.values[5].array_element_vartype(),
        Some(VT_DECIMAL_VALUE)
    );
    assert_eq!(outcome.values[5].safearray_bounds_len(), None);
    assert_eq!(
        outcome.values[6].array_element_vartype(),
        Some(VT_DISPATCH_VALUE)
    );
    assert_eq!(
        outcome.values[7].array_element_vartype(),
        Some(VT_RECORD_VALUE)
    );
}

#[test]
fn jit_declines_project_object_instructions_with_specific_diagnostics() {
    let other = || OxOperand::local(LocalId(1));
    let flag = OxPlace::Local(LocalId(2));
    let cases = vec![(
        "TypeOfIs",
        OxInst::TypeOfIs {
            dst: flag,
            object: other(),
            type_name: "IWidget".to_string(),
        },
        "TypeOfIs",
    )];

    for (label, inst, expected_instruction) in cases {
        assert_jit_declines_project_object_instruction(label, inst, expected_instruction);
    }
}

#[test]
fn jit_typeof_nothing_is_false_without_object_descriptors() {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("is_widget", OxTy::Bool, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::TypeOfIs {
                    dst: OxPlace::Local(LocalId(0)),
                    object: OxOperand::Const(OxConst::Nothing),
                    type_name: "Widget".to_string(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };
    assert_eq!(verify_program(&program), Ok(()));

    let outcome = run_jit_program(&program);
    assert_eq!(outcome.values[0].as_bool(), Some(false));
}

#[test]
fn jit_copies_boxes_and_unboxes_supported_carriers() {
    let record = OxTy::Record(RecordLayoutId(0));
    let array = OxTy::Array(Box::new(OxTy::Decimal), ArrayShape::Dynamic);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("obj0", OxTy::Object(ObjClass::Untyped), None),
            local("objVariant", OxTy::Variant, None),
            local("obj1", OxTy::Object(ObjClass::Untyped), None),
            local("dec0", OxTy::Decimal, None),
            local("decVariant", OxTy::Variant, None),
            local("dec1", OxTy::Decimal, None),
            local("rec0", record.clone(), None),
            local("recVariant", OxTy::Variant, None),
            local("rec1", record, None),
            local("proc0", OxTy::ProcRef, None),
            local("procVariant", OxTy::Variant, None),
            local("proc1", OxTy::ProcRef, None),
            local("array0", array.clone(), None),
            local("arrayVariant", OxTy::Variant, None),
            local("array1", array, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Nothing),
                    },
                    OxInst::Box {
                        dst: OxPlace::Local(LocalId(1)),
                        src: OxOperand::local(LocalId(0)),
                        from: OxTy::Object(ObjClass::Untyped),
                    },
                    OxInst::Unbox {
                        dst: OxPlace::Local(LocalId(2)),
                        src: OxOperand::local(LocalId(1)),
                        to: OxTy::Object(ObjClass::Untyped),
                        checked: true,
                    },
                    OxInst::Box {
                        dst: OxPlace::Local(LocalId(4)),
                        src: OxOperand::local(LocalId(3)),
                        from: OxTy::Decimal,
                    },
                    OxInst::Unbox {
                        dst: OxPlace::Local(LocalId(5)),
                        src: OxOperand::local(LocalId(4)),
                        to: OxTy::Decimal,
                        checked: true,
                    },
                    OxInst::Box {
                        dst: OxPlace::Local(LocalId(7)),
                        src: OxOperand::local(LocalId(6)),
                        from: OxTy::Record(RecordLayoutId(0)),
                    },
                    OxInst::Unbox {
                        dst: OxPlace::Local(LocalId(8)),
                        src: OxOperand::local(LocalId(7)),
                        to: OxTy::Record(RecordLayoutId(0)),
                        checked: true,
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(9)),
                        proc: FuncId(0),
                    },
                    OxInst::Box {
                        dst: OxPlace::Local(LocalId(10)),
                        src: OxOperand::local(LocalId(9)),
                        from: OxTy::ProcRef,
                    },
                    OxInst::Unbox {
                        dst: OxPlace::Local(LocalId(11)),
                        src: OxOperand::local(LocalId(10)),
                        to: OxTy::ProcRef,
                        checked: true,
                    },
                    OxInst::Box {
                        dst: OxPlace::Local(LocalId(13)),
                        src: OxOperand::local(LocalId(12)),
                        from: OxTy::Array(Box::new(OxTy::Decimal), ArrayShape::Dynamic),
                    },
                    OxInst::Unbox {
                        dst: OxPlace::Local(LocalId(14)),
                        src: OxOperand::local(LocalId(13)),
                        to: OxTy::Array(Box::new(OxTy::Decimal), ArrayShape::Dynamic),
                        checked: true,
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![main],
        record_layouts: vec![vec![ArrayElementType::Long]],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };

    let outcome = run_jit_program(&program);
    assert_eq!(outcome.values[2].vtype(), VarType::Object);
    assert!(outcome.values[2].as_object_ref().is_none());
    assert_eq!(outcome.values[5].as_decimal96(), Some(Decimal96::default()));
    assert_eq!(outcome.values[8].vtype(), VarType::Record);
    assert_eq!(outcome.values[11].as_proc_ref(), Some(0));
    assert_eq!(
        outcome.values[14].array_element_vartype(),
        Some(VT_DECIMAL_VALUE)
    );
}

#[test]
fn jit_static_calls_move_supported_carriers_byval_byref_and_return() {
    let record = OxTy::Record(RecordLayoutId(0));
    let object = OxTy::Object(ObjClass::Untyped);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("recordIn", record.clone(), None),
            local("recordOut", record.clone(), None),
            local("objectIn", object.clone(), None),
            local("objectOut", object.clone(), None),
            local("isNothing", OxTy::Bool, None),
            escaped_local("procSlot", OxTy::ProcRef, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::CallProc {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        proc: FuncId(1),
                        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
                    },
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(2)),
                        value: OxOperand::Const(OxConst::Nothing),
                    },
                    OxInst::CallProc {
                        dst: Some(OxPlace::Local(LocalId(3))),
                        proc: FuncId(2),
                        args: vec![OxArg::ByVal(OxOperand::local(LocalId(2)))],
                    },
                    OxInst::CompareObjectIs {
                        dst: OxPlace::Local(LocalId(4)),
                        lhs: OxOperand::local(LocalId(3)),
                        rhs: OxOperand::Const(OxConst::Nothing),
                    },
                    OxInst::CallProc {
                        dst: None,
                        proc: FuncId(3),
                        args: vec![OxArg::ByRef(OxPlace::Local(LocalId(5)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let echo_record = OxFunc {
        name: "EchoRecord".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                record.clone(),
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoRecord", record, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let echo_object = OxFunc {
        name: "EchoObject".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                object.clone(),
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoObject", object, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let set_proc = OxFunc {
        name: "SetProc".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "target",
            OxTy::ProcRef,
            Some(OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Local(LocalId(0)),
                    proc: FuncId(0),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![main, echo_record, echo_object, set_proc],
        record_layouts: vec![vec![ArrayElementType::Long]],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };

    let outcome = run_jit_program(&program);
    assert_eq!(outcome.values[1].vtype(), VarType::Record);
    assert_eq!(outcome.values[3].vtype(), VarType::Object);
    assert!(outcome.values[3].as_object_ref().is_none());
    assert_eq!(outcome.values[4].as_bool(), Some(true));
    assert_eq!(outcome.values[5].as_proc_ref(), Some(0));
}

#[test]
fn jit_static_calls_move_array_carriers_byval_byref_and_return() {
    let array = OxTy::Array(Box::new(OxTy::Long), ArrayShape::Dynamic);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("arrayIn", array.clone(), None),
            local("arrayOut", array.clone(), None),
            escaped_local("arrayByRef", array.clone(), None),
            local("upper", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::ArrayRedim {
                        dst: OxPlace::Local(LocalId(0)),
                        upper_bounds: vec![OxOperand::Const(OxConst::I32(0))],
                        lower_bounds: vec![OxOperand::Const(OxConst::I32(0))],
                        element: ArrayElementType::Long,
                        preserve: false,
                        fixed: false,
                    },
                    OxInst::ArraySet {
                        array: OxPlace::Local(LocalId(0)),
                        indices: vec![OxOperand::Const(OxConst::I32(0))],
                        value: OxOperand::Const(OxConst::I32(11)),
                    },
                    OxInst::CallProc {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        proc: FuncId(1),
                        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
                    },
                    OxInst::CallProc {
                        dst: None,
                        proc: FuncId(2),
                        args: vec![OxArg::ByRef(OxPlace::Local(LocalId(2)))],
                    },
                    OxInst::Bound {
                        dst: OxPlace::Local(LocalId(3)),
                        which: BoundWhich::Upper,
                        array: OxOperand::local(LocalId(2)),
                        dimension: None,
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let echo_array = OxFunc {
        name: "EchoArray".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                array.clone(),
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoArray", array.clone(), None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let fill_array = OxFunc {
        name: "FillArray".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "target",
            array,
            Some(OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::ArrayRedim {
                        dst: OxPlace::Local(LocalId(0)),
                        upper_bounds: vec![OxOperand::Const(OxConst::I32(1))],
                        lower_bounds: vec![OxOperand::Const(OxConst::I32(0))],
                        element: ArrayElementType::Long,
                        preserve: false,
                        fixed: false,
                    },
                    OxInst::ArraySet {
                        array: OxPlace::Local(LocalId(0)),
                        indices: vec![OxOperand::Const(OxConst::I32(1))],
                        value: OxOperand::Const(OxConst::I32(21)),
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            fault_block(1, 0, 2),
            return_block(2),
        ],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![main, echo_array, fill_array],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };

    let outcome = run_jit_program(&program);
    assert_eq!(outcome.values[1].array_element_vartype(), Some(VT_I4_VALUE));
    assert_eq!(
        outcome.values[1]
            .safearray_element(0)
            .transpose()
            .expect("arrayOut element"),
        Some(Variant::from_i32(11))
    );
    assert_eq!(outcome.values[3].as_i32(), Some(1));
    assert_eq!(
        outcome.values[2]
            .safearray_element(1)
            .transpose()
            .expect("arrayByRef element"),
        Some(Variant::from_i32(21))
    );
}

fn proc_ref_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("arg", OxTy::Long, None),
            local("f", OxTy::ProcRef, None),
            local("n", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::I32(21)),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(1)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(2))),
                        target: OxOperand::local(LocalId(1)),
                        args: vec![oxvba_oxir::OxArg::ByVal(OxOperand::local(LocalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let double = OxFunc {
        name: "Double".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "x",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("Double", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Arith {
                    dst: OxPlace::Local(LocalId(1)),
                    op: ArithOp::Add,
                    lhs: OxOperand::local(LocalId(0)),
                    rhs: OxOperand::local(LocalId(0)),
                    mode: long,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, double],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_two_arg_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("n", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(19))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(23))),
                        ],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let sum2 = OxFunc {
        name: "Sum2".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "a",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local(
                "b",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("Sum2", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 2,
        return_local: Some(LocalId(2)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Arith {
                    dst: OxPlace::Local(LocalId(2)),
                    op: ArithOp::Add,
                    lhs: OxOperand::local(LocalId(0)),
                    rhs: OxOperand::local(LocalId(1)),
                    mode: long,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, sum2],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_four_arg_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("n", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(1))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(2))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(3))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(4))),
                        ],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let sum4 = OxFunc {
        name: "Sum4".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "a",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local(
                "b",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local(
                "c",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local(
                "d",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("Sum4", OxTy::Long, None),
        ],
        temps: vec![OxTy::Long],
        param_count: 4,
        return_local: Some(LocalId(4)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Arith {
                        dst: OxPlace::Temp(TempId(0)),
                        op: ArithOp::Add,
                        lhs: OxOperand::local(LocalId(0)),
                        rhs: OxOperand::local(LocalId(1)),
                        mode: long,
                    },
                    OxInst::Arith {
                        dst: OxPlace::Temp(TempId(0)),
                        op: ArithOp::Add,
                        lhs: OxOperand::temp(TempId(0)),
                        rhs: OxOperand::local(LocalId(2)),
                        mode: long,
                    },
                    OxInst::Arith {
                        dst: OxPlace::Local(LocalId(4)),
                        op: ArithOp::Add,
                        lhs: OxOperand::temp(TempId(0)),
                        rhs: OxOperand::local(LocalId(3)),
                        mode: long,
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, sum4],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_five_arg_program() -> OxProgram {
    let long = NumericMode::Checked(NumericCoerceTarget::Long);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("n", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(1))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(2))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(3))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(4))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(5))),
                        ],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let mut locals = Vec::new();
    for name in ["a", "b", "c", "d", "e"] {
        locals.push(local(
            name,
            OxTy::Long,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ));
    }
    locals.push(local("Sum5", OxTy::Long, None));
    let sum5 = OxFunc {
        name: "Sum5".to_string(),
        kind: ProcedureKind::Function,
        locals,
        temps: vec![OxTy::Long],
        param_count: 5,
        return_local: Some(LocalId(5)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Arith {
                        dst: OxPlace::Temp(TempId(0)),
                        op: ArithOp::Add,
                        lhs: OxOperand::local(LocalId(0)),
                        rhs: OxOperand::local(LocalId(1)),
                        mode: long,
                    },
                    OxInst::Arith {
                        dst: OxPlace::Temp(TempId(0)),
                        op: ArithOp::Add,
                        lhs: OxOperand::temp(TempId(0)),
                        rhs: OxOperand::local(LocalId(2)),
                        mode: long,
                    },
                    OxInst::Arith {
                        dst: OxPlace::Temp(TempId(0)),
                        op: ArithOp::Add,
                        lhs: OxOperand::temp(TempId(0)),
                        rhs: OxOperand::local(LocalId(3)),
                        mode: long,
                    },
                    OxInst::Arith {
                        dst: OxPlace::Local(LocalId(5)),
                        op: ArithOp::Add,
                        lhs: OxOperand::temp(TempId(0)),
                        rhs: OxOperand::local(LocalId(4)),
                        mode: long,
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, sum5],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_double_return_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("d", OxTy::Double, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![OxArg::ByVal(OxOperand::Const(OxConst::F64(
                            21.25f64.to_bits(),
                        )))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let id_double = OxFunc {
        name: "IdDouble".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "x",
                OxTy::Double,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("IdDouble", OxTy::Double, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, id_double],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_long_return_to_variant_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("value", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: Vec::new(),
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let forty_two = OxFunc {
        name: "FortyTwo".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("FortyTwo", OxTy::Long, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::I32(42)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, forty_two],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_ambiguous_same_signature_double_program() -> OxProgram {
    let double = NumericMode::Checked(NumericCoerceTarget::Double);
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("d", OxTy::Double, None),
            local("value", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(2),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![OxArg::ByVal(OxOperand::Const(OxConst::F64(
                            11.5f64.to_bits(),
                        )))],
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(2))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![OxArg::ByVal(OxOperand::Const(OxConst::F64(
                            12.25f64.to_bits(),
                        )))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let id_double = OxFunc {
        name: "IdDouble".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "x",
                OxTy::Double,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("IdDouble", OxTy::Double, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let double_double = OxFunc {
        name: "DoubleDouble".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "x",
                OxTy::Double,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("DoubleDouble", OxTy::Double, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Arith {
                    dst: OxPlace::Local(LocalId(1)),
                    op: ArithOp::Add,
                    lhs: OxOperand::local(LocalId(0)),
                    rhs: OxOperand::local(LocalId(0)),
                    mode: double,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, id_double, double_double],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_currency_byref_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            escaped_local("value", OxTy::Currency, None),
            local("f", OxTy::ProcRef, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Currency(100_000)),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(1)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: None,
                        target: OxOperand::local(LocalId(1)),
                        args: vec![OxArg::ByRef(OxPlace::Local(LocalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let set_currency = OxFunc {
        name: "SetCurrency".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "x",
            OxTy::Currency,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Currency(125_000)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, set_currency],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_string_return_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("text", OxTy::Str, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![OxArg::ByVal(OxOperand::Const(OxConst::Str(
                            "alpha".to_string(),
                        )))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let echo_string = OxFunc {
        name: "EchoString".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoString", OxTy::Str, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, echo_string],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_ambiguous_same_signature_string_return_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("text", OxTy::Str, None),
            local("value", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(2),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: Vec::new(),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(2))),
                        target: OxOperand::local(LocalId(0)),
                        args: Vec::new(),
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let alpha_string = OxFunc {
        name: "AlphaString".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("AlphaString", OxTy::Str, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let beta_string = OxFunc {
        name: "BetaString".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("BetaString", OxTy::Str, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Str("beta".to_string())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, alpha_string, beta_string],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_ambiguous_same_signature_string_byval_return_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("text", OxTy::Str, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(2),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![OxArg::ByVal(OxOperand::Const(OxConst::Str(
                            "alpha".to_string(),
                        )))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let fixed_string = OxFunc {
        name: "FixedString".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("FixedString", OxTy::Str, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::Const(OxConst::Str("gamma".to_string())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let echo_string = OxFunc {
        name: "EchoString".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoString", OxTy::Str, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, fixed_string, echo_string],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_string_return_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("text", OxTy::Str, None),
            local("value", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(0))),
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: Vec::new(),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: Vec::new(),
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let alpha_string = OxFunc {
        name: "AlphaString".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("AlphaString", OxTy::Str, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, alpha_string],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_string_byval_return_program(dst_ty: OxTy) -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("text", OxTy::Str, None),
            local("result", dst_ty, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let echo_string = OxFunc {
        name: "EchoString".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "text",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoString", OxTy::Str, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, echo_string],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_string_byval_variant_return_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("text", OxTy::Str, None),
            local("result", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let echo_text_variant = OxFunc {
        name: "EchoTextVariant".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "text",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoTextVariant", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Box {
                    dst: OxPlace::Local(LocalId(1)),
                    src: OxOperand::local(LocalId(0)),
                    from: OxTy::Str,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, echo_text_variant],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_two_string_byval_return_program(dst_ty: OxTy) -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("first", OxTy::Str, None),
            local("result", dst_ty, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: vec![
                            OxArg::ByVal(OxOperand::local(LocalId(0))),
                            OxArg::ByVal(OxOperand::Const(OxConst::Str("beta".to_string()))),
                        ],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let pick_second = OxFunc {
        name: "PickSecond".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "first",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local(
                "second",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("PickSecond", OxTy::Str, None),
        ],
        temps: Vec::new(),
        param_count: 2,
        return_local: Some(LocalId(2)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(2)),
                    value: OxOperand::local(LocalId(1)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, pick_second],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_two_string_byval_variant_return_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_two_string_byval_return_program(OxTy::Variant);
    program.funcs[2].name = "PickSecondVariant".to_string();
    program.funcs[2].locals[2].name = "PickSecondVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![OxInst::Box {
        dst: OxPlace::Local(LocalId(2)),
        src: OxOperand::local(LocalId(1)),
        from: OxTy::Str,
    }];
    program
}

fn proc_ref_unknown_signature_two_string_variant_byval_return_program(dst_ty: OxTy) -> OxProgram {
    proc_ref_unknown_signature_two_string_variant_byval_return_with_second_setup_program(
        dst_ty,
        vec![OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::Str("beta".to_string())),
            from: OxTy::Str,
        }],
    )
}

fn proc_ref_unknown_signature_two_string_variant_byval_return_with_second_setup_program(
    dst_ty: OxTy,
    mut second_setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_two_string_byval_return_program(dst_ty.clone());
    program.funcs[1].locals = vec![
        local("first_payload", OxTy::Variant, None),
        local("second_payload", OxTy::Variant, None),
        local("result", dst_ty, None),
    ];
    let mut instrs = vec![OxInst::Box {
        dst: OxPlace::Local(LocalId(0)),
        src: OxOperand::Const(OxConst::Str("alpha".to_string())),
        from: OxTy::Str,
    }];
    instrs.append(&mut second_setup);
    instrs.push(OxInst::CallProcRef {
        dst: Some(OxPlace::Local(LocalId(2))),
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByVal(OxOperand::local(LocalId(0))),
            OxArg::ByVal(OxOperand::local(LocalId(1))),
        ],
    });
    program.funcs[1].blocks[0].instrs = instrs;
    program
}

fn proc_ref_unknown_signature_two_string_variant_byval_variant_return_program() -> OxProgram {
    proc_ref_unknown_signature_two_string_variant_byval_variant_return_with_second_setup_program(
        vec![OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::Str("beta".to_string())),
            from: OxTy::Str,
        }],
    )
}

fn proc_ref_unknown_signature_two_string_variant_byval_variant_return_with_second_setup_program(
    mut second_setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_two_string_byval_variant_return_program();
    program.funcs[1].locals = vec![
        local("first_payload", OxTy::Variant, None),
        local("second_payload", OxTy::Variant, None),
        local("result", OxTy::Variant, None),
    ];
    let mut instrs = vec![OxInst::Box {
        dst: OxPlace::Local(LocalId(0)),
        src: OxOperand::Const(OxConst::Str("alpha".to_string())),
        from: OxTy::Str,
    }];
    instrs.append(&mut second_setup);
    instrs.push(OxInst::CallProcRef {
        dst: Some(OxPlace::Local(LocalId(2))),
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByVal(OxOperand::local(LocalId(0))),
            OxArg::ByVal(OxOperand::local(LocalId(1))),
        ],
    });
    program.funcs[1].blocks[0].instrs = instrs;
    program
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_return_with_setup_program(
    dst_ty: OxTy,
    mut setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_two_string_byval_return_program(dst_ty.clone());
    program.funcs[1].locals = vec![
        local("first", OxTy::Str, None),
        local("second_payload", OxTy::Variant, None),
        local("result", dst_ty, None),
    ];
    let mut instrs = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(0)),
        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
    }];
    instrs.append(&mut setup);
    instrs.push(OxInst::CallProcRef {
        dst: Some(OxPlace::Local(LocalId(2))),
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByVal(OxOperand::local(LocalId(0))),
            OxArg::ByVal(OxOperand::local(LocalId(1))),
        ],
    });
    program.funcs[1].blocks[0].instrs = instrs;
    program
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_return_program(dst_ty: OxTy) -> OxProgram {
    proc_ref_unknown_signature_mixed_string_variant_byval_return_with_setup_program(
        dst_ty,
        vec![OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::Str("beta".to_string())),
            from: OxTy::Str,
        }],
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_variant_return_with_setup_program(
    mut setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_two_string_byval_variant_return_program();
    program.funcs[1].locals = vec![
        local("first", OxTy::Str, None),
        local("second_payload", OxTy::Variant, None),
        local("result", OxTy::Variant, None),
    ];
    let mut instrs = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(0)),
        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
    }];
    instrs.append(&mut setup);
    instrs.push(OxInst::CallProcRef {
        dst: Some(OxPlace::Local(LocalId(2))),
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByVal(OxOperand::local(LocalId(0))),
            OxArg::ByVal(OxOperand::local(LocalId(1))),
        ],
    });
    program.funcs[1].blocks[0].instrs = instrs;
    program
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_variant_return_program() -> OxProgram {
    proc_ref_unknown_signature_mixed_string_variant_byval_variant_return_with_setup_program(vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::Str("beta".to_string())),
            from: OxTy::Str,
        },
    ])
}

fn proc_ref_unknown_signature_string_byval_sub_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("text", OxTy::Str, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                    },
                    OxInst::CallProcRef {
                        dst: None,
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let store_text = OxFunc {
        name: "StoreText".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "text",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Global(GlobalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, store_text],
        globals: vec![
            OxGlobal {
                name: "f".to_string(),
                ty: OxTy::ProcRef,
                array_element: None,
            },
            OxGlobal {
                name: "result".to_string(),
                ty: OxTy::Str,
                array_element: None,
            },
        ],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_string_global_byval_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_sub_program();
    program.globals.push(OxGlobal {
        name: "source".to_string(),
        ty: OxTy::Str,
        array_element: None,
    });
    program.funcs[1].locals.clear();
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(2)),
            value: OxOperand::Const(OxConst::Str("alpha".to_string())),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::Use(OxPlace::Global(GlobalId(2))))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_temp_byval_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_sub_program();
    program.funcs[1].locals.clear();
    program.funcs[1].temps = vec![OxTy::Str];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Temp(TempId(0)),
            value: OxOperand::Const(OxConst::Str("alpha".to_string())),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::temp(TempId(0)))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_variant_byval_sub_with_setup_program(
    mut setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_sub_program();
    program.funcs[1].locals = vec![local("payload", OxTy::Variant, None)];
    setup.push(OxInst::CallProcRef {
        dst: None,
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
    });
    program.funcs[1].blocks[0].instrs = setup;
    program
}

fn proc_ref_unknown_signature_string_variant_byval_sub_program() -> OxProgram {
    proc_ref_unknown_signature_string_variant_byval_sub_with_setup_program(vec![OxInst::Box {
        dst: OxPlace::Local(LocalId(0)),
        src: OxOperand::Const(OxConst::Str("alpha".to_string())),
        from: OxTy::Str,
    }])
}

fn proc_ref_unknown_signature_string_variant_byval_sub_long_payload_program() -> OxProgram {
    proc_ref_unknown_signature_string_variant_byval_sub_with_setup_program(vec![OxInst::Box {
        dst: OxPlace::Local(LocalId(0)),
        src: OxOperand::Const(OxConst::I32(42)),
        from: OxTy::Long,
    }])
}

fn proc_ref_unknown_signature_string_variant_byval_return_with_setup_program(
    dst_ty: OxTy,
    mut setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_return_program(dst_ty.clone());
    program.funcs[1].locals = vec![
        local("payload", OxTy::Variant, None),
        local("result", dst_ty, None),
    ];
    setup.push(OxInst::CallProcRef {
        dst: Some(OxPlace::Local(LocalId(1))),
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
    });
    program.funcs[1].blocks[0].instrs = setup;
    program
}

fn proc_ref_unknown_signature_string_variant_byval_return_program(dst_ty: OxTy) -> OxProgram {
    proc_ref_unknown_signature_string_variant_byval_return_with_setup_program(
        dst_ty,
        vec![OxInst::Box {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::Str("alpha".to_string())),
            from: OxTy::Str,
        }],
    )
}

fn proc_ref_unknown_signature_string_variant_byval_return_long_payload_program(
    dst_ty: OxTy,
) -> OxProgram {
    proc_ref_unknown_signature_string_variant_byval_return_with_setup_program(
        dst_ty,
        vec![OxInst::Box {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::I32(42)),
            from: OxTy::Long,
        }],
    )
}

fn proc_ref_unknown_signature_string_variant_byval_variant_return_with_setup_program(
    mut setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_variant_return_program();
    program.funcs[1].locals = vec![
        local("payload", OxTy::Variant, None),
        local("result", OxTy::Variant, None),
    ];
    setup.push(OxInst::CallProcRef {
        dst: Some(OxPlace::Local(LocalId(1))),
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
    });
    program.funcs[1].blocks[0].instrs = setup;
    program
}

fn proc_ref_unknown_signature_string_variant_byval_variant_return_program() -> OxProgram {
    proc_ref_unknown_signature_string_variant_byval_variant_return_with_setup_program(vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::Str("alpha".to_string())),
            from: OxTy::Str,
        },
    ])
}

fn proc_ref_unknown_signature_string_variant_byval_variant_return_long_payload_program() -> OxProgram
{
    proc_ref_unknown_signature_string_variant_byval_variant_return_with_setup_program(vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::I32(42)),
            from: OxTy::Long,
        },
    ])
}

fn proc_ref_unknown_signature_string_fixed_local_byval_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_sub_program();
    program.funcs[1].locals = vec![local("fixed", OxTy::FixedStr(3), None)];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Coerce {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::Str("ab".to_string())),
            target: OxCoerceTarget::FixedStr(3),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_sub_program();
    program.globals.truncate(1);
    program.funcs[1].locals = vec![escaped_local("text", OxTy::Str, None)];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Str("alpha".to_string())),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByRef(OxPlace::Local(LocalId(0)))],
        },
    ];
    program.funcs[2].name = "SetText".to_string();
    program.funcs[2].locals = vec![local(
        "text",
        OxTy::Str,
        Some(oxvba_oxir::OxParamInfo {
            optional: false,
            by_ref: true,
            variadic: false,
        }),
    )];
    program.funcs[2].blocks[0].instrs = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(0)),
        value: OxOperand::Const(OxConst::Str("beta".to_string())),
    }];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_sub_with_setup_program(
    mut setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_sub_program();
    program.funcs[1].locals = vec![
        escaped_local("text", OxTy::Str, None),
        local("payload", OxTy::Variant, None),
    ];
    let mut instrs = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(0)),
        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
    }];
    instrs.append(&mut setup);
    instrs.push(OxInst::CallProcRef {
        dst: None,
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByRef(OxPlace::Local(LocalId(0))),
            OxArg::ByVal(OxOperand::local(LocalId(1))),
        ],
    });
    program.funcs[1].blocks[0].instrs = instrs;
    program.funcs[2].name = "ReplaceText".to_string();
    program.funcs[2].locals = vec![
        local(
            "text",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        ),
        local(
            "value",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
    ];
    program.funcs[2].param_count = 2;
    program.funcs[2].blocks[0].instrs = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(0)),
        value: OxOperand::local(LocalId(1)),
    }];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_sub_program() -> OxProgram {
    proc_ref_unknown_signature_string_byref_variant_byval_sub_with_setup_program(vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::I32(42)),
            from: OxTy::Long,
        },
    ])
}

fn variant_byte_payload_setup(dst: LocalId) -> Vec<OxInst> {
    vec![
        OxInst::Coerce {
            dst: OxPlace::Temp(TempId(0)),
            src: OxOperand::Const(OxConst::I32(7)),
            target: OxCoerceTarget::Numeric(NumericCoerceTarget::Byte),
        },
        OxInst::Box {
            dst: OxPlace::Local(dst),
            src: OxOperand::temp(TempId(0)),
            from: OxTy::Byte,
        },
    ]
}

fn proc_ref_unknown_signature_string_byref_variant_byval_byte_payload_setup() -> Vec<OxInst> {
    variant_byte_payload_setup(LocalId(1))
}

fn with_byte_payload_temp(mut program: OxProgram) -> OxProgram {
    program.funcs[1].temps = vec![OxTy::Byte];
    program
}

fn proc_ref_unknown_signature_string_variant_byval_byte_sub_program() -> OxProgram {
    with_byte_payload_temp(
        proc_ref_unknown_signature_string_variant_byval_sub_with_setup_program(
            variant_byte_payload_setup(LocalId(0)),
        ),
    )
}

fn proc_ref_unknown_signature_string_variant_byval_byte_return_program(dst_ty: OxTy) -> OxProgram {
    with_byte_payload_temp(
        proc_ref_unknown_signature_string_variant_byval_return_with_setup_program(
            dst_ty,
            variant_byte_payload_setup(LocalId(0)),
        ),
    )
}

fn proc_ref_unknown_signature_string_variant_byval_byte_variant_return_program() -> OxProgram {
    with_byte_payload_temp(
        proc_ref_unknown_signature_string_variant_byval_variant_return_with_setup_program(
            variant_byte_payload_setup(LocalId(0)),
        ),
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_byte_sub_program() -> OxProgram {
    with_byte_payload_temp(
        proc_ref_unknown_signature_mixed_string_variant_byval_sub_with_setup_program(
            variant_byte_payload_setup(LocalId(1)),
        ),
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_byte_return_program(
    dst_ty: OxTy,
) -> OxProgram {
    with_byte_payload_temp(
        proc_ref_unknown_signature_mixed_string_variant_byval_return_with_setup_program(
            dst_ty,
            variant_byte_payload_setup(LocalId(1)),
        ),
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_byte_variant_return_program() -> OxProgram
{
    with_byte_payload_temp(
        proc_ref_unknown_signature_mixed_string_variant_byval_variant_return_with_setup_program(
            variant_byte_payload_setup(LocalId(1)),
        ),
    )
}

fn proc_ref_unknown_signature_two_string_variant_byval_byte_sub_program() -> OxProgram {
    with_byte_payload_temp(
        proc_ref_unknown_signature_two_string_variant_byval_sub_with_second_setup_program(
            variant_byte_payload_setup(LocalId(1)),
        ),
    )
}

fn proc_ref_unknown_signature_two_string_variant_byval_byte_return_program(
    dst_ty: OxTy,
) -> OxProgram {
    with_byte_payload_temp(
        proc_ref_unknown_signature_two_string_variant_byval_return_with_second_setup_program(
            dst_ty,
            variant_byte_payload_setup(LocalId(1)),
        ),
    )
}

fn proc_ref_unknown_signature_two_string_variant_byval_byte_variant_return_program() -> OxProgram {
    with_byte_payload_temp(
        proc_ref_unknown_signature_two_string_variant_byval_variant_return_with_second_setup_program(
            variant_byte_payload_setup(LocalId(1)),
        ),
    )
}

fn proc_ref_unknown_signature_string_byref_variant_byval_byte_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_variant_byval_sub_with_setup_program(
        proc_ref_unknown_signature_string_byref_variant_byval_byte_payload_setup(),
    );
    program.funcs[1].temps = vec![OxTy::Byte];
    program
}

fn vba_conversion_import(member: &str) -> BundleImport {
    BundleImport {
        unit: "VBA".to_string(),
        token: ExportToken::ModuleFunc {
            module: "Conversion".to_string(),
            member: member.to_string(),
            kind: ProjectMemberKind::Method,
        },
    }
}

fn extern_payload_setup(dst: LocalId, src: OxOperand) -> Vec<OxInst> {
    vec![OxInst::CallExtern {
        dst: Some(OxPlace::Local(dst)),
        import: ImportId(0),
        args: vec![OxArg::ByVal(src)],
    }]
}

fn with_vba_conversion_import(mut program: OxProgram, member: &str) -> OxProgram {
    program.imports = vec![vba_conversion_import(member)];
    program
}

fn proc_ref_unknown_signature_string_variant_byval_extern_payload_sub_program(
    member: &str,
    src: OxOperand,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_string_variant_byval_sub_with_setup_program(
            extern_payload_setup(LocalId(0), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_string_variant_byval_extern_payload_return_program(
    member: &str,
    src: OxOperand,
    dst_ty: OxTy,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_string_variant_byval_return_with_setup_program(
            dst_ty,
            extern_payload_setup(LocalId(0), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_string_variant_byval_extern_payload_variant_return_program(
    member: &str,
    src: OxOperand,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_string_variant_byval_variant_return_with_setup_program(
            extern_payload_setup(LocalId(0), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_extern_payload_sub_program(
    member: &str,
    src: OxOperand,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_mixed_string_variant_byval_sub_with_setup_program(
            extern_payload_setup(LocalId(1), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_extern_payload_return_program(
    member: &str,
    src: OxOperand,
    dst_ty: OxTy,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_mixed_string_variant_byval_return_with_setup_program(
            dst_ty,
            extern_payload_setup(LocalId(1), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_extern_payload_variant_return_program(
    member: &str,
    src: OxOperand,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_mixed_string_variant_byval_variant_return_with_setup_program(
            extern_payload_setup(LocalId(1), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_two_string_variant_byval_extern_payload_sub_program(
    member: &str,
    src: OxOperand,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_two_string_variant_byval_sub_with_second_setup_program(
            extern_payload_setup(LocalId(1), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_two_string_variant_byval_extern_payload_return_program(
    member: &str,
    src: OxOperand,
    dst_ty: OxTy,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_two_string_variant_byval_return_with_second_setup_program(
            dst_ty,
            extern_payload_setup(LocalId(1), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_two_string_variant_byval_extern_payload_variant_return_program(
    member: &str,
    src: OxOperand,
) -> OxProgram {
    with_vba_conversion_import(
        proc_ref_unknown_signature_two_string_variant_byval_variant_return_with_second_setup_program(
            extern_payload_setup(LocalId(1), src),
        ),
        member,
    )
}

fn proc_ref_unknown_signature_string_byref_variant_byval_extern_payload_sub_program(
    member: &str,
    src: OxOperand,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_variant_byval_sub_with_setup_program(
        extern_payload_setup(LocalId(1), src),
    );
    program.imports = vec![vba_conversion_import(member)];
    program
}

fn proc_ref_unknown_signature_string_byref_return_program(dst_ty: OxTy) -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_sub_program();
    program.funcs[1].locals = vec![
        escaped_local("text", OxTy::Str, None),
        local("result", dst_ty, None),
    ];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Str("alpha".to_string())),
        },
        OxInst::CallProcRef {
            dst: Some(OxPlace::Local(LocalId(1))),
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByRef(OxPlace::Local(LocalId(0)))],
        },
    ];
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2].name = "SetTextAndReturn".to_string();
    program.funcs[2].locals = vec![
        local(
            "text",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        ),
        local("SetTextAndReturn", OxTy::Str, None),
    ];
    program.funcs[2].return_local = Some(LocalId(1));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Str("beta".to_string())),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(1)),
            value: OxOperand::local(LocalId(0)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_return_with_setup_program(
    dst_ty: OxTy,
    setup: Vec<OxInst>,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_variant_byval_sub_with_setup_program(setup);
    program.funcs[1].locals.push(local("result", dst_ty, None));
    if let Some(OxInst::CallProcRef { dst, .. }) = program.funcs[1].blocks[0].instrs.last_mut() {
        *dst = Some(OxPlace::Local(LocalId(2)));
    }
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2].name = "ReplaceTextAndReturn".to_string();
    program.funcs[2].locals = vec![
        local(
            "text",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        ),
        local(
            "value",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
        local("ReplaceTextAndReturn", OxTy::Str, None),
    ];
    program.funcs[2].return_local = Some(LocalId(2));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(2)),
            value: OxOperand::local(LocalId(0)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_return_program(dst_ty: OxTy) -> OxProgram {
    proc_ref_unknown_signature_string_byref_variant_byval_return_with_setup_program(
        dst_ty,
        vec![OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::I32(42)),
            from: OxTy::Long,
        }],
    )
}

fn proc_ref_unknown_signature_string_byref_variant_byval_byte_return_program(
    dst_ty: OxTy,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_variant_byval_return_with_setup_program(
            dst_ty,
            proc_ref_unknown_signature_string_byref_variant_byval_byte_payload_setup(),
        );
    program.funcs[1].temps = vec![OxTy::Byte];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_extern_payload_return_program(
    member: &str,
    src: OxOperand,
    dst_ty: OxTy,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_variant_byval_return_with_setup_program(
            dst_ty,
            extern_payload_setup(LocalId(1), src),
        );
    program.imports = vec![vba_conversion_import(member)];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_return_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_return_program(OxTy::Variant);
    program.funcs[2].name = "SetTextAndReturnVariant".to_string();
    program.funcs[2].locals[1].name = "SetTextAndReturnVariant".to_string();
    program.funcs[2].locals[1].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Str("beta".to_string())),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_variant_return_with_setup_program(
    setup: Vec<OxInst>,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_variant_byval_return_with_setup_program(
            OxTy::Variant,
            setup,
        );
    program.funcs[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(2)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_variant_return_program() -> OxProgram {
    proc_ref_unknown_signature_string_byref_variant_byval_variant_return_with_setup_program(vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::I32(42)),
            from: OxTy::Long,
        },
    ])
}

fn proc_ref_unknown_signature_string_byref_variant_byval_byte_variant_return_program() -> OxProgram
{
    let mut program =
        proc_ref_unknown_signature_string_byref_variant_byval_variant_return_with_setup_program(
            proc_ref_unknown_signature_string_byref_variant_byval_byte_payload_setup(),
        );
    program.funcs[1].temps = vec![OxTy::Byte];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_extern_payload_variant_return_program(
    member: &str,
    src: OxOperand,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_variant_byval_variant_return_with_setup_program(
            extern_payload_setup(LocalId(1), src),
        );
    program.imports = vec![vba_conversion_import(member)];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_function_statement_with_setup_program(
    setup: Vec<OxInst>,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_variant_byval_sub_with_setup_program(setup);
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2].name = "ReplaceTextAndReturn".to_string();
    program.funcs[2]
        .locals
        .push(local("ReplaceTextAndReturn", OxTy::Str, None));
    program.funcs[2].return_local = Some(LocalId(2));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(2)),
            value: OxOperand::local(LocalId(0)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_variant_byval_variant_function_statement_with_setup_program(
    setup: Vec<OxInst>,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_variant_byval_function_statement_with_setup_program(
            setup,
        );
    program.funcs[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(2)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_string_byval_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_sub_program();
    program.funcs[1].locals = vec![
        escaped_local("text", OxTy::Str, None),
        local("value", OxTy::Str, None),
    ];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Str("alpha".to_string())),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(1)),
            value: OxOperand::Const(OxConst::Str("gamma".to_string())),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![
                OxArg::ByRef(OxPlace::Local(LocalId(0))),
                OxArg::ByVal(OxOperand::local(LocalId(1))),
            ],
        },
    ];
    program.funcs[2].name = "ReplaceText".to_string();
    program.funcs[2].locals = vec![
        local(
            "text",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        ),
        local(
            "value",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
    ];
    program.funcs[2].param_count = 2;
    program.funcs[2].blocks[0].instrs = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(0)),
        value: OxOperand::local(LocalId(1)),
    }];
    program
}

fn proc_ref_unknown_signature_string_byref_string_byval_return_program(dst_ty: OxTy) -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_string_byval_sub_program();
    program.funcs[1].locals.push(local("result", dst_ty, None));
    if let Some(OxInst::CallProcRef { dst, .. }) = program.funcs[1].blocks[0].instrs.last_mut() {
        *dst = Some(OxPlace::Local(LocalId(2)));
    }
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2].name = "ReplaceTextAndReturn".to_string();
    program.funcs[2]
        .locals
        .push(local("ReplaceTextAndReturn", OxTy::Str, None));
    program.funcs[2].return_local = Some(LocalId(2));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(2)),
            value: OxOperand::local(LocalId(0)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_string_byval_variant_return_program() -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_string_byval_return_program(OxTy::Variant);
    program.funcs[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(2)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_string_literal_byval_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_string_byval_sub_program();
    program.funcs[1].locals = vec![escaped_local("text", OxTy::Str, None)];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Str("alpha".to_string())),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![
                OxArg::ByRef(OxPlace::Local(LocalId(0))),
                OxArg::ByVal(OxOperand::Const(OxConst::Str("delta".to_string()))),
            ],
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_static_byval_return_program(
    mut program: OxProgram,
    dst_ty: OxTy,
) -> OxProgram {
    let result_id = LocalId(program.funcs[1].locals.len());
    program.funcs[1].locals.push(local("result", dst_ty, None));
    if let Some(OxInst::CallProcRef { dst, .. }) = program.funcs[1].blocks[0].instrs.last_mut() {
        *dst = Some(OxPlace::Local(result_id));
    }
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2].name = "ReplaceTextAndReturn".to_string();
    program.funcs[2]
        .locals
        .push(local("ReplaceTextAndReturn", OxTy::Str, None));
    program.funcs[2].return_local = Some(LocalId(2));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(2)),
            value: OxOperand::local(LocalId(0)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_static_byval_variant_return_program(
    program: OxProgram,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_static_byval_return_program(program, OxTy::Variant);
    program.funcs[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(2)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_static_byval_function_statement_program(
    mut program: OxProgram,
) -> OxProgram {
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2].name = "ReplaceTextAndReturn".to_string();
    program.funcs[2]
        .locals
        .push(local("ReplaceTextAndReturn", OxTy::Str, None));
    program.funcs[2].return_local = Some(LocalId(2));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(2)),
            value: OxOperand::local(LocalId(0)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_static_byval_variant_function_statement_program(
    program: OxProgram,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_string_byref_static_byval_function_statement_program(program);
    program.funcs[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].name = "ReplaceTextAndReturnVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(2)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_string_byval_function_statement_program() -> OxProgram {
    proc_ref_unknown_signature_string_byref_static_byval_function_statement_program(
        proc_ref_unknown_signature_string_byref_string_byval_sub_program(),
    )
}

fn proc_ref_unknown_signature_string_byref_string_byval_variant_function_statement_program()
-> OxProgram {
    proc_ref_unknown_signature_string_byref_static_byval_variant_function_statement_program(
        proc_ref_unknown_signature_string_byref_string_byval_sub_program(),
    )
}

fn proc_ref_unknown_signature_string_byref_string_literal_byval_return_program(
    dst_ty: OxTy,
) -> OxProgram {
    proc_ref_unknown_signature_string_byref_static_byval_return_program(
        proc_ref_unknown_signature_string_byref_string_literal_byval_sub_program(),
        dst_ty,
    )
}

fn proc_ref_unknown_signature_string_byref_string_literal_byval_variant_return_program() -> OxProgram
{
    proc_ref_unknown_signature_string_byref_static_byval_variant_return_program(
        proc_ref_unknown_signature_string_byref_string_literal_byval_sub_program(),
    )
}

fn proc_ref_unknown_signature_string_byref_string_literal_byval_function_statement_program()
-> OxProgram {
    proc_ref_unknown_signature_string_byref_static_byval_function_statement_program(
        proc_ref_unknown_signature_string_byref_string_literal_byval_sub_program(),
    )
}

fn proc_ref_unknown_signature_string_byref_string_literal_byval_variant_function_statement_program()
-> OxProgram {
    proc_ref_unknown_signature_string_byref_static_byval_variant_function_statement_program(
        proc_ref_unknown_signature_string_byref_string_literal_byval_sub_program(),
    )
}

fn proc_ref_unknown_signature_string_byref_fixed_string_byval_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byref_string_byval_sub_program();
    program.funcs[1].locals = vec![
        escaped_local("text", OxTy::Str, None),
        local("fixed", OxTy::FixedStr(3), None),
    ];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Str("alpha".to_string())),
        },
        OxInst::Coerce {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::Str("xy".to_string())),
            target: OxCoerceTarget::FixedStr(3),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![
                OxArg::ByRef(OxPlace::Local(LocalId(0))),
                OxArg::ByVal(OxOperand::local(LocalId(1))),
            ],
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byref_fixed_string_byval_return_program(
    dst_ty: OxTy,
) -> OxProgram {
    proc_ref_unknown_signature_string_byref_static_byval_return_program(
        proc_ref_unknown_signature_string_byref_fixed_string_byval_sub_program(),
        dst_ty,
    )
}

fn proc_ref_unknown_signature_string_byref_fixed_string_byval_variant_return_program() -> OxProgram
{
    proc_ref_unknown_signature_string_byref_static_byval_variant_return_program(
        proc_ref_unknown_signature_string_byref_fixed_string_byval_sub_program(),
    )
}

fn proc_ref_unknown_signature_string_byref_fixed_string_byval_function_statement_program()
-> OxProgram {
    proc_ref_unknown_signature_string_byref_static_byval_function_statement_program(
        proc_ref_unknown_signature_string_byref_fixed_string_byval_sub_program(),
    )
}

fn proc_ref_unknown_signature_string_byref_fixed_string_byval_variant_function_statement_program()
-> OxProgram {
    proc_ref_unknown_signature_string_byref_static_byval_variant_function_statement_program(
        proc_ref_unknown_signature_string_byref_fixed_string_byval_sub_program(),
    )
}

fn proc_ref_unknown_signature_mixed_long_string_byval_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_sub_program();
    program.funcs[1].locals.clear();
    program.funcs[1].blocks[0].instrs = vec![OxInst::CallProcRef {
        dst: None,
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByVal(OxOperand::Const(OxConst::I32(1))),
            OxArg::ByVal(OxOperand::Const(OxConst::Str("alpha".to_string()))),
        ],
    }];
    program
}

fn proc_ref_unknown_signature_string_byval_function_statement_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_sub_program();
    program.funcs[2].name = "StoreAndReturnText".to_string();
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2]
        .locals
        .push(local("StoreAndReturnText", OxTy::Str, None));
    program.funcs[2].return_local = Some(LocalId(1));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::local(LocalId(0)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(1)),
            value: OxOperand::local(LocalId(0)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_string_byval_variant_function_statement_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_string_byval_function_statement_program();
    program.funcs[2].name = "StoreAndReturnTextVariant".to_string();
    program.funcs[2].locals[1].name = "StoreAndReturnTextVariant".to_string();
    program.funcs[2].locals[1].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::local(LocalId(0)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_two_string_byval_sub_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("first", OxTy::Str, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                    },
                    OxInst::CallProcRef {
                        dst: None,
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: vec![
                            OxArg::ByVal(OxOperand::local(LocalId(0))),
                            OxArg::ByVal(OxOperand::Const(OxConst::Str("beta".to_string()))),
                        ],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let store_second = OxFunc {
        name: "StoreSecond".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local(
                "first",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local(
                "second",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
        ],
        temps: Vec::new(),
        param_count: 2,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Global(GlobalId(1)),
                    value: OxOperand::local(LocalId(1)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, store_second],
        globals: vec![
            OxGlobal {
                name: "f".to_string(),
                ty: OxTy::ProcRef,
                array_element: None,
            },
            OxGlobal {
                name: "result".to_string(),
                ty: OxTy::Str,
                array_element: None,
            },
        ],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_two_string_variant_byval_sub_with_second_setup_program(
    mut second_setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_two_string_byval_sub_program();
    program.funcs[1].locals = vec![
        local("first_payload", OxTy::Variant, None),
        local("second_payload", OxTy::Variant, None),
    ];
    let mut instrs = vec![OxInst::Box {
        dst: OxPlace::Local(LocalId(0)),
        src: OxOperand::Const(OxConst::Str("alpha".to_string())),
        from: OxTy::Str,
    }];
    instrs.append(&mut second_setup);
    instrs.push(OxInst::CallProcRef {
        dst: None,
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByVal(OxOperand::local(LocalId(0))),
            OxArg::ByVal(OxOperand::local(LocalId(1))),
        ],
    });
    program.funcs[1].blocks[0].instrs = instrs;
    program
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_sub_with_setup_program(
    mut setup: Vec<OxInst>,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_two_string_byval_sub_program();
    program.funcs[1].locals = vec![
        local("first", OxTy::Str, None),
        local("second_payload", OxTy::Variant, None),
    ];
    let mut instrs = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(0)),
        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
    }];
    instrs.append(&mut setup);
    instrs.push(OxInst::CallProcRef {
        dst: None,
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByVal(OxOperand::local(LocalId(0))),
            OxArg::ByVal(OxOperand::local(LocalId(1))),
        ],
    });
    program.funcs[1].blocks[0].instrs = instrs;
    program
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_sub_program() -> OxProgram {
    proc_ref_unknown_signature_mixed_string_variant_byval_sub_with_setup_program(vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::Str("beta".to_string())),
            from: OxTy::Str,
        },
    ])
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_program() -> OxProgram {
    proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_with_setup_program(
        vec![OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::Str("beta".to_string())),
            from: OxTy::Str,
        }],
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_with_setup_program(
    setup: Vec<OxInst>,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_mixed_string_variant_byval_sub_with_setup_program(setup);
    program.funcs[2].name = "StoreSecondAndReturnText".to_string();
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2]
        .locals
        .push(local("StoreSecondAndReturnText", OxTy::Str, None));
    program.funcs[2].return_local = Some(LocalId(2));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(2)),
            value: OxOperand::local(LocalId(1)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_variant_function_statement_program()
-> OxProgram {
    proc_ref_unknown_signature_mixed_string_variant_byval_variant_function_statement_with_setup_program(
        vec![OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::Const(OxConst::Str("beta".to_string())),
            from: OxTy::Str,
        }],
    )
}

fn proc_ref_unknown_signature_mixed_string_variant_byval_variant_function_statement_with_setup_program(
    setup: Vec<OxInst>,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_with_setup_program(
            setup,
        );
    program.funcs[2].name = "StoreSecondAndReturnTextVariant".to_string();
    program.funcs[2].locals[2].name = "StoreSecondAndReturnTextVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(2)),
            src: OxOperand::local(LocalId(1)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_two_string_variant_byval_function_statement_with_second_setup_program(
    second_setup: Vec<OxInst>,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_two_string_variant_byval_sub_with_second_setup_program(
            second_setup,
        );
    program.funcs[2].name = "StoreSecondAndReturnText".to_string();
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2]
        .locals
        .push(local("StoreSecondAndReturnText", OxTy::Str, None));
    program.funcs[2].return_local = Some(LocalId(2));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(2)),
            value: OxOperand::local(LocalId(1)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_two_string_variant_byval_variant_function_statement_with_second_setup_program(
    second_setup: Vec<OxInst>,
) -> OxProgram {
    let mut program =
        proc_ref_unknown_signature_two_string_variant_byval_function_statement_with_second_setup_program(
            second_setup,
        );
    program.funcs[2].name = "StoreSecondAndReturnTextVariant".to_string();
    program.funcs[2].locals[2].name = "StoreSecondAndReturnTextVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::local(LocalId(1)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(2)),
            src: OxOperand::local(LocalId(1)),
            from: OxTy::Str,
        },
    ];
    program
}

fn proc_ref_unknown_signature_long_return_to_variant_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("value", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: Some(OxPlace::Local(LocalId(0))),
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: Vec::new(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let forty_two = OxFunc {
        name: "FortyTwo".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("FortyTwo", OxTy::Long, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::I32(42)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, forty_two],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_variant_return_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("value", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: Some(OxPlace::Local(LocalId(0))),
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: Vec::new(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let give_variant = OxFunc {
        name: "GiveVariant".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("GiveVariant", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Box {
                    dst: OxPlace::Local(LocalId(0)),
                    src: OxOperand::Const(OxConst::I32(42)),
                    from: OxTy::Long,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, give_variant],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_double_return_to_variant_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("value", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: Some(OxPlace::Local(LocalId(0))),
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: Vec::new(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let half = OxFunc {
        name: "Half".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("Half", OxTy::Double, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, half],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_double_return_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("value", OxTy::Double, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: Some(OxPlace::Local(LocalId(0))),
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: Vec::new(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let half = OxFunc {
        name: "Half".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("Half", OxTy::Double, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, half],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_no_arg_return_program(
    return_ty: OxTy,
    dst_ty: OxTy,
    value: OxConst,
) -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("value", dst_ty, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: Some(OxPlace::Local(LocalId(0))),
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: Vec::new(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let return_value_inst = if matches!(return_ty, OxTy::Byte) {
        OxInst::Coerce {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(value),
            target: OxCoerceTarget::Numeric(NumericCoerceTarget::Byte),
        }
    } else {
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(value),
        }
    };
    let ret = OxFunc {
        name: "ReturnValue".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local("ReturnValue", return_ty, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![return_value_inst],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, ret],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_no_arg_function_statement_program(
    name: &str,
    return_ty: OxTy,
    result_ty: OxTy,
    callee_instrs: Vec<OxInst>,
) -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: None,
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: Vec::new(),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let callee = OxFunc {
        name: name.to_string(),
        kind: ProcedureKind::Function,
        locals: vec![local(name, return_ty, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: callee_instrs,
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, callee],
        globals: vec![
            OxGlobal {
                name: "f".to_string(),
                ty: OxTy::ProcRef,
                array_element: None,
            },
            OxGlobal {
                name: "result".to_string(),
                ty: result_ty,
                array_element: None,
            },
        ],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_no_arg_long_function_statement_program() -> OxProgram {
    proc_ref_unknown_signature_no_arg_function_statement_program(
        "StoreAndReturn",
        OxTy::Long,
        OxTy::Long,
        vec![
            OxInst::Assign {
                dst: OxPlace::Global(GlobalId(1)),
                value: OxOperand::Const(OxConst::I32(42)),
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::I32(42)),
            },
        ],
    )
}

fn proc_ref_unknown_signature_no_arg_string_function_statement_program() -> OxProgram {
    proc_ref_unknown_signature_no_arg_function_statement_program(
        "StoreAndReturnText",
        OxTy::Str,
        OxTy::Str,
        vec![
            OxInst::Assign {
                dst: OxPlace::Global(GlobalId(1)),
                value: OxOperand::Const(OxConst::Str("alpha".to_string())),
            },
            OxInst::Assign {
                dst: OxPlace::Local(LocalId(0)),
                value: OxOperand::Const(OxConst::Str("alpha".to_string())),
            },
        ],
    )
}

fn proc_ref_unknown_signature_no_arg_variant_function_statement_program() -> OxProgram {
    proc_ref_unknown_signature_no_arg_function_statement_program(
        "StoreAndReturnVariant",
        OxTy::Variant,
        OxTy::Long,
        vec![
            OxInst::Assign {
                dst: OxPlace::Global(GlobalId(1)),
                value: OxOperand::Const(OxConst::I32(42)),
            },
            OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::I32(42)),
                from: OxTy::Long,
            },
        ],
    )
}

fn proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
    name: &str,
    return_ty: OxTy,
    return_inst: OxInst,
) -> OxProgram {
    proc_ref_unknown_signature_no_arg_function_statement_program(
        name,
        return_ty,
        OxTy::Long,
        vec![
            OxInst::Assign {
                dst: OxPlace::Global(GlobalId(1)),
                value: OxOperand::Const(OxConst::I32(42)),
            },
            return_inst,
        ],
    )
}

fn proc_ref_unknown_signature_long_sub_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: None,
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: vec![OxArg::ByVal(OxOperand::Const(OxConst::I32(42)))],
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let store_value = OxFunc {
        name: "StoreValue".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "value",
            OxTy::Long,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Global(GlobalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, store_value],
        globals: vec![
            OxGlobal {
                name: "f".to_string(),
                ty: OxTy::ProcRef,
                array_element: None,
            },
            OxGlobal {
                name: "result".to_string(),
                ty: OxTy::Long,
                array_element: None,
            },
        ],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_long_sub_with_byval_arg_program(arg: OxOperand) -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_sub_program();
    program.funcs[1].blocks[0].instrs = vec![OxInst::CallProcRef {
        dst: None,
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![OxArg::ByVal(arg)],
    }];
    program
}

fn proc_ref_unknown_signature_scalar_local_byval_long_sub_program(
    name: &str,
    ty: OxTy,
    value: OxOperand,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_sub_program();
    program.funcs[1].locals = vec![local(name, ty, None)];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value,
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_bool_local_byval_long_sub_program() -> OxProgram {
    proc_ref_unknown_signature_scalar_local_byval_long_sub_program(
        "flag",
        OxTy::Bool,
        OxOperand::Const(OxConst::Bool(true)),
    )
}

fn proc_ref_unknown_signature_byte_local_byval_long_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_sub_program();
    program.funcs[1].locals = vec![local("byte_value", OxTy::Byte, None)];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Coerce {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::I32(7)),
            target: OxCoerceTarget::Numeric(NumericCoerceTarget::Byte),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_variant_byval_long_sub_program(
    src: OxOperand,
    from: OxTy,
) -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_sub_program();
    program.funcs[1].locals = vec![local("payload", OxTy::Variant, None)];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(0)),
            src,
            from,
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_byte_variant_byval_long_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_sub_program();
    program.funcs[1].locals = vec![
        local("byte_value", OxTy::Byte, None),
        local("payload", OxTy::Variant, None),
    ];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Coerce {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::I32(7)),
            target: OxCoerceTarget::Numeric(NumericCoerceTarget::Byte),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Byte,
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::local(LocalId(1)))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_two_long_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_sub_program();
    program.funcs[1].blocks[0].instrs = vec![OxInst::CallProcRef {
        dst: None,
        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
        args: vec![
            OxArg::ByVal(OxOperand::Const(OxConst::I32(40))),
            OxArg::ByVal(OxOperand::Const(OxConst::I32(2))),
        ],
    }];
    program.funcs[2].name = "StoreSum".to_string();
    program.funcs[2].locals = vec![
        local(
            "left",
            OxTy::Long,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
        local(
            "right",
            OxTy::Long,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
    ];
    program.funcs[2].temps = vec![OxTy::Long];
    program.funcs[2].param_count = 2;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Arith {
            dst: OxPlace::Temp(TempId(0)),
            op: ArithOp::Add,
            lhs: OxOperand::local(LocalId(0)),
            rhs: OxOperand::local(LocalId(1)),
            mode: NumericMode::Checked(NumericCoerceTarget::Long),
        },
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::Use(OxPlace::Temp(TempId(0))),
        },
    ];
    program
}

fn proc_ref_unknown_signature_long_function_statement_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_sub_program();
    program.funcs[2].name = "StoreAndReturn".to_string();
    program.funcs[2].kind = ProcedureKind::Function;
    program.funcs[2].locals = vec![
        local(
            "value",
            OxTy::Long,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
        local("StoreAndReturn", OxTy::Long, None),
    ];
    program.funcs[2].return_local = Some(LocalId(1));
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::local(LocalId(0)),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(1)),
            value: OxOperand::local(LocalId(0)),
        },
    ];
    program
}

fn proc_ref_unknown_signature_long_variant_function_statement_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_function_statement_program();
    program.funcs[2].name = "StoreAndReturnVariant".to_string();
    program.funcs[2].locals[1].name = "StoreAndReturnVariant".to_string();
    program.funcs[2].locals[1].ty = OxTy::Variant;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Global(GlobalId(1)),
            value: OxOperand::local(LocalId(0)),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(1)),
            src: OxOperand::local(LocalId(0)),
            from: OxTy::Long,
        },
    ];
    program
}

fn proc_ref_unknown_signature_long_byref_sub_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![escaped_local("value", OxTy::Long, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::I32(41)),
                    },
                    OxInst::CallProcRef {
                        dst: None,
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: vec![OxArg::ByRef(OxPlace::Local(LocalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let set_value = OxFunc {
        name: "SetValue".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "value",
            OxTy::Long,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::I32(42)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, set_value],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_long_byref_byval_sub_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_byref_sub_program();
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::I32(40)),
        },
        OxInst::CallProcRef {
            dst: None,
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![
                OxArg::ByRef(OxPlace::Local(LocalId(0))),
                OxArg::ByVal(OxOperand::Const(OxConst::I32(2))),
            ],
        },
    ];
    program.funcs[2].name = "AddAndStoreSub".to_string();
    program.funcs[2].locals = vec![
        local(
            "value",
            OxTy::Long,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        ),
        local(
            "delta",
            OxTy::Long,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
        ),
    ];
    program.funcs[2].temps = vec![OxTy::Long];
    program.funcs[2].param_count = 2;
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Arith {
            dst: OxPlace::Temp(TempId(0)),
            op: ArithOp::Add,
            lhs: OxOperand::local(LocalId(0)),
            rhs: OxOperand::local(LocalId(1)),
            mode: NumericMode::Checked(NumericCoerceTarget::Long),
        },
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Use(OxPlace::Temp(TempId(0))),
        },
    ];
    program
}

fn proc_ref_unknown_signature_long_return_program(dst_ty: OxTy) -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("result", dst_ty, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: Some(OxPlace::Local(LocalId(0))),
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: vec![OxArg::ByVal(OxOperand::Const(OxConst::I32(42)))],
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let echo_long = OxFunc {
        name: "EchoLong".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoLong", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, echo_long],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_long_variant_arg_return_program(dst_ty: OxTy) -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_return_program(dst_ty.clone());
    program.funcs[1].locals = vec![
        local("payload", OxTy::Variant, None),
        local("result", dst_ty, None),
    ];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::I32(42)),
            from: OxTy::Long,
        },
        OxInst::CallProcRef {
            dst: Some(OxPlace::Local(LocalId(1))),
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_variant_arg_variant_return_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_long_arg_variant_return_program();
    program.funcs[1].locals = vec![
        local("payload", OxTy::Variant, None),
        local("result", OxTy::Variant, None),
    ];
    program.funcs[1].blocks[0].instrs = vec![
        OxInst::Box {
            dst: OxPlace::Local(LocalId(0)),
            src: OxOperand::Const(OxConst::I32(42)),
            from: OxTy::Long,
        },
        OxInst::CallProcRef {
            dst: Some(OxPlace::Local(LocalId(1))),
            target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
            args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
        },
    ];
    program
}

fn proc_ref_unknown_signature_long_arg_variant_return_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("result", OxTy::Variant, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: Some(OxPlace::Local(LocalId(0))),
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: vec![OxArg::ByVal(OxOperand::Const(OxConst::I32(42)))],
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let echo_variant = OxFunc {
        name: "EchoVariant".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                OxTy::Long,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoVariant", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Box {
                    dst: OxPlace::Local(LocalId(1)),
                    src: OxOperand::local(LocalId(0)),
                    from: OxTy::Long,
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, echo_variant],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_two_long_return_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local("result", OxTy::Long, None)],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::CallProcRef {
                    dst: Some(OxPlace::Local(LocalId(0))),
                    target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                    args: vec![
                        OxArg::ByVal(OxOperand::Const(OxConst::I32(40))),
                        OxArg::ByVal(OxOperand::Const(OxConst::I32(2))),
                    ],
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let add_longs = OxFunc {
        name: "AddLongs".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "left",
                OxTy::Long,
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local(
                "right",
                OxTy::Long,
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("AddLongs", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 2,
        return_local: Some(LocalId(2)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Arith {
                    dst: OxPlace::Local(LocalId(2)),
                    op: ArithOp::Add,
                    lhs: OxOperand::local(LocalId(0)),
                    rhs: OxOperand::local(LocalId(1)),
                    mode: NumericMode::Checked(NumericCoerceTarget::Long),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, add_longs],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_two_long_variant_return_program() -> OxProgram {
    let mut program = proc_ref_unknown_signature_two_long_return_program();
    program.funcs[1].locals[0].ty = OxTy::Variant;
    program.funcs[2].name = "AddLongsVariant".to_string();
    program.funcs[2].locals[2].name = "AddLongsVariant".to_string();
    program.funcs[2].locals[2].ty = OxTy::Variant;
    program.funcs[2].temps = vec![OxTy::Long];
    program.funcs[2].blocks[0].instrs = vec![
        OxInst::Arith {
            dst: OxPlace::Temp(TempId(0)),
            op: ArithOp::Add,
            lhs: OxOperand::local(LocalId(0)),
            rhs: OxOperand::local(LocalId(1)),
            mode: NumericMode::Checked(NumericCoerceTarget::Long),
        },
        OxInst::Box {
            dst: OxPlace::Local(LocalId(2)),
            src: OxOperand::Use(OxPlace::Temp(TempId(0))),
            from: OxTy::Long,
        },
    ];
    program
}

fn proc_ref_unknown_signature_long_byref_return_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            escaped_local("value", OxTy::Long, None),
            local("result", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::I32(40)),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: vec![
                            OxArg::ByRef(OxPlace::Local(LocalId(0))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(2))),
                        ],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let add_and_store = OxFunc {
        name: "AddAndStore".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                OxTy::Long,
                Some(OxParamInfo {
                    optional: false,
                    by_ref: true,
                    variadic: false,
                }),
            ),
            local(
                "delta",
                OxTy::Long,
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("AddAndStore", OxTy::Long, None),
        ],
        temps: Vec::new(),
        param_count: 2,
        return_local: Some(LocalId(2)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Arith {
                        dst: OxPlace::Local(LocalId(0)),
                        op: ArithOp::Add,
                        lhs: OxOperand::local(LocalId(0)),
                        rhs: OxOperand::local(LocalId(1)),
                        mode: NumericMode::Checked(NumericCoerceTarget::Long),
                    },
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(2)),
                        value: OxOperand::local(LocalId(0)),
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, add_and_store],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_unknown_signature_long_byref_variant_return_program() -> OxProgram {
    let init = OxFunc {
        name: "__GlobalInit".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::LoadProcRef {
                    dst: OxPlace::Global(GlobalId(0)),
                    proc: FuncId(2),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            escaped_local("value", OxTy::Long, None),
            local("result", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::I32(40)),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::Use(OxPlace::Global(GlobalId(0))),
                        args: vec![
                            OxArg::ByRef(OxPlace::Local(LocalId(0))),
                            OxArg::ByVal(OxOperand::Const(OxConst::I32(2))),
                        ],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let add_and_store_variant = OxFunc {
        name: "AddAndStoreVariant".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                OxTy::Long,
                Some(OxParamInfo {
                    optional: false,
                    by_ref: true,
                    variadic: false,
                }),
            ),
            local(
                "delta",
                OxTy::Long,
                Some(OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("AddAndStoreVariant", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 2,
        return_local: Some(LocalId(2)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Arith {
                        dst: OxPlace::Local(LocalId(0)),
                        op: ArithOp::Add,
                        lhs: OxOperand::local(LocalId(0)),
                        rhs: OxOperand::local(LocalId(1)),
                        mode: NumericMode::Checked(NumericCoerceTarget::Long),
                    },
                    OxInst::Box {
                        dst: OxPlace::Local(LocalId(2)),
                        src: OxOperand::local(LocalId(0)),
                        from: OxTy::Long,
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![init, main, add_and_store_variant],
        globals: vec![OxGlobal {
            name: "f".to_string(),
            ty: OxTy::ProcRef,
            array_element: None,
        }],
        entry: Some(FuncId(1)),
        global_initializer: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_string_return_to_variant_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            local("f", OxTy::ProcRef, None),
            local("value", OxTy::Variant, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(0)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(1))),
                        target: OxOperand::local(LocalId(0)),
                        args: vec![OxArg::ByVal(OxOperand::Const(OxConst::Str(
                            "alpha".to_string(),
                        )))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let echo_string = OxFunc {
        name: "EchoString".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![
            local(
                "value",
                OxTy::Str,
                Some(oxvba_oxir::OxParamInfo {
                    optional: false,
                    by_ref: false,
                    variadic: false,
                }),
            ),
            local("EchoString", OxTy::Str, None),
        ],
        temps: Vec::new(),
        param_count: 1,
        return_local: Some(LocalId(1)),
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(1)),
                    value: OxOperand::local(LocalId(0)),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, echo_string],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_string_byref_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            escaped_local("value", OxTy::Str, None),
            local("f", OxTy::ProcRef, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(1)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: None,
                        target: OxOperand::local(LocalId(1)),
                        args: vec![OxArg::ByRef(OxPlace::Local(LocalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let set_string = OxFunc {
        name: "SetString".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "value",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Str("beta".to_string())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, set_string],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

fn proc_ref_ambiguous_same_signature_string_byref_program() -> OxProgram {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![
            escaped_local("value", OxTy::Str, None),
            local("f", OxTy::ProcRef, None),
        ],
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::Str("alpha".to_string())),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(1)),
                        proc: FuncId(1),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(1)),
                        proc: FuncId(2),
                    },
                    OxInst::CallProcRef {
                        dst: None,
                        target: OxOperand::local(LocalId(1)),
                        args: vec![OxArg::ByRef(OxPlace::Local(LocalId(0)))],
                    },
                ],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let set_alpha = OxFunc {
        name: "SetAlpha".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "value",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Str("gamma".to_string())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    let set_beta = OxFunc {
        name: "SetBeta".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![local(
            "value",
            OxTy::Str,
            Some(oxvba_oxir::OxParamInfo {
                optional: false,
                by_ref: true,
                variadic: false,
            }),
        )],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![
            OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Str("beta".to_string())),
                }],
                fault_target: Some(BlockId(1)),
                terminator: OxTerminator::Return,
            },
            OxBlock {
                id: BlockId(1),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::FaultDispatch {
                    resume: BlockId(0),
                    resume_next: BlockId(2),
                },
            },
            OxBlock {
                id: BlockId(2),
                instrs: Vec::new(),
                fault_target: None,
                terminator: OxTerminator::Return,
            },
        ],
        entry: BlockId(0),
    };
    OxProgram {
        funcs: vec![main, set_alpha, set_beta],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    }
}

#[test]
fn jit_call_proc_ref_dispatches_through_address_of() {
    let program = proc_ref_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(2).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.get(1).and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_two_long_arguments() {
    let program = proc_ref_two_arg_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_four_long_arguments() {
    let program = proc_ref_four_arg_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(10));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_five_long_arguments() {
    let program = proc_ref_five_arg_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(15));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_known_double_return() {
    let program = proc_ref_double_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_f64), Some(21.25));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_known_long_return_to_variant() {
    let program = proc_ref_long_return_to_variant_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_ambiguous_same_signature_double_return() {
    let program = proc_ref_ambiguous_same_signature_double_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_f64), Some(23.0));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_ambiguous_same_signature_double_return_to_variant() {
    let program = proc_ref_ambiguous_same_signature_double_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(2).and_then(Variant::as_f64), Some(24.5));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_known_currency_byref() {
    let program = proc_ref_currency_byref_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .first()
            .and_then(Variant::as_currency_scaled_i64),
        Some(125_000)
    );
    assert_eq!(
        outcome.values.get(1).and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_known_string_return() {
    let program = proc_ref_string_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_ambiguous_same_signature_string_return() {
    let program = proc_ref_ambiguous_same_signature_string_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_ambiguous_same_signature_string_byval_return() {
    let program = proc_ref_ambiguous_same_signature_string_byval_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_ambiguous_same_signature_string_return_to_variant() {
    let program = proc_ref_ambiguous_same_signature_string_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_known_string_return_to_variant() {
    let program = proc_ref_string_return_to_variant_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_return() {
    let program = proc_ref_unknown_signature_string_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_return_to_variant() {
    let program = proc_ref_unknown_signature_string_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byval_return() {
    let program = proc_ref_unknown_signature_string_byval_return_program(OxTy::Str);
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_string_byval_return() {
    let program = proc_ref_unknown_signature_two_string_byval_return_program(OxTy::Str);
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_string_byval_return_to_variant() {
    let program = proc_ref_unknown_signature_two_string_byval_return_program(OxTy::Variant);
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_string_variant_byval_return() {
    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program = proc_ref_unknown_signature_two_string_variant_byval_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("beta".to_string())
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_two_string_variant_byval_selected_payloads() {
    let cases = vec![
        (
            "long",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I32(42)),
                from: OxTy::Long,
            }],
            "42",
        ),
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        (
            "single",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F32(12.5f32.to_bits())),
                from: OxTy::Single,
            }],
            "12.5",
        ),
        (
            "currency",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Currency(123_456)),
                from: OxTy::Currency,
            }],
            "12.3456",
        ),
        (
            "integer",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I16(44)),
                from: OxTy::Integer,
            }],
            "44",
        ),
        (
            "longlong",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I64(5_000_000_012)),
                from: OxTy::LongLong,
            }],
            "5000000012",
        ),
        (
            "date",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                from: OxTy::Date,
            }],
            "1/15/2020",
        ),
        (
            "string",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Str("omega".to_string())),
                from: OxTy::Str,
            }],
            "omega",
        ),
        ("empty", Vec::new(), ""),
    ];

    for (label, setup, expected) in cases {
        let program =
            proc_ref_unknown_signature_two_string_variant_byval_sub_with_second_setup_program(
                setup.clone(),
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: sub");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: sub: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: sub"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: sub"
        );

        for dst_ty in [OxTy::Str, OxTy::Variant] {
            let program =
                proc_ref_unknown_signature_two_string_variant_byval_return_with_second_setup_program(
                    dst_ty,
                    setup.clone(),
                );
            assert_eq!(verify_program(&program), Ok(()), "{label}: return");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: return: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(3)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: return"
            );
        }

        let program =
            proc_ref_unknown_signature_two_string_variant_byval_variant_return_with_second_setup_program(
                setup,
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: variant-return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(
            !outcome.raised,
            "{label}: variant-return: {:?}",
            outcome.err
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: variant-return"
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_two_string_variant_byval_byte_payload() {
    let programs = vec![
        (
            "sub",
            proc_ref_unknown_signature_two_string_variant_byval_byte_sub_program(),
            1,
        ),
        (
            "actual-variant-return",
            proc_ref_unknown_signature_two_string_variant_byval_byte_variant_return_program(),
            3,
        ),
    ];
    for (label, program, result_index) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(result_index)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("7".to_string()),
            "{label}"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}"
        );
    }

    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program =
            proc_ref_unknown_signature_two_string_variant_byval_byte_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()), "return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "return: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("7".to_string()),
            "return"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "return"
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_two_string_variant_byval_error_decimal_payloads() {
    let cases = vec![
        (
            "error",
            "CVErr",
            OxOperand::Const(OxConst::I32(1234)),
            "Error 1234",
        ),
        (
            "decimal",
            "CDec",
            OxOperand::Const(OxConst::I32(12345)),
            "12345",
        ),
    ];

    for (label, member, src, expected) in cases {
        let program =
            proc_ref_unknown_signature_two_string_variant_byval_extern_payload_sub_program(
                member,
                src.clone(),
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: sub");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: sub: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: sub"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: sub"
        );

        for dst_ty in [OxTy::Str, OxTy::Variant] {
            let program =
                proc_ref_unknown_signature_two_string_variant_byval_extern_payload_return_program(
                    member,
                    src.clone(),
                    dst_ty,
                );
            assert_eq!(verify_program(&program), Ok(()), "{label}: return");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: return: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(3)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: return"
            );
        }

        let program =
            proc_ref_unknown_signature_two_string_variant_byval_extern_payload_variant_return_program(
                member,
                src,
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: variant-return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(
            !outcome.raised,
            "{label}: variant-return: {:?}",
            outcome.err
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: variant-return"
        );
    }
}

#[test]
fn jit_call_proc_ref_seats_unknown_signature_two_string_variant_byval_null_payload_errors() {
    let null_setup = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(1)),
        value: OxOperand::Const(OxConst::Null),
    }];
    let programs = vec![
        (
            "sub",
            proc_ref_unknown_signature_two_string_variant_byval_sub_with_second_setup_program(
                null_setup.clone(),
            ),
        ),
        (
            "string-dst",
            proc_ref_unknown_signature_two_string_variant_byval_return_with_second_setup_program(
                OxTy::Str,
                null_setup.clone(),
            ),
        ),
        (
            "variant-dst",
            proc_ref_unknown_signature_two_string_variant_byval_return_with_second_setup_program(
                OxTy::Variant,
                null_setup.clone(),
            ),
        ),
        (
            "actual-variant-return",
            proc_ref_unknown_signature_two_string_variant_byval_variant_return_with_second_setup_program(
                null_setup,
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised, "{label}");
        assert_eq!(outcome.err.number, 94, "{label}");
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_mixed_string_variant_byval_return() {
    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program = proc_ref_unknown_signature_mixed_string_variant_byval_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("beta".to_string())
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_mixed_string_variant_byval_variant_return() {
    let program = proc_ref_unknown_signature_mixed_string_variant_byval_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(3)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_mixed_string_variant_byval_selected_payloads() {
    let cases = vec![
        (
            "long",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I32(42)),
                from: OxTy::Long,
            }],
            "42",
        ),
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        (
            "single",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F32(12.5f32.to_bits())),
                from: OxTy::Single,
            }],
            "12.5",
        ),
        (
            "currency",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Currency(123_456)),
                from: OxTy::Currency,
            }],
            "12.3456",
        ),
        (
            "integer",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I16(44)),
                from: OxTy::Integer,
            }],
            "44",
        ),
        (
            "longlong",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I64(5_000_000_012)),
                from: OxTy::LongLong,
            }],
            "5000000012",
        ),
        (
            "date",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                from: OxTy::Date,
            }],
            "1/15/2020",
        ),
        (
            "string",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Str("omega".to_string())),
                from: OxTy::Str,
            }],
            "omega",
        ),
        ("empty", Vec::new(), ""),
    ];
    for (label, setup, expected) in cases {
        let program = proc_ref_unknown_signature_mixed_string_variant_byval_sub_with_setup_program(
            setup.clone(),
        );
        assert_eq!(verify_program(&program), Ok(()), "{label}: sub");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: sub: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: sub"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: sub"
        );

        for dst_ty in [OxTy::Str, OxTy::Variant] {
            let program =
                proc_ref_unknown_signature_mixed_string_variant_byval_return_with_setup_program(
                    dst_ty,
                    setup.clone(),
                );
            assert_eq!(verify_program(&program), Ok(()), "{label}: return");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: return: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(3)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: return"
            );
        }

        let program =
            proc_ref_unknown_signature_mixed_string_variant_byval_variant_return_with_setup_program(
                setup,
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: variant-return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(
            !outcome.raised,
            "{label}: variant-return: {:?}",
            outcome.err
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: variant-return"
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_mixed_string_variant_byval_error_decimal_payloads() {
    let cases = vec![
        (
            "error",
            "CVErr",
            OxOperand::Const(OxConst::I32(1234)),
            "Error 1234",
        ),
        (
            "decimal",
            "CDec",
            OxOperand::Const(OxConst::I32(12345)),
            "12345",
        ),
    ];

    for (label, member, src, expected) in cases {
        let program =
            proc_ref_unknown_signature_mixed_string_variant_byval_extern_payload_sub_program(
                member,
                src.clone(),
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: sub");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: sub: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: sub"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: sub"
        );

        for dst_ty in [OxTy::Str, OxTy::Variant] {
            let program =
                proc_ref_unknown_signature_mixed_string_variant_byval_extern_payload_return_program(
                    member,
                    src.clone(),
                    dst_ty,
                );
            assert_eq!(verify_program(&program), Ok(()), "{label}: return");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: return: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(3)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: return"
            );
        }

        let program =
            proc_ref_unknown_signature_mixed_string_variant_byval_extern_payload_variant_return_program(
                member,
                src,
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: variant-return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(
            !outcome.raised,
            "{label}: variant-return: {:?}",
            outcome.err
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: variant-return"
        );
    }
}

#[test]
fn jit_call_proc_ref_seats_unknown_signature_mixed_string_variant_byval_null_payload_errors() {
    let null_setup = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(1)),
        value: OxOperand::Const(OxConst::Null),
    }];
    let programs = vec![
        (
            "sub",
            proc_ref_unknown_signature_mixed_string_variant_byval_sub_with_setup_program(
                null_setup.clone(),
            ),
        ),
        (
            "string-dst",
            proc_ref_unknown_signature_mixed_string_variant_byval_return_with_setup_program(
                OxTy::Str,
                null_setup.clone(),
            ),
        ),
        (
            "variant-dst",
            proc_ref_unknown_signature_mixed_string_variant_byval_return_with_setup_program(
                OxTy::Variant,
                null_setup.clone(),
            ),
        ),
        (
            "actual-variant-return",
            proc_ref_unknown_signature_mixed_string_variant_byval_variant_return_with_setup_program(
                null_setup,
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised, "{label}");
        assert_eq!(outcome.err.number, 94, "{label}");
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byval_sub() {
    let program = proc_ref_unknown_signature_string_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_global_byval_sub() {
    let program = proc_ref_unknown_signature_string_global_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_temp_byval_sub() {
    let program = proc_ref_unknown_signature_string_temp_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_variant_byval_sub() {
    let program = proc_ref_unknown_signature_string_variant_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_variant_byval_sub_long_payload() {
    let program = proc_ref_unknown_signature_string_variant_byval_sub_long_payload_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("42".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_variant_byval_sub_selected_payloads() {
    let cases = vec![
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        (
            "single",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::F32(12.5f32.to_bits())),
                from: OxTy::Single,
            }],
            "12.5",
        ),
        (
            "currency",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Currency(123_456)),
                from: OxTy::Currency,
            }],
            "12.3456",
        ),
        (
            "integer",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::I16(44)),
                from: OxTy::Integer,
            }],
            "44",
        ),
        (
            "longlong",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::I64(5_000_000_012)),
                from: OxTy::LongLong,
            }],
            "5000000012",
        ),
        (
            "date",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                from: OxTy::Date,
            }],
            "1/15/2020",
        ),
        (
            "string",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Str("omega".to_string())),
                from: OxTy::Str,
            }],
            "omega",
        ),
        ("empty", Vec::new(), ""),
    ];
    for (label, setup, expected) in cases {
        let program = proc_ref_unknown_signature_string_variant_byval_sub_with_setup_program(setup);
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}"
        );
    }
}

#[test]
fn jit_call_proc_ref_seats_unknown_signature_string_variant_byval_sub_null_payload_error() {
    let program = proc_ref_unknown_signature_string_variant_byval_sub_with_setup_program(vec![
        OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Null),
        },
    ]);
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(outcome.raised);
    assert_eq!(outcome.err.number, 94);
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_variant_byval_return() {
    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program = proc_ref_unknown_signature_string_variant_byval_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(2)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("alpha".to_string())
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_variant_byval_return_long_payload() {
    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program =
            proc_ref_unknown_signature_string_variant_byval_return_long_payload_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(2)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("42".to_string())
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_variant_byval_return_selected_payloads() {
    let cases = vec![
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        (
            "single",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::F32(12.5f32.to_bits())),
                from: OxTy::Single,
            }],
            "12.5",
        ),
        (
            "currency",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Currency(123_456)),
                from: OxTy::Currency,
            }],
            "12.3456",
        ),
        (
            "integer",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::I16(44)),
                from: OxTy::Integer,
            }],
            "44",
        ),
        (
            "longlong",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::I64(5_000_000_012)),
                from: OxTy::LongLong,
            }],
            "5000000012",
        ),
        (
            "date",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                from: OxTy::Date,
            }],
            "1/15/2020",
        ),
        (
            "string",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Str("omega".to_string())),
                from: OxTy::Str,
            }],
            "omega",
        ),
        ("empty", Vec::new(), ""),
    ];
    for (label, setup, expected) in cases {
        for dst_ty in [OxTy::Str, OxTy::Variant] {
            let program = proc_ref_unknown_signature_string_variant_byval_return_with_setup_program(
                dst_ty,
                setup.clone(),
            );
            assert_eq!(verify_program(&program), Ok(()), "{label}");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(2)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}"
            );
        }
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_variant_byval_error_decimal_payloads() {
    let cases = vec![
        (
            "error",
            "CVErr",
            OxOperand::Const(OxConst::I32(1234)),
            "Error 1234",
        ),
        (
            "decimal",
            "CDec",
            OxOperand::Const(OxConst::I32(12345)),
            "12345",
        ),
    ];

    for (label, member, src, expected) in cases {
        let program = proc_ref_unknown_signature_string_variant_byval_extern_payload_sub_program(
            member,
            src.clone(),
        );
        assert_eq!(verify_program(&program), Ok(()), "{label}: sub");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: sub: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: sub"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: sub"
        );

        for dst_ty in [OxTy::Str, OxTy::Variant] {
            let program =
                proc_ref_unknown_signature_string_variant_byval_extern_payload_return_program(
                    member,
                    src.clone(),
                    dst_ty,
                );
            assert_eq!(verify_program(&program), Ok(()), "{label}: return");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: return: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(2)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: return"
            );
        }

        let program =
            proc_ref_unknown_signature_string_variant_byval_extern_payload_variant_return_program(
                member, src,
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: variant-return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(
            !outcome.raised,
            "{label}: variant-return: {:?}",
            outcome.err
        );
        assert_eq!(
            outcome
                .values
                .get(2)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: variant-return"
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_variant_byval_byte_payloads() {
    let mut scenarios = vec![
        (
            "string-sub",
            proc_ref_unknown_signature_string_variant_byval_byte_sub_program(),
            1usize,
        ),
        (
            "string-variant-return",
            proc_ref_unknown_signature_string_variant_byval_byte_variant_return_program(),
            2usize,
        ),
        (
            "mixed-sub",
            proc_ref_unknown_signature_mixed_string_variant_byval_byte_sub_program(),
            1usize,
        ),
        (
            "mixed-variant-return",
            proc_ref_unknown_signature_mixed_string_variant_byval_byte_variant_return_program(),
            3usize,
        ),
    ];
    for dst_ty in [OxTy::Str, OxTy::Variant] {
        scenarios.push((
            "string-return",
            proc_ref_unknown_signature_string_variant_byval_byte_return_program(dst_ty.clone()),
            2usize,
        ));
        scenarios.push((
            "mixed-return",
            proc_ref_unknown_signature_mixed_string_variant_byval_byte_return_program(dst_ty),
            3usize,
        ));
    }

    for (label, program, result_index) in scenarios {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(result_index)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("7".to_string()),
            "{label}"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}"
        );
    }
}

#[test]
fn jit_call_proc_ref_seats_unknown_signature_string_variant_byval_return_null_payload_errors() {
    let null_setup = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(0)),
        value: OxOperand::Const(OxConst::Null),
    }];
    let programs = vec![
        (
            "string-dst",
            proc_ref_unknown_signature_string_variant_byval_return_with_setup_program(
                OxTy::Str,
                null_setup.clone(),
            ),
        ),
        (
            "variant-dst",
            proc_ref_unknown_signature_string_variant_byval_return_with_setup_program(
                OxTy::Variant,
                null_setup.clone(),
            ),
        ),
        (
            "actual-variant-return",
            proc_ref_unknown_signature_string_variant_byval_variant_return_with_setup_program(
                null_setup,
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised, "{label}");
        assert_eq!(outcome.err.number, 94, "{label}");
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_fixed_local_byval_sub() {
    let program = proc_ref_unknown_signature_string_fixed_local_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("ab ".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byref_sub() {
    let program = proc_ref_unknown_signature_string_byref_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_sub() {
    let program = proc_ref_unknown_signature_string_byref_variant_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("42".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byref_return() {
    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program = proc_ref_unknown_signature_string_byref_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("beta".to_string())
        );
        assert_eq!(
            outcome
                .values
                .get(2)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("beta".to_string())
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_return() {
    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program = proc_ref_unknown_signature_string_byref_variant_byval_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("42".to_string())
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("42".to_string())
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byref_variant_return() {
    let program = proc_ref_unknown_signature_string_byref_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_variant_return() {
    let program = proc_ref_unknown_signature_string_byref_variant_byval_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("42".to_string())
    );
    assert_eq!(
        outcome
            .values
            .get(3)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("42".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_selected_payloads() {
    let cases = vec![
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        ("empty", Vec::new(), ""),
    ];

    for (label, setup, expected) in cases {
        let program = proc_ref_unknown_signature_string_byref_variant_byval_sub_with_setup_program(
            setup.clone(),
        );
        assert_eq!(verify_program(&program), Ok(()), "{label}: sub");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: sub: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: sub alias"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: sub proc ref"
        );

        for dst_ty in [OxTy::Str, OxTy::Variant] {
            let program =
                proc_ref_unknown_signature_string_byref_variant_byval_return_with_setup_program(
                    dst_ty,
                    setup.clone(),
                );
            assert_eq!(verify_program(&program), Ok(()), "{label}: return");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: return: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return alias"
            );
            assert_eq!(
                outcome
                    .values
                    .get(3)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return result"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: return proc ref"
            );
        }

        let program =
            proc_ref_unknown_signature_string_byref_variant_byval_variant_return_with_setup_program(
                setup,
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: variant-return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(
            !outcome.raised,
            "{label}: variant-return: {:?}",
            outcome.err
        );
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return alias"
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return result"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: variant-return proc ref"
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_byte_payload() {
    let program = proc_ref_unknown_signature_string_byref_variant_byval_byte_sub_program();
    assert_eq!(verify_program(&program), Ok(()), "sub");
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "sub: {:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("7".to_string()),
        "sub alias"
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2),
        "sub proc ref"
    );

    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program =
            proc_ref_unknown_signature_string_byref_variant_byval_byte_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()), "return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "return: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("7".to_string()),
            "return alias"
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("7".to_string()),
            "return result"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "return proc ref"
        );
    }

    let program =
        proc_ref_unknown_signature_string_byref_variant_byval_byte_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()), "variant-return");
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "variant-return: {:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("7".to_string()),
        "variant-return alias"
    );
    assert_eq!(
        outcome
            .values
            .get(3)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("7".to_string()),
        "variant-return result"
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2),
        "variant-return proc ref"
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_error_decimal_payloads() {
    let cases = vec![
        (
            "error",
            "CVErr",
            OxOperand::Const(OxConst::I32(1234)),
            "Error 1234",
        ),
        (
            "decimal",
            "CDec",
            OxOperand::Const(OxConst::I32(12345)),
            "12345",
        ),
    ];

    for (label, member, src, expected) in cases {
        let program =
            proc_ref_unknown_signature_string_byref_variant_byval_extern_payload_sub_program(
                member,
                src.clone(),
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: sub");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: sub: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: sub alias"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: sub proc ref"
        );

        for dst_ty in [OxTy::Str, OxTy::Variant] {
            let program =
                proc_ref_unknown_signature_string_byref_variant_byval_extern_payload_return_program(
                    member,
                    src.clone(),
                    dst_ty,
                );
            assert_eq!(verify_program(&program), Ok(()), "{label}: return");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: return: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return alias"
            );
            assert_eq!(
                outcome
                    .values
                    .get(3)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: return result"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: return proc ref"
            );
        }

        let program =
            proc_ref_unknown_signature_string_byref_variant_byval_extern_payload_variant_return_program(
                member,
                src,
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}: variant-return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(
            !outcome.raised,
            "{label}: variant-return: {:?}",
            outcome.err
        );
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return alias"
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: variant-return result"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: variant-return proc ref"
        );
    }
}

#[test]
fn jit_call_proc_ref_seats_unknown_signature_string_byref_variant_byval_null_payload_errors() {
    let null_setup = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(1)),
        value: OxOperand::Const(OxConst::Null),
    }];
    let programs = vec![
        (
            "sub",
            proc_ref_unknown_signature_string_byref_variant_byval_sub_with_setup_program(
                null_setup.clone(),
            ),
        ),
        (
            "string-dst",
            proc_ref_unknown_signature_string_byref_variant_byval_return_with_setup_program(
                OxTy::Str,
                null_setup.clone(),
            ),
        ),
        (
            "variant-dst",
            proc_ref_unknown_signature_string_byref_variant_byval_return_with_setup_program(
                OxTy::Variant,
                null_setup.clone(),
            ),
        ),
        (
            "actual-variant-return",
            proc_ref_unknown_signature_string_byref_variant_byval_variant_return_with_setup_program(
                null_setup,
            ),
        ),
    ];

    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised, "{label}");
        assert_eq!(outcome.err.number, 94, "{label}");
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_function_statement_selected_payloads()
 {
    let cases = vec![
        (
            "long",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I32(42)),
                from: OxTy::Long,
            }],
            "42",
        ),
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        (
            "single",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F32(12.5f32.to_bits())),
                from: OxTy::Single,
            }],
            "12.5",
        ),
        (
            "currency",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Currency(123_456)),
                from: OxTy::Currency,
            }],
            "12.3456",
        ),
        (
            "integer",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I16(44)),
                from: OxTy::Integer,
            }],
            "44",
        ),
        (
            "longlong",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I64(5_000_000_012)),
                from: OxTy::LongLong,
            }],
            "5000000012",
        ),
        (
            "date",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                from: OxTy::Date,
            }],
            "1/15/2020",
        ),
        (
            "string",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Str("omega".to_string())),
                from: OxTy::Str,
            }],
            "omega",
        ),
        ("empty", Vec::new(), ""),
    ];

    for (label, setup, expected) in cases {
        let programs = vec![
            (
                "string-fn",
                proc_ref_unknown_signature_string_byref_variant_byval_function_statement_with_setup_program(
                    setup.clone(),
                ),
            ),
            (
                "variant-fn",
                proc_ref_unknown_signature_string_byref_variant_byval_variant_function_statement_with_setup_program(
                    setup,
                ),
            ),
        ];
        for (shape, program) in programs {
            assert_eq!(verify_program(&program), Ok(()), "{label}: {shape}");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: {shape}: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: {shape}: alias"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: {shape}: proc ref"
            );
        }
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_function_statement_byte_payload()
 {
    let programs = vec![
        (
            "string-fn",
            with_byte_payload_temp(
                proc_ref_unknown_signature_string_byref_variant_byval_function_statement_with_setup_program(
                    variant_byte_payload_setup(LocalId(1)),
                ),
            ),
        ),
        (
            "variant-fn",
            with_byte_payload_temp(
                proc_ref_unknown_signature_string_byref_variant_byval_variant_function_statement_with_setup_program(
                    variant_byte_payload_setup(LocalId(1)),
                ),
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("7".to_string()),
            "{label}: alias"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: proc ref"
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_byref_variant_byval_function_statement_error_decimal_payloads()
 {
    let cases = vec![
        (
            "error",
            "CVErr",
            OxOperand::Const(OxConst::I32(1234)),
            "Error 1234",
        ),
        (
            "decimal",
            "CDec",
            OxOperand::Const(OxConst::I32(12345)),
            "12345",
        ),
    ];

    for (label, member, src, expected) in cases {
        let programs = vec![
            (
                "string-fn",
                with_vba_conversion_import(
                    proc_ref_unknown_signature_string_byref_variant_byval_function_statement_with_setup_program(
                        extern_payload_setup(LocalId(1), src.clone()),
                    ),
                    member,
                ),
            ),
            (
                "variant-fn",
                with_vba_conversion_import(
                    proc_ref_unknown_signature_string_byref_variant_byval_variant_function_statement_with_setup_program(
                        extern_payload_setup(LocalId(1), src),
                    ),
                    member,
                ),
            ),
        ];
        for (shape, program) in programs {
            assert_eq!(verify_program(&program), Ok(()), "{label}: {shape}");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: {shape}: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: {shape}: alias"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: {shape}: proc ref"
            );
        }
    }
}

#[test]
fn jit_call_proc_ref_seats_unknown_signature_string_byref_variant_byval_function_statement_null_payload_errors()
 {
    let null_setup = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(1)),
        value: OxOperand::Const(OxConst::Null),
    }];
    let programs = vec![
        (
            "string-fn",
            proc_ref_unknown_signature_string_byref_variant_byval_function_statement_with_setup_program(
                null_setup.clone(),
            ),
        ),
        (
            "variant-fn",
            proc_ref_unknown_signature_string_byref_variant_byval_variant_function_statement_with_setup_program(
                null_setup,
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised, "{label}");
        assert_eq!(outcome.err.number, 94, "{label}");
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byref_string_byval_source_place() {
    let program = proc_ref_unknown_signature_string_byref_string_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()), "sub");
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "sub: {:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("gamma".to_string()),
        "sub alias"
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2),
        "sub proc ref"
    );

    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let program = proc_ref_unknown_signature_string_byref_string_byval_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()), "return");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "return: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("gamma".to_string()),
            "return alias"
        );
        assert_eq!(
            outcome
                .values
                .get(3)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("gamma".to_string()),
            "return result"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "return proc ref"
        );
    }

    let program = proc_ref_unknown_signature_string_byref_string_byval_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()), "variant-return");
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "variant-return: {:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("gamma".to_string()),
        "variant-return alias"
    );
    assert_eq!(
        outcome
            .values
            .get(3)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("gamma".to_string()),
        "variant-return result"
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2),
        "variant-return proc ref"
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byref_static_byval_carriers() {
    let cases = vec![
        (
            "literal",
            proc_ref_unknown_signature_string_byref_string_literal_byval_sub_program(),
            "delta",
        ),
        (
            "fixed-string",
            proc_ref_unknown_signature_string_byref_fixed_string_byval_sub_program(),
            "xy ",
        ),
    ];

    for (label, program, expected) in cases {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: alias"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: proc ref"
        );
    }

    for dst_ty in [OxTy::Str, OxTy::Variant] {
        let cases = vec![
            (
                "literal-return",
                proc_ref_unknown_signature_string_byref_string_literal_byval_return_program(
                    dst_ty.clone(),
                ),
                "delta",
                2,
            ),
            (
                "fixed-string-return",
                proc_ref_unknown_signature_string_byref_fixed_string_byval_return_program(dst_ty),
                "xy ",
                3,
            ),
        ];

        for (label, program, expected, result_index) in cases {
            assert_eq!(verify_program(&program), Ok(()), "{label}");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: alias"
            );
            assert_eq!(
                outcome
                    .values
                    .get(result_index)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: result"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: proc ref"
            );
        }
    }

    let cases = vec![
        (
            "literal-variant-return",
            proc_ref_unknown_signature_string_byref_string_literal_byval_variant_return_program(),
            "delta",
            2,
        ),
        (
            "fixed-string-variant-return",
            proc_ref_unknown_signature_string_byref_fixed_string_byval_variant_return_program(),
            "xy ",
            3,
        ),
    ];

    for (label, program, expected, result_index) in cases {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: alias"
        );
        assert_eq!(
            outcome
                .values
                .get(result_index)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: result"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: proc ref"
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byref_static_byval_function_statements() {
    let cases = vec![
        (
            "source-place-string-fn",
            proc_ref_unknown_signature_string_byref_string_byval_function_statement_program(),
            "gamma",
        ),
        (
            "source-place-variant-fn",
            proc_ref_unknown_signature_string_byref_string_byval_variant_function_statement_program(),
            "gamma",
        ),
        (
            "literal-string-fn",
            proc_ref_unknown_signature_string_byref_string_literal_byval_function_statement_program(
            ),
            "delta",
        ),
        (
            "literal-variant-fn",
            proc_ref_unknown_signature_string_byref_string_literal_byval_variant_function_statement_program(
            ),
            "delta",
        ),
        (
            "fixed-string-fn",
            proc_ref_unknown_signature_string_byref_fixed_string_byval_function_statement_program(
            ),
            "xy ",
        ),
        (
            "fixed-variant-fn",
            proc_ref_unknown_signature_string_byref_fixed_string_byval_variant_function_statement_program(
            ),
            "xy ",
        ),
    ];

    for (label, program, expected) in cases {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}: alias"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}: proc ref"
        );
    }
}

#[test]
fn jit_call_proc_ref_declines_unknown_signature_mixed_long_string_byval_sub() {
    let program = proc_ref_unknown_signature_mixed_long_string_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let err = match engine.compile_image(&[&program]) {
        Ok(_) => panic!("mixed Long/String unknown-signature ProcRef unexpectedly compiled"),
        Err(err) => err,
    };
    let message = err
        .unsupported_message()
        .expect("mixed Long/String ProcRef should be an unsupported JIT boundary");
    assert!(
        message.contains("M4-4")
            && (message.contains("i32 operand")
                || message.contains("Long")
                || message.contains("String ByVal")),
        "{message}"
    );
}

#[test]
fn jit_call_proc_ref_declines_unknown_signature_wider_scalar_byval_long_sub() {
    let cases = vec![
        OxOperand::Const(OxConst::I64(5_000_000_012)),
        OxOperand::Const(OxConst::Currency(123_456)),
        OxOperand::Const(OxConst::F32(2.5f32.to_bits())),
        OxOperand::Const(OxConst::F64(42.5f64.to_bits())),
        OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
    ];
    for arg in cases {
        let program = proc_ref_unknown_signature_long_sub_with_byval_arg_program(arg.clone());
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let err = match engine.compile_image(&[&program]) {
            Ok(_) => {
                panic!("wider scalar unknown-signature ProcRef unexpectedly compiled for {arg:?}")
            }
            Err(err) => err,
        };
        let message = err
            .unsupported_message()
            .expect("wider scalar ProcRef should be an unsupported JIT boundary");
        assert!(
            message.contains("M4-4") && message.contains("i32 operand"),
            "{message}"
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byval_function_statement() {
    let program = proc_ref_unknown_signature_string_byval_function_statement_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byval_variant_function_statement() {
    let program = proc_ref_unknown_signature_string_byval_variant_function_statement_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_string_byval_sub() {
    let program = proc_ref_unknown_signature_two_string_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_mixed_string_variant_byval_sub() {
    let program = proc_ref_unknown_signature_mixed_string_variant_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_mixed_string_variant_byval_function_statement() {
    let program =
        proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_mixed_string_variant_byval_variant_function_statement()
 {
    let program =
        proc_ref_unknown_signature_mixed_string_variant_byval_variant_function_statement_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(1)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_mixed_string_variant_byval_function_statement_selected_payloads()
 {
    let cases = vec![
        (
            "long",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I32(42)),
                from: OxTy::Long,
            }],
            "42",
        ),
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        (
            "single",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F32(12.5f32.to_bits())),
                from: OxTy::Single,
            }],
            "12.5",
        ),
        (
            "currency",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Currency(123_456)),
                from: OxTy::Currency,
            }],
            "12.3456",
        ),
        (
            "integer",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I16(44)),
                from: OxTy::Integer,
            }],
            "44",
        ),
        (
            "longlong",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I64(5_000_000_012)),
                from: OxTy::LongLong,
            }],
            "5000000012",
        ),
        (
            "date",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                from: OxTy::Date,
            }],
            "1/15/2020",
        ),
        (
            "string",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Str("omega".to_string())),
                from: OxTy::Str,
            }],
            "omega",
        ),
        ("empty", Vec::new(), ""),
    ];

    for (label, setup, expected) in cases {
        let programs = vec![
            (
                "string-fn",
                proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_with_setup_program(
                    setup.clone(),
                ),
            ),
            (
                "variant-fn",
                proc_ref_unknown_signature_mixed_string_variant_byval_variant_function_statement_with_setup_program(
                    setup,
                ),
            ),
        ];
        for (shape, program) in programs {
            assert_eq!(verify_program(&program), Ok(()), "{label}: {shape}");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: {shape}: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: {shape}"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: {shape}"
            );
        }
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_mixed_string_variant_byval_function_statement_byte_payload()
 {
    let programs = vec![
        (
            "string-fn",
            with_byte_payload_temp(
                proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_with_setup_program(
                    variant_byte_payload_setup(LocalId(1)),
                ),
            ),
        ),
        (
            "variant-fn",
            with_byte_payload_temp(
                proc_ref_unknown_signature_mixed_string_variant_byval_variant_function_statement_with_setup_program(
                    variant_byte_payload_setup(LocalId(1)),
                ),
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("7".to_string()),
            "{label}"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}"
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_mixed_string_variant_byval_function_statement_error_decimal_payloads()
 {
    let cases = vec![
        (
            "error",
            "CVErr",
            OxOperand::Const(OxConst::I32(1234)),
            "Error 1234",
        ),
        (
            "decimal",
            "CDec",
            OxOperand::Const(OxConst::I32(12345)),
            "12345",
        ),
    ];

    for (label, member, src, expected) in cases {
        let programs = vec![
            (
                "string-fn",
                with_vba_conversion_import(
                    proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_with_setup_program(
                        extern_payload_setup(LocalId(1), src.clone()),
                    ),
                    member,
                ),
            ),
            (
                "variant-fn",
                with_vba_conversion_import(
                    proc_ref_unknown_signature_mixed_string_variant_byval_variant_function_statement_with_setup_program(
                        extern_payload_setup(LocalId(1), src),
                    ),
                    member,
                ),
            ),
        ];
        for (shape, program) in programs {
            assert_eq!(verify_program(&program), Ok(()), "{label}: {shape}");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: {shape}: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: {shape}"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: {shape}"
            );
        }
    }
}

#[test]
fn jit_call_proc_ref_seats_unknown_signature_mixed_string_variant_byval_function_statement_null_payload_errors()
 {
    let null_setup = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(1)),
        value: OxOperand::Const(OxConst::Null),
    }];
    let programs = vec![
        (
            "string-fn",
            proc_ref_unknown_signature_mixed_string_variant_byval_function_statement_with_setup_program(
                null_setup.clone(),
            ),
        ),
        (
            "variant-fn",
            proc_ref_unknown_signature_mixed_string_variant_byval_variant_function_statement_with_setup_program(
                null_setup,
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised, "{label}");
        assert_eq!(outcome.err.number, 94, "{label}");
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_two_string_variant_byval_function_statement_selected_payloads()
 {
    let cases = vec![
        (
            "long",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I32(42)),
                from: OxTy::Long,
            }],
            "42",
        ),
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        (
            "single",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::F32(12.5f32.to_bits())),
                from: OxTy::Single,
            }],
            "12.5",
        ),
        (
            "currency",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Currency(123_456)),
                from: OxTy::Currency,
            }],
            "12.3456",
        ),
        (
            "integer",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I16(44)),
                from: OxTy::Integer,
            }],
            "44",
        ),
        (
            "longlong",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::I64(5_000_000_012)),
                from: OxTy::LongLong,
            }],
            "5000000012",
        ),
        (
            "date",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                from: OxTy::Date,
            }],
            "1/15/2020",
        ),
        (
            "string",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(1)),
                src: OxOperand::Const(OxConst::Str("omega".to_string())),
                from: OxTy::Str,
            }],
            "omega",
        ),
        ("empty", Vec::new(), ""),
    ];

    for (label, setup, expected) in cases {
        let programs = vec![
            (
                "string-fn",
                proc_ref_unknown_signature_two_string_variant_byval_function_statement_with_second_setup_program(
                    setup.clone(),
                ),
            ),
            (
                "variant-fn",
                proc_ref_unknown_signature_two_string_variant_byval_variant_function_statement_with_second_setup_program(
                    setup,
                ),
            ),
        ];
        for (shape, program) in programs {
            assert_eq!(verify_program(&program), Ok(()), "{label}: {shape}");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: {shape}: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: {shape}"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: {shape}"
            );
        }
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_two_string_variant_byval_function_statement_byte_payload()
 {
    let programs = vec![
        (
            "string-fn",
            with_byte_payload_temp(
                proc_ref_unknown_signature_two_string_variant_byval_function_statement_with_second_setup_program(
                    variant_byte_payload_setup(LocalId(1)),
                ),
            ),
        ),
        (
            "variant-fn",
            with_byte_payload_temp(
                proc_ref_unknown_signature_two_string_variant_byval_variant_function_statement_with_second_setup_program(
                    variant_byte_payload_setup(LocalId(1)),
                ),
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(1)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some("7".to_string()),
            "{label}"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}"
        );
    }
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_two_string_variant_byval_function_statement_error_decimal_payloads()
 {
    let cases = vec![
        (
            "error",
            "CVErr",
            OxOperand::Const(OxConst::I32(1234)),
            "Error 1234",
        ),
        (
            "decimal",
            "CDec",
            OxOperand::Const(OxConst::I32(12345)),
            "12345",
        ),
    ];

    for (label, member, src, expected) in cases {
        let programs = vec![
            (
                "string-fn",
                with_vba_conversion_import(
                    proc_ref_unknown_signature_two_string_variant_byval_function_statement_with_second_setup_program(
                        extern_payload_setup(LocalId(1), src.clone()),
                    ),
                    member,
                ),
            ),
            (
                "variant-fn",
                with_vba_conversion_import(
                    proc_ref_unknown_signature_two_string_variant_byval_variant_function_statement_with_second_setup_program(
                        extern_payload_setup(LocalId(1), src),
                    ),
                    member,
                ),
            ),
        ];
        for (shape, program) in programs {
            assert_eq!(verify_program(&program), Ok(()), "{label}: {shape}");
            let engine = JitEngine;
            let compiled = engine.compile_image(&[&program]).expect("compile");
            let host = NullHostServices::new(HostPolicy::default());
            let outcome = compiled.run(&host).expect("run");
            assert!(!outcome.raised, "{label}: {shape}: {:?}", outcome.err);
            assert_eq!(
                outcome
                    .values
                    .get(1)
                    .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                Some(expected.to_string()),
                "{label}: {shape}"
            );
            assert_eq!(
                outcome.values.first().and_then(Variant::as_proc_ref),
                Some(2),
                "{label}: {shape}"
            );
        }
    }
}

#[test]
fn jit_call_proc_ref_seats_unknown_signature_two_string_variant_byval_function_statement_null_payload_errors()
 {
    let null_setup = vec![OxInst::Assign {
        dst: OxPlace::Local(LocalId(1)),
        value: OxOperand::Const(OxConst::Null),
    }];
    let programs = vec![
        (
            "string-fn",
            proc_ref_unknown_signature_two_string_variant_byval_function_statement_with_second_setup_program(
                null_setup.clone(),
            ),
        ),
        (
            "variant-fn",
            proc_ref_unknown_signature_two_string_variant_byval_variant_function_statement_with_second_setup_program(
                null_setup,
            ),
        ),
    ];
    for (label, program) in programs {
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised, "{label}");
        assert_eq!(outcome.err.number, 94, "{label}");
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byval_return_to_variant() {
    let program = proc_ref_unknown_signature_string_byval_return_program(OxTy::Variant);
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_byval_variant_return() {
    let program = proc_ref_unknown_signature_string_byval_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_string_variant_byval_variant_return() {
    let program = proc_ref_unknown_signature_string_variant_byval_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("alpha".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_variant_byval_variant_return_long_payload() {
    let program =
        proc_ref_unknown_signature_string_variant_byval_variant_return_long_payload_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("42".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_coerces_unknown_signature_string_variant_byval_variant_return_selected_payloads()
 {
    let cases = vec![
        (
            "bool",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Bool(true)),
                from: OxTy::Bool,
            }],
            "True",
        ),
        (
            "double",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                from: OxTy::Double,
            }],
            "12.5",
        ),
        (
            "single",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::F32(12.5f32.to_bits())),
                from: OxTy::Single,
            }],
            "12.5",
        ),
        (
            "currency",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Currency(123_456)),
                from: OxTy::Currency,
            }],
            "12.3456",
        ),
        (
            "integer",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::I16(44)),
                from: OxTy::Integer,
            }],
            "44",
        ),
        (
            "longlong",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::I64(5_000_000_012)),
                from: OxTy::LongLong,
            }],
            "5000000012",
        ),
        (
            "date",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                from: OxTy::Date,
            }],
            "1/15/2020",
        ),
        (
            "string",
            vec![OxInst::Box {
                dst: OxPlace::Local(LocalId(0)),
                src: OxOperand::Const(OxConst::Str("omega".to_string())),
                from: OxTy::Str,
            }],
            "omega",
        ),
        ("empty", Vec::new(), ""),
    ];
    for (label, setup, expected) in cases {
        let program =
            proc_ref_unknown_signature_string_variant_byval_variant_return_with_setup_program(
                setup,
            );
        assert_eq!(verify_program(&program), Ok(()), "{label}");
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{label}: {:?}", outcome.err);
        assert_eq!(
            outcome
                .values
                .get(2)
                .and_then(|value| value.as_bstr().map(|text| text.as_str())),
            Some(expected.to_string()),
            "{label}"
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2),
            "{label}"
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_string_byval_variant_return() {
    let program = proc_ref_unknown_signature_two_string_byval_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(2)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_string_variant_byval_variant_return() {
    let program = proc_ref_unknown_signature_two_string_variant_byval_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .get(3)
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_return_to_variant() {
    let program = proc_ref_unknown_signature_long_return_to_variant_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_variant_return() {
    let program = proc_ref_unknown_signature_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_double_return_to_variant() {
    let program = proc_ref_unknown_signature_double_return_to_variant_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_f64), Some(12.5));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_double_return() {
    let program = proc_ref_unknown_signature_double_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_f64), Some(12.5));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_exact_scalar_returns() {
    enum Expected {
        I64(i64),
        Currency(i64),
        F32(f32),
        Date(f64),
        U8(u8),
        I16(i16),
        Bool(bool),
    }

    let cases = [
        (
            OxTy::LongLong,
            OxConst::I64(5_000_000_012),
            Expected::I64(5_000_000_012),
        ),
        (
            OxTy::Currency,
            OxConst::Currency(123_456),
            Expected::Currency(123_456),
        ),
        (
            OxTy::Single,
            OxConst::F32(2.5f32.to_bits()),
            Expected::F32(2.5),
        ),
        (
            OxTy::Date,
            OxConst::Date(43_845.0f64.to_bits()),
            Expected::Date(43_845.0),
        ),
        (OxTy::Byte, OxConst::I32(7), Expected::U8(7)),
        (OxTy::Integer, OxConst::I16(44), Expected::I16(44)),
        (OxTy::Bool, OxConst::Bool(true), Expected::Bool(true)),
    ];

    for (return_ty, value, expected) in cases {
        let program =
            proc_ref_unknown_signature_no_arg_return_program(return_ty.clone(), return_ty, value);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        let actual = outcome.values.get(1).expect("return local snapshot");
        match expected {
            Expected::I64(value) => assert_eq!(actual.as_i64(), Some(value)),
            Expected::Currency(value) => {
                assert_eq!(actual.as_currency_scaled_i64(), Some(value));
            }
            Expected::F32(value) => assert_eq!(actual.as_f32(), Some(value)),
            Expected::Date(value) => assert_eq!(actual.as_date_f64(), Some(value)),
            Expected::U8(value) => assert_eq!(actual.as_u8(), Some(value)),
            Expected::I16(value) => assert_eq!(actual.as_i16(), Some(value)),
            Expected::Bool(value) => assert_eq!(actual.as_bool(), Some(value)),
        }
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_scalar_returns_to_variant() {
    enum Expected {
        I64(i64),
        Currency(i64),
        F32(f32),
        Date(f64),
        U8(u8),
        I16(i16),
        Bool(bool),
    }

    let cases = [
        (
            OxTy::LongLong,
            OxConst::I64(5_000_000_012),
            Expected::I64(5_000_000_012),
        ),
        (
            OxTy::Currency,
            OxConst::Currency(123_456),
            Expected::Currency(123_456),
        ),
        (
            OxTy::Single,
            OxConst::F32(2.5f32.to_bits()),
            Expected::F32(2.5),
        ),
        (
            OxTy::Date,
            OxConst::Date(43_845.0f64.to_bits()),
            Expected::Date(43_845.0),
        ),
        (OxTy::Byte, OxConst::I32(7), Expected::U8(7)),
        (OxTy::Integer, OxConst::I16(44), Expected::I16(44)),
        (OxTy::Bool, OxConst::Bool(true), Expected::Bool(true)),
    ];

    for (return_ty, value, expected) in cases {
        let program =
            proc_ref_unknown_signature_no_arg_return_program(return_ty, OxTy::Variant, value);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        let actual = outcome.values.get(1).expect("return local snapshot");
        match expected {
            Expected::I64(value) => assert_eq!(actual.as_i64(), Some(value)),
            Expected::Currency(value) => {
                assert_eq!(actual.as_currency_scaled_i64(), Some(value));
            }
            Expected::F32(value) => assert_eq!(actual.as_f32(), Some(value)),
            Expected::Date(value) => assert_eq!(actual.as_date_f64(), Some(value)),
            Expected::U8(value) => assert_eq!(actual.as_u8(), Some(value)),
            Expected::I16(value) => assert_eq!(actual.as_i16(), Some(value)),
            Expected::Bool(value) => assert_eq!(actual.as_bool(), Some(value)),
        }
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_sub() {
    let program = proc_ref_unknown_signature_long_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_i32_compatible_byval_long_sub() {
    let cases = vec![
        (
            proc_ref_unknown_signature_long_sub_with_byval_arg_program(OxOperand::Const(
                OxConst::I16(42),
            )),
            42,
        ),
        (
            proc_ref_unknown_signature_long_sub_with_byval_arg_program(OxOperand::Const(
                OxConst::Bool(true),
            )),
            -1,
        ),
        (
            proc_ref_unknown_signature_bool_local_byval_long_sub_program(),
            -1,
        ),
        (
            proc_ref_unknown_signature_byte_local_byval_long_sub_program(),
            7,
        ),
        (
            proc_ref_unknown_signature_scalar_local_byval_long_sub_program(
                "integer_value",
                OxTy::Integer,
                OxOperand::Const(OxConst::I16(42)),
            ),
            42,
        ),
    ];
    for (program, expected) in cases {
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(
            outcome.values.get(1).and_then(Variant::as_i32),
            Some(expected)
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_variant_byval_long_sub() {
    let cases = vec![
        (
            proc_ref_unknown_signature_variant_byval_long_sub_program(
                OxOperand::Const(OxConst::I32(42)),
                OxTy::Long,
            ),
            42,
        ),
        (
            proc_ref_unknown_signature_variant_byval_long_sub_program(
                OxOperand::Const(OxConst::I16(42)),
                OxTy::Integer,
            ),
            42,
        ),
        (
            proc_ref_unknown_signature_byte_variant_byval_long_sub_program(),
            7,
        ),
        (
            proc_ref_unknown_signature_variant_byval_long_sub_program(
                OxOperand::Const(OxConst::Bool(true)),
                OxTy::Bool,
            ),
            -1,
        ),
    ];
    for (program, expected) in cases {
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(
            outcome.values.get(1).and_then(Variant::as_i32),
            Some(expected)
        );
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_long_sub() {
    let program = proc_ref_unknown_signature_two_long_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_function_statement() {
    let program = proc_ref_unknown_signature_long_function_statement_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_variant_function_statement() {
    let program = proc_ref_unknown_signature_long_variant_function_statement_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_no_arg_function_statements() {
    enum Expected {
        Long(i32),
        Str(&'static str),
    }

    let cases = vec![
        (
            proc_ref_unknown_signature_no_arg_long_function_statement_program(),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
                "StoreAndReturnLongLong",
                OxTy::LongLong,
                OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::I64(5_000_000_012)),
                },
            ),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
                "StoreAndReturnCurrency",
                OxTy::Currency,
                OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Currency(123_456)),
                },
            ),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
                "StoreAndReturnSingle",
                OxTy::Single,
                OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::F32(2.5f32.to_bits())),
                },
            ),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
                "StoreAndReturnDouble",
                OxTy::Double,
                OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::F64(12.5f64.to_bits())),
                },
            ),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
                "StoreAndReturnDate",
                OxTy::Date,
                OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Date(43_845.0f64.to_bits())),
                },
            ),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
                "StoreAndReturnByte",
                OxTy::Byte,
                OxInst::Coerce {
                    dst: OxPlace::Local(LocalId(0)),
                    src: OxOperand::Const(OxConst::I32(7)),
                    target: OxCoerceTarget::Numeric(NumericCoerceTarget::Byte),
                },
            ),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
                "StoreAndReturnInteger",
                OxTy::Integer,
                OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::I16(44)),
                },
            ),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_scalar_function_statement_program(
                "StoreAndReturnBool",
                OxTy::Bool,
                OxInst::Assign {
                    dst: OxPlace::Local(LocalId(0)),
                    value: OxOperand::Const(OxConst::Bool(true)),
                },
            ),
            Expected::Long(42),
        ),
        (
            proc_ref_unknown_signature_no_arg_string_function_statement_program(),
            Expected::Str("alpha"),
        ),
        (
            proc_ref_unknown_signature_no_arg_variant_function_statement_program(),
            Expected::Long(42),
        ),
    ];

    for (program, expected) in cases {
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        match expected {
            Expected::Long(expected) => {
                assert_eq!(
                    outcome.values.get(1).and_then(Variant::as_i32),
                    Some(expected)
                );
            }
            Expected::Str(expected) => {
                assert_eq!(
                    outcome
                        .values
                        .get(1)
                        .and_then(|value| value.as_bstr().map(|text| text.as_str())),
                    Some(expected.to_string())
                );
            }
        }
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_byref_sub() {
    let program = proc_ref_unknown_signature_long_byref_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_byref_byval_sub() {
    let program = proc_ref_unknown_signature_long_byref_byval_sub_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_return() {
    let program = proc_ref_unknown_signature_long_return_program(OxTy::Long);
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_return_with_arg_to_variant() {
    let program = proc_ref_unknown_signature_long_return_program(OxTy::Variant);
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_variant_arg_long_return() {
    for dst_ty in [OxTy::Long, OxTy::Variant] {
        let program = proc_ref_unknown_signature_long_variant_arg_return_program(dst_ty);
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised, "{:?}", outcome.err);
        assert_eq!(outcome.values.get(2).and_then(Variant::as_i32), Some(42));
        assert_eq!(
            outcome.values.first().and_then(Variant::as_proc_ref),
            Some(2)
        );
    }
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_arg_variant_return() {
    let program = proc_ref_unknown_signature_long_arg_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_variant_arg_variant_return() {
    let program = proc_ref_unknown_signature_variant_arg_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(2).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_long_return() {
    let program = proc_ref_unknown_signature_two_long_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_two_long_variant_return() {
    let program = proc_ref_unknown_signature_two_long_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_byref_return() {
    let program = proc_ref_unknown_signature_long_byref_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(outcome.values.get(2).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_unknown_signature_long_byref_variant_return() {
    let program = proc_ref_unknown_signature_long_byref_variant_return_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(42));
    assert_eq!(outcome.values.get(2).and_then(Variant::as_i32), Some(42));
    assert_eq!(
        outcome.values.first().and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_known_string_byref() {
    let program = proc_ref_string_byref_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .first()
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.get(1).and_then(Variant::as_proc_ref),
        Some(1)
    );
}

#[test]
fn jit_call_proc_ref_dispatches_ambiguous_same_signature_string_byref() {
    let program = proc_ref_ambiguous_same_signature_string_byref_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised, "{:?}", outcome.err);
    assert_eq!(
        outcome
            .values
            .first()
            .and_then(|value| value.as_bstr().map(|text| text.as_str())),
        Some("beta".to_string())
    );
    assert_eq!(
        outcome.values.get(1).and_then(Variant::as_proc_ref),
        Some(2)
    );
}

#[test]
fn jit_call_proc_ref_invalid_target_seats_error_490() {
    let mut program = proc_ref_program();
    program.funcs[0].blocks[0]
        .instrs
        .retain(|inst| !matches!(inst, OxInst::LoadProcRef { .. }));
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(outcome.raised);
    assert_eq!(outcome.err.number, 490);
    assert_eq!(outcome.err.description, "invalid procedure reference");
}

#[test]
fn jit_call_proc_ref_out_of_range_target_seats_error_490() {
    let program = proc_ref_program();
    let mut globals = Vec::new();
    let mut globals_table = vec![&mut globals as *mut Vec<Variant>];
    let program_images = [JitProgramImage {
        program: &program,
        functions: std::ptr::null(),
        function_count: 2,
    }];
    let mut frame = new_jit_frame(&program, 0, &program.funcs[0]).expect("frame");
    frame.locals[1] = Variant::from_proc_ref(99);
    let mut run = JitRun {
        globals: globals_table.as_mut_ptr(),
        global_count: globals_table.len(),
        frames: vec![frame],
        explicit_refs: Vec::new(),
        for_each: HashMap::new(),
        as_new_slots: HashMap::new(),
        param_array_aliases: HashMap::new(),
        next_collection_instance_id: i32::MIN + 1,
        programs: program_images.as_ptr(),
        program_count: program_images.len(),
    };
    let host = NullHostServices::new(HostPolicy::default());
    let mut exec = ExecState::new(&host);
    exec.programs = vec![build_loaded(&program).expect("loaded")];
    let state = exec_state_as_raw(&mut exec);
    let args = [JitCallArgDesc {
        kind: 0,
        aux: 0,
        value: 21,
        area: AREA_GLOBAL as i32,
        index: 0,
    }];
    // SAFETY: owned `run` and the uniquely borrowed state remain live; `args`
    // contains the single initialized descriptor required by `argc`.
    let status = unsafe {
        rt_jit_call_proc_ref_i32(
            &mut run,
            state,
            AREA_LOCAL as i32,
            1,
            -1,
            JIT_PROC_REF_RET_LONG,
            1,
            args.as_ptr(),
            AREA_LOCAL as i32,
            2,
        )
    };
    assert_eq!(status, ST_FAULT);
    assert_eq!(exec.err_engine.err.number, 490);
    assert_eq!(
        exec.err_engine.err.description,
        "invalid procedure reference"
    );
}

#[test]
fn jit_expect_proc_ref_helper_seats_error_490_for_out_of_range_target() {
    let program = proc_ref_program();
    let mut globals = Vec::new();
    let mut globals_table = vec![&mut globals as *mut Vec<Variant>];
    let program_images = [JitProgramImage {
        program: &program,
        functions: std::ptr::null(),
        function_count: 2,
    }];
    let mut frame = new_jit_frame(&program, 0, &program.funcs[0]).expect("frame");
    frame.locals[1] = Variant::from_proc_ref(99);
    let mut run = JitRun {
        globals: globals_table.as_mut_ptr(),
        global_count: globals_table.len(),
        frames: vec![frame],
        explicit_refs: Vec::new(),
        for_each: HashMap::new(),
        as_new_slots: HashMap::new(),
        param_array_aliases: HashMap::new(),
        next_collection_instance_id: i32::MIN + 1,
        programs: program_images.as_ptr(),
        program_count: program_images.len(),
    };
    let host = NullHostServices::new(HostPolicy::default());
    let mut exec = ExecState::new(&host);
    exec.programs = vec![build_loaded(&program).expect("loaded")];
    let state = exec_state_as_raw(&mut exec);
    // SAFETY: owned `run` and the uniquely borrowed execution state remain live;
    // the helper reads only the initialized procedure-reference slot.
    let status = unsafe { rt_jit_expect_proc_ref_i32(&mut run, state, AREA_LOCAL as i32, 1, 1) };
    assert_eq!(status, ST_FAULT);
    assert_eq!(exec.err_engine.err.number, 490);
    assert_eq!(
        exec.err_engine.err.description,
        "invalid procedure reference"
    );
}

#[test]
fn jit_call_helper_installs_missing_for_omitted_variant_arg() {
    unsafe extern "C" fn assert_missing_entry(run: *mut JitRun, _state: *mut RawExecState) -> i32 {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and this test owns the synthetic run.
        let run = unsafe { &mut *run };
        let got_missing = run
            .frames
            .last()
            .and_then(|frame| frame.locals.first())
            .and_then(Variant::as_error_code)
            == Some(MISSING_ARG);
        if got_missing { ST_OK } else { ST_FAULT }
    }

    let entry = OxBlock {
        id: BlockId(0),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::Return,
    };
    let callee = OxFunc {
        name: "Touch".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![OxLocal {
            name: "value".to_string(),
            ty: OxTy::Variant,
            array_element: None,
            param: Some(OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
            escaped: false,
        }],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![entry],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![callee],
        entry: None,
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };
    let mut globals = Vec::new();
    let functions: Vec<JitEntryFn> = vec![assert_missing_entry];
    let mut globals_table = vec![&mut globals as *mut Vec<Variant>];
    let program_images = [JitProgramImage {
        program: &program,
        functions: functions.as_ptr(),
        function_count: functions.len(),
    }];
    let mut run = JitRun {
        globals: globals_table.as_mut_ptr(),
        global_count: globals_table.len(),
        frames: vec![new_jit_frame(&program, 0, &program.funcs[0]).expect("frame")],
        explicit_refs: Vec::new(),
        for_each: HashMap::new(),
        as_new_slots: HashMap::new(),
        param_array_aliases: HashMap::new(),
        next_collection_instance_id: i32::MIN + 1,
        programs: program_images.as_ptr(),
        program_count: program_images.len(),
    };
    let host = NullHostServices::new(HostPolicy::default());
    let mut exec = ExecState::new(&host);
    exec.programs = vec![build_loaded(&program).expect("loaded")];
    let state = exec_state_as_raw(&mut exec);
    let args = [JitCallArgDesc {
        kind: JIT_CALL_ARG_OMITTED,
        aux: 0,
        value: 0,
        area: 0,
        index: 0,
    }];
    // SAFETY: owned `run` and the uniquely borrowed state remain live; `args`
    // contains the single initialized descriptor required by `argc`.
    let status = unsafe { rt_jit_call_proc_i32(&mut run, state, 0, 1, args.as_ptr(), -1, -1) };
    assert_eq!(status, ST_OK);
}

#[test]
fn jit_call_helper_copies_variant_return() {
    unsafe extern "C" fn set_variant_return(run: *mut JitRun, _state: *mut RawExecState) -> i32 {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and this test owns the synthetic run.
        let run = unsafe { &mut *run };
        let Some(ret) = run
            .frames
            .last_mut()
            .and_then(|frame| frame.locals.first_mut())
        else {
            return ST_FAULT;
        };
        *ret = Variant::from_i32(77);
        ST_OK
    }

    let entry = OxBlock {
        id: BlockId(0),
        instrs: Vec::new(),
        fault_target: None,
        terminator: OxTerminator::Return,
    };
    let callee = OxFunc {
        name: "GetValue".to_string(),
        kind: ProcedureKind::Function,
        locals: vec![OxLocal {
            name: "GetValue".to_string(),
            ty: OxTy::Variant,
            array_element: None,
            param: None,
            escaped: false,
        }],
        temps: Vec::new(),
        param_count: 0,
        return_local: Some(LocalId(0)),
        blocks: vec![entry],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![callee],
        globals: vec![OxGlobal {
            name: "g".to_string(),
            ty: OxTy::Variant,
            array_element: None,
        }],
        entry: None,
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };
    let mut globals = vec![Variant::empty()];
    let functions: Vec<JitEntryFn> = vec![set_variant_return];
    let mut globals_table = vec![&mut globals as *mut Vec<Variant>];
    let program_images = [JitProgramImage {
        program: &program,
        functions: functions.as_ptr(),
        function_count: functions.len(),
    }];
    let mut run = JitRun {
        globals: globals_table.as_mut_ptr(),
        global_count: globals_table.len(),
        frames: vec![new_jit_frame(&program, 0, &program.funcs[0]).expect("frame")],
        explicit_refs: Vec::new(),
        for_each: HashMap::new(),
        as_new_slots: HashMap::new(),
        param_array_aliases: HashMap::new(),
        next_collection_instance_id: i32::MIN + 1,
        programs: program_images.as_ptr(),
        program_count: program_images.len(),
    };
    let host = NullHostServices::new(HostPolicy::default());
    let mut exec = ExecState::new(&host);
    exec.programs = vec![build_loaded(&program).expect("loaded")];
    let state = exec_state_as_raw(&mut exec);
    // SAFETY: owned `run` and the uniquely borrowed state remain live; this
    // zero-argument call permits null args and writes to a valid global slot.
    let status = unsafe {
        rt_jit_call_proc_i32(
            &mut run,
            state,
            0,
            0,
            std::ptr::null(),
            AREA_GLOBAL as i32,
            0,
        )
    };
    assert_eq!(status, ST_OK);
    assert_eq!(globals.first().and_then(Variant::as_i32), Some(77));
}

unsafe extern "C" fn nested_as_new_initializer_entry(
    run: *mut JitRun,
    state: *mut RawExecState,
) -> i32 {
    if run.is_null() || state.is_null() {
        return ST_FAULT;
    }
    let depth = {
        // SAFETY: the fake entry is invoked only through `jit_proc_invoke` with
        // the live synthetic run; this borrow ends before nested entry.
        unsafe { &*run }.frames.len()
    };
    if depth == 1 {
        // SAFETY: the test owns the live state/run pair; this shared As-New
        // path synchronously re-enters the same initializer at depth two.
        let nested = match unsafe {
            instantiate_as_new_for_jit(run, state, 0, OxAsNew::ProjectClass { class: ClassId(0) })
        } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: nested entry returned and no typed run borrow spans it.
        unsafe { &mut *run }.explicit_refs.push(nested);
    }
    // SAFETY: all nested entry work returned.
    unsafe { &mut *run }
        .explicit_refs
        .push(Variant::from_i32(depth as i32));
    ST_OK
}

#[test]
fn jit_fake_entry_reenters_as_new_and_registration_clears_before_context_drop() {
    let initializer = OxFunc {
        name: "Class_Initialize".to_string(),
        kind: ProcedureKind::Sub,
        locals: vec![OxLocal {
            name: "Me".to_string(),
            ty: OxTy::Variant,
            array_element: None,
            param: Some(OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
            escaped: false,
        }],
        temps: Vec::new(),
        param_count: 1,
        return_local: None,
        blocks: vec![OxBlock {
            id: BlockId(0),
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::Return,
        }],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![initializer],
        classes: vec![OxClass {
            name: "Widget".to_string(),
            predeclared: false,
            initialize: Some(FuncId(0)),
            terminate: None,
            fields: Vec::new(),
            methods: Vec::new(),
            as_new_fields: Vec::new(),
            implements: Vec::new(),
        }],
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };
    let functions: Vec<JitEntryFn> = vec![nested_as_new_initializer_entry];
    let program_images = [JitProgramImage {
        program: &program,
        functions: functions.as_ptr(),
        function_count: functions.len(),
    }];
    let mut globals = Vec::new();
    let mut globals_table = vec![&mut globals as *mut Vec<Variant>];
    let mut run = JitRun {
        globals: globals_table.as_mut_ptr(),
        global_count: globals_table.len(),
        frames: Vec::new(),
        explicit_refs: Vec::new(),
        for_each: HashMap::new(),
        as_new_slots: HashMap::new(),
        param_array_aliases: HashMap::new(),
        next_collection_instance_id: i32::MIN + 1,
        programs: program_images.as_ptr(),
        program_count: program_images.len(),
    };
    let host = NullHostServices::new(HostPolicy::default());
    let mut exec = ExecState::new(&host);
    exec.programs = vec![build_loaded(&program).expect("loaded")];
    let state = exec_state_as_raw(&mut exec);
    let mut bridge_ctx = JitProcInvokeCtx {
        run: &raw mut run,
        state,
    };
    assert_eq!(
        // SAFETY: state, run, callback context, program, and fake entry table all
        // remain live until the registration guard is explicitly dropped.
        unsafe {
            rt_install_proc_invoker(
                state,
                (&raw mut bridge_ctx).cast::<c_void>(),
                Some(jit_proc_invoke),
            )
        },
        ST_OK
    );
    let registration = ProcInvokerRegistration { state };
    let mut object = Variant::empty();

    // SAFETY: the installed bridge and initialized output satisfy the object
    // helper contract; the fake initializer performs one nested As-New call.
    let status = unsafe { rt_project_new_object(state, 0, 0, &mut object) };

    assert_eq!(status, ST_OK);
    assert!(object.as_object_ref().is_some());
    assert!(run.frames.is_empty());
    assert!(run.param_array_aliases.is_empty());
    assert_eq!(
        run.explicit_refs
            .iter()
            .filter_map(Variant::as_i32)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );

    drop(registration);
    exec.err_engine.clear_err();
    let mut after_clear = Variant::empty();
    // SAFETY: state/output remain live; this call must fail closed because
    // the RAII guard cleared the bridge before `bridge_ctx` can expire.
    let status_after_clear = unsafe { rt_project_new_object(state, 0, 0, &mut after_clear) };
    assert_eq!(status_after_clear, ST_FAULT);
    assert_eq!(exec.err_engine.err.number, 5);
    assert!(run.frames.is_empty());
}

#[test]
fn failing_local_and_extern_calls_restore_err_frames_and_real_paramarray_aliases() {
    unsafe extern "C" fn fail_after_observing_aliases(
        run: *mut JitRun,
        state: *mut RawExecState,
    ) -> i32 {
        if run.is_null() || state.is_null() {
            return ST_FAULT;
        }
        let alias_present = {
            // SAFETY: the fake entry receives the live stable run root and this
            // read-only inspection ends before seating the failure.
            let run = unsafe { &*run };
            let callee = run.frames.len().checked_sub(1);
            callee.is_some_and(|callee| {
                run.param_array_aliases.contains_key(&SlotAlias {
                    frame: Some(callee),
                    area: AREA_LOCAL,
                    index: 0,
                })
            })
        };
        if !alias_present {
            return ST_FAULT;
        }
        // SAFETY: the test owns the live state root for this synchronous entry.
        unsafe { rt_raise_runtime_error_number(state, 91) }
    }

    let program = paramarray_no_alias_call_program();
    let functions: Vec<JitEntryFn> = vec![fail_after_observing_aliases; 2];
    let program_images = [JitProgramImage {
        program: &program,
        functions: functions.as_ptr(),
        function_count: functions.len(),
    }];
    let mut globals = vec![Variant::from_i32(10)];
    let mut globals_table = vec![&mut globals as *mut Vec<Variant>];
    let caller_array =
        Variant::from_safearray(SafeArray::from_variants(vec![Variant::from_i32(10)]));
    let caller_alias = SlotAlias {
        frame: Some(0),
        area: AREA_TEMP,
        index: 0,
    };
    let element_alias = SlotAlias {
        frame: Some(0),
        area: AREA_GLOBAL,
        index: 0,
    };
    let mut caller_frame = new_jit_frame(&program, 0, &program.funcs[0]).expect("frame");
    caller_frame.temps[0] = caller_array;
    let mut run = JitRun {
        globals: globals_table.as_mut_ptr(),
        global_count: globals_table.len(),
        frames: vec![caller_frame],
        explicit_refs: Vec::new(),
        for_each: HashMap::new(),
        as_new_slots: HashMap::new(),
        param_array_aliases: HashMap::from([(caller_alias, vec![Some(element_alias)])]),
        next_collection_instance_id: i32::MIN + 1,
        programs: program_images.as_ptr(),
        program_count: program_images.len(),
    };
    let host = NullHostServices::new(HostPolicy::default());
    let mut exec = ExecState::new(&host);
    exec.programs = vec![build_loaded(&program).expect("loaded")];
    let state = exec_state_as_raw(&mut exec);
    let args = [JitCallArgDesc {
        kind: JIT_CALL_ARG_BYVAL_VARIANT,
        aux: JIT_VARIANT_OPERAND_PLACE,
        value: 0,
        area: AREA_TEMP as i32,
        index: 0,
    }];

    for call_extern in [false, true] {
        exec.err_engine
            .raise(Fault::new(7, "saved caller Err"), "AliasTest");
        exec.err_engine.error_mode = oxvba_rt_abi::ErrorMode::Goto(BlockId(7));
        // SAFETY: run/state/descriptor storage remains live and uniquely owned.
        let status = unsafe {
            if call_extern {
                rt_jit_call_extern_proc_i32(&mut run, state, 0, 1, 1, args.as_ptr(), -1, -1)
            } else {
                rt_jit_call_proc_i32(&mut run, state, 1, 1, args.as_ptr(), -1, -1)
            }
        };

        assert_eq!(status, ST_FAULT);
        assert_eq!(exec.err_engine.err.number, 91);
        assert_eq!(
            exec.err_engine.error_mode,
            oxvba_rt_abi::ErrorMode::Goto(BlockId(7))
        );
        assert_eq!(run.frames.len(), 1);
        assert_eq!(run.param_array_aliases.len(), 1);
        assert!(run.param_array_aliases.get(&caller_alias) == Some(&vec![Some(element_alias)]));
    }
}

#[test]
fn jit_lowers_explicit_termination_drain_with_current_run_handle() {
    let main = OxFunc {
        name: "Main".to_string(),
        kind: ProcedureKind::Sub,
        locals: Vec::new(),
        temps: Vec::new(),
        param_count: 0,
        return_local: None,
        blocks: vec![OxBlock {
            id: BlockId(0),
            instrs: vec![OxInst::DrainTerminations],
            fault_target: None,
            terminator: OxTerminator::Return,
        }],
        entry: BlockId(0),
    };
    let program = OxProgram {
        funcs: vec![main],
        entry: Some(FuncId(0)),
        unit_name: "VBAProject".to_string(),
        ..OxProgram::empty()
    };
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());

    let outcome = compiled.run(&host).expect("run");

    assert!(!outcome.raised, "{:?}", outcome.err);
}

#[test]
fn jit_lowers_compare_and_branch_control_flow() {
    let program = branch_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(42));
    assert_eq!(outcome.values.get(1).and_then(Variant::as_bool), Some(true));
}

#[test]
fn jit_lowers_bool_logical_and_not_control_flow() {
    let program = bool_logical_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(1));
}

#[test]
fn jit_lowers_numeric_logical_long() {
    let program = numeric_logical_program();
    assert_eq!(verify_program(&program), Ok(()));
    let engine = JitEngine;
    let compiled = engine.compile_image(&[&program]).expect("compile");
    let host = NullHostServices::new(HostPolicy::default());
    let outcome = compiled.run(&host).expect("run");
    assert!(!outcome.raised);
    assert_eq!(outcome.values.first().and_then(Variant::as_i32), Some(10));
}

#[test]
fn symbol_sanitizer_keeps_stable_names() {
    let name = sanitize_symbol("Main.Worker$");
    assert_eq!(name, "Main_Worker_");
    let _ = ProjectMemberKind::Method;
    let _ = GlobalId(0);
}
