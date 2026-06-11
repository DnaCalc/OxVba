//! Smoke tests over hand-authored bundles, exercising the execution model:
//! arithmetic, the frame-based calling convention (true ByRef aliasing,
//! function return, recursion), module globals, a library built-in, a counted
//! loop, and the `On Error` paths including statement-granular `Resume Next`.
//!
//! With `global_count = 0`, every slot operand is a current-frame local, so a
//! procedure's slot `n` and the caller's slot `n` are distinct storage.

use oxvba_bundle::{
    Bundle, BundleImport, ClassDescriptor, ClassMethod, ComMemberSelector, DeclareParamType,
    EventRoute, ExportToken, ExternalCallDescriptor, NativeCallee, NativeImplId, NumericMode, Op,
    ProcArg, ProcedureDescriptor, ProcedureKind, ProjectMemberKind, StringCompareMode,
    isa::CallArg,
};
use oxvba_com::{
    ComCallbackPayload, ComCallbackToken, ComCallbackValue, ComMemberToken, ComObjectDescriptor,
    ComSubscriptionToken, ComValue,
};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_hal::error::HalResult;
use oxvba_hal::model::{HalDescriptor, HalProfileId};
use oxvba_hal::traits::{
    ComHal, ConsoleHal, DiagnosticsHal, DynLinkDescriptorView, DynamicLinkHal, EventPumpHal,
    FileSystemHal, HostServices, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope,
    TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity, UiInteractionHal,
};
use oxvba_runtime::object_ref::ObjectRef;
use oxvba_runtime::{DynLinkSymbol, Variant};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::run;

/// A host that delivers a single COM event callback (for `subscription` 7,
/// arg = `arg`) once a subscription exists, delegating everything else to a
/// `NullHostServices`.
struct MockEventHost {
    inner: NullHostServices,
    subscribed: AtomicBool,
    delivered: AtomicBool,
    arg: i32,
}

impl MockEventHost {
    fn new(arg: i32) -> Self {
        Self {
            inner: NullHostServices::new(HostPolicy::deterministic_runtime()),
            subscribed: AtomicBool::new(false),
            delivered: AtomicBool::new(false),
            arg,
        }
    }
}

impl HostServices for MockEventHost {
    fn profile(&self) -> HalProfileId {
        self.inner.profile()
    }
    fn descriptor(&self) -> HalDescriptor {
        self.inner.descriptor()
    }
    fn policy(&self) -> &HostPolicy {
        self.inner.policy()
    }
    fn console(&self) -> &dyn ConsoleHal {
        self.inner.console()
    }
    fn ui(&self) -> &dyn UiInteractionHal {
        self.inner.ui()
    }
    fn events(&self) -> &dyn EventPumpHal {
        self.inner.events()
    }
    fn fs(&self) -> &dyn FileSystemHal {
        self.inner.fs()
    }
    fn process(&self) -> &dyn ProcessEnvHal {
        self.inner.process()
    }
    fn com(&self) -> &dyn ComHal {
        self
    }
    fn time_locale(&self) -> &dyn TimeLocaleHal {
        self.inner.time_locale()
    }
    fn dynlink(&self) -> &dyn DynamicLinkHal {
        self.inner.dynlink()
    }
    fn diag(&self) -> &dyn DiagnosticsHal {
        self.inner.diag()
    }
}

