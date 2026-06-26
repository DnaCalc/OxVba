//! `oxvba-vm3` — the typed-CFG interpreter of OxIR.
//!
//! vm3 is a fresh **executor core** (typed register file + place model,
//! block-threaded dispatch, frame/linkage + ByRef-aliasing, error/Resume routing,
//! object-lifecycle / Terminate-drain timing, RaiseEvent/WithEvents, COM-event pump)
//! re-expressed against OxIR's typed basic-block CFG. It does **not** re-implement
//! VBA: it reuses the value/interop/lib substrate (`oxvba-runtime`, `oxvba-lib`,
//! `oxvba-hal`, `oxvba-com`/`oxvba-comhost`) and the shared `oxvba-eval` semantic
//! kernel — refactoring upstream where that improves the whole.
//!
//! vm3 is OxIR's **executable specification**: its observable behaviour defines what
//! OxIR means, and the Cranelift JIT must match it. During the transition, the
//! legacy `oxvba-vm2` (`Op` bundle) remains the **golden oracle** until vm3 reaches
//! full-corpus parity (the "oracle handoff"), after which vm2 is frozen.
//!
//! # Status (M2 bring-up)
//!
//! This first cut runs the **scalar / string / Boolean value core + control flow**:
//! `Assign`, all arithmetic (`Arith`/`Div`/`Pow`/`Neg`), `Concat`, `Compare`,
//! `Logical`/`Not`, `Coerce`, and the `Jump`/`Branch`/`Return` terminators, plus the
//! statement-boundary marker. The value semantics go through the shared
//! [`oxvba_eval::arith`] kernel — the *same* functions vm2 calls — so a successful
//! run is vm2-identical by construction.
//!
//! The frame holds its values as `Variant`s (the shareable slot layout the JIT
//! side-exits into); the **typed unboxed lanes** + per-site type profiler are the M6
//! speculation tier, an addition over this layout rather than a retrofit. Procedure
//! calls, builtins, the full error/`Resume` model, objects, COM, arrays, and records
//! return [`Vm3Error::Unimplemented`] for now (M2-b/M2-c/M3) — never a silent
//! mis-execution.

use std::collections::HashMap;

use oxvba_eval::arith::{self, ArithError};
use oxvba_hal::HostServices;
use oxvba_lib::LibContext;
use oxvba_oxir::value::{ArithOp, CmpOp, LogicalOp, OxCoerceTarget, OxConst, OxOperand, OxPlace};
use oxvba_oxir::{FuncId, OxBlock, OxInst, OxProgram, OxTerminator};
use oxvba_runtime::Variant;

/// A run-time fault carrying its VBA error code (the value `Err.Number` takes).
#[derive(Debug, Clone)]
pub struct Fault {
    pub code: i32,
    pub message: String,
}

impl Fault {
    fn from_arith(e: ArithError) -> Self {
        Self {
            code: e.code,
            message: e.message,
        }
    }
}

/// A failure to execute an OxIR program on vm3.
#[derive(Debug, Clone)]
pub enum Vm3Error {
    /// An uncaught VBA run-time fault propagated to the top level.
    Fault(Fault),
    /// An OxIR construct vm3 does not yet execute (honest and explicit — never a
    /// silent mis-execution). Lands in a later milestone (calls/builtins: M2-b;
    /// error/`Resume`: M2-c; objects/COM/arrays: M3).
    Unimplemented { what: &'static str },
    /// A structurally invalid program (should not occur for verifier-clean OxIR).
    Malformed(String),
}

impl std::fmt::Display for Vm3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vm3Error::Fault(fault) => write!(f, "uncaught error {}: {}", fault.code, fault.message),
            Vm3Error::Unimplemented { what } => write!(f, "vm3 does not yet execute: {what}"),
            Vm3Error::Malformed(m) => write!(f, "malformed OxIR: {m}"),
        }
    }
}

impl std::error::Error for Vm3Error {}

/// The active `On Error` handler state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorMode {
    None,
    #[allow(dead_code)] // wired by `SetErrorHandler`; consumed in M2-c.
    ResumeNext,
    #[allow(dead_code)]
    Goto(usize),
}

