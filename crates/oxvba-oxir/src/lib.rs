//! `oxvba-oxir` — OxIR, the typed, backend-neutral mid-level IR for OxVBA.
//!
//! OxIR is a **typed, basic-block CFG with typed locals/places** — MIR-like, *not*
//! authored SSA. It is produced from the resolved Core IR tree by an elaboration
//! pass, and it is the single canonical executable-semantic artifact: the new
//! interpreter `oxvba-vm3` executes it (and is its executable specification) and a
//! Cranelift backend lowers it to native code. The IR carries no backend types, so
//! future wasm / copy-and-patch / LLVM backends stay reachable behind one semantic
//! kernel.
//!
//! Layers:
//! - [`ty`] — the type lattice ([`OxTy`]): the static type each value/local carries
//!   (the information the legacy `linearize` discards).
//! - [`ids`] — structural index newtypes.
//! - [`value`] — operands, places, constants, and the operator enums.
//! - [`inst`] — the typed instruction set, terminators, and basic blocks.
//! - [`program`] — functions, classes, and the whole compilation unit
//!   ([`OxProgram`]).
//! - [`verify`] — a structural verifier.
//!
//! Still to land (next sub-sections): the typed COM interface + method-descriptor
//! tables with the COM-call instructions, then the `Core IR → OxIR` elaboration pass.

pub mod ids;
pub mod inst;
pub mod program;
pub mod ty;
pub mod value;
pub mod verify;

pub use ids::{BlockId, FuncId, GlobalId, ImportId, LocalId, TempId};
pub use inst::{
    ErrorHandler, OxBlock, OxInst, OxTerminator, terminator_operand, terminator_successors,
};
pub use program::{OxClass, OxClassMethod, OxFunc, OxGlobal, OxLocal, OxParamInfo, OxProgram};
pub use ty::{ArrayShape, ClassId, IfaceId, ObjClass, OxTy, RecordLayoutId};
pub use value::{
    ArithOp, BoundWhich, CmpOp, DeclarePtrWriteback, ErrField, LogicalOp, OxArg, OxCallArg,
    OxCoerceTarget, OxConst, OxNativeCallee, OxOperand, OxPlace, PtrKind, PtrWritebackKind,
};
pub use verify::{VerifyError, verify_program};

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_bundle::{NumericCoerceTarget, NumericMode, ProcedureKind};

    /// A small well-formed program: `Sub Main()` computing `n = (10 + 5) * 2` into a
    /// `Long` local, with a fault landing pad for the two checked-arithmetic ops.
    fn sample_program() -> OxProgram {
        let long = NumericMode::Checked(NumericCoerceTarget::Long);
        let n = LocalId(0);
        let t0 = TempId(0);

        // Block 0 (entry): the statement body; fallible ops fault to block 1.
        let entry = OxBlock {
            id: BlockId(0),
            instrs: vec![
                OxInst::StmtBoundary { stmt: 0 },
                OxInst::Arith {
                    dst: OxPlace::Temp(t0),
                    op: ArithOp::Add,
                    lhs: OxOperand::Const(OxConst::I32(10)),
                    rhs: OxOperand::Const(OxConst::I32(5)),
                    mode: long,
                },
                OxInst::Arith {
                    dst: OxPlace::Local(n),
                    op: ArithOp::Mul,
                    lhs: OxOperand::temp(t0),
                    rhs: OxOperand::Const(OxConst::I32(2)),
                    mode: long,
                },
            ],
            fault_target: Some(BlockId(1)),
            terminator: OxTerminator::Jump(BlockId(2)),
        };
        // Block 1: the landing pad (no active handler ⇒ just return).
        let landing = OxBlock::new(BlockId(1), OxTerminator::Return);
        // Block 2: normal exit.
        let exit = OxBlock::new(BlockId(2), OxTerminator::Return);

        let main = OxFunc {
            name: "Main".to_string(),
            kind: ProcedureKind::Sub,
            locals: vec![OxLocal {
                name: "n".to_string(),
                ty: OxTy::Long,
                param: None,
                escaped: false,
            }],
            param_count: 0,
            return_local: None,
            blocks: vec![entry, landing, exit],
            entry: BlockId(0),
        };

        OxProgram {
            funcs: vec![main],
            entry: Some(FuncId(0)),
            unit_name: "Sample".to_string(),
            ..OxProgram::empty()
        }
    }

    #[test]
    fn sample_program_verifies() {
        let p = sample_program();
        assert_eq!(verify_program(&p), Ok(()));
    }

    #[test]
    fn structural_round_trip_through_json() {
        let p = sample_program();
        let json = serde_json::to_string(&p).expect("serialize");
        let back: OxProgram = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p, back, "OxProgram must round-trip structurally");
    }

    #[test]
    fn verifier_catches_dangling_entry() {
        let mut p = sample_program();
        p.funcs[0].entry = BlockId(99);
        let errs = verify_program(&p).expect_err("dangling entry must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadEntry { entry: 99, .. }
            )),
            "expected BadEntry, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_missing_fault_target() {
        let mut p = sample_program();
        // Drop the landing pad on the entry block; its checked-arith ops are fallible.
        p.funcs[0].blocks[0].fault_target = None;
        let errs = verify_program(&p).expect_err("fallible block without fault_target must be rejected");
        assert!(
            errs.iter()
                .any(|e| matches!(e, VerifyError::MissingFaultTarget { .. })),
            "expected MissingFaultTarget, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_bad_successor() {
        let mut p = sample_program();
        p.funcs[0].blocks[0].terminator = OxTerminator::Jump(BlockId(42));
        let errs = verify_program(&p).expect_err("dangling jump must be rejected");
        assert!(
            errs.iter()
                .any(|e| matches!(e, VerifyError::BadSuccessor { target: 42, .. })),
            "expected BadSuccessor, got {errs:?}"
        );
    }
}
