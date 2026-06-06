//! End-to-end for the **object / class / event** Core IR: hand-built
//! [`CoreProgram`] → `oxvba_bundle::linearize` → run on this VM.
//!
//! The procedural `linearize_roundtrip.rs` suite never builds `classes`,
//! `event_routes`, or object places, so the `linearize → vm2` path for objects
//! and events had no coverage. These tests lock that path down — and double as
//! the **executable spec** for the conventions the `oxvba-bind` binder must emit:
//!
//! - **A class-member proc has `Me` as synthetic parameter 0.** `linearize` lays
//!   out `LocalId(0..params) = params`, and vm2's `run_proc_with_me` binds the
//!   receiver at frame slot 0 and the i-th call arg at slot `1+i`. So a method
//!   frame is `[Me, real params.., locals, return_local]`.
//! - **Object method/property calls dispatch by name** via
//!   `CoreCallee::LateDispatch` with the receiver as `args[0]` (a direct
//!   `VbaProc`/`Op::CallProc` would not bind `Me`).
//! - **Instance fields** are an arbitrary stable `i32` token in
//!   `CorePlace::Field` (vm2 stores them in a dynamic map).
//! - **Events**: a `WithEvents` field carries a `binding` token; `RaiseEvent`
//!   carries the source-class event index; `EventRoute{binding,event,handler}`
//!   ties a sink field + source event to the handler proc.

use oxvba_bundle::coreir::*;
use oxvba_bundle::linearize::linearize;
use oxvba_bundle::{
    AssignmentIntent, AssignmentTargetKind, EventRoute, ProcedureKind, ProjectMemberKind,
    StringCompareMode,
};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;

// ── Builders ─────────────────────────────────────────────────────────────────

fn ci(v: i32) -> CoreValue {
    CoreValue::Const(CoreConst::I32(v))
}
fn local_load(slot: usize) -> CoreValue {
    CoreValue::Load(CorePlace::Local(LocalId(slot)))
}
/// `Me` inside a class-member proc — synthetic parameter 0.
fn me() -> CoreValue {
    CoreValue::Load(CorePlace::Local(LocalId(0)))
}
fn field(object: CoreValue, token: i32) -> CorePlace {
    CorePlace::Field { object: Box::new(object), field: token }
}
fn bin(op: CoreBinOp, lhs: CoreValue, rhs: CoreValue) -> CoreValue {
    CoreValue::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), mode: StringCompareMode::Binary }
}
fn param(name: &str) -> CoreParam {
    CoreParam { name: name.into(), by_ref: false, variadic: false }
}
fn local(name: &str) -> CoreLocal {
    CoreLocal { name: name.into(), array_element: None }
}

fn assign(place: CorePlace, value: CoreValue, intent: AssignmentIntent, target_kind: AssignmentTargetKind) -> CoreStmt {
    CoreStmt::Assign {
        place,
        value,
        intent,
        target_kind,
        target_name: "t".into(),
        target_type_name: "t".into(),
    }
}
fn let_local(slot: usize, value: CoreValue) -> CoreStmt {
    assign(CorePlace::Local(LocalId(slot)), value, AssignmentIntent::Let, AssignmentTargetKind::Variant)
}
fn set_local(slot: usize, value: CoreValue) -> CoreStmt {
    assign(CorePlace::Local(LocalId(slot)), value, AssignmentIntent::Set, AssignmentTargetKind::Object)
}

/// A by-name object dispatch (`receiver.<name>(args)`), receiver passed as arg0.
fn late_call(name: &str, kind: ProjectMemberKind, mut method_args: Vec<CoreArg>, receiver: CoreValue) -> CoreValue {
    let mut args = vec![CoreArg::ByVal(receiver)];
    args.append(&mut method_args);
    CoreValue::Call { callee: CoreCallee::LateDispatch { name: name.into(), kind: Some(kind) }, args }
}