impl ComHal for MockEventHost {
    fn subscribe_event(
        &self,
        _object: ObjectRef,
        event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken> {
        self.subscribed.store(true, Ordering::SeqCst);
        Ok(ComSubscriptionToken::new(event.raw()))
    }
    fn unsubscribe_event_variant(&self, _s: ComSubscriptionToken) -> HalResult<Variant> {
        Ok(Variant::empty())
    }
    fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>> {
        if self.subscribed.load(Ordering::SeqCst) && !self.delivered.swap(true, Ordering::SeqCst) {
            Ok(Some(ComCallbackPayload {
                callback: ComCallbackToken::new(7),
                subscription: ComSubscriptionToken::new(7),
                object: ObjectRef::from_compat_identity(99),
                event: ComMemberToken::new(7),
                // The payload is self-contained: the event pump reads its args
                // directly (the per-arg `event_callback_*` accessors below are the
                // retired path), so carry the delivered arg here.
                args: vec![ComCallbackValue::from_com_value(ComValue::I32(self.arg))],
            }))
        } else {
            Ok(None)
        }
    }
    fn event_callback_arity(&self, _c: ComCallbackToken) -> HalResult<usize> {
        Ok(1)
    }
    fn event_callback_variant(&self, _c: ComCallbackToken, _i: usize) -> HalResult<Variant> {
        Ok(Variant::from_i32(self.arg))
    }
    fn release_event_callback_variant(&self, _c: ComCallbackToken) -> HalResult<Variant> {
        Ok(Variant::empty())
    }
    fn describe_object(&self, object: ObjectRef) -> HalResult<Option<ComObjectDescriptor>> {
        self.inner.describe_object(object)
    }
    fn event_callback_subscription(
        &self,
        callback: ComCallbackToken,
    ) -> HalResult<ComSubscriptionToken> {
        self.inner.event_callback_subscription(callback)
    }
    fn resolve_typelib_reference(
        &self,
        request: &TypeLibResolveRequest,
    ) -> HalResult<TypeLibResolvedIdentity> {
        self.inner.resolve_typelib_reference(request)
    }
    fn load_typelib_metadata(
        &self,
        identity: &TypeLibResolvedIdentity,
    ) -> HalResult<TypeLibMetadataBlob> {
        self.inner.load_typelib_metadata(identity)
    }
    fn invalidate_typelib_cache(
        &self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> HalResult<Variant> {
        self.inner.invalidate_typelib_cache(scope, reference_name)
    }
}

/// A host whose dynlink echoes each argument incremented by 100 (a fake native
/// "Bump"), to exercise per-call-site `Declare` ByRef write-back.
struct MockDynlinkHost {
    inner: NullHostServices,
}
impl MockDynlinkHost {
    fn new() -> Self {
        Self {
            inner: NullHostServices::new(HostPolicy::deterministic_runtime()),
        }
    }
}
impl HostServices for MockDynlinkHost {
    fn profile(&self) -> HalProfileId {
        self.inner.profile()
    }
    fn descriptor(&self) -> HalDescriptor {
        self.inner.descriptor()
    }
    fn policy(&self) -> &HostPolicy {
        self.inner.policy()
    }
    fn console(&self) -> &dyn ConsoleHal {
        self.inner.console()
    }
    fn ui(&self) -> &dyn UiInteractionHal {
        self.inner.ui()
    }
    fn events(&self) -> &dyn EventPumpHal {
        self.inner.events()
    }
    fn fs(&self) -> &dyn FileSystemHal {
        self.inner.fs()
    }
    fn process(&self) -> &dyn ProcessEnvHal {
        self.inner.process()
    }
    fn com(&self) -> &dyn ComHal {
        self.inner.com()
    }
    fn time_locale(&self) -> &dyn TimeLocaleHal {
        self.inner.time_locale()
    }
    fn dynlink(&self) -> &dyn DynamicLinkHal {
        self
    }
    fn diag(&self) -> &dyn DiagnosticsHal {
        self.inner.diag()
    }
}
impl DynamicLinkHal for MockDynlinkHost {
    fn invoke_descriptor_variants(
        &self,
        _descriptor: &DynLinkDescriptorView<'_>,
        args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        let wb = args
            .iter()
            .map(|a| {
                a.as_i32()
                    .map(|n| Variant::from_i32(n + 100))
                    .unwrap_or_else(|| a.clone())
            })
            .collect();
        Ok((Variant::empty(), wb))
    }
}

#[test]
fn declare_byref_writes_back_to_call_site_slot() {
    // `Declare Sub Bump Lib "t" (ByRef n As Long)` adds 100; r=5; `Bump r` → r=105.
    // Proves the write-back targets the *call-site* CallArg::ByRef slot.
    let mut b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 5 },
            Op::CallNative {
                dst: None,
                callee: NativeCallee::Declare {
                    descriptor_id: 0,
                    ptr_writebacks: Vec::new(),
                },
                args: vec![CallArg::ByRef(0)],
            },
            Op::Halt,
        ],
        1, // entry locals: r (slot 0)
        vec![proc("Main", 0, 0, 1, None)],
    );
    b.external_calls = vec![ExternalCallDescriptor {
        descriptor_id: 0,
        declared_name: "Bump".into(),
        library: "t".into(),
        alias: "Bump".into(),
        ordinal_alias: false,
        symbol: DynLinkSymbol::new(0),
        marshal_lane: "m0-deterministic".into(),
        calling_convention: "platform-default".into(),
        selection_policy: "case-insensitive-canonical".into(),
        param_count: 1,
        param_types: vec![DeclareParamType::Long],
        param_by_ref: vec![true],
        return_type: None,
    }];
    let host = MockDynlinkHost::new();
    let vm = run(&b, &host).unwrap();
    assert_eq!(
        vm.slot(0).unwrap().as_i32(),
        Some(105),
        "Declare ByRef wrote back to the caller slot"
    );
}