/// One procedure activation: its value slots. ByRef aliasing + the caller-return
/// continuation are added with `CallProc` (M2-b).
struct Frame {
    /// Frame locals (parameters first, then declared locals), indexed by `LocalId`.
    locals: Vec<Variant>,
    /// Single-assignment temporaries, indexed by `TempId` (sparse — written before read).
    temps: HashMap<usize, Variant>,
}

/// The `Err` object's observable state.
#[derive(Debug, Clone, Default)]
struct ErrState {
    number: i32,
    description: String,
    source: String,
}

/// The vm3 interpreter over a typed OxIR program.
pub struct Vm3<'h> {
    program: &'h OxProgram,
    #[allow(dead_code)] // used by builtins / HAL from M2-b.
    host: &'h dyn HostServices,
    #[allow(dead_code)]
    lib: LibContext,
    globals: Vec<Variant>,
    /// `Main`'s final frame (the entry frame never pops), kept for the result snapshot.
    entry_frame: Option<Frame>,
    error_mode: ErrorMode,
    err: ErrState,
    /// The fault currently being routed (set when a fallible op transfers to a pad).
    pending_fault: Option<Fault>,
}

impl<'h> Vm3<'h> {
    /// Run `program` to completion and return the finished VM (read the result snapshot
    /// with [`Vm3::slot`]). Mirrors vm2: the global initializer runs first, then `Main`
    /// in an entry frame that is never popped.
    pub fn run(program: &'h OxProgram, host: &'h dyn HostServices) -> Result<Self, Vm3Error> {
        let mut vm = Vm3 {
            program,
            host,
            lib: LibContext::default(),
            globals: vec![Variant::empty(); program.globals.len()],
            entry_frame: None,
            error_mode: ErrorMode::None,
            err: ErrState::default(),
            pending_fault: None,
        };

        if let Some(init) = program.global_initializer {
            let frame = vm.new_frame(init);
            // The initializer writes module globals; its own frame is discarded.
            vm.run_frame(init, frame)?;
        }
        if let Some(entry) = program.entry {
            let frame = vm.new_frame(entry);
            let finished = vm.run_frame(entry, frame)?;
            vm.entry_frame = Some(finished);
        }
        Ok(vm)
    }

    /// The result snapshot slot `i`: module globals occupy `[0, globals.len())`; higher
    /// indices are the entry (`Main`) frame's locals (the same layout vm2 exposes).
    pub fn slot(&self, i: usize) -> Option<Variant> {
        let global_count = self.globals.len();
        if i < global_count {
            self.globals.get(i).cloned()
        } else {
            let rel = i - global_count;
            self.entry_frame.as_ref()?.locals.get(rel).cloned()
        }
    }

    /// The final `Err` state (number / description / source) for the error axis.
    pub fn err_number(&self) -> i32 {
        self.err.number
    }
    pub fn err_description(&self) -> &str {
        &self.err.description
    }
    pub fn err_source(&self) -> &str {
        &self.err.source
    }

    fn new_frame(&self, func: FuncId) -> Frame {
        let f = &self.program.funcs[func.0];
        Frame {
            locals: vec![Variant::empty(); f.locals.len()],
            temps: HashMap::new(),
        }
    }