fn class(name: &str, initialize: Option<usize>, terminate: Option<usize>, methods: Vec<(&str, ProjectMemberKind, usize)>) -> CoreClass {
    CoreClass {
        name: name.into(),
        initialize: initialize.map(ProcId),
        terminate: terminate.map(ProcId),
        methods: methods
            .into_iter()
            .map(|(n, kind, p)| CoreClassMethod { name: n.into(), kind, proc: ProcId(p) })
            .collect(),
    }
}

fn program(globals: usize, procs: Vec<CoreProc>, classes: Vec<CoreClass>, event_routes: Vec<EventRoute>) -> CoreProgram {
    CoreProgram {
        globals: (0..globals).map(|i| CoreGlobal { name: format!("g{i}"), array_element: None }).collect(),
        procs,
        classes,
        event_routes,
        external_calls: Vec::new(),
        com_class_exports: Vec::new(),
        entry: Some(ProcId(0)),
    }
}

// ── Runners ──────────────────────────────────────────────────────────────────

fn first_local_i32(p: &CoreProgram) -> Option<i32> {
    let bundle = linearize(p).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    vm.slot(bundle.global_count)?.as_i32()
}

fn global0_i32(p: &CoreProgram) -> Option<i32> {
    let bundle = linearize(p).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    vm.slot(0)?.as_i32()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[test]
fn new_runs_initialize_then_method_reads_field() {
    // Class C { Private f; Sub Class_Initialize(): Me.f = 42; Function Get(): Get = Me.f }
    // Main: Set obj = New C; r = obj.Get()   → 42
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: vec![],
        locals: vec![local("r"), local("obj")],
        return_local: None,
        body: vec![
            set_local(1, CoreValue::New(ClassId(0))),
            let_local(0, late_call("Get", ProjectMemberKind::Method, vec![], local_load(1))),
        ],
    };
    let init = CoreProc {
        name: "Class_Initialize".into(),
        kind: ProcedureKind::Sub,
        params: vec![param("Me")], // Me = synthetic param 0
        locals: vec![],
        return_local: None,
        body: vec![assign(field(me(), 0), ci(42), AssignmentIntent::Let, AssignmentTargetKind::Variant)],
    };
    let get = CoreProc {
        name: "Get".into(),
        kind: ProcedureKind::Function,
        params: vec![param("Me")],
        locals: vec![local("Get")], // return local = LocalId(1) (after Me)
        return_local: Some(LocalId(1)),
        body: vec![let_local(1, CoreValue::Load(field(me(), 0)))],
    };
    let p = program(
        0,
        vec![main, init, get],
        vec![class("C", Some(1), None, vec![("Get", ProjectMemberKind::Method, 2)])],
        Vec::new(),
    );
    assert_eq!(first_local_i32(&p), Some(42));
}

#[test]
fn late_method_dispatch_with_argument() {
    // Class C { Function Inc(n): Inc = n + 1 }   Main: r = (New C).Inc(41)  → 42
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: vec![],
        locals: vec![local("r"), local("obj")],
        return_local: None,
        body: vec![
            set_local(1, CoreValue::New(ClassId(0))),
            let_local(0, late_call("Inc", ProjectMemberKind::Method, vec![CoreArg::ByVal(ci(41))], local_load(1))),
        ],
    };
    let inc = CoreProc {
        name: "Inc".into(),
        kind: ProcedureKind::Function,
        params: vec![param("Me"), param("n")], // Me=0, n=1
        locals: vec![local("Inc")],            // return local = LocalId(2)
        return_local: Some(LocalId(2)),
        body: vec![let_local(2, bin(CoreBinOp::Add, local_load(1), ci(1)))],
    };
    let p = program(0, vec![main, inc], vec![class("C", None, None, vec![("Inc", ProjectMemberKind::Method, 1)])], Vec::new());
    assert_eq!(first_local_i32(&p), Some(42));
}