fn host() -> NullHostServices {
    NullHostServices::new(HostPolicy::deterministic_runtime())
}

fn bundle_full(
    ops: Vec<Op>,
    global_count: usize,
    entry_frame_slots: usize,
    statement_starts: Vec<usize>,
    procedures: Vec<ProcedureDescriptor>,
    classes: Vec<ClassDescriptor>,
) -> Bundle {
    Bundle {
        ops,
        procedures,
        entry_pc: 0,
        global_count,
        entry_frame_slots,
        statement_starts,
        external_calls: Vec::new(),
        source_map: Vec::new(),
        com_class_exports: Vec::new(),
        classes,
        event_routes: Vec::new(),
        unit_name: String::new(),
        exports: Vec::new(),
        imports: Vec::new(),
    }
}

fn bundle(ops: Vec<Op>, entry_frame_slots: usize, procedures: Vec<ProcedureDescriptor>) -> Bundle {
    bundle_full(
        ops,
        0,
        entry_frame_slots,
        Vec::new(),
        procedures,
        Vec::new(),
    )
}

fn func(
    name: &str,
    entry_pc: usize,
    frame_slots: usize,
    return_slot: Option<usize>,
) -> ProcedureDescriptor {
    proc(name, entry_pc, 1, frame_slots, return_slot)
}

fn proc(
    name: &str,
    entry_pc: usize,
    param_count: usize,
    frame_slots: usize,
    return_slot: Option<usize>,
) -> ProcedureDescriptor {
    ProcedureDescriptor {
        name: name.to_string(),
        entry_pc,
        kind: if return_slot.is_some() {
            ProcedureKind::Function
        } else {
            ProcedureKind::Sub
        },
        param_count,
        frame_slots,
        return_slot,
        native: None,
    }
}

fn class(name: &str, initialize: Option<usize>, terminate: Option<usize>) -> ClassDescriptor {
    ClassDescriptor {
        name: name.to_string(),
        initialize,
        terminate,
        methods: Vec::new(),
        implements: Vec::new(),
    }
}

#[test]
fn arithmetic() {
    // (10 + 5) * 2 = 30
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 10 },
            Op::LoadI32 { slot: 1, value: 5 },
            Op::Add {
                dst: 2,
                lhs: 0,
                rhs: 1,
                mode: NumericMode::Widening,
            },
            Op::LoadI32 { slot: 3, value: 2 },
            Op::Mul {
                dst: 4,
                lhs: 2,
                rhs: 3,
                mode: NumericMode::Widening,
            },
            Op::Halt,
        ],
        5,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    // Integer arithmetic stays integer: `(10+5)*2` is `Long(30)`, not Double.
    assert_eq!(vm.slot(4).unwrap().as_i32(), Some(30));
}

#[test]
fn builtin_len() {
    let b = bundle(
        vec![
            Op::LoadString {
                slot: 0,
                value: "hello".to_string(),
            },
            Op::CallNative {
                dst: Some(1),
                callee: NativeCallee::Builtin(NativeImplId::Len),
                args: vec![CallArg::Slot(0)],
            },
            Op::Halt,
        ],
        2,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(5));
}

#[test]
fn proc_byref_aliasing() {
    // Sub Inc(ByRef n): n = n + 1.  Caller passes local 0 (= 41) ByRef → 42.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 41 }, // 0
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![ProcArg::ByRef(0)],
            }, // 1
            Op::Halt,                           // 2
            Op::IncSlot { slot: 0 },            // 3 (Inc entry; local 0 = aliased param)
            Op::Return,                         // 4
        ],
        1,
        vec![func("Inc", 3, 1, None)],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(42));
}

