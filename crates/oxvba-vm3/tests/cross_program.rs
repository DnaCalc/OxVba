//! Multi-`OxProgram` cross-project linking + execution on vm3 (the W2 executor): two
//! hand-built [`CoreProgram`]s → `elaborate` → `Vm3::link` → run, exercising a free/module
//! function call across a program boundary (`OxInst::CallExtern` to another program's exported
//! proc), a fault that unwinds across the boundary to the caller's handler, and an unresolved
//! reference. The cross-project session scope vm3 must support (cross-bundle calls, faults
//! unwinding across program boundaries, and unresolved-reference rejection).

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
        label_lines: Vec::new(),
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
        label_lines: Vec::new(),
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
        label_lines: Vec::new(),
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
        label_lines: Vec::new(),
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

/// Library program: `Function Apply(o)` returns `o.GetVal()` via a late-bound dispatch — the
/// receiver `o` is an object minted by a DIFFERENT program, so dispatch must resolve its class
/// in the object's own program, not Lib's.
fn lib_apply_program() -> CoreProgram {
    let apply = CoreProc {
        name: "Apply".into(),
        kind: ProcedureKind::Function,
        params: vec![CoreParam {
            name: "o".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            by_ref: false,
            variadic: false,
        }],
        locals: vec![CoreLocal {
            name: "Apply".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            array_element: None,
        }],
        return_local: Some(LocalId(1)),
        label_lines: Vec::new(),
        body: vec![assign(
            CorePlace::Local(LocalId(1)),
            CoreValue::Call {
                callee: CoreCallee::LateDispatch {
                    name: "GetVal".into(),
                    kind: Some(ProjectMemberKind::Method),
                },
                args: vec![CoreArg::ByVal(CoreValue::Load(CorePlace::Local(LocalId(0))))],
            },
            "Apply",
        )],
    };
    CoreProgram {
        procs: vec![apply],
        unit_name: "Lib".into(),
        exports: vec![BundleExport {
            token: ExportToken::ModuleFunc {
                module: "Lib".into(),
                member: "Apply".into(),
                kind: ProjectMemberKind::Method,
            },
            target: ExportTarget::Proc(0),
        }],
        ..Default::default()
    }
}

