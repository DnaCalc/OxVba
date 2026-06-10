//! Multi-bundle (".NET assembly") linking + cross-bundle execution: two
//! hand-built [`CoreProgram`]s → two `Bundle`s → `Vm::link` → run, exercising
//! `Op::CallExtern` (a free/module function call across a bundle boundary,
//! including a ByRef arg that aliases back into the referrer's bundle).

use oxvba_bundle::coreir::*;
use oxvba_bundle::linearize::linearize;
use oxvba_bundle::{
    AssignmentIntent, AssignmentTargetKind, BundleExport, BundleImport, ExportTarget, ExportToken,
    NumericMode, ProcedureKind, ProjectMemberKind, StringCompareMode,
};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;

fn add_token() -> ExportToken {
    ExportToken::ModuleFunc {
        module: "Lib".into(),
        member: "Add".into(),
        kind: ProjectMemberKind::Method,
    }
}

fn assign(place: CorePlace, value: CoreValue, name: &str) -> CoreStmt {
    CoreStmt::Assign {
        place,
        value,
        intent: AssignmentIntent::Let,
        target_kind: AssignmentTargetKind::Variant,
        target_name: name.into(),
        target_type_name: "Variant".into(),
    }
}

/// Library bundle: `Public Function Add(a, b) As Long` → `a + b`, exported.
fn lib_program() -> CoreProgram {
    let add = CoreProc {
        name: "Add".into(),
        kind: ProcedureKind::Function,
        params: vec![
            CoreParam {
                name: "a".into(),
                by_ref: false,
                variadic: false,
            },
            CoreParam {
                name: "b".into(),
                by_ref: false,
                variadic: false,
            },
        ],
        // params occupy slots 0,1; the synthetic return local is slot 2.
        locals: vec![CoreLocal {
            name: "Add".into(),
            array_element: None,
        }],
        return_local: Some(LocalId(2)),
        body: vec![assign(
            CorePlace::Local(LocalId(2)),
            CoreValue::Binary {
                op: CoreBinOp::Add,
                lhs: Box::new(CoreValue::Load(CorePlace::Local(LocalId(0)))),
                rhs: Box::new(CoreValue::Load(CorePlace::Local(LocalId(1)))),
                mode: StringCompareMode::Binary,
                num: NumericMode::Widening,
            },
            "Add",
        )],
    };
    CoreProgram {
        procs: vec![add],
        unit_name: "Lib".into(),
        exports: vec![BundleExport {
            token: add_token(),
            target: ExportTarget::Proc(0),
        }],
        ..Default::default()
    }
}

/// Referrer bundle: `Sub Main()` calls `Lib.Add(2, 3)` and stores it into global 0.
fn app_program() -> CoreProgram {
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: Vec::new(),
        return_local: None,
        body: vec![assign(
            CorePlace::Global(GlobalId(0)),
            CoreValue::Call {
                callee: CoreCallee::ExternProc { import: 0 },
                args: vec![
                    CoreArg::ByVal(CoreValue::Const(CoreConst::I32(2))),
                    CoreArg::ByVal(CoreValue::Const(CoreConst::I32(3))),
                ],
            },
            "result",
        )],
    };
    CoreProgram {
        globals: vec![CoreGlobal {
            name: "result".into(),
            array_element: None,
        }],
        procs: vec![main],
        imports: vec![BundleImport {
            unit: "Lib".into(),
            token: add_token(),
        }],
        unit_name: "App".into(),
        entry: Some(ProcId(0)),
        ..Default::default()
    }
}

#[test]
fn cross_bundle_call_runs_in_the_referenced_bundle() {
    let lib = linearize(&lib_program()).expect("linearize lib");
    let app = linearize(&app_program()).expect("linearize app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    // Referenced bundle first, referrer (entry) last.
    let mut vm = oxvba_vm2::Vm::link(&[&lib, &app], &host).expect("link");
    vm.run().expect("run");
    // The referrer's global 0 holds Lib.Add(2, 3) = 5, computed in Lib's bundle.
    assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(5));
}

/// Library bundle whose `Boom` raises VBA error 5.
fn boom_lib_program() -> CoreProgram {
    let boom = CoreProc {
        name: "Boom".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: Vec::new(),
        return_local: None,
        body: vec![CoreStmt::Error(ErrorOp::Raise { code: 5 })],
    };
    CoreProgram {
        procs: vec![boom],
        unit_name: "Lib".into(),
        exports: vec![BundleExport {
            token: ExportToken::ModuleFunc {
                module: "Lib".into(),
                member: "Boom".into(),
                kind: ProjectMemberKind::Method,
            },
            target: ExportTarget::Proc(0),
        }],
        ..Default::default()
    }
}

/// Referrer: `Sub Main()` with `On Error Resume Next`, calls `Lib.Boom` (which
/// raises across the bundle boundary), then stores 42. The fault must unwind to
/// Main's handler in App's bundle, so the resume + the store run in App.
fn error_app_program() -> CoreProgram {
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: Vec::new(),
        return_local: None,
        body: vec![
            CoreStmt::Error(ErrorOp::OnErrorResumeNext),
            // Faults inside Lib; Resume Next skips this statement.
            assign(
                CorePlace::Global(GlobalId(0)),
                CoreValue::Call {
                    callee: CoreCallee::ExternProc { import: 0 },
                    args: Vec::new(),
                },
                "result",
            ),
            // Resumed-to statement — must execute in App's bundle.
            assign(
                CorePlace::Global(GlobalId(0)),
                CoreValue::Const(CoreConst::I32(42)),
                "result",
            ),
        ],
    };
    CoreProgram {
        globals: vec![CoreGlobal {
            name: "result".into(),
            array_element: None,
        }],
        procs: vec![main],
        imports: vec![BundleImport {
            unit: "Lib".into(),
            token: ExportToken::ModuleFunc {
                module: "Lib".into(),
                member: "Boom".into(),
                kind: ProjectMemberKind::Method,
            },
        }],
        unit_name: "App".into(),
        entry: Some(ProcId(0)),
        ..Default::default()
    }
}

#[test]
fn cross_bundle_fault_unwinds_to_the_callers_bundle_handler() {
    let lib = linearize(&boom_lib_program()).expect("linearize lib");
    let app = linearize(&error_app_program()).expect("linearize app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let mut vm = oxvba_vm2::Vm::link(&[&lib, &app], &host).expect("link");
    vm.run().expect("run");
    // The error raised in Lib was caught by App's `On Error Resume Next`, and the
    // resumed statement ran in App's bundle → global 0 == 42. (Regression for the
    // route_fault `cur` restore.)
    assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));
}

#[test]
fn link_rejects_an_unresolved_reference() {
    // App imports unit "Lib", but we only load App → the link must fail.
    let app = linearize(&app_program()).expect("linearize app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    match oxvba_vm2::Vm::link(&[&app], &host) {
        Err(e) => assert!(
            e.message.contains("Lib"),
            "error names the missing unit: {}",
            e.message
        ),
        Ok(_) => panic!("a missing referenced unit must not link"),
    }
}