#[test]
fn call_proc_ref_dispatches_through_address_of() {
    // A procedure reference (AddressOf, materialized by LoadProcRef) is called
    // through CallProcRef: Double(21) = 42 via a runtime-resolved proc index.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 21 },  // 0
            Op::LoadProcRef { dst: 1, proc: 0 }, // 1
            Op::CallProcRef {
                dst: Some(2),
                target: 1,
                args: vec![ProcArg::ByVal(0)],
            }, // 2
            Op::Halt,                            // 3
            Op::Add {
                dst: 1,
                lhs: 0,
                rhs: 0,
                mode: NumericMode::Widening,
            }, // 4 (Double entry; local 1 = return)
            Op::Return,                          // 5
        ],
        3,
        vec![func("Double", 4, 2, Some(1))],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(2).unwrap().as_i32(), Some(42));
}

#[test]
fn proc_function_return() {
    // Function Double(ByVal n) = n + n.  Double(21) → 42 into caller local 1.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 21 }, // 0
            Op::CallProc {
                proc: 0,
                dst: Some(1),
                args: vec![ProcArg::ByVal(0)],
            }, // 1
            Op::Halt,                           // 2
            Op::Add {
                dst: 1,
                lhs: 0,
                rhs: 0,
                mode: NumericMode::Widening,
            }, // 3 (Double entry; local 1 = return)
            Op::Return,                         // 4
        ],
        2,
        vec![func("Double", 3, 2, Some(1))],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(42));
}

#[test]
fn recursion_factorial() {
    // Function Fact(n) = If n <= 1 Then 1 Else n * Fact(n - 1).  Fact(5) = 120.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 5 }, // 0
            Op::CallProc {
                proc: 0,
                dst: Some(1),
                args: vec![ProcArg::ByVal(0)],
            }, // 1
            Op::Halt,                          // 2
            // Fact entry (pc 3): locals 0=n, 1=const1, 2=cond, 3=n-1, 4=Fact(n-1), 5=result
            Op::LoadI32 { slot: 1, value: 1 }, // 3
            Op::CmpLe {
                dst: 2,
                lhs: 0,
                rhs: 1,
                mode: StringCompareMode::Binary,
            }, // 4
            Op::JumpIfZero {
                cond_slot: 2,
                target_pc: 8,
            }, // 5 -> recurse
            Op::LoadI32 { slot: 5, value: 1 }, // 6 base: result = 1
            Op::Return,                        // 7
            Op::Copy { dst: 3, src: 0 },       // 8 n-1 = n
            Op::SubConstI32 { slot: 3, value: 1 }, // 9 n-1
            Op::CallProc {
                proc: 0,
                dst: Some(4),
                args: vec![ProcArg::ByVal(3)],
            }, // 10 Fact(n-1)
            Op::Mul {
                dst: 5,
                lhs: 0,
                rhs: 4,
                mode: NumericMode::Widening,
            }, // 11 result = n * Fact(n-1)
            Op::Return,                        // 12
        ],
        2,
        vec![func("Fact", 3, 6, Some(5))],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    // `5! = 120` via integer `Mul` is `Long(120)`, not Double.
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(120));
}

#[test]
fn global_persists_across_calls() {
    // global 0 (slot 0, since global_count = 1); Sub Bump increments it; call twice → 2.
    let b = bundle_full(
        vec![
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![],
            }, // 0
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![],
            }, // 1
            Op::Halt,                // 2
            Op::IncSlot { slot: 0 }, // 3 (Bump entry; slot 0 = global)
            Op::Return,              // 4
        ],
        1, // global_count
        0, // entry_frame_slots
        Vec::new(),
        vec![ProcedureDescriptor {
            name: "Bump".to_string(),
            entry_pc: 3,
            kind: ProcedureKind::Sub,
            param_count: 0,
            frame_slots: 0,
            return_slot: None,
            native: None,
        }],
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_f64(), Some(2.0));
}

#[test]
fn counted_loop_sum() {
    // acc = 0; For i = 1 To 5: acc = acc + i.  Sum = 15.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 0 }, // 0 acc
            Op::LoadI32 { slot: 1, value: 1 }, // 1 i
            Op::LoadI32 { slot: 2, value: 6 }, // 2 limit
            Op::CmpLt {
                dst: 3,
                lhs: 1,
                rhs: 2,
                mode: StringCompareMode::Binary,
            }, // 3 i<6
            Op::JumpIfZero {
                cond_slot: 3,
                target_pc: 8,
            }, // 4 exit
            Op::Add {
                dst: 0,
                lhs: 0,
                rhs: 1,
                mode: NumericMode::Widening,
            }, // 5 acc+=i
            Op::IncSlot { slot: 1 },           // 6 i++
            Op::Jump { target_pc: 3 },         // 7 loop
            Op::Halt,                          // 8
        ],
        4,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(15));
}

