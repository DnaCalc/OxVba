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
//! - [`analysis`] — shared backend/verifier facts derived from canonical OxIR.
//! - [`passes`] — canonical behavior-preserving IR normalization passes.
//! - [`ty`] — the type lattice ([`OxTy`]): the static type each value/local carries
//!   (the information the legacy `linearize` discards).
//! - [`ids`] — structural index newtypes.
//! - [`value`] — operands, places, constants, and the operator enums.
//! - [`com`] — the typed COM model: the interface + method-descriptor tables and the
//!   IID / wire-shape vocabulary that keep COM calls typed end-to-end.
//! - [`inst`] — the typed instruction set, terminators, and basic blocks.
//! - [`program`] — functions, classes, and the whole compilation unit
//!   ([`OxProgram`]).
//! - [`verify`] — a structural verifier.
//! - [`elaborate`] — the `Core IR → OxIR` elaboration pass that produces these typed
//!   structures from the binder's resolved tree (landing incrementally, starting with
//!   the `VarTypeRef → OxTy` type lowering).

pub mod analysis;
pub mod com;
pub mod elaborate;
pub mod ids;
pub mod image;
pub mod inst;
pub mod passes;
pub mod program;
pub mod ty;
pub mod value;
pub mod verify;

pub use analysis::{EscapeFacts, apply_escape_analysis, escape_facts};
pub use com::{ComInterface, ComMethodRef, ProjectIfaceMethod, ProjectInterface};
pub use elaborate::{
    NameResolver, ResolvedTypeName, lower_declared_var_type_with_longptr_width,
    lower_var_type_with_longptr_width,
};
pub use ids::{BlockId, FuncId, GlobalId, ImportId, LocalId, TempId};
pub use image::{OX_IMAGE_FORMAT, OX_IMAGE_VERSION, OxImage, OxImageError};
pub use inst::{
    ErrorHandler, OxBlock, OxInst, OxTerminator, terminator_operands, terminator_successors,
};
pub use passes::normalize_assigns;
pub use program::{
    OxClass, OxClassField, OxClassMethod, OxFunc, OxGlobal, OxLocal, OxParamInfo, OxProgram,
};
pub use ty::{ArrayShape, ClassId, IfaceId, ObjClass, OxTy, RecordLayoutId};
pub use value::{
    ArithOp, BoundWhich, CmpOp, DeclarePtrWriteback, ErrField, LogicalOp, OxArg, OxCallArg,
    OxCoerceTarget, OxConst, OxNativeCallee, OxOperand, OxPlace, PtrKind, PtrWritebackKind,
};
pub use verify::{VerifyError, verify_program};

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_bundle::{NumericCoerceTarget, NumericMode, ProcedureKind, ProjectMemberKind};

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
                OxInst::StmtBoundary {
                    stmt: 0,
                    clear_temps_from: 0,
                },
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
                array_element: None,
                param: None,
                escaped: false,
            }],
            temps: vec![OxTy::Long],
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
            errs.iter()
                .any(|e| matches!(e, VerifyError::BadEntry { entry: 99, .. })),
            "expected BadEntry, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_missing_fault_target() {
        let mut p = sample_program();
        // Drop the landing pad on the entry block; its checked-arith ops are fallible.
        p.funcs[0].blocks[0].fault_target = None;
        let errs =
            verify_program(&p).expect_err("fallible block without fault_target must be rejected");
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

    #[test]
    fn verifier_catches_bad_fault_dispatch_seed() {
        let mut p = sample_program();
        p.funcs[0].blocks[1].terminator = OxTerminator::FaultDispatch {
            resume: BlockId(99),
            resume_next: BlockId(2),
        };
        let errs = verify_program(&p).expect_err("dangling fault seed must be rejected");
        assert!(
            errs.iter()
                .any(|e| matches!(e, VerifyError::BadSuccessor { target: 99, .. })),
            "expected BadSuccessor, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_bad_gosub_return_target() {
        let mut p = sample_program();
        p.funcs[0].blocks[0].terminator = OxTerminator::GoSub {
            target: BlockId(1),
            ret: BlockId(99),
        };
        let errs = verify_program(&p).expect_err("dangling GoSub return must be rejected");
        assert!(
            errs.iter()
                .any(|e| matches!(e, VerifyError::BadSuccessor { target: 99, .. })),
            "expected BadSuccessor, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_bad_temp_ref() {
        let mut p = sample_program();
        p.funcs[0].temps.clear();
        let errs = verify_program(&p).expect_err("dangling temp must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadTempRef {
                    temp: 0,
                    temps: 0,
                    ..
                }
            )),
            "expected BadTempRef, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_bad_stmt_boundary_temp_floor() {
        let mut p = sample_program();
        p.funcs[0].blocks[0].instrs[0] = OxInst::StmtBoundary {
            stmt: 0,
            clear_temps_from: 99,
        };
        let errs = verify_program(&p).expect_err("out-of-range temp clear floor must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadStmtBoundaryTemp {
                    clear_temps_from: 99,
                    temps: 1,
                    ..
                }
            )),
            "expected BadStmtBoundaryTemp, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_stale_escape_flag() {
        let mut p = sample_program();
        p.funcs[0].locals[0].escaped = true;
        let errs = verify_program(&p).expect_err("stale escape flag must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadEscapedLocal {
                    local: 0,
                    expected: false,
                    actual: true,
                    ..
                }
            )),
            "expected BadEscapedLocal, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_raw_representation_changing_assign() {
        let mut p = sample_program();
        p.funcs[0].blocks[0].instrs[1] = OxInst::Assign {
            dst: OxPlace::Local(LocalId(0)),
            value: OxOperand::Const(OxConst::Str("7".to_string())),
        };
        let errs =
            verify_program(&p).expect_err("raw representation-changing Assign must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadAssignRepresentation {
                    dst: OxTy::Long,
                    src: OxTy::Str,
                    ..
                }
            )),
            "expected BadAssignRepresentation, got {errs:?}"
        );
    }

    // ── Project class metadata ──────────────────────────────────────────────

    fn sample_class_field_program() -> OxProgram {
        OxProgram {
            classes: vec![OxClass {
                name: "Widget".to_string(),
                predeclared: false,
                initialize: None,
                terminate: None,
                fields: vec![OxClassField {
                    name: "child".to_string(),
                    token: 0,
                    ty: OxTy::Object(ObjClass::Class(ClassId(0))),
                    array_element: None,
                }],
                methods: Vec::new(),
                as_new_fields: vec![crate::program::OxClassAsNewField {
                    field: 0,
                    binding: crate::inst::OxAsNew::ProjectClass { class: ClassId(0) },
                }],
                implements: Vec::new(),
            }],
            unit_name: "ClassSample".to_string(),
            ..OxProgram::empty()
        }
    }

    #[test]
    fn class_field_as_new_metadata_verifies() {
        let p = sample_class_field_program();
        assert_eq!(verify_program(&p), Ok(()));
    }

    #[test]
    fn verifier_catches_duplicate_class_field_token() {
        let mut p = sample_class_field_program();
        p.classes[0].fields.push(OxClassField {
            name: "alias".to_string(),
            token: 0,
            ty: OxTy::Variant,
            array_element: None,
        });
        let errs = verify_program(&p).expect_err("duplicate class field token must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::DuplicateClassFieldToken {
                    class_index: 0,
                    field: 0
                }
            )),
            "expected DuplicateClassFieldToken, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_as_new_field_missing_from_class_table() {
        let mut p = sample_class_field_program();
        p.classes[0].as_new_fields[0].field = 99;
        let errs = verify_program(&p).expect_err("stale As New field token must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadClassFieldAsNewFieldRef {
                    class_index: 0,
                    field: 99
                }
            )),
            "expected BadClassFieldAsNewFieldRef, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_bad_class_initialize_proc_ref() {
        let mut p = sample_class_field_program();
        p.classes[0].initialize = Some(FuncId(99));
        let errs = verify_program(&p).expect_err("dangling Class_Initialize proc");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadClassLifecycleProcRef {
                    class_index: 0,
                    hook: "Class_Initialize",
                    proc: 99,
                    funcs: 0
                }
            )),
            "expected BadClassLifecycleProcRef for initialize, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_bad_class_terminate_proc_ref() {
        let mut p = sample_class_field_program();
        p.classes[0].terminate = Some(FuncId(99));
        let errs = verify_program(&p).expect_err("dangling Class_Terminate proc");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadClassLifecycleProcRef {
                    class_index: 0,
                    hook: "Class_Terminate",
                    proc: 99,
                    funcs: 0
                }
            )),
            "expected BadClassLifecycleProcRef for terminate, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_bad_class_method_proc_ref() {
        let mut p = sample_class_field_program();
        p.classes[0].methods.push(OxClassMethod {
            name: "Touch".to_string(),
            kind: ProjectMemberKind::Method,
            proc: FuncId(99),
            dispid: None,
            vtable_slot: None,
            is_default_member: false,
            is_enumerator_member: false,
        });
        let errs = verify_program(&p).expect_err("dangling class method proc");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::BadClassMethodProcRef {
                    class_index: 0,
                    method_index: 0,
                    proc: 99,
                    funcs: 0
                }
            )),
            "expected BadClassMethodProcRef, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_duplicate_class_method_dispatch_key() {
        let mut p = sample_class_field_program();
        let mut proc_program = sample_program();
        p.funcs.push(proc_program.funcs.remove(0));
        p.classes[0].methods.extend([
            OxClassMethod {
                name: "Value".to_string(),
                kind: ProjectMemberKind::PropertyGet,
                proc: FuncId(0),
                dispid: None,
                vtable_slot: None,
                is_default_member: false,
                is_enumerator_member: false,
            },
            OxClassMethod {
                name: "value".to_string(),
                kind: ProjectMemberKind::PropertyGet,
                proc: FuncId(0),
                dispid: None,
                vtable_slot: None,
                is_default_member: false,
                is_enumerator_member: false,
            },
        ]);
        let errs = verify_program(&p).expect_err("duplicate class method key");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                VerifyError::DuplicateClassMethod {
                    class_index: 0,
                    method_index: 1,
                    name,
                    kind: ProjectMemberKind::PropertyGet,
                } if name == "value"
            )),
            "expected DuplicateClassMethod, got {errs:?}"
        );
    }

    // ── Typed COM model ──────────────────────────────────────────────────────

    use oxvba_com::{
        ComInterfaceIid, SourceTypeKind, TypeLibInterfaceMetadata, TypeLibMemberInvokeKind,
        TypeLibMemberMetadata, TypeLibParamType,
    };

    /// The canonical (reused) typed descriptor for `Excel.Range.Value` — a `[propget]`
    /// on a dual interface's vtable slot 7 returning a Variant, with no parameters.
    fn value_member(iid: ComInterfaceIid) -> TypeLibMemberMetadata {
        TypeLibMemberMetadata {
            name: "Value".to_string(),
            token: 6,
            vtable_slot: Some(7),
            requires_argument: false,
            invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            parameter_names: vec![],
            parameter_optional: vec![],
            parameter_optional_defaults: vec![],
            is_default_member: true,
            parameter_types: vec![],
            parameter_wire_types: vec![],
            parameter_iids: vec![],
            return_type: Some(TypeLibParamType::Variant),
            return_wire_type: None,
            callconv_is_stdcall: true,
            is_dual: true,
            interface_iid: Some(iid),
            source_typekind: Some(SourceTypeKind::Interface),
            vtable_slot_bound: Some(20),
        }
    }

    /// A small program exercising the typed COM table: `Dim r As Excel.Range`,
    /// `result = r.Value`, with `Value` resolved through the reused
    /// `TypeLibInterfaceMetadata`/`TypeLibMemberMetadata` descriptors.
    fn sample_com_program() -> OxProgram {
        let recv = LocalId(0);
        let result = LocalId(1);

        // A plausible (not load-bearing) IID for the test.
        let iid = ComInterfaceIid {
            data1: 0x0002_0846,
            data2: 0x0000,
            data3: 0x0000,
            data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
        };

        let range_iface = ComInterface::Com(TypeLibInterfaceMetadata {
            name: "Excel.Range".to_string(),
            iid: Some(iid),
            members: vec![value_member(iid)],
        });

        let entry = OxBlock {
            id: BlockId(0),
            instrs: vec![
                OxInst::StmtBoundary {
                    stmt: 0,
                    clear_temps_from: 0,
                },
                OxInst::ComCallEarly {
                    dst: Some(OxPlace::Local(result)),
                    method: ComMethodRef {
                        iface: IfaceId(0),
                        member: 0,
                    },
                    invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                    recv: OxOperand::local(recv),
                    args: vec![],
                },
            ],
            fault_target: Some(BlockId(1)),
            terminator: OxTerminator::Return,
        };
        let landing = OxBlock::new(BlockId(1), OxTerminator::Return);

        let main = OxFunc {
            name: "Main".to_string(),
            kind: ProcedureKind::Sub,
            locals: vec![
                OxLocal {
                    name: "r".to_string(),
                    ty: OxTy::Object(ObjClass::ComIface(IfaceId(0))),
                    array_element: None,
                    param: None,
                    escaped: false,
                },
                OxLocal {
                    name: "result".to_string(),
                    ty: OxTy::Variant,
                    array_element: None,
                    param: None,
                    escaped: false,
                },
            ],
            temps: Vec::new(),
            param_count: 0,
            return_local: None,
            blocks: vec![entry, landing],
            entry: BlockId(0),
        };

        OxProgram {
            funcs: vec![main],
            entry: Some(FuncId(0)),
            unit_name: "ComSample".to_string(),
            com_interfaces: vec![range_iface],
            ..OxProgram::empty()
        }
    }

    #[test]
    fn com_program_verifies_and_resolves_member() {
        let p = sample_com_program();
        assert_eq!(verify_program(&p), Ok(()));
        // The call-site key resolves to the reused, fully-typed descriptor.
        let m = p
            .com_method(ComMethodRef {
                iface: IfaceId(0),
                member: 0,
            })
            .expect("ComMethodRef resolves");
        assert_eq!(m.name, "Value");
        assert_eq!(m.return_type, Some(TypeLibParamType::Variant));
        assert_eq!(m.vtable_slot, Some(7));
    }

    #[test]
    fn com_program_round_trips_through_json() {
        let p = sample_com_program();
        let json = serde_json::to_string(&p).expect("serialize");
        let back: OxProgram = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p, back, "typed COM table must round-trip structurally");
    }

    #[test]
    fn verifier_catches_dangling_com_interface() {
        let mut p = sample_com_program();
        p.funcs[0].blocks[0].instrs[1] = OxInst::ComCallEarly {
            dst: Some(OxPlace::Local(LocalId(1))),
            method: ComMethodRef {
                iface: IfaceId(9),
                member: 0,
            },
            invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            recv: OxOperand::local(LocalId(0)),
            args: vec![],
        };
        let errs = verify_program(&p).expect_err("dangling interface must be rejected");
        assert!(
            errs.iter()
                .any(|e| matches!(e, VerifyError::BadComIfaceRef { iface: 9, .. })),
            "expected BadComIfaceRef, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_bad_com_member() {
        let mut p = sample_com_program();
        p.funcs[0].blocks[0].instrs[1] = OxInst::ComCallEarly {
            dst: Some(OxPlace::Local(LocalId(1))),
            method: ComMethodRef {
                iface: IfaceId(0),
                member: 99,
            },
            invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            recv: OxOperand::local(LocalId(0)),
            args: vec![],
        };
        let errs = verify_program(&p).expect_err("out-of-range member must be rejected");
        assert!(
            errs.iter()
                .any(|e| matches!(e, VerifyError::BadComMemberRef { member: 99, .. })),
            "expected BadComMemberRef, got {errs:?}"
        );
    }

    #[test]
    fn verifier_catches_early_call_on_project_interface() {
        let mut p = sample_com_program();
        // Swap the COM interface for a project `Implements` interface; the early-bound
        // call now targets a non-COM interface and must be rejected.
        p.com_interfaces[0] = ComInterface::Project(ProjectInterface {
            name: "IShape".to_string(),
            methods: vec![ProjectIfaceMethod {
                name: "Value".to_string(),
                kind: ProjectMemberKind::PropertyGet,
            }],
        });
        let errs =
            verify_program(&p).expect_err("early call on a project interface must be rejected");
        assert!(
            errs.iter()
                .any(|e| matches!(e, VerifyError::ComCallEarlyOnProjectIface { iface: 0, .. })),
            "expected ComCallEarlyOnProjectIface, got {errs:?}"
        );
    }
}
