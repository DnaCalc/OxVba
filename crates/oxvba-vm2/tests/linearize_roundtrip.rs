//! End-to-end: hand-built [`CoreProgram`] → `oxvba_bundle::linearize` → run on
//! this VM.
//!
//! These build the resolved Core IR directly (no binder, no symbol model — the
//! whole point of a symbol-free IR), exercising every linearizer path against
//! the real interpreter. They live here (not inside `oxvba-bundle`) because a
//! round-trip test needs `oxvba-vm2`, and `oxvba-vm2` depends on `oxvba-bundle`
//! — putting the test in `oxvba-bundle` would form a dev-dependency cycle that
//! duplicates the `Bundle` type.

use oxvba_bundle::coreir::*;
use oxvba_bundle::linearize::linearize;
use oxvba_bundle::native::NativeImplId;
use oxvba_bundle::{
    ArrayElementType, AssignmentIntent, AssignmentTargetKind, NumericMode, ProcedureKind,
    StringCompareMode,
};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_runtime::safe_array::{
    VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE, VT_I2_VALUE, VT_I4_VALUE,
    VT_I8_VALUE, VT_R4_VALUE,
};

// ── Builders ───────────────────────────────────────────────────────────────

fn local(name: &str) -> CoreLocal {
    CoreLocal {
        name: name.into(),
        array_element: None,
    }
}

fn main_proc(local_count: usize, body: Vec<CoreStmt>) -> CoreProc {
    CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: (0..local_count).map(|i| local(&format!("v{i}"))).collect(),
        return_local: None,
        body,
    }
}

fn program(procs: Vec<CoreProc>) -> CoreProgram {
    CoreProgram {
        globals: Vec::new(),
        procs,
        classes: Vec::new(),
        event_routes: Vec::new(),
        external_calls: Vec::new(),
        com_class_exports: Vec::new(),
        entry: None,
        ..Default::default()
    }
}

fn single(local_count: usize, body: Vec<CoreStmt>) -> CoreProgram {
    program(vec![main_proc(local_count, body)])
}

fn ci(value: i32) -> CoreValue {
    CoreValue::Const(CoreConst::I32(value))
}
fn cs(value: &str) -> CoreValue {
    CoreValue::Const(CoreConst::Str(value.into()))
}
fn load(slot: usize) -> CoreValue {
    CoreValue::Load(CorePlace::Local(LocalId(slot)))
}
fn bin(op: CoreBinOp, lhs: CoreValue, rhs: CoreValue) -> CoreValue {
    CoreValue::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        mode: StringCompareMode::Binary,
        num: NumericMode::Widening,
    }
}
fn set(slot: usize, value: CoreValue) -> CoreStmt {
    CoreStmt::Assign {
        place: CorePlace::Local(LocalId(slot)),
        value,
        intent: AssignmentIntent::Let,
        target_kind: AssignmentTargetKind::Variant,
        target_name: format!("v{slot}"),
        target_type_name: "Variant".into(),
    }
}
fn arm(condition: CoreValue, body: Vec<CoreStmt>) -> CoreIfArm {
    CoreIfArm { condition, body }
}

// ── Runners ────────────────────────────────────────────────────────────────