#[test]
fn on_error_resume_next() {
    let b = bundle(
        vec![
            Op::SetOnErrorResumeNext,           // 0
            Op::RaiseError { code: 11 },        // 1 (resume → 2)
            Op::LoadI32 { slot: 0, value: 99 }, // 2
            Op::Halt,                           // 3
        ],
        1,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(99));
}

#[test]
fn resume_next_is_statement_granular() {
    // Statement at pc 1..3 errors mid-way; Resume Next skips the *rest of the
    // statement* (pc 3) and continues at the next statement (pc 4).
    let b = bundle_full(
        vec![
            Op::SetOnErrorResumeNext,           // 0
            Op::LoadI32 { slot: 0, value: 1 },  // 1 (statement start)
            Op::RaiseError { code: 5 },         // 2 (errors → next statement)
            Op::LoadI32 { slot: 0, value: 2 },  // 3 (same statement — must be skipped)
            Op::LoadI32 { slot: 1, value: 99 }, // 4 (next statement)
            Op::Halt,                           // 5
        ],
        0,
        2,
        vec![0, 1, 4, 5],
        Vec::new(),
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(
        vm.slot(0).unwrap().as_i32(),
        Some(1),
        "pc 3 must be skipped"
    );
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(99));
}

#[test]
fn on_error_goto_handler() {
    let b = bundle(
        vec![
            Op::SetOnErrorGotoLabel { target_pc: 4 }, // 0
            Op::RaiseError { code: 11 },              // 1 → handler
            Op::LoadI32 { slot: 0, value: 1 },        // 2 (skipped)
            Op::Halt,                                 // 3
            Op::LoadI32 { slot: 0, value: 7 },        // 4 handler
            Op::Halt,                                 // 5
        ],
        1,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(7));
}

#[test]
fn uncaught_error_surfaces() {
    let b = bundle(vec![Op::RaiseError { code: 6 }, Op::Halt], 1, Vec::new());
    let h = host();
    assert!(matches!(run(&b, &h), Err(e) if e.code == 6));
}

// ── Object lifecycle (New / Initialize / fields / refcount / Terminate) ───────

#[test]
fn new_runs_initialize_and_fields() {
    // obj = New D ; x = obj.f10 ; where Class_Initialize sets f10 = 42.
    let b = bundle(
        vec![
            Op::NewObject { dst: 0, class: 0 }, // 0 obj = New D
            Op::FieldGet {
                dst: 1,
                object: 0,
                field: 10,
            }, // 1 x = obj.f10
            Op::Halt,                           // 2
            Op::LoadI32 { slot: 1, value: 42 }, // 3 Initialize: temp = 42 (Me = local 0)
            Op::FieldSet {
                object: 0,
                field: 10,
                src: 1,
            }, // 4 Me.f10 = 42
            Op::Return,                         // 5
        ],
        2,
        vec![proc("Init", 3, 1, 2, None)],
    );
    let b = with_classes(b, vec![class("D", Some(0), None)]);
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(42));
}

#[test]
fn terminate_runs_on_scope_exit() {
    // Sub Make: Dim o As New C (local) ; on End Sub the local releases → Terminate
    // sets global 0 = 7.
    let b = object_program(
        vec![
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![],
            }, // 0
            Op::Halt,                           // 1
            Op::NewObject { dst: 1, class: 0 }, // 2 Make: local 0 (slot 1) = New C
            Op::Return,                         // 3
            Op::LoadI32 { slot: 0, value: 7 },  // 4 Terminate: global 0 = 7
            Op::Return,                         // 5
        ],
        vec![proc("Make", 2, 0, 1, None), proc("Term", 4, 1, 1, None)],
        Some(1),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(7));
}