    /// Execute a function's CFG in `frame` until it returns, then hand the frame back.
    fn run_frame(&mut self, func: FuncId, mut frame: Frame) -> Result<Frame, Vm3Error> {
        // `program` is a `'h` reference, independent of the `&mut self` exec borrows.
        let program = self.program;
        let f = &program.funcs[func.0];
        let mut cur = f.entry;
        loop {
            let block: &OxBlock = f
                .blocks
                .get(cur.0)
                .ok_or_else(|| Vm3Error::Malformed(format!("block {} out of range", cur.0)))?;

            // Straight-line body; a fallible op transfers to the block's fault pad.
            let mut faulted = None;
            for inst in &block.instrs {
                if let Err(e) = self.exec(inst, &mut frame) {
                    match e {
                        Vm3Error::Fault(fault) => {
                            faulted = Some(fault);
                            break;
                        }
                        other => return Err(other),
                    }
                }
            }
            if let Some(fault) = faulted {
                let pad = block.fault_target.ok_or_else(|| {
                    Vm3Error::Malformed("fallible instruction in a block with no fault_target".into())
                })?;
                self.raise(fault);
                cur = pad;
                continue;
            }

            match &block.terminator {
                OxTerminator::Jump(b) => cur = *b,
                OxTerminator::Branch {
                    cond,
                    then_blk,
                    else_blk,
                } => {
                    let v = self.operand(cond, &frame);
                    let taken = arith::is_truthy(&v).map_err(|e| Vm3Error::Fault(Fault::from_arith(e)))?;
                    cur = if taken { *then_blk } else { *else_blk };
                }
                OxTerminator::Return | OxTerminator::Halt => return Ok(frame),
                OxTerminator::FaultDispatch { .. } => match self.error_mode {
                    // No active handler ⇒ propagate the fault as an early return.
                    ErrorMode::None => {
                        let fault = self
                            .pending_fault
                            .take()
                            .unwrap_or_else(|| Fault { code: 0, message: String::new() });
                        return Err(Vm3Error::Fault(fault));
                    }
                    // `On Error Resume Next` / `GoTo` routing is M2-c.
                    _ => return Err(Vm3Error::Unimplemented { what: "On Error Resume/GoTo routing" }),
                },
                OxTerminator::Unreachable => {
                    return Err(Vm3Error::Malformed("reached an Unreachable terminator".into()));
                }
                // Resume / Raise / GoSub are the error-model + subroutine work (M2-c).
                _ => return Err(Vm3Error::Unimplemented { what: "terminator (Resume/Raise/GoSub)" }),
            }
        }
    }

    /// Populate `Err` and stash the in-flight fault for the landing pad.
    fn raise(&mut self, fault: Fault) {
        self.err.number = fault.code;
        self.err.description = fault.message.clone();
        self.pending_fault = Some(fault);
    }