/// Referrer program owning `Class Thing` (`Function GetVal() = 99`). `Sub Main()` creates a
/// Thing and passes it to `Lib.Apply` — so the object is minted in App but dispatched in Lib.
fn app_with_thing_program() -> CoreProgram {
    let getval = CoreProc {
        name: "GetVal".into(),
        kind: ProcedureKind::Function,
        params: vec![CoreParam {
            name: "me".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            by_ref: false,
            variadic: false,
        }],
        locals: vec![CoreLocal {
            name: "GetVal".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            array_element: None,
        }],
        return_local: Some(LocalId(1)),
        label_lines: Vec::new(),
        body: vec![assign(
            CorePlace::Local(LocalId(1)),
            CoreValue::Const(CoreConst::I32(99)),
            "GetVal",
        )],
    };
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: Vec::new(),
        return_local: None,
        label_lines: Vec::new(),
        body: vec![assign(
            CorePlace::Global(GlobalId(0)),
            CoreValue::Call {
                callee: CoreCallee::ExternProc { import: 0 },
                args: vec![CoreArg::ByVal(CoreValue::New(ClassId(0)))],
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
        procs: vec![main, getval], // Main = ProcId 0, GetVal = ProcId 1
        classes: vec![CoreClass {
            name: "Thing".into(),
            initialize: None,
            terminate: None,
            methods: vec![CoreClassMethod {
                name: "GetVal".into(),
                kind: ProjectMemberKind::Method,
                proc: ProcId(1),
                is_default_member: false,
            }],
            implements: Vec::new(),
        }],
        imports: vec![BundleImport {
            unit: "Lib".into(),
            token: ExportToken::ModuleFunc {
                module: "Lib".into(),
                member: "Apply".into(),
                kind: ProjectMemberKind::Method,
            },
        }],
        unit_name: "App".into(),
        entry: Some(ProcId(0)),
        ..Default::default()
    }
}

#[test]
fn cross_program_dispatch_uses_the_objects_program_not_the_executing_one() {
    // App owns class Thing (GetVal = 99). App.Main creates a Thing and passes it to Lib.Apply,
    // which late-binds `o.GetVal()` while executing in LIB's program. Dispatch must resolve the
    // object's class in ITS OWN program (App, via bundle_id), not the executing program (Lib) —
    // otherwise it mis-dispatches or raises 438. Result (App global 0) == 99. (Slice 5b: object
    // dispatch keyed on ObjectRef.bundle_id, not the executing cur.)
    let lib = elaborate(&lib_apply_program()).expect("elaborate lib");
    let app = elaborate(&app_with_thing_program()).expect("elaborate app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let mut vm = Vm3::link(&[&lib, &app], &host).expect("link");
    vm.run_entry().expect("run");
    assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(99));
}

/// Library program: `Function Apply(o)` sets ITS module global `gLib = 55` then returns
/// `o.Echo(gLib)` — the late-bound method argument names a LIB global, so it must resolve in
/// Lib (the dispatching program), not the object's program.
fn lib_apply_global_arg_program() -> CoreProgram {
    let apply = CoreProc {
        name: "Apply".into(),
        kind: ProcedureKind::Function,
        params: vec![CoreParam {
            name: "o".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            by_ref: false,
            variadic: false,
        }],
        locals: vec![CoreLocal {
            name: "Apply".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            array_element: None,
        }],
        return_local: Some(LocalId(1)),
        label_lines: Vec::new(),
        body: vec![
            assign(
                CorePlace::Global(GlobalId(0)),
                CoreValue::Const(CoreConst::I32(55)),
                "gLib",
            ),
            assign(
                CorePlace::Local(LocalId(1)),
                CoreValue::Call {
                    callee: CoreCallee::LateDispatch {
                        name: "Echo".into(),
                        kind: Some(ProjectMemberKind::Method),
                    },
                    args: vec![
                        CoreArg::ByVal(CoreValue::Load(CorePlace::Local(LocalId(0)))),
                        CoreArg::ByVal(CoreValue::Load(CorePlace::Global(GlobalId(0)))),
                    ],
                },
                "Apply",
            ),
        ],
    };
    CoreProgram {
        globals: vec![CoreGlobal {
            name: "gLib".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            array_element: None,
        }],
        procs: vec![apply],
        unit_name: "Lib".into(),
        exports: vec![BundleExport {
            token: ExportToken::ModuleFunc {
                module: "Lib".into(),
                member: "Apply".into(),
                kind: ProjectMemberKind::Method,
            },
            target: ExportTarget::Proc(0),
        }],
        ..Default::default()
    }
}

/// Referrer owning `Class Thing` (`Function Echo(n) = n`); `Sub Main()` does
/// `result = Lib.Apply(New Thing)`.
fn app_echo_program() -> CoreProgram {
    let echo = CoreProc {
        name: "Echo".into(),
        kind: ProcedureKind::Function,
        params: vec![
            CoreParam {
                name: "me".into(),
                ty: oxvba_bundle::VarTypeRef::Variant,
                by_ref: false,
                variadic: false,
            },
            CoreParam {
                name: "n".into(),
                ty: oxvba_bundle::VarTypeRef::Variant,
                by_ref: false,
                variadic: false,
            },
        ],
        locals: vec![CoreLocal {
            name: "Echo".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            array_element: None,
        }],
        return_local: Some(LocalId(2)),
        label_lines: Vec::new(),
        body: vec![assign(
            CorePlace::Local(LocalId(2)),
            CoreValue::Load(CorePlace::Local(LocalId(1))),
            "Echo",
        )],
    };
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: Vec::new(),
        return_local: None,
        label_lines: Vec::new(),
        body: vec![assign(
            CorePlace::Global(GlobalId(0)),
            CoreValue::Call {
                callee: CoreCallee::ExternProc { import: 0 },
                args: vec![CoreArg::ByVal(CoreValue::New(ClassId(0)))],
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
        procs: vec![main, echo], // Main = ProcId 0, Echo = ProcId 1
        classes: vec![CoreClass {
            name: "Thing".into(),
            initialize: None,
            terminate: None,
            methods: vec![CoreClassMethod {
                name: "Echo".into(),
                kind: ProjectMemberKind::Method,
                proc: ProcId(1),
                is_default_member: false,
            }],
            implements: Vec::new(),
        }],
        imports: vec![BundleImport {
            unit: "Lib".into(),
            token: ExportToken::ModuleFunc {
                module: "Lib".into(),
                member: "Apply".into(),
                kind: ProjectMemberKind::Method,
            },
        }],
        unit_name: "App".into(),
        entry: Some(ProcId(0)),
        ..Default::default()
    }
}

#[test]
fn cross_program_method_arg_resolves_in_the_callers_program() {
    // Lib.Apply sets its OWN module global gLib=55 and calls o.Echo(gLib) on an App-minted Thing.
    // The argument names a LIB global, so it must resolve in Lib (the dispatching program), not
    // the object's program (App) — so Echo receives 55, and result == 55. (Slice 5b review fix:
    // method args are resolved in the caller's program BEFORE cur switches to the object's; the
    // bug read App's global slot 0, which is `result`, still Empty -> wrong value.)
    let lib = elaborate(&lib_apply_global_arg_program()).expect("elaborate lib");
    let app = elaborate(&app_echo_program()).expect("elaborate app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let mut vm = Vm3::link(&[&lib, &app], &host).expect("link");
    vm.run_entry().expect("run");
    assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(55));
}

/// Library program owning `Class Widget` (`Function Val() = 42`), exported as a public class.
fn lib_widget_program() -> CoreProgram {
    let val = CoreProc {
        name: "Val".into(),
        kind: ProcedureKind::Function,
        params: vec![CoreParam {
            name: "me".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            by_ref: false,
            variadic: false,
        }],
        locals: vec![CoreLocal {
            name: "Val".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            array_element: None,
        }],
        return_local: Some(LocalId(1)),
        label_lines: Vec::new(),
        body: vec![assign(
            CorePlace::Local(LocalId(1)),
            CoreValue::Const(CoreConst::I32(42)),
            "Val",
        )],
    };
    CoreProgram {
        procs: vec![val],
        classes: vec![CoreClass {
            name: "Widget".into(),
            initialize: None,
            terminate: None,
            methods: vec![CoreClassMethod {
                name: "Val".into(),
                kind: ProjectMemberKind::Method,
                proc: ProcId(0),
                is_default_member: false,
            }],
            implements: Vec::new(),
        }],
        unit_name: "Lib".into(),
        exports: vec![BundleExport {
            token: ExportToken::Class {
                name: "Widget".into(),
            },
            target: ExportTarget::Class(0),
        }],
        ..Default::default()
    }
}

/// Referrer: `Sub Main()` does `Set w = New Lib.Widget` then `result = w.Val()`.
fn app_new_extern_program() -> CoreProgram {
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: vec![CoreLocal {
            name: "w".into(),
            ty: oxvba_bundle::VarTypeRef::Variant,
            array_element: None,
        }],
        return_local: None,
        label_lines: Vec::new(),
        body: vec![
            // Set w = New Lib.Widget
            CoreStmt::Assign {
                place: CorePlace::Local(LocalId(0)),
                value: CoreValue::NewExtern { import: 0 },
                intent: AssignmentIntent::Set,
                target_kind: AssignmentTargetKind::Object,
                target_name: "w".into(),
                target_type_name: "Object".into(),
            },
            // result = w.Val()
            assign(
                CorePlace::Global(GlobalId(0)),
                CoreValue::Call {
                    callee: CoreCallee::LateDispatch {
                        name: "Val".into(),
                        kind: Some(ProjectMemberKind::Method),
                    },
                    args: vec![CoreArg::ByVal(CoreValue::Load(CorePlace::Local(LocalId(0))))],
                },
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
            token: ExportToken::Class {
                name: "Widget".into(),
            },
        }],
        unit_name: "App".into(),
        entry: Some(ProcId(0)),
        ..Default::default()
    }
}

/// Referrer: `Sub Main()` does `result = Lib.Widget.Val()` referencing Widget as a predeclared
/// singleton of the referenced project (a `PredeclaredExtern`, no `New`).
fn app_predeclared_extern_program() -> CoreProgram {
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: Vec::new(),
        return_local: None,
        label_lines: Vec::new(),
        body: vec![assign(
            CorePlace::Global(GlobalId(0)),
            CoreValue::Call {
                callee: CoreCallee::LateDispatch {
                    name: "Val".into(),
                    kind: Some(ProjectMemberKind::Method),
                },
                args: vec![CoreArg::ByVal(CoreValue::PredeclaredExtern { import: 0 })],
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
            token: ExportToken::Class {
                name: "Widget".into(),
            },
        }],
        unit_name: "App".into(),
        entry: Some(ProcId(0)),
        ..Default::default()
    }
}

#[test]
fn cross_program_predeclared_resolves_the_referenced_singleton() {
    // App references Lib.Widget as a predeclared singleton: result = Lib.Widget.Val() -> 42, the
    // singleton minted + cached in Lib (bundle_id = Lib). (Slice 5b: cross-project
    // PredeclaredExtern, the no-New sibling of NewExtern.)
    let lib = elaborate(&lib_widget_program()).expect("elaborate lib");
    let app = elaborate(&app_predeclared_extern_program()).expect("elaborate app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let mut vm = Vm3::link(&[&lib, &app], &host).expect("link");
    vm.run_entry().expect("run");
    assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));
}

#[test]
fn cross_program_new_creates_and_dispatches_the_referenced_class() {
    // App does `Set w = New Lib.Widget : result = w.Val()`. The instance is minted in Lib (its
    // bundle_id + descriptor + Class_Initialize), and w.Val() dispatches back into Lib -> 42.
    // (Slice 5b item 5: cross-project object CREATION via NewExtern of a referenced class.)
    let lib = elaborate(&lib_widget_program()).expect("elaborate lib");
    let app = elaborate(&app_new_extern_program()).expect("elaborate app");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let mut vm = Vm3::link(&[&lib, &app], &host).expect("link");
    vm.run_entry().expect("run");
    assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));
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