#[test]
fn two_holders_terminate_exactly_once() {
    // a = New C ; b = a (two references) ; on scope exit Terminate fires ONCE.
    let b = object_program(
        vec![
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![],
            }, // 0
            Op::Halt,                           // 1
            Op::NewObject { dst: 1, class: 0 }, // 2 a = New C
            Op::Copy { dst: 2, src: 1 },        // 3 b = a (refcount 2)
            Op::Return,                         // 4
            Op::IncSlot { slot: 0 },            // 5 Terminate: global 0 += 1
            Op::Return,                         // 6
        ],
        vec![proc("MakeTwo", 2, 0, 3, None), proc("Term", 5, 1, 1, None)],
        Some(1),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    // `Empty + 1` promotes to Double in VBA, so the counter reads as f64.
    assert_eq!(
        vm.slot(0).unwrap().as_f64(),
        Some(1.0),
        "exactly one Class_Terminate"
    );
}

#[test]
fn terminate_runs_during_error_unwind() {
    // Bad creates a local, then errors; the error unwinds Bad (releasing the
    // local) and is caught at the top by Resume Next — Terminate must still run.
    let b = object_program(
        vec![
            Op::SetOnErrorResumeNext, // 0
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![],
            }, // 1
            Op::Halt,                 // 2
            Op::NewObject { dst: 1, class: 0 }, // 3 Bad: a = New C
            Op::RaiseError { code: 5 }, // 4 uncaught in Bad → unwind
            Op::Return,               // 5
            Op::LoadI32 { slot: 0, value: 7 }, // 6 Terminate: global 0 = 7
            Op::Return,               // 7
        ],
        vec![proc("Bad", 3, 0, 1, None), proc("Term", 6, 1, 1, None)],
        Some(1),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(
        vm.slot(0).unwrap().as_i32(),
        Some(7),
        "Terminate runs on error unwind"
    );
}

#[test]
fn error_in_terminate_is_suppressed() {
    // Class_Terminate sets global 0 = 7 then raises — the error must be swallowed
    // (run succeeds, the side effect is visible).
    let b = object_program(
        vec![
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![],
            }, // 0
            Op::Halt,                           // 1
            Op::NewObject { dst: 1, class: 0 }, // 2 Make
            Op::Return,                         // 3
            Op::LoadI32 { slot: 0, value: 7 },  // 4 Terminate: global 0 = 7
            Op::RaiseError { code: 5 },         // 5 uncaught in Terminate → suppressed
            Op::Return,                         // 6
        ],
        vec![proc("Make", 2, 0, 1, None), proc("Term", 4, 1, 1, None)],
        Some(1),
    );
    let h = host();
    let vm = run(&b, &h).expect("error in Terminate must not propagate");
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(7));
}

#[test]
fn reference_cycle_leaks_without_terminate() {
    // a.f = b ; b.f = a ; on scope exit each is still referenced by the other →
    // neither reaches refcount 0 → Class_Terminate never runs (VBA-consistent).
    let b = object_program(
        vec![
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![],
            }, // 0
            Op::Halt,                           // 1
            Op::NewObject { dst: 1, class: 0 }, // 2 a
            Op::NewObject { dst: 2, class: 0 }, // 3 b
            Op::FieldSet {
                object: 1,
                field: 10,
                src: 2,
            }, // 4 a.f = b
            Op::FieldSet {
                object: 2,
                field: 10,
                src: 1,
            }, // 5 b.f = a
            Op::Return,                         // 6
            Op::IncSlot { slot: 0 },            // 7 Terminate: global 0 += 1
            Op::Return,                         // 8
        ],
        vec![
            proc("MakeCycle", 2, 0, 3, None),
            proc("Term", 7, 1, 1, None),
        ],
        Some(1),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    // Terminate never ran, so the counter is untouched (Empty ⇒ 0).
    assert_eq!(
        vm.slot(0).unwrap().as_f64().unwrap_or(0.0),
        0.0,
        "cycle leaks; no Terminate"
    );
}