#[test]
fn project_method_byref_mutates_caller() {
    // Class C { Sub Inc(ByRef n As Long): n = n + 100 }
    // Main: r = 5; Set o = New C; o.Inc r  → r = 105 (ByRef through method dispatch:
    // CallArg::ByRef → dispatch_project_method → ProcArg::ByRef → true alias).
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: vec![],
        locals: vec![local("r"), local("o")],
        return_local: None,
        body: vec![
            let_local(0, ci(5)),
            set_local(1, CoreValue::New(ClassId(0))),
            CoreStmt::Eval(late_call(
                "Inc",
                ProjectMemberKind::Method,
                vec![CoreArg::ByRef(CorePlace::Local(LocalId(0)))],
                local_load(1),
            )),
        ],
    };
    let inc = CoreProc {
        name: "Inc".into(),
        kind: ProcedureKind::Sub,
        params: vec![param("Me"), param("n")], // Me=0, n=1 (ByRef)
        locals: vec![],
        return_local: None,
        body: vec![assign(
            CorePlace::Local(LocalId(1)),
            bin(CoreBinOp::Add, local_load(1), ci(100)),
            AssignmentIntent::Let,
            AssignmentTargetKind::Variant,
        )],
    };
    let p = program(0, vec![main, inc], vec![class("C", None, None, vec![("Inc", ProjectMemberKind::Method, 1)])], Vec::new());
    assert_eq!(first_local_i32(&p), Some(105));
}

#[test]
fn set_of_nonobject_into_object_target_faults() {
    // Set o = 5 — ValidateAssignment must reject a non-object Set (error 424).
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: vec![],
        locals: vec![local("o")],
        return_local: None,
        body: vec![set_local(0, ci(5))],
    };
    let p = program(0, vec![main], Vec::new(), Vec::new());
    let bundle = linearize(&p).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    assert!(oxvba_vm2::run(&bundle, &host).is_err(), "Set of a non-object must fault");
}

#[test]
fn withevents_raise_event_reaches_handler() {
    // Snk { WithEvents x As Src (binding 0); Sub x_Fired(arg): g0 = arg }
    // Src { Event Fired (index 0); Sub DoFire(): RaiseEvent Fired(42) }
    // Main: Set snk = New Snk; Set src = New Src; Set snk.x = src; src.DoFire()  → g0 = 42
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: vec![],
        locals: vec![local("snk"), local("src")],
        return_local: None,
        body: vec![
            set_local(0, CoreValue::New(ClassId(0))),
            set_local(1, CoreValue::New(ClassId(1))),
            assign(
                // A non-trivial binding token (100): with the pre-fix linearize bug
                // it would be passed as *slot* 100, so the stored token would not be
                // 100 and the route lookup would miss — this test would then fail.
                CorePlace::WithEvents { owner: Box::new(local_load(0)), binding: 100 },
                local_load(1),
                AssignmentIntent::Set,
                AssignmentTargetKind::Object,
            ),
            CoreStmt::Eval(late_call("DoFire", ProjectMemberKind::Method, vec![], local_load(1))),
        ],
    };
    let handler = CoreProc {
        name: "x_Fired".into(),
        kind: ProcedureKind::Sub,
        params: vec![param("Me"), param("arg")], // Me=0, arg=1
        locals: vec![],
        return_local: None,
        body: vec![assign(
            CorePlace::Global(GlobalId(0)),
            local_load(1),
            AssignmentIntent::Let,
            AssignmentTargetKind::Variant,
        )],
    };
    let do_fire = CoreProc {
        name: "DoFire".into(),
        kind: ProcedureKind::Sub,
        params: vec![param("Me")],
        locals: vec![],
        return_local: None,
        body: vec![CoreStmt::RaiseEvent { source: me(), event: 0, args: vec![CoreArg::ByVal(ci(42))] }],
    };
    let p = program(
        1, // global 0 = the flag the handler writes
        vec![main, handler, do_fire],
        vec![
            class("Snk", None, None, Vec::new()),
            class("Src", None, None, vec![("DoFire", ProjectMemberKind::Method, 2)]),
        ],
        vec![EventRoute { binding: 100, event: 0, handler: 1 }],
    );
    assert_eq!(global0_i32(&p), Some(42), "the event handler ran with the event arg");
}