fn first_local_f64(program: &CoreProgram) -> Option<f64> {
    let bundle = linearize(program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    let value = vm.slot(bundle.global_count)?;
    value
        .as_f64()
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
}

fn first_local_string(program: &CoreProgram) -> Option<String> {
    let bundle = linearize(program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    vm.slot(bundle.global_count)?.as_bstr().map(|b| b.as_str())
}

fn run_program(program: &CoreProgram) -> (&'static oxvba_bundle::Bundle, oxvba_vm2::Vm<'static>) {
    let bundle = Box::leak(Box::new(linearize(program).expect("linearize")));
    let host = Box::leak(Box::new(NullHostServices::new(
        HostPolicy::deterministic_runtime(),
    )));
    let vm = oxvba_vm2::run(bundle, host).expect("run");
    (bundle, vm)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[test]
fn arithmetic_precedence() {
    // v0 = 1 + 2 * 3
    let p = single(
        1,
        vec![set(
            0,
            bin(CoreBinOp::Add, ci(1), bin(CoreBinOp::Mul, ci(2), ci(3))),
        )],
    );
    assert_eq!(first_local_f64(&p), Some(7.0));
}

#[test]
fn concat_strings() {
    let p = single(1, vec![set(0, bin(CoreBinOp::Concat, cs("ab"), cs("cd")))]);
    assert_eq!(first_local_string(&p).as_deref(), Some("abcd"));
}

#[test]
fn if_elseif_else() {
    // v0 = 0; If 2>3 {1} ElseIf 5>4 {42} Else {7}  → 42
    let p = single(
        1,
        vec![
            set(0, ci(0)),
            CoreStmt::If {
                arms: vec![
                    arm(bin(CoreBinOp::Gt, ci(2), ci(3)), vec![set(0, ci(1))]),
                    arm(bin(CoreBinOp::Gt, ci(5), ci(4)), vec![set(0, ci(42))]),
                ],
                else_body: vec![set(0, ci(7))],
            },
        ],
    );
    assert_eq!(first_local_f64(&p), Some(42.0));
}

#[test]
fn do_while_pre_check() {
    // n=0; Do While n<10: n=n+1  → 10
    let p = single(
        1,
        vec![
            set(0, ci(0)),
            CoreStmt::DoLoop {
                condition: bin(CoreBinOp::Lt, load(0), ci(10)),
                until: false,
                post_check: false,
                body: vec![set(0, bin(CoreBinOp::Add, load(0), ci(1)))],
            },
        ],
    );
    assert_eq!(first_local_f64(&p), Some(10.0));
}

#[test]
fn do_until_post_check() {
    // n=0; Do: n=n+1 Loop Until n>=5  → 5
    let p = single(
        1,
        vec![
            set(0, ci(0)),
            CoreStmt::DoLoop {
                condition: bin(CoreBinOp::Ge, load(0), ci(5)),
                until: true,
                post_check: true,
                body: vec![set(0, bin(CoreBinOp::Add, load(0), ci(1)))],
            },
        ],
    );
    assert_eq!(first_local_f64(&p), Some(5.0));
}

#[test]
fn for_range_with_exit_for() {
    // total=0; For i=1 To 10: If i>5 Then Exit For; total=total+i  → 15
    let p = single(
        2,
        vec![
            set(0, ci(0)),
            CoreStmt::ForRange {
                var: CorePlace::Local(LocalId(1)),
                start: ci(1),
                end: ci(10),
                step: None,
                body: vec![
                    CoreStmt::If {
                        arms: vec![arm(
                            bin(CoreBinOp::Gt, load(1), ci(5)),
                            vec![CoreStmt::Exit(ExitKind::For)],
                        )],
                        else_body: Vec::new(),
                    },
                    set(0, bin(CoreBinOp::Add, load(0), load(1))),
                ],
            },
        ],
    );
    assert_eq!(first_local_f64(&p), Some(15.0));
}

#[test]
fn for_each_over_array_literal() {
    // total=0; For Each item In Array(10,20,30): total=total+item  → 60
    let p = single(
        2,
        vec![
            set(0, ci(0)),
            CoreStmt::ForEach {
                item: CorePlace::Local(LocalId(1)),
                source: CoreValue::ArrayLiteral(vec![ci(10), ci(20), ci(30)]),
                body: vec![set(0, bin(CoreBinOp::Add, load(0), load(1)))],
            },
        ],
    );
    assert_eq!(first_local_f64(&p), Some(60.0));
}

#[test]
fn select_case_value_list_and_is() {
    // r=0; x=2; Select x: Case 1 {10} Case 2,3 {20} Case Is>5 {30} Else {99}  → r=20
    let p = single(
        2,
        vec![
            set(0, ci(0)),
            set(1, ci(2)),
            CoreStmt::Select {
                selector: load(1),
                cases: vec![
                    CoreCaseBlock {
                        clauses: vec![CaseClause::Value(ci(1))],
                        body: vec![set(0, ci(10))],
                    },
                    CoreCaseBlock {
                        clauses: vec![CaseClause::Value(ci(2)), CaseClause::Value(ci(3))],
                        body: vec![set(0, ci(20))],
                    },
                    CoreCaseBlock {
                        clauses: vec![CaseClause::Is {
                            op: CoreBinOp::Gt,
                            value: ci(5),
                        }],
                        body: vec![set(0, ci(30))],
                    },
                ],
                case_else: vec![set(0, ci(99))],
            },
        ],
    );
    assert_eq!(first_local_f64(&p), Some(20.0));
}

#[test]
fn call_proc_by_ref_mutates_caller() {
    // Main: v0 = 5; Inc(v0)   Inc(ByRef n): n = n + 100   → v0 = 105
    let inc = CoreProc {
        name: "Inc".into(),
        kind: ProcedureKind::Sub,
        params: vec![CoreParam {
            name: "n".into(),
            by_ref: true,
            variadic: false,
        }],
        locals: vec![local("n")],
        return_local: None,
        body: vec![set(0, bin(CoreBinOp::Add, load(0), ci(100)))],
    };
    let main = main_proc(
        1,
        vec![
            set(0, ci(5)),
            CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::VbaProc { proc: ProcId(1) },
                args: vec![CoreArg::ByRef(CorePlace::Local(LocalId(0)))],
            }),
        ],
    );
    let p = program(vec![main, inc]);
    assert_eq!(first_local_f64(&p), Some(105.0));
}

#[test]
fn call_native_len() {
    // v0 = Len("hello")  → 5
    let p = single(
        1,
        vec![set(
            0,
            CoreValue::Call {
                callee: CoreCallee::Native(NativeImplId::Len),
                args: vec![CoreArg::ByVal(cs("hello"))],
            },
        )],
    );
    assert_eq!(first_local_f64(&p), Some(5.0));
}

#[test]
fn redim_array_set_get_roundtrip() {
    // ReDim v1(0 To 2); v1(1) = 77; v0 = v1(1)  → 77
    let elem = |idx: i32| CorePlace::Index {
        array: Box::new(CorePlace::Local(LocalId(1))),
        indices: vec![ci(idx)],
    };
    let p = single(
        2,
        vec![
            CoreStmt::ReDim {
                array: CorePlace::Local(LocalId(1)),
                bounds: vec![CoreBound {
                    upper: ci(2),
                    lower: 0,
                }],
                element_type: ArrayElementType::Variant,
                preserve: false,
            },
            CoreStmt::Assign {
                place: elem(1),
                value: ci(77),
                intent: AssignmentIntent::Let,
                target_kind: AssignmentTargetKind::Variant,
                target_name: "v1".into(),
                target_type_name: "Variant".into(),
            },
            set(0, CoreValue::Load(elem(1))),
        ],
    );
    assert_eq!(first_local_f64(&p), Some(77.0));
}

#[test]
fn redim_long_array_uses_typed_safearray_storage() {
    let elem = |idx: i32| CorePlace::Index {
        array: Box::new(CorePlace::Local(LocalId(1))),
        indices: vec![ci(idx)],
    };
    let p = single(
        2,
        vec![
            CoreStmt::ReDim {
                array: CorePlace::Local(LocalId(1)),
                bounds: vec![CoreBound {
                    upper: ci(2),
                    lower: 0,
                }],
                element_type: ArrayElementType::Long,
                preserve: false,
            },
            CoreStmt::Assign {
                place: elem(1),
                value: ci(77),
                intent: AssignmentIntent::Let,
                target_kind: AssignmentTargetKind::Scalar,
                target_name: "v1".into(),
                target_type_name: "Long".into(),
            },
            set(0, CoreValue::Load(elem(1))),
        ],
    );
    let (bundle, vm) = run_program(&p);
    let array = vm
        .slot(bundle.global_count + 1)
        .and_then(|value| value.as_safearray())
        .expect("array slot");
    assert_eq!(array.element_vartype(), VT_I4_VALUE);
    assert_eq!(
        array.variant_elements().expect("elements"),
        vec![
            oxvba_runtime::Variant::from_i32(0),
            oxvba_runtime::Variant::from_i32(77),
            oxvba_runtime::Variant::from_i32(0),
        ]
    );
    assert_eq!(
        vm.slot(bundle.global_count)
            .and_then(|value| value.as_i32()),
        Some(77)
    );
}

#[test]
fn record_field_set_and_get_use_backing_record_slot() {
    let field = |base: usize, index: usize| CorePlace::RecordField {
        base: Box::new(CorePlace::Local(LocalId(base))),
        index,
    };
    let p = single(
        2,
        vec![
            set(1, CoreValue::NewRecord { fields: 2 }),
            CoreStmt::Assign {
                place: field(1, 0),
                value: ci(42),
                intent: AssignmentIntent::Let,
                target_kind: AssignmentTargetKind::Variant,
                target_name: "v1.X".into(),
                target_type_name: "Variant".into(),
            },
            set(0, CoreValue::Load(field(1, 0))),
        ],
    );

    let (bundle, vm) = run_program(&p);
    assert_eq!(
        vm.slot(bundle.global_count)
            .and_then(|value| value.as_i32()),
        Some(42)
    );
    let record = vm
        .slot(bundle.global_count + 1)
        .and_then(|value| value.as_safearray())
        .expect("record slot");
    assert_eq!(
        record.variant_element(0).expect("record field").as_i32(),
        Some(42)
    );
    assert_eq!(
        record.variant_element(1).expect("record field").vtype(),
        oxvba_runtime::Variant::empty().vtype()
    );
}

#[test]
fn redim_udt_array_uses_native_vba_record_elements() {
    let p = single(
        1,
        vec![CoreStmt::ReDim {
            array: CorePlace::Local(LocalId(0)),
            bounds: vec![CoreBound {
                upper: ci(0),
                lower: 0,
            }],
            element_type: ArrayElementType::Record(vec![
                ArrayElementType::Long,
                ArrayElementType::String,
            ]),
            preserve: false,
        }],
    );

    let (bundle, vm) = run_program(&p);
    let array = vm
        .slot(bundle.global_count)
        .and_then(|value| value.as_safearray())
        .expect("array slot");
    let element = array.variant_element(0).expect("record element");
    assert_eq!(element.vtype(), oxvba_runtime::VarType::Record);
    assert!(element.as_safearray().is_none());
    let record = element.as_vba_record().expect("native VBA record");
    assert_eq!(
        record.read_field_variant(0).expect("long field").as_i32(),
        Some(0)
    );
    assert_eq!(
        record
            .read_field_variant(1)
            .expect("string field")
            .as_bstr()
            .map(|text| text.as_str()),
        Some(String::new())
    );
}

#[test]
fn redim_scalar_arrays_seed_matching_exact_carriers() {
    let cases = [
        (ArrayElementType::Integer, VT_I2_VALUE),
        (ArrayElementType::Long, VT_I4_VALUE),
        (
            ArrayElementType::LongPtr,
            if core::mem::size_of::<usize>() == 8 {
                VT_I8_VALUE
            } else {
                VT_I4_VALUE
            },
        ),
        (ArrayElementType::Single, VT_R4_VALUE),
        (ArrayElementType::Currency, VT_CY_VALUE),
        (ArrayElementType::Date, VT_DATE_VALUE),
        (ArrayElementType::String, VT_BSTR_VALUE),
        (ArrayElementType::Boolean, VT_BOOL_VALUE),
    ];

    for (element_type, expected_vt) in cases {
        let p = single(
            1,
            vec![CoreStmt::ReDim {
                array: CorePlace::Local(LocalId(0)),
                bounds: vec![CoreBound {
                    upper: ci(0),
                    lower: 0,
                }],
                element_type,
                preserve: false,
            }],
        );
        let (bundle, vm) = run_program(&p);
        let array = vm
            .slot(bundle.global_count)
            .and_then(|value| value.as_safearray())
            .expect("array slot");
        assert_eq!(array.element_vartype(), expected_vt);
        assert_eq!(array.variant_elements().expect("elements").len(), 1);
    }
}

#[test]
fn array_set_out_of_range_still_raises_error_9() {
    let elem = |idx: i32| CorePlace::Index {
        array: Box::new(CorePlace::Local(LocalId(0))),
        indices: vec![ci(idx)],
    };
    let p = single(
        1,
        vec![
            CoreStmt::ReDim {
                array: CorePlace::Local(LocalId(0)),
                bounds: vec![CoreBound {
                    upper: ci(0),
                    lower: 0,
                }],
                element_type: ArrayElementType::Long,
                preserve: false,
            },
            CoreStmt::Assign {
                place: elem(1),
                value: ci(77),
                intent: AssignmentIntent::Let,
                target_kind: AssignmentTargetKind::Scalar,
                target_name: "v0".into(),
                target_type_name: "Long".into(),
            },
        ],
    );
    let bundle = linearize(&p).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let err = match oxvba_vm2::run(&bundle, &host) {
        Ok(_) => panic!("out of range assignment should fail"),
        Err(err) => err,
    };
    assert_eq!(err.code, 9);
}

#[test]
fn on_error_resume_next_records_err_number() {
    // On Error Resume Next; Err.Raise 11; v0 = Err.Number  → 11
    let p = single(
        1,
        vec![
            CoreStmt::Error(ErrorOp::OnErrorResumeNext),
            CoreStmt::Error(ErrorOp::Raise { code: 11 }),
            set(0, CoreValue::ErrField(ErrField::Number)),
        ],
    );
    assert_eq!(first_local_f64(&p), Some(11.0));
}

#[test]
fn goto_backward_jump() {
    // n=0; top: n=n+1; If n<3 Then Goto top  → 3
    let p = single(
        1,
        vec![
            set(0, ci(0)),
            CoreStmt::Label(LabelId(0)),
            set(0, bin(CoreBinOp::Add, load(0), ci(1))),
            CoreStmt::If {
                arms: vec![arm(
                    bin(CoreBinOp::Lt, load(0), ci(3)),
                    vec![CoreStmt::Goto(LabelId(0))],
                )],
                else_body: Vec::new(),
            },
        ],
    );
    assert_eq!(first_local_f64(&p), Some(3.0));
}

#[test]
fn gosub_two_sites_return_correctly() {
    // n=0; GoSub add; GoSub add; Exit Sub; add: n=n+10; Return  → 20
    let p = single(
        1,
        vec![
            set(0, ci(0)),
            CoreStmt::GoSub(LabelId(0)),
            CoreStmt::GoSub(LabelId(0)),
            CoreStmt::Exit(ExitKind::Proc),
            CoreStmt::Label(LabelId(0)),
            set(0, bin(CoreBinOp::Add, load(0), ci(10))),
            CoreStmt::GoSubReturn,
        ],
    );
    assert_eq!(first_local_f64(&p), Some(20.0));
}