#[test]
fn array_element_release_runs_terminate() {
    // arr = Array(a) AddRefs a ; clearing both the local and the array drops the
    // last reference → Terminate fires (proves array elements are refcounted).
    let b = object_program(
        vec![
            Op::CallProc {
                proc: 0,
                dst: None,
                args: vec![],
            }, // 0
            Op::Halt,                           // 1
            Op::NewObject { dst: 1, class: 0 }, // 2 a = New C (refcount 1)
            Op::ArrayLiteral {
                dst: 2,
                values: vec![1],
            }, // 3 arr = Array(a) (refcount 2)
            Op::LoadEmpty { slot: 1 },          // 4 a = Nothing (refcount 1)
            Op::LoadEmpty { slot: 2 },          // 5 arr = Nothing → element released (0)
            Op::Return,                         // 6
            Op::IncSlot { slot: 0 },            // 7 Terminate: global 0 += 1
            Op::Return,                         // 8
        ],
        vec![proc("MakeArr", 2, 0, 3, None), proc("Term", 7, 1, 1, None)],
        Some(1),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(
        vm.slot(0).unwrap().as_f64(),
        Some(1.0),
        "array element release runs Terminate"
    );
}

/// Helper: a program with one class `C` whose `Class_Terminate` is the proc at
/// `terminate` (and no `Class_Initialize`), `global_count = 1`, no entry locals.
fn object_program(
    ops: Vec<Op>,
    procedures: Vec<ProcedureDescriptor>,
    terminate: Option<usize>,
) -> Bundle {
    bundle_full(
        ops,
        1,
        0,
        Vec::new(),
        procedures,
        vec![class("C", None, terminate)],
    )
}

fn with_classes(mut b: Bundle, classes: Vec<ClassDescriptor>) -> Bundle {
    b.classes = classes;
    b
}

// ── Events and late-bound dispatch ────────────────────────────────────────────

#[test]
fn raise_event_dispatches_to_withevents_handler() {
    // snk has `WithEvents x As Src` (binding token 100) and a handler routed for
    // event 7. Subscribe snk.x = src, then RaiseEvent 7 on src with arg 42 — the
    // handler runs with the sink's `Me` and sets global 0 = 42.
    let mut b = bundle_full(
        vec![
            Op::NewObject { dst: 1, class: 0 }, // 0 snk = New Snk (local 0)
            Op::NewObject { dst: 2, class: 1 }, // 1 src = New Src (local 1)
            Op::LoadI32 {
                slot: 3,
                value: 100,
            }, // 2 binding token (local 2)
            Op::WithEventsSet {
                dst: 4,
                owner: 1,
                binding: 3,
                value: 2,
            }, // 3 snk.x = src
            Op::LoadI32 { slot: 5, value: 42 }, // 4 event arg (local 4)
            Op::RaiseEvent {
                source: 2,
                event: 7,
                args: vec![ProcArg::ByVal(5)],
            }, // 5
            Op::Halt,                           // 6
            Op::Copy { dst: 0, src: 2 },        // 7 Handler: global 0 = arg (local 1 = slot 2)
            Op::Return,                         // 8
        ],
        1, // global_count (global 0 = flag)
        6, // entry locals: snk, src, btok, tmp, arg
        Vec::new(),
        vec![proc("x_Fired", 7, 2, 2, None)], // handler(Me, arg)
        vec![class("Snk", None, None), class("Src", None, None)],
    );
    b.event_routes = vec![EventRoute {
        binding: 100,
        event: 7,
        handler: 0,
    }];
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(
        vm.slot(0).unwrap().as_i32(),
        Some(42),
        "handler ran with the event arg"
    );
}

#[test]
fn late_bound_method_dispatch() {
    // r = obj.Inc(41) where the receiver is dispatched late by name → 42.
    let mut b = bundle(
        vec![
            Op::NewObject { dst: 0, class: 0 }, // 0 obj = New C
            Op::LoadI32 { slot: 2, value: 41 }, // 1 arg = 41
            Op::CallNative {
                dst: Some(1),
                callee: NativeCallee::ComDispatch {
                    selector: ComMemberSelector::Name("Inc".to_string()),
                    early_bound: false,
                    kind_hint: Some(ProjectMemberKind::Method),
                },
                args: vec![CallArg::Slot(0), CallArg::Slot(2)], // receiver, n
            }, // 2 r = obj.Inc(41)
            Op::Halt,                           // 3
            Op::Copy { dst: 2, src: 1 },        // 4 Inc: result(local2) = n(local1)
            Op::AddConstI32 { slot: 2, value: 1 }, // 5 result += 1
            Op::Return,                         // 6
        ],
        3,                                   // entry locals: obj, r, arg
        vec![proc("Inc", 4, 2, 3, Some(2))], // Inc(Me, n) -> result
    );
    b.classes = vec![ClassDescriptor {
        name: "C".to_string(),
        initialize: None,
        terminate: None,
        methods: vec![ClassMethod {
            name: "Inc".to_string(),
            kind: ProjectMemberKind::Method,
            proc: 0,
        }],
        implements: Vec::new(),
    }];
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(42));
}