    /// Execute one straight-line instruction.
    fn exec(&mut self, inst: &OxInst, frame: &mut Frame) -> Result<(), Vm3Error> {
        match inst {
            OxInst::Assign { dst, value } => {
                let v = self.operand(value, frame);
                self.store(dst, v, frame);
            }
            OxInst::Arith {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => {
                let l = self.operand(lhs, frame);
                let r = self.operand(rhs, frame);
                let out = match op {
                    ArithOp::Add => arith::add(&l, &r, *mode),
                    ArithOp::Sub => arith::sub(&l, &r, *mode),
                    ArithOp::Mul => arith::mul(&l, &r, *mode),
                    ArithOp::IntDiv => arith::int_div(&l, &r, *mode),
                    ArithOp::Mod => arith::modulo(&l, &r, *mode),
                };
                self.store_arith(dst, out, frame)?;
            }
            OxInst::Div { dst, lhs, rhs } => {
                let l = self.operand(lhs, frame);
                let r = self.operand(rhs, frame);
                self.store_arith(dst, arith::div(&l, &r), frame)?;
            }
            OxInst::Pow { dst, lhs, rhs } => {
                let l = self.operand(lhs, frame);
                let r = self.operand(rhs, frame);
                self.store_arith(dst, arith::pow(&l, &r), frame)?;
            }
            OxInst::Neg { dst, src, mode } => {
                let v = self.operand(src, frame);
                self.store_arith(dst, arith::neg(&v, *mode), frame)?;
            }
            OxInst::Concat { dst, lhs, rhs } => {
                let l = self.operand(lhs, frame);
                let r = self.operand(rhs, frame);
                self.store_arith(dst, arith::concat(&l, &r), frame)?;
            }
            OxInst::Compare {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => {
                let l = self.operand(lhs, frame);
                let r = self.operand(rhs, frame);
                self.store_arith(dst, arith::compare(&l, &r, *mode, cmp_op(*op)), frame)?;
            }
            OxInst::Logical {
                dst,
                op,
                lhs,
                rhs,
            } => {
                let l = self.operand(lhs, frame);
                let r = self.operand(rhs, frame);
                let out = match op {
                    LogicalOp::And => arith::and(&l, &r),
                    LogicalOp::Or => arith::or(&l, &r),
                    LogicalOp::Xor => arith::xor(&l, &r),
                    LogicalOp::Eqv => arith::eqv(&l, &r),
                    LogicalOp::Imp => arith::imp(&l, &r),
                };
                self.store_arith(dst, out, frame)?;
            }
            OxInst::Not { dst, src } => {
                let v = self.operand(src, frame);
                self.store_arith(dst, arith::not(&v), frame)?;
            }
            OxInst::Coerce { dst, src, target } => {
                let v = self.operand(src, frame);
                let out = match target {
                    OxCoerceTarget::Numeric(t) => arith::coerce_numeric(&v, *t),
                    OxCoerceTarget::Str => arith::coerce_string(&v),
                    OxCoerceTarget::FixedStr(n) => arith::coerce_fixed_string(&v, *n as usize),
                    // A widen-to-Variant carries no value change.
                    OxCoerceTarget::ImplicitVariant => Ok(v),
                };
                self.store_arith(dst, out, frame)?;
            }
            // Statement boundaries drive Resume granularity + finalization timing; with
            // no objects in scope yet the drain is a no-op.
            OxInst::StmtBoundary { .. } => {}
            OxInst::ClearErr => self.err = ErrState::default(),

            // Everything else is a later milestone (calls/builtins M2-b; error setters
            // M2-c; objects/COM/arrays/records M3) — explicit, never a silent no-op.
            other => return Err(Vm3Error::Unimplemented { what: inst_kind(other) }),
        }
        Ok(())
    }

    /// Store the result of a fallible kernel op, raising its fault on error.
    fn store_arith(
        &mut self,
        dst: &OxPlace,
        out: Result<Variant, ArithError>,
        frame: &mut Frame,
    ) -> Result<(), Vm3Error> {
        let v = out.map_err(|e| Vm3Error::Fault(Fault::from_arith(e)))?;
        self.store(dst, v, frame);
        Ok(())
    }

    fn store(&mut self, place: &OxPlace, v: Variant, frame: &mut Frame) {
        match place {
            OxPlace::Local(l) => {
                if let Some(slot) = frame.locals.get_mut(l.0) {
                    *slot = v;
                }
            }
            OxPlace::Global(g) => {
                if let Some(slot) = self.globals.get_mut(g.0) {
                    *slot = v;
                }
            }
            OxPlace::Temp(t) => {
                frame.temps.insert(t.0, v);
            }
        }
    }

    fn operand(&self, op: &OxOperand, frame: &Frame) -> Variant {
        match op {
            OxOperand::Const(c) => const_variant(c),
            OxOperand::Use(p) => self.read(p, frame),
        }
    }

    fn read(&self, place: &OxPlace, frame: &Frame) -> Variant {
        match place {
            OxPlace::Local(l) => frame.locals.get(l.0).cloned().unwrap_or_else(Variant::empty),
            OxPlace::Global(g) => self.globals.get(g.0).cloned().unwrap_or_else(Variant::empty),
            OxPlace::Temp(t) => frame.temps.get(&t.0).cloned().unwrap_or_else(Variant::empty),
        }
    }
}

fn cmp_op(op: CmpOp) -> arith::CmpOp {
    match op {
        CmpOp::Eq => arith::CmpOp::Eq,
        CmpOp::Ne => arith::CmpOp::Ne,
        CmpOp::Lt => arith::CmpOp::Lt,
        CmpOp::Le => arith::CmpOp::Le,
        CmpOp::Gt => arith::CmpOp::Gt,
        CmpOp::Ge => arith::CmpOp::Ge,
    }
}

fn const_variant(c: &OxConst) -> Variant {
    match c {
        OxConst::Empty => Variant::empty(),
        OxConst::Null => Variant::null(),
        // `Nothing` is a null object reference; like vm2's `LoadEmpty`, an empty
        // Variant stands in (the runtime treats Empty/0 as Nothing for `Is`).
        OxConst::Nothing => Variant::empty(),
        OxConst::Bool(b) => Variant::from_bool(*b),
        OxConst::I32(n) => Variant::from_i32(*n),
        OxConst::I64(n) => Variant::from_i64(*n),
        OxConst::F32(bits) => Variant::from_f32(f32::from_bits(*bits)),
        OxConst::F64(bits) => Variant::from_f64(f64::from_bits(*bits)),
        OxConst::Currency(scaled) => Variant::from_currency_scaled_i64(*scaled),
        OxConst::Date(bits) => Variant::from_date_f64(f64::from_bits(*bits)),
        OxConst::Str(s) => Variant::from_string(s.clone()),
    }
}

/// A short label for an instruction kind (for the `Unimplemented` message).
fn inst_kind(inst: &OxInst) -> &'static str {
    match inst {
        OxInst::CallProc { .. } => "CallProc",
        OxInst::CallProcRef { .. } => "CallProcRef",
        OxInst::CallExtern { .. } => "CallExtern",
        OxInst::CallNative { .. } => "CallNative (builtin/Declare)",
        OxInst::CallByName { .. } => "CallByName",
        OxInst::ComCallEarly { .. } => "ComCallEarly",
        OxInst::ComCallLate { .. } => "ComCallLate",
        OxInst::Box { .. } => "Box",
        OxInst::Unbox { .. } => "Unbox",
        OxInst::ValidateAssignment { .. } => "ValidateAssignment",
        OxInst::LoadProcRef { .. } => "LoadProcRef",
        OxInst::CompareObjectIs { .. } => "CompareObjectIs",
        OxInst::TypeOfIs { .. } => "TypeOfIs",
        OxInst::NewObject { .. } | OxInst::NewExtern { .. } => "New",
        OxInst::Predeclared { .. } | OxInst::PredeclaredExtern { .. } => "Predeclared",
        OxInst::NewRecord { .. } => "NewRecord",
        OxInst::FieldGet { .. } | OxInst::FieldSet { .. } => "object field access",
        OxInst::RecordGet { .. } | OxInst::RecordSet { .. } => "record field access",
        OxInst::ArrayLiteral { .. }
        | OxInst::ArrayAppend { .. }
        | OxInst::ArrayRedim { .. }
        | OxInst::ArrayGet { .. }
        | OxInst::ArraySet { .. }
        | OxInst::ArrayErase { .. }
        | OxInst::Bound { .. } => "array op",
        OxInst::ForEachInit { .. } | OxInst::ForEachNext { .. } => "For Each",
        OxInst::WithEventsGet { .. }
        | OxInst::WithEventsSet { .. }
        | OxInst::WithEventsClearOwner { .. }
        | OxInst::WithEventsFirstOwner { .. }
        | OxInst::WithEventsNextOwner { .. } => "WithEvents",
        OxInst::RaiseEvent { .. } => "RaiseEvent",
        OxInst::Ptr { .. } => "pointer helper",
        OxInst::ErrFieldGet { .. } => "Err field read",
        OxInst::SetErrorHandler(_) => "On Error",
        OxInst::AddRef { .. } | OxInst::Release { .. } => "refcount effect",
        OxInst::DrainTerminations => "DrainTerminations",
        // The handled instructions never reach here.
        _ => "instruction",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_bundle::coreir::{
        CoreBinOp, CoreConst, CoreGlobal, CoreLocal, CorePlace, CoreProc, CoreProgram, CoreStmt,
        CoreValue, LocalId as CoreLocalId,
    };
    use oxvba_bundle::{
        AssignmentIntent, AssignmentTargetKind, BuiltinType, NumericCoerceTarget, NumericMode,
        ProcedureKind, StringCompareMode, VarTypeRef,
    };
    use oxvba_hal::HostPolicy;
    use oxvba_hal::adapters::null::NullHostServices;
    use oxvba_runtime::variant::VarType;

    /// Bind-free: hand-build a `CoreProgram`, elaborate it to OxIR, run it on vm3, and
    /// read back a snapshot slot.
    fn run_core(prog: &CoreProgram) -> Vm3<'_> {
        // Leak the elaborated program so the returned VM can borrow it for the test.
        let oxp: &'static OxProgram =
            Box::leak(Box::new(oxvba_oxir::elaborate::elaborate(prog).expect("elaborate")));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        Vm3::run(oxp, host).expect("vm3 run")
    }

    fn assign(place: CorePlace, value: CoreValue) -> CoreStmt {
        CoreStmt::Assign {
            place,
            value,
            intent: AssignmentIntent::Let,
            target_kind: AssignmentTargetKind::Scalar,
            target_name: String::new(),
            target_type_name: String::new(),
        }
    }

    fn local(name: &str, ty: VarTypeRef) -> CoreLocal {
        CoreLocal {
            name: name.into(),
            ty,
            array_element: None,
        }
    }

    fn main_proc(locals: Vec<CoreLocal>, body: Vec<CoreStmt>) -> CoreProgram {
        CoreProgram {
            procs: vec![CoreProc {
                name: "Main".into(),
                kind: ProcedureKind::Sub,
                params: Vec::new(),
                locals,
                return_local: None,
                body,
            }],
            entry: Some(oxvba_bundle::coreir::ProcId(0)),
            unit_name: "T".into(),
            ..Default::default()
        }
    }

    #[test]
    fn checked_long_arithmetic_matches() {
        // Sub Main(): n = (10 + 5) * 2  →  n is Long 30.
        let n = || CorePlace::Local(CoreLocalId(0));
        let long = NumericMode::Checked(NumericCoerceTarget::Long);
        let bin = |op, l, r| CoreValue::Binary {
            op,
            lhs: Box::new(l),
            rhs: Box::new(r),
            mode: StringCompareMode::Binary,
            num: long,
        };
        let expr = bin(
            CoreBinOp::Mul,
            bin(
                CoreBinOp::Add,
                CoreValue::Const(CoreConst::I32(10)),
                CoreValue::Const(CoreConst::I32(5)),
            ),
            CoreValue::Const(CoreConst::I32(2)),
        );
        let prog = main_proc(
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            vec![assign(n(), expr)],
        );
        let vm = run_core(&prog);
        let v = vm.slot(0).expect("slot n"); // global_count is 0, so slot 0 = Main local 0
        assert_eq!(v.vtype(), VarType::Long);
        assert_eq!(v.as_i32(), Some(30));
    }

    #[test]
    fn string_concat_matches() {
        // Sub Main(): s = "ab" & "cd"  →  s is "abcd".
        let s = || CorePlace::Local(CoreLocalId(0));
        let expr = CoreValue::Binary {
            op: CoreBinOp::Concat,
            lhs: Box::new(CoreValue::Const(CoreConst::Str("ab".into()))),
            rhs: Box::new(CoreValue::Const(CoreConst::Str("cd".into()))),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        };
        let prog = main_proc(
            vec![local("s", VarTypeRef::Builtin(BuiltinType::String))],
            vec![assign(s(), expr)],
        );
        let vm = run_core(&prog);
        let v = vm.slot(0).expect("slot s");
        let s = oxvba_runtime::variant_to_vba_string(&v)
            .map(|b| b.as_str())
            .unwrap_or_default();
        assert_eq!(s, "abcd");
    }

    #[test]
    fn comparison_into_boolean_matches() {
        // Sub Main(): x = 3.5 : b = (x > 1)  →  b is True.
        let x = || CorePlace::Local(CoreLocalId(0));
        let b = || CorePlace::Local(CoreLocalId(1));
        let cmp = CoreValue::Binary {
            op: CoreBinOp::Gt,
            lhs: Box::new(CoreValue::Load(x())),
            rhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        };
        let prog = main_proc(
            vec![
                local("x", VarTypeRef::Variant),
                local("b", VarTypeRef::Builtin(BuiltinType::Boolean)),
            ],
            vec![
                assign(x(), CoreValue::Const(CoreConst::F64(3.5f64.to_bits()))),
                assign(b(), cmp),
            ],
        );
        let vm = run_core(&prog);
        let bv = vm.slot(1).expect("slot b");
        assert_eq!(bv.as_bool(), Some(true));
    }

    #[test]
    fn module_globals_lead_the_snapshot() {
        // A module global `g As Long` then `Sub Main(): g = 7`.
        let g = CorePlace::Global(oxvba_bundle::coreir::GlobalId(0));
        let mut prog = main_proc(Vec::new(), vec![assign(g, CoreValue::Const(CoreConst::I32(7)))]);
        prog.globals = vec![CoreGlobal {
            name: "g".into(),
            ty: VarTypeRef::Builtin(BuiltinType::Long),
            array_element: None,
        }];
        let vm = run_core(&prog);
        // Slot 0 is the global (globals lead), and there are no Main locals after it.
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(7));
    }
}
