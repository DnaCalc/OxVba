//! Multi-`OxProgram` cross-project linking + execution on vm3 (the W2 executor): two
//! hand-built [`CoreProgram`]s → `elaborate` → `Vm3::link` → run, exercising a free/module
//! function call across a program boundary (`OxInst::CallExtern` to another program's exported
//! proc), a fault that unwinds across the boundary to the caller's handler, and an unresolved
//! reference. The vm3 counterpart of `oxvba-vm2/tests/cross_bundle_roundtrip.rs` — the tested
//! scope vm3 must match so cross-project sessions never regress when vm2 retires.

use oxvba_bundle::coreir::*;
use oxvba_bundle::{
    AssignmentIntent, AssignmentTargetKind, BundleExport, BundleImport, ExportTarget, ExportToken,
    NumericMode, ProcedureKind, ProjectMemberKind, StringCompareMode,
};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_oxir::elaborate::elaborate;
use oxvba_vm3::{Vm3, Vm3Error};

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

/// Library program: `Public Function Add(a, b) As Long` → `a + b`, exported.
fn lib_program() -> CoreProgram {
    let add = CoreProc {
        name: "Add".into(),
        kind: ProcedureKind::Function,
        params: vec![
            CoreParam {
                name: "a".into(),
                ty: oxvba_bundle::VarTypeRef::Variant,
                by_ref: false,
                variadic: false,
            },
            CoreParam {
                name: "b".into(),
                ty: oxvba_bundle::VarTypeRef::Variant,
                by_ref: false,
                variadic: false,
            },
        ],
        // params occupy slots 0,1; the synthetic return local is slot 2.
        locals: vec![CoreLocal {
            name: "Add".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
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

/// Referrer program: `Sub Main()` calls `Lib.Add(2, 3)` and stores it into global 0.
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
            ty: oxvba_bundle::VarTypeRef::Variant,
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
fn cross_program_call_runs_in_the_referenced_program() {
    let lib = elaborate(&lib_program()).expect("elaborate lib");
    let app = elaborate(&app_program()).expect("elaborate app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    // Referenced program first, referrer (entry) last.
    let mut vm = Vm3::link(&[&lib, &app], &host).expect("link");
    vm.run_entry().expect("run");
    // The referrer's global 0 holds Lib.Add(2, 3) = 5, computed in Lib's program but written
    // back into App's global (the dst stays bound to App via Loc::Global tagging).
    assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(5));
}

/// Library program whose `Boom` raises VBA error 5.
fn boom_lib_program() -> CoreProgram {
    let boom = CoreProc {
        name: "Boom".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: Vec::new(),
        return_local: None,
        body: vec![CoreStmt::Error(ErrorOp::Raise {
            number: CoreValue::Const(CoreConst::I32(5)),
            source: None,
            description: None,
            inherit: true,
        })],
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

/// Referrer: `Sub Main()` with `On Error Resume Next`, calls `Lib.Boom` (which raises across the
/// program boundary), then stores 42. The fault must unwind to Main's handler in App, so the
/// resume + the store run in App.
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
            // Resumed-to statement — must execute in App's program.
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
            ty: oxvba_bundle::VarTypeRef::Variant,
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
fn cross_program_fault_unwinds_to_the_callers_handler() {
    let lib = elaborate(&boom_lib_program()).expect("elaborate lib");
    let app = elaborate(&error_app_program()).expect("elaborate app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let mut vm = Vm3::link(&[&lib, &app], &host).expect("link");
    vm.run_entry().expect("run");
    // The error raised in Lib was caught by App's `On Error Resume Next`, and the resumed
    // statement ran in App → global 0 == 42 (the per-iteration `cur` re-derivation restores the
    // caller's program as the fault unwinds, with no explicit route_fault bookkeeping).
    assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));
    // Err carries the ORIGIN project's name as it unwinds across the boundary (Resume Next does
    // not clear Err): the error came from Lib, even though App's handler caught it.
    assert_eq!(vm.err_number(), 5);
    assert_eq!(
        vm.err_source(),
        "Lib",
        "Err.Source is the ORIGIN project, not the catching project"
    );
}

#[test]
fn link_rejects_an_unresolved_reference() {
    // App imports unit "Lib", but we only load App → the link must fail naming the missing unit.
    let app = elaborate(&app_program()).expect("elaborate app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    match Vm3::link(&[&app], &host) {
        Err(Vm3Error::Malformed(m)) => {
            assert!(m.contains("Lib"), "error names the missing unit: {m}")
        }
        Err(e) => panic!("expected Malformed naming Lib, got {e:?}"),
        Ok(_) => panic!("a missing referenced unit must not link"),
    }
}