#[test]
fn vba_collection_new_extern_dispatch() {
    // An app bundle imports `VBA.Collection`, `New`s one, adds two items, then
    // reads Count and Item(2). Exercises the synthetic `VBA` bundle that
    // `Vm::link` appends, the cross-bundle `NewExtern` mint, and native-bodied
    // (`NativeMethodId`) method dispatch through the ordinary class machinery.
    let add = |arg: usize| Op::CallNative {
        dst: None,
        callee: NativeCallee::ComDispatch {
            selector: ComMemberSelector::Name("Add".to_string()),
            early_bound: false,
            kind_hint: Some(ProjectMemberKind::Method),
        },
        args: vec![CallArg::Slot(0), CallArg::Slot(arg)],
    };
    let mut b = bundle(
        vec![
            Op::NewExtern { dst: 0, import: 0 }, // 0  c = New Collection
            Op::LoadI32 { slot: 1, value: 10 },  // 1
            add(1),                              // 2  c.Add 10
            Op::LoadI32 { slot: 1, value: 20 },  // 3
            add(1),                              // 4  c.Add 20
            Op::CallNative {
                dst: Some(2),
                callee: NativeCallee::ComDispatch {
                    selector: ComMemberSelector::Name("Count".to_string()),
                    early_bound: false,
                    kind_hint: Some(ProjectMemberKind::PropertyGet),
                },
                args: vec![CallArg::Slot(0)],
            }, // 5  cnt = c.Count
            Op::LoadI32 { slot: 3, value: 2 },   // 6
            Op::CallNative {
                dst: Some(4),
                callee: NativeCallee::ComDispatch {
                    selector: ComMemberSelector::Name("Item".to_string()),
                    early_bound: false,
                    kind_hint: Some(ProjectMemberKind::Method),
                },
                args: vec![CallArg::Slot(0), CallArg::Slot(3)],
            }, // 7  item = c.Item(2)
            Op::Halt,                            // 8
        ],
        5, // entry locals: c, arg, cnt, idx, item
        vec![],
    );
    b.imports = vec![BundleImport {
        unit: "VBA".to_string(),
        token: ExportToken::Class {
            name: "Collection".to_string(),
        },
    }];
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(
        vm.slot(2).unwrap().as_i32(),
        Some(2),
        "Count after two Adds"
    );
    assert_eq!(vm.slot(4).unwrap().as_i32(), Some(20), "Item(2)");
}

#[test]
fn inbound_com_event_dispatches_to_handler() {
    // src is a (non-project) COM-like object; snk subscribes to it via the host.
    // The mock host delivers one callback for that subscription with arg 42; the
    // pump (run at statement boundaries) routes it to snk's handler → global 0 = 42.
    let mut b = bundle_full(
        vec![
            Op::LoadProjectObjectRef { dst: 1, handle: 99 }, // 0 src (a COM-like object, not a project instance)
            Op::LoadI32 { slot: 2, value: 7 },               // 1 binding token = 7
            Op::NewObject { dst: 3, class: 0 },              // 2 snk = New Snk
            Op::WithEventsSet {
                dst: 4,
                owner: 3,
                binding: 2,
                value: 1,
            }, // 3 subscribe snk.x = src
            Op::Halt,                                        // 4 (pump fires at this boundary)
            Op::Copy { dst: 0, src: 2 },                     // 5 Handler: global 0 = arg (local 1)
            Op::Return,                                      // 6
        ],
        1,                   // global_count
        4,                   // entry locals: src, btok, snk, tmp
        vec![0, 1, 2, 3, 4], // statement boundaries (drive the pump)
        vec![proc("x_Fired", 5, 2, 2, None)],
        vec![class("Snk", None, None)],
    );
    b.event_routes = vec![EventRoute {
        binding: 7,
        event: 7,
        handler: 0,
    }];
    let host = MockEventHost::new(42);
    let vm = run(&b, &host).unwrap();
    assert_eq!(
        vm.slot(0).unwrap().as_i32(),
        Some(42),
        "inbound COM event reached the handler"
    );
}
