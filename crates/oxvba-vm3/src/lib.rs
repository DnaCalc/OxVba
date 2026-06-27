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
//! This cut runs the **scalar / string / Boolean value core + control flow + calls**:
//! `Assign`, all arithmetic (`Arith`/`Div`/`Pow`/`Neg`), `Concat`, `Compare`,
//! `Logical`/`Not`, `Coerce`, the `Jump`/`Branch`/`Return` terminators and the
//! statement-boundary marker, plus (M2-b) compiled procedure calls (`CallProc`) with
//! **true ByRef aliasing** and base-library built-ins (`CallNative` → the shared
//! `oxvba_lib::invoke`). The value semantics go through the shared [`oxvba_eval::arith`]
//! kernel — the *same* functions vm2 calls — so a successful run is vm2-identical by
//! construction.
//!
//! Dispatch is an explicit **block-threaded loop over a heap frame stack** (no native
//! recursion), so a `CallProc` pushes a callee and the loop continues with it, `Return`
//! pops back, and deep recursion is bounded by the frame ceiling (error 28) rather than
//! overflowing the host stack — the iterative model vm2 uses.
//!
//! The frame holds its values as `Variant`s (the shareable slot layout the JIT
//! side-exits into); the **typed unboxed lanes** + per-site type profiler are the M6
//! speculation tier, an addition over this layout rather than a retrofit. The full
//! error/`Resume` model (M2-c), and objects, COM, arrays, records, `Declare`, and
//! cross-bundle calls (M3) return [`Vm3Error::Unimplemented`] for now — never a silent
//! mis-execution.

use std::collections::HashMap;

use oxvba_bundle::{
    ArrayElementType, AssignmentIntent, AssignmentTargetKind, NativeImplId, default_array_element,
    redim_safearray_from_elements, vba_record_layout_for_fields,
};
use oxvba_eval::arith::{self, ArithError};
use oxvba_hal::HostServices;
use oxvba_lib::LibContext;
use oxvba_oxir::value::{
    ArithOp, BoundWhich, CmpOp, ErrField, LogicalOp, OxArg, OxCallArg, OxCoerceTarget, OxConst,
    OxNativeCallee, OxOperand, OxPlace,
};
use oxvba_oxir::{
    BlockId, ErrorHandler, FuncId, ImportId, LocalId, OxBlock, OxInst, OxProgram, OxTerminator,
};
use oxvba_runtime::object_ref::{
    ObjectRef, RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR, RuntimeClassDescriptor,
    RuntimeInterfaceDescriptor,
};
use oxvba_runtime::safe_array::{SafeArray, SafeArrayBound};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{Variant, VbaRecord};

/// `DISP_E_PARAMNOTFOUND` — the sentinel an omitted optional argument carries into a
/// callee slot, so `IsMissing`/`IsError` observe it exactly as vm2 does.
const MISSING_ARG: i32 = 0x8002_0004u32 as i32;

/// Project-instance ids start above any class route key (a class's route key is its index),
/// so `compat_identity != route_key` — every allocation reads as a true project instance, not
/// a template/compat object. Mirrors vm2's `INSTANCE_ID_BASE`.
const INSTANCE_ID_BASE: i32 = 0x1000_0000;

/// A run-time fault carrying the `Err` state it populates: the VBA error code
/// (`Err.Number`), the message (`Err.Description`), and an optional explicit
/// `Err.Source`. `source: None` means "use the raise-time default" — the project name —
/// which is what every in-project fault (system or `Err.Raise` with the argument
/// omitted) takes (see [`Vm3::raise`]); `Some` is an explicit `Err.Raise … Source`.
#[derive(Debug, Clone)]
pub struct Fault {
    pub code: i32,
    pub message: String,
    pub source: Option<String>,
}

impl Fault {
    /// A fault with an explicit VBA error code and message (no explicit `Err.Source`, so it
    /// takes the raise-time default — the project name). Used by the array/record ops, which
    /// raise specific codes (subscript out of range 9, type mismatch 13, out of memory 7).
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }
    fn from_arith(e: ArithError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            source: None,
        }
    }
    /// A built-in library error already carries its VBA error code structurally.
    fn from_lib(e: oxvba_lib::LibError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            source: None,
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

/// The active `On Error` **handler policy** of a procedure activation (MS-VBAL §5.4.4).
/// This is the spec's policy only — the orthogonal "active error" liveness is the
/// separate [`Vm3::active_error`] latch (vm2 conflates the two, the root of several
/// divergences; see `docs/OXIR_VM3_ERROR_MODEL.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorMode {
    /// Default: an unhandled fault propagates to the caller.
    None,
    /// `On Error Resume Next`.
    ResumeNext,
    /// `On Error GoTo <label>` — the handler block.
    Goto(BlockId),
}

/// The seeds a `Resume`/`Resume Next` uses, captured from the firing `FaultDispatch`:
/// `resume` is the faulting statement's start block, `resume_next` the next statement's.
/// Holding `Some` is exactly the spec's "active error" liveness for the activation.
#[derive(Debug, Clone, Copy)]
struct ResumePoint {
    resume: BlockId,
    resume_next: BlockId,
    /// The handler policy active when the error was caught. A `Goto` catch demotes
    /// `error_mode` to `None` (single-shot *while in the handler*, so a re-raise there
    /// propagates); `Resume`/`Resume Next`/`Resume <label>` then RE-ARM this handler on
    /// the way out, so a fault after the resume is caught again (standard VBA — the
    /// demotion is only for the duration of the handler).
    handler: ErrorMode,
}

/// A resolved runtime storage location on the frame stack — what an [`OxPlace`]
/// denotes once ByRef aliasing is applied. `Local`/`Temp` name a specific frame by
/// index so a callee's ByRef parameter can point at one of its caller's cells (which
/// always outlives it, since callers sit below callees and pop later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Loc {
    Global(usize),
    Local(usize, usize),
    Temp(usize, usize),
}

/// `For Each` iterator state: the snapshot of source elements (taken at loop entry,
/// matching vm2) and the current position. Keyed in [`Vm3::for_each`] by the loop
/// variable's resolved [`Loc`], so concurrent/reentrant loops never alias.
struct ForEachState {
    elements: Vec<Variant>,
    position: usize,
}

/// One procedure activation: its dispatch position, value slots, ByRef aliasing, and
/// the linkage back to its caller. The activation stack holds these so dispatch is an
/// explicit loop (no native recursion → deep VBA recursion is bounded by the frame
/// ceiling, error 28, not a host stack overflow — and matches vm2's iterative model).
struct Frame {
    /// The function this frame is executing.
    func: FuncId,
    /// The current block and the index of the *next* instruction within it.
    block: BlockId,
    ip: usize,
    /// Frame locals (parameters first, then declared locals), indexed by `LocalId`.
    locals: Vec<Variant>,
    /// Single-assignment temporaries, indexed by `TempId` (sparse — written before read).
    temps: HashMap<usize, Variant>,
    /// ByRef parameters: a parameter's frame-local index → the caller location it
    /// aliases, resolved to its ultimate backing at call time (so aliases never chain).
    /// Writes through such a parameter hit the backing live — vm2's true aliasing.
    aliases: HashMap<usize, Loc>,
    /// Where this call's return value is written (resolved in the caller at call time);
    /// `None` for a statement call or the entry/initializer frame.
    dst: Option<Loc>,
    /// The local holding this function's result (`None` for a `Sub`).
    return_local: Option<LocalId>,
    /// The caller's error mode, restored when this frame returns (each callee starts
    /// with no handler).
    saved_error_mode: ErrorMode,
    /// The caller's active-error latch, restored on return (each callee starts with no
    /// active error). Keeping it per-activation is what makes a propagated `Resume`
    /// re-run the *caller's* call-site statement.
    saved_active_error: Option<ResumePoint>,
    /// The `GoSub` Resumption List — a per-activation LIFO stack of return blocks
    /// (MS-VBAL §5.4.2.14). `GoSub` pushes its `ret`; `Return` pops the most recent.
    gosub_stack: Vec<BlockId>,
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
    host: &'h dyn HostServices,
    lib: LibContext,
    globals: Vec<Variant>,
    /// The activation stack. `frames[0]` is the entry (`Main`) frame and is never
    /// popped — it backs the result snapshot; deeper frames are `CallProc` callees.
    frames: Vec<Frame>,
    /// The current activation's `On Error` handler policy (saved/restored per frame).
    error_mode: ErrorMode,
    /// The current activation's "active error" latch (the spec's per-activation fault
    /// state): `Some` while an error is being handled, carrying the `Resume` seeds.
    /// Gates `Resume` legality (empty ⇒ error 20) and is cleared by `Resume*`/`Exit *`.
    active_error: Option<ResumePoint>,
    err: ErrState,
    /// `Err.LastDllError` — refreshed after a `Declare Lib` call (M3); `0` until then.
    last_dll_error: i32,
    /// The fault currently being routed (set when a fallible op transfers to a pad).
    pending_fault: Option<Fault>,
    /// `For Each` iterator state, keyed by the loop variable's resolved [`Loc`] (so
    /// reentrant/nested loops that reuse a slot number never alias) — mirrors vm2's
    /// `for_each` map.
    for_each: HashMap<Loc, ForEachState>,
    /// Monotonic project-instance id counter (starts at [`INSTANCE_ID_BASE`]).
    next_instance_id: i32,
    /// Re-entrancy guard for [`Vm3::maybe_drain`] (a `Class_Terminate` can itself drop the
    /// last reference to another object — the guard keeps the drain a single fixpoint loop).
    draining: bool,
    /// Per-class leaked `&'static` runtime descriptors (1:1 with `program.classes`), built
    /// once in [`Vm3::run`] — `ObjectRef::from_project_instance` requires a `'static`
    /// descriptor, exactly as vm2's `LoadedBundle` leaks them.
    class_descriptors: Vec<&'static RuntimeClassDescriptor>,
    /// `VB_PredeclaredId` singleton cache, keyed by class index (allocate-once + run
    /// `Class_Initialize` once), mirroring vm2's per-bundle `predeclared_singletons`.
    predeclared_singletons: HashMap<usize, Variant>,
}

impl<'h> Vm3<'h> {
    /// Run `program` to completion and return the finished VM (read the result snapshot
    /// with [`Vm3::slot`]). Mirrors vm2: the global initializer runs first, then `Main`
    /// in an entry frame that is never popped.
    pub fn run(program: &'h OxProgram, host: &'h dyn HostServices) -> Result<Self, Vm3Error> {
        // One leaked `&'static` runtime descriptor per project class (its name + the universal
        // IUnknown interface) — the shape `ObjectRef::from_project_instance` requires, exactly
        // as vm2's `LoadedBundle::load` leaks them. The leak is per-run and bounded by the
        // class count (matching vm2); a future arena can reclaim it.
        let class_descriptors: Vec<&'static RuntimeClassDescriptor> = program
            .classes
            .iter()
            .map(|class| {
                let name: &'static str = Box::leak(class.name.clone().into_boxed_str());
                let interfaces: &'static [RuntimeInterfaceDescriptor] =
                    Box::leak(Box::new([RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR]));
                &*Box::leak(Box::new(RuntimeClassDescriptor { name, interfaces }))
            })
            .collect();
        let mut vm = Vm3 {
            program,
            host,
            lib: LibContext::default(),
            globals: vec![Variant::empty(); program.globals.len()],
            frames: Vec::new(),
            error_mode: ErrorMode::None,
            active_error: None,
            err: ErrState::default(),
            last_dll_error: 0,
            pending_fault: None,
            for_each: HashMap::new(),
            next_instance_id: INSTANCE_ID_BASE,
            draining: false,
            class_descriptors,
            predeclared_singletons: HashMap::new(),
        };

        if let Some(init) = program.global_initializer {
            let frame = vm.new_frame(init);
            vm.frames.push(frame);
            let r = vm.run_loop(0);
            // The initializer writes module globals; its own frame is discarded.
            vm.frames.pop();
            r?;
        }
        // Isolate this run from any prior run on the shared thread-local termination
        // queue, before the entry runs — matching vm2's per-run reset (object-lifecycle
        // drains land in M3, but the isolation is run-lifecycle plumbing).
        oxvba_runtime::reset_pending_terminations();
        if let Some(entry) = program.entry {
            let frame = vm.new_frame(entry);
            vm.frames.push(frame);
            // The entry frame is never popped — it stays as `frames[0]` for the snapshot.
            let r = vm.run_loop(0);
            // Run any `Class_Terminate`s parked while the run unwound — including objects an
            // uncaught fault released as it propagated out of called procs (vm2 drains on the
            // fault path; without this a Terminate would be lost on an error exit). On a clean
            // finish this is a no-op (statement boundaries already drained; the entry frame's
            // own locals stay live, un-terminated, exactly as vm2 leaves them).
            vm.maybe_drain();
            r?;
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
            self.frames.first()?.locals.get(rel).cloned()
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
    /// `Err.LastDllError` — the OS last-error captured after the most recent `Declare Lib`
    /// call (M3-7); `0` until a Declare runs.
    pub fn last_dll_error(&self) -> i32 {
        self.last_dll_error
    }

    fn new_frame(&self, func: FuncId) -> Frame {
        let f = &self.program.funcs[func.0];
        Frame {
            func,
            block: f.entry,
            ip: 0,
            locals: vec![Variant::empty(); f.locals.len()],
            temps: HashMap::new(),
            aliases: HashMap::new(),
            dst: None,
            return_local: f.return_local,
            saved_error_mode: ErrorMode::None,
            saved_active_error: None,
            gosub_stack: Vec::new(),
        }
    }

    /// The block-threaded dispatch loop: run the top frame until the frame at depth
    /// `base` returns. A `CallProc` pushes a callee and the loop simply continues with
    /// it; `Return` pops back to the caller. There is no native recursion, so deep VBA
    /// recursion is bounded by the frame ceiling (error 28), never a host stack
    /// overflow — and the model mirrors vm2's iterative dispatch.
    fn run_loop(&mut self, base: usize) -> Result<(), Vm3Error> {
        // `program` is a `'h` reference, independent of the `&mut self` exec borrows.
        let program = self.program;
        while self.frames.len() > base {
            let top = self.frames.len() - 1;
            let (func, block, ip) = {
                let fr = &self.frames[top];
                (fr.func, fr.block, fr.ip)
            };
            let blk: &OxBlock = program.funcs[func.0]
                .blocks
                .get(block.0)
                .ok_or_else(|| Vm3Error::Malformed(format!("block {} out of range", block.0)))?;

            if ip < blk.instrs.len() {
                // Advance past this instruction first, so a `CallProc` it performs
                // resumes the caller at the *next* instruction when the callee returns.
                self.frames[top].ip = ip + 1;
                if let Err(e) = self.exec(&blk.instrs[ip]) {
                    match e {
                        Vm3Error::Fault(fault) => self.route_fault(fault)?,
                        other => return Err(other),
                    }
                }
                continue;
            }

            match &blk.terminator {
                OxTerminator::Jump(b) => self.goto(top, *b),
                OxTerminator::Branch {
                    cond,
                    then_blk,
                    else_blk,
                } => {
                    // `cond` is a pre-computed Boolean: the elaboration emits a `Truthy`
                    // before *every* conditional Branch (a statically-Bool operand is not
                    // a guaranteed runtime Boolean — an unassigned `Dim b As Boolean` is
                    // Empty, `Not b` of an Empty Bool is a Long, a Variant compare is
                    // Null), so the terminator is a pure transfer and any truthiness fault
                    // already routed through the pad at the `Truthy`.
                    let v = self.operand(cond)?;
                    let taken = v.as_bool().ok_or_else(|| {
                        Vm3Error::Malformed("Branch condition is not a pre-computed Boolean".into())
                    })?;
                    self.goto(top, if taken { *then_blk } else { *else_blk });
                }
                OxTerminator::Return => {
                    if self.frames.len() == base + 1 {
                        // The base frame returned — leave it on the stack (the entry frame
                        // backs the result snapshot) and end this run.
                        break;
                    }
                    self.do_return()?;
                }
                // VBA `End`: stop the *entire* program immediately at any call depth — no
                // return to the caller, no finalization. Unwind to the base frame (which
                // stays on the stack to back the snapshot) and end the run.
                OxTerminator::Halt => {
                    self.frames.truncate(base + 1);
                    break;
                }
                // The landing pad: dispatch the in-flight fault on the activation's
                // handler policy (MS-VBAL §5.4.4; doc rules R4/R9).
                OxTerminator::FaultDispatch { resume, resume_next } => {
                    let rp = ResumePoint {
                        resume: *resume,
                        resume_next: *resume_next,
                        // Captured before the Goto arm demotes `error_mode`, so a later
                        // `Resume*` re-arms the handler that was active at the catch.
                        handler: self.error_mode,
                    };
                    match self.error_mode {
                        // Default: no enabled handler ⇒ propagate to the caller (or, at the
                        // base, out of the run). A pad is only entered via `route_fault`
                        // (which always `raise`s), so a missing fault is a structural defect.
                        ErrorMode::None => {
                            let fault = self.pending_fault.take().ok_or_else(|| {
                                Vm3Error::Malformed(
                                    "FaultDispatch reached with no pending fault".into(),
                                )
                            })?;
                            self.propagate_fault(fault, base)?;
                        }
                        // Caught by `On Error Resume Next`: latch the active error, consume
                        // the fault, continue past the faulting statement.
                        ErrorMode::ResumeNext => {
                            self.pending_fault = None;
                            self.active_error = Some(rp);
                            self.goto(top, *resume_next);
                        }
                        // Caught by `On Error GoTo h`: the handler is single-shot — demote
                        // the policy to Default before entering it (so a re-raise inside the
                        // handler propagates to the caller, not back into the same handler),
                        // latch the active error, transfer to the handler block.
                        ErrorMode::Goto(handler) => {
                            self.pending_fault = None;
                            self.error_mode = ErrorMode::None;
                            self.active_error = Some(rp);
                            self.goto(top, handler);
                        }
                    }
                }
                // The three `Resume` forms (R6/R7/R8): with no active error, raise error 20
                // ("Resume without error"); otherwise reset `Err`, clear the latch, RE-ARM
                // the handler that caught the error (so a fault after the resume is caught
                // again), and transfer.
                OxTerminator::Resume => match self.active_error.take() {
                    Some(rp) => {
                        self.err = ErrState::default();
                        self.error_mode = rp.handler;
                        self.goto(top, rp.resume);
                    }
                    None => self.raise_runtime_error(20)?,
                },
                OxTerminator::ResumeNext => match self.active_error.take() {
                    Some(rp) => {
                        self.err = ErrState::default();
                        self.error_mode = rp.handler;
                        self.goto(top, rp.resume_next);
                    }
                    None => self.raise_runtime_error(20)?,
                },
                OxTerminator::ResumeLabel(b) => match self.active_error.take() {
                    Some(rp) => {
                        self.err = ErrState::default();
                        self.error_mode = rp.handler;
                        self.goto(top, *b);
                    }
                    None => self.raise_runtime_error(20)?,
                },
                // `Err.Raise Number[, Source][, Description]` / `Error n`: build the `Err`
                // state from the number plus any explicit Source/Description, then route
                // through the statement pad so an active `On Error` can catch it (R11).
                //
                // MS-VBAL §9071 (oracle-confirmed): an omitted argument INHERITS the
                // current `Err` field **when `Err` is un-cleared** (`Err.Number != 0`,
                // regardless of whether that error came from a prior `Err.Raise` or a
                // system fault); when `Err` is cleared, an omitted Source falls back to
                // the project name and an omitted Description to the standard message for
                // the number. This is per-field. (System faults never inherit — that path
                // is `from_arith`/`route_fault`, which always builds fresh fields.)
                OxTerminator::Raise {
                    number,
                    source,
                    description,
                    inherit,
                } => {
                    let num_v = self.operand(number)?;
                    match arith::coerce_numeric(&num_v, oxvba_bundle::NumericCoerceTarget::Long) {
                        Ok(code_v) => {
                            let code = code_v.as_i32().unwrap_or(0);
                            // §9071 inherit applies only to `Err.Raise` (inherit=true);
                            // the legacy `Error <n>` statement (inherit=false) never
                            // inherits — oracle-confirmed. Inherit needs an un-cleared Err.
                            let inherit = *inherit && self.err.number != 0;
                            let message = match description {
                                Some(op) => self.operand_string(op)?,
                                None if inherit => self.err.description.clone(),
                                None => default_error_message(code),
                            };
                            let source = match source {
                                Some(op) => Some(self.operand_string(op)?),
                                None if inherit => Some(self.err.source.clone()),
                                None => None, // -> project name in `raise`
                            };
                            self.route_fault(Fault {
                                code,
                                message,
                                source,
                            })?;
                        }
                        // A non-numeric raise code is itself a coercion fault (e.g. 13).
                        Err(e) => self.route_fault(Fault::from_arith(e))?,
                    }
                }
                // GoSub / Return: a per-activation LIFO resumption list (R12).
                OxTerminator::GoSub { target, ret } => {
                    self.frames[top].gosub_stack.push(*ret);
                    self.goto(top, *target);
                }
                OxTerminator::GoSubReturn => match self.frames[top].gosub_stack.pop() {
                    Some(ret) => self.goto(top, ret),
                    None => self.raise_runtime_error(3)?, // Return without GoSub
                },
                OxTerminator::Unreachable => {
                    return Err(Vm3Error::Malformed("reached an Unreachable terminator".into()));
                }
            }
        }
        Ok(())
    }

    /// Jump frame `top` to the start of `block`.
    fn goto(&mut self, top: usize, block: BlockId) {
        let fr = &mut self.frames[top];
        fr.block = block;
        fr.ip = 0;
    }

    /// Route an in-flight fault to the current frame's block fault pad (intra-frame);
    /// the pad's `FaultDispatch` then consults the error mode.
    fn route_fault(&mut self, fault: Fault) -> Result<(), Vm3Error> {
        let top = self.frames.len() - 1;
        let (func, block) = (self.frames[top].func, self.frames[top].block);
        let pad = self.program.funcs[func.0].blocks[block.0]
            .fault_target
            .ok_or_else(|| {
                Vm3Error::Malformed(
                    "fallible instruction in a block with no fault_target".into(),
                )
            })?;
        self.raise(fault);
        self.goto(top, pad);
        Ok(())
    }

    /// Propagate an unhandled fault out of the current frame: pop it (restoring the
    /// caller's error mode) and re-route at the caller's call site, or — at the base
    /// frame — surface it as the run's result.
    fn propagate_fault(&mut self, fault: Fault, base: usize) -> Result<(), Vm3Error> {
        if self.frames.len() <= base + 1 {
            return Err(Vm3Error::Fault(fault));
        }
        let callee = self.frames.pop().expect("frame to unwind");
        self.error_mode = callee.saved_error_mode;
        self.active_error = callee.saved_active_error;
        // The caller's `CallProc` faulted: route to *its* block's fault pad.
        self.route_fault(fault)
    }

    /// Pop a returning callee: restore the caller's error mode and copy out the
    /// function's return value (true aliasing already propagated ByRef writes live).
    fn do_return(&mut self) -> Result<(), Vm3Error> {
        let callee = self.frames.pop().expect("returning frame");
        self.error_mode = callee.saved_error_mode;
        self.active_error = callee.saved_active_error;
        if let (Some(loc), Some(rl)) = (callee.dst, callee.return_local)
            && let Some(v) = callee.locals.get(rl.0).cloned()
        {
            self.write_loc(loc, v)?;
        }
        // Proc epilogue: drop the callee frame (releasing the objects its locals held) and then
        // run any parked `Class_Terminate`s — vm2's epilogue drain timing.
        drop(callee);
        self.maybe_drain();
        Ok(())
    }

    /// Populate `Err` from a raised fault and stash it for the landing pad. Number and
    /// Description come from the fault; `Err.Source` is the fault's explicit source if it
    /// carries one (`Err.Raise … Source`), else the **project name** — the VBA default
    /// for any error generated within the project, matching the Excel/VBA 7.1 oracle
    /// (`Err.Source = "VBAProject"`; see `docs/VBA_ERROR_MODEL_ORACLE_FINDINGS.md`).
    fn raise(&mut self, fault: Fault) {
        self.err.number = fault.code;
        self.err.description = fault.message.clone();
        self.err.source = fault
            .source
            .clone()
            .unwrap_or_else(|| self.program.unit_name.clone());
        self.pending_fault = Some(fault);
    }

    /// Evaluate an operand and coerce it to its VBA string form — used for an explicit
    /// `Err.Raise` Source/Description argument.
    fn operand_string(&mut self, op: &OxOperand) -> Result<String, Vm3Error> {
        let v = self.operand(op)?;
        Ok(oxvba_runtime::variant_to_vba_string(&v)
            .map(|b| b.as_str())
            .unwrap_or_default())
    }

    /// Raise a vm3-internal run-time error (a code with its default message) by routing
    /// it through the current statement's fault pad — so an active `On Error` can catch
    /// it (used for error 20 "Resume without error" and error 3 "Return without GoSub").
    fn raise_runtime_error(&mut self, code: i32) -> Result<(), Vm3Error> {
        self.route_fault(Fault {
            code,
            message: default_error_message(code),
            source: None,
        })
    }

    /// Execute one straight-line instruction against the top frame.
    fn exec(&mut self, inst: &OxInst) -> Result<(), Vm3Error> {
        match inst {
            OxInst::Assign { dst, value } => {
                let v = self.operand(value)?;
                self.store(dst, v)?;
            }
            OxInst::Arith {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                let out = match op {
                    ArithOp::Add => arith::add(&l, &r, *mode),
                    ArithOp::Sub => arith::sub(&l, &r, *mode),
                    ArithOp::Mul => arith::mul(&l, &r, *mode),
                    ArithOp::IntDiv => arith::int_div(&l, &r, *mode),
                    ArithOp::Mod => arith::modulo(&l, &r, *mode),
                };
                self.store_arith(dst, out)?;
            }
            OxInst::Div { dst, lhs, rhs } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                self.store_arith(dst, arith::div(&l, &r))?;
            }
            OxInst::Pow { dst, lhs, rhs } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                self.store_arith(dst, arith::pow(&l, &r))?;
            }
            OxInst::Neg { dst, src, mode } => {
                let v = self.operand(src)?;
                self.store_arith(dst, arith::neg(&v, *mode))?;
            }
            OxInst::Concat { dst, lhs, rhs } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                self.store_arith(dst, arith::concat(&l, &r))?;
            }
            OxInst::Compare {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                self.store_arith(dst, arith::compare(&l, &r, *mode, cmp_op(*op)))?;
            }
            OxInst::Logical {
                dst,
                op,
                lhs,
                rhs,
            } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                let out = match op {
                    LogicalOp::And => arith::and(&l, &r),
                    LogicalOp::Or => arith::or(&l, &r),
                    LogicalOp::Xor => arith::xor(&l, &r),
                    LogicalOp::Eqv => arith::eqv(&l, &r),
                    LogicalOp::Imp => arith::imp(&l, &r),
                };
                self.store_arith(dst, out)?;
            }
            OxInst::Not { dst, src } => {
                let v = self.operand(src)?;
                self.store_arith(dst, arith::not(&v))?;
            }
            // Reduce a condition to a Boolean by VBA truthiness (the elaboration emits
            // this before a conditional `Branch`); the `is_truthy` rule + error code are
            // exactly what vm2's `JumpIfZero` uses.
            OxInst::Truthy { dst, src } => {
                let v = self.operand(src)?;
                let out = arith::is_truthy(&v).map(Variant::from_bool);
                self.store_arith(dst, out)?;
            }
            OxInst::Coerce { dst, src, target } => {
                let v = self.operand(src)?;
                let out = match target {
                    OxCoerceTarget::Numeric(t) => arith::coerce_numeric(&v, *t),
                    OxCoerceTarget::Str => arith::coerce_string(&v),
                    OxCoerceTarget::FixedStr(n) => arith::coerce_fixed_string(&v, *n as usize),
                    // A widen-to-Variant carries no value change.
                    OxCoerceTarget::ImplicitVariant => Ok(v),
                };
                self.store_arith(dst, out)?;
            }
            // A compiled VBA procedure call (intra-unit). The `AddressOf`-reference
            // `CallProcRef` is M3-7.
            OxInst::CallProc { dst, proc, args } => self.call_proc(*dst, *proc, args)?,
            // A cross-bundle call. vm3 links only the synthetic `VBA` library bundle today,
            // so this resolves to a native library function (`Strings.Left`, `Math.Abs`, …)
            // run through the same `invoke_native_lib` bridge as `CallNative { Builtin }`. A
            // reference to another VBA *project* needs a multi-`OxProgram` linker (deferred —
            // surfaced as an explicit `Unimplemented`, never a silent skip).
            OxInst::CallExtern { dst, import, args } => self.call_extern(*dst, *import, args)?,
            // A base-library built-in funnels through the single shared `oxvba_lib::invoke`
            // (the identical bridge vm2 uses); `Declare Lib` marshalling is M3.
            OxInst::CallNative { dst, callee, args } => match callee {
                OxNativeCallee::Builtin(id) => {
                    let argv = self.native_args(args)?;
                    let result = self.invoke_native_lib(*id, &argv)?;
                    if let Some(dst) = dst {
                        self.store(dst, result)?;
                    }
                }
                OxNativeCallee::Declare { .. } => {
                    return Err(Vm3Error::Unimplemented { what: "Declare Lib call" });
                }
            },
            // `On Error` sets the activation's handler policy and — per MS-VBAL §5.4.4.1
            // (doc rule R5) — unconditionally resets the `Err` object. (The active-error
            // latch is cleared only by `Resume`/`Exit`, not here.)
            OxInst::SetErrorHandler(handler) => {
                self.err = ErrState::default();
                match handler {
                    // `On Error GoTo -1` clears the active-error latch (so the current
                    // handler can re-catch) but KEEPS the handler policy — unlike the
                    // others, it does not set `error_mode` (R13; oracle `oe_goto_minus1`).
                    ErrorHandler::GotoMinus1 => self.active_error = None,
                    ErrorHandler::ResumeNext => self.error_mode = ErrorMode::ResumeNext,
                    ErrorHandler::Goto0 => self.error_mode = ErrorMode::None,
                    ErrorHandler::GotoLabel(b) => self.error_mode = ErrorMode::Goto(*b),
                }
            }
            // Read an `Err` property.
            OxInst::ErrFieldGet { dst, field } => {
                let v = match field {
                    ErrField::Number => Variant::from_i32(self.err.number),
                    ErrField::Description => Variant::from_string(self.err.description.clone()),
                    ErrField::Source => Variant::from_string(self.err.source.clone()),
                    ErrField::LastDllError => Variant::from_i32(self.last_dll_error),
                };
                self.store(dst, v)?;
            }
            // `Err.Clear` → reset the `Err` object.
            OxInst::ClearErr => self.err = ErrState::default(),
            // A statement boundary drives finalization timing: run any parked
            // `Class_Terminate`s released by the previous statement (the error model takes its
            // `Resume` seeds from `FaultDispatch`, not from here).
            OxInst::StmtBoundary { .. } => self.maybe_drain(),

            // `Let`/`Set` legality check (M3-4).
            OxInst::ValidateAssignment {
                src,
                intent,
                target_kind,
                target_name,
                target_type_name,
            } => self.validate_assignment(
                src,
                *intent,
                *target_kind,
                target_name,
                target_type_name,
            )?,

            // ── Arrays / For Each (M3-2) ─────────────────────────────────────────────
            OxInst::ArrayLiteral { dst, values } => {
                let elems = values
                    .iter()
                    .map(|v| self.operand(v))
                    .collect::<Result<Vec<_>, _>>()?;
                self.store(dst, Variant::from_safearray(SafeArray::from_variants(elems)))?;
            }
            OxInst::ArrayAppend { dst, array, item } => {
                let mut elems = match self.operand(array)?.as_safearray() {
                    Some(arr) => arr.variant_elements().unwrap_or_default(),
                    None => Vec::new(),
                };
                elems.push(self.operand(item)?);
                self.store(dst, Variant::from_safearray(SafeArray::from_variants(elems)))?;
            }
            OxInst::ArrayRedim {
                dst,
                upper_bounds,
                lower_bounds,
                element,
                preserve,
            } => self.array_redim(dst, upper_bounds, lower_bounds, element, *preserve)?,
            OxInst::ArrayGet {
                dst,
                array,
                indices,
            } => {
                // `x(i…)` where `x` is a bare `Variant`/`As Object` resolves at run time: an
                // OBJECT receiver makes the parentheses a default-member (`Item`, dispid 0)
                // call — the late-bound leg, which lands with COM late dispatch (M3-8).
                let recv = self.operand(array)?;
                if recv.as_safearray().is_none() && recv.as_object_ref().is_some() {
                    return Err(Vm3Error::Unimplemented {
                        what: "array-index default-member call on an object",
                    });
                }
                let arr = self.array_of(array)?;
                let bounds = arr
                    .bounds()
                    .ok_or_else(|| Vm3Error::Fault(Fault::new(9, "array has no bounds")))?;
                let flat = self.flat_index(indices, &bounds)?;
                if flat >= arr.len() {
                    return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
                }
                let value = arr
                    .variant_element(flat)
                    .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                self.store(dst, value)?;
            }
            OxInst::ArraySet {
                array,
                indices,
                value,
            } => {
                let recv = self.read(array)?;
                if recv.as_safearray().is_none() && recv.as_object_ref().is_some() {
                    return Err(Vm3Error::Unimplemented {
                        what: "array-index default-member assignment on an object",
                    });
                }
                let arr = recv
                    .as_safearray()
                    .ok_or_else(|| Vm3Error::Fault(Fault::new(13, "expected an array")))?;
                let bounds = arr
                    .bounds()
                    .ok_or_else(|| Vm3Error::Fault(Fault::new(9, "array has no bounds")))?;
                let flat = self.flat_index(indices, &bounds)?;
                if flat >= arr.len() {
                    return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
                }
                let v = self.operand(value)?;
                // Mutate the array's element, then write the (alias-resolved) place back so a
                // ByRef-aliased array sees the change — equivalent to vm2's in-place
                // `read_place_mut(...).set_safearray_element(...)`.
                let mut arr_v = recv;
                arr_v
                    .set_safearray_element(flat, &v)
                    .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                self.store(array, arr_v)?;
            }
            OxInst::ArrayErase { array, .. } => {
                // vm2 lowers `Erase` to "set the array variable to Empty"; match that.
                // (Faithful dynamic-deallocate vs fixed-array-reset semantics need a live
                // oracle probe — a deferred refinement.)
                self.store(array, Variant::empty())?;
            }
            OxInst::Bound {
                dst,
                which,
                array,
                dimension,
            } => {
                let arr = self.array_of(array)?;
                let bounds = arr
                    .bounds()
                    .ok_or_else(|| Vm3Error::Fault(Fault::new(9, "array has no bounds")))?;
                let bound = &bounds[self.array_bound_index(dimension.as_ref(), &bounds)?];
                let value = match which {
                    BoundWhich::Lower => bound.lower,
                    BoundWhich::Upper => bound.lower + bound.count as i32 - 1,
                };
                self.store(dst, Variant::from_i32(value))?;
            }
            OxInst::ForEachInit { iter, source } => {
                let src = self.operand(source)?;
                // Snapshot the source's elements at loop entry (matching vm2). An array
                // enumerates its elements; a `Collection`/COM object needs the object model
                // (M3-5/M3-8); anything else is an empty iteration.
                let elements = if let Some(arr) = src.as_safearray() {
                    arr.variant_elements().unwrap_or_default()
                } else if src.as_object_ref().is_some() {
                    return Err(Vm3Error::Unimplemented {
                        what: "For Each over an object (Collection / COM enumerator)",
                    });
                } else {
                    Vec::new()
                };
                let key = self.resolve(iter);
                self.for_each
                    .insert(key, ForEachState { elements, position: 0 });
            }
            OxInst::ForEachNext {
                iter,
                item,
                has_value,
            } => {
                let key = self.resolve(iter);
                let next = self.for_each.get_mut(&key).and_then(|state| {
                    let value = state.elements.get(state.position).cloned();
                    if value.is_some() {
                        state.position += 1;
                    }
                    value
                });
                match next {
                    Some(value) => {
                        self.store(item, value)?;
                        self.store(has_value, Variant::from_bool(true))?;
                    }
                    None => self.store(has_value, Variant::from_bool(false))?,
                }
            }

            // ── Records / UDT (M3-3) — value aggregates with native VBA layout ────────
            OxInst::NewRecord { dst, fields } => {
                let layout = vba_record_layout_for_fields(fields)
                    .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                let record =
                    VbaRecord::new_default(layout).map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                self.store(dst, Variant::from_vba_record(record))?;
            }
            OxInst::RecordGet { dst, record, index } => {
                let source = self.operand(record)?;
                // Native VBA records read by field index; a legacy SAFEARRAY-backed record
                // bag (old/hand-built internal values) reads its element, bounds-checked.
                let value = if let Some(rec) = source.as_safearray() {
                    if *index >= rec.len() {
                        return Err(Vm3Error::Fault(Fault::new(9, "record field out of range")));
                    }
                    rec.variant_element(*index)
                        .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?
                } else {
                    source
                        .read_record_field_variant(*index)
                        .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?
                };
                self.store(dst, value)?;
            }
            OxInst::RecordSet {
                record,
                index,
                value,
            } => {
                let v = self.operand(value)?;
                // Read the (alias-resolved) record, write the field, store it back — value
                // semantics: the record's data is owned, so a ByRef-aliased record's backing
                // receives the write (equivalent to vm2's in-place `read_place_mut`).
                let mut target = self.read(record)?;
                if let Some(rec) = target.as_safearray() {
                    if *index >= rec.len() {
                        return Err(Vm3Error::Fault(Fault::new(9, "record field out of range")));
                    }
                    target
                        .set_safearray_element(*index, &v)
                        .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                } else {
                    target
                        .write_record_field_variant(*index, &v)
                        .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                }
                self.store(record, target)?;
            }

            // ── Objects / lifecycle / type identity (M3-5) ───────────────────────────
            OxInst::NewObject { dst, class } => {
                let object = self.new_project_instance(class.0)?;
                self.store(dst, object)?;
            }
            OxInst::Predeclared { dst, class } => {
                let object = self.predeclared_instance(class.0)?;
                self.store(dst, object)?;
            }
            OxInst::FieldGet { dst, object, field } => {
                let recv = self.operand(object)?;
                let instance = variant_to_object(&recv)?;
                // A missing field reads as `Empty` (vm2 parity — field storage is sparse).
                let value = instance
                    .project_field_get(*field)
                    .unwrap_or_else(Variant::empty);
                self.store(dst, value)?;
            }
            OxInst::FieldSet {
                object,
                field,
                value,
            } => {
                let v = self.operand(value)?;
                let recv = self.operand(object)?;
                let instance = variant_to_object(&recv)?;
                instance.project_field_set(*field, v);
            }
            OxInst::CompareObjectIs { dst, lhs, rhs } => {
                let a = object_identity(&self.operand(lhs)?);
                let b = object_identity(&self.operand(rhs)?);
                self.store(dst, Variant::from_bool(a == b))?;
            }
            OxInst::TypeOfIs {
                dst,
                object,
                type_name,
            } => {
                let matches = self.type_of_is(object, type_name)?;
                self.store(dst, Variant::from_bool(matches))?;
            }

            // Everything else is a later milestone (cross-bundle calls / objects / COM /
            // arrays / records M3) — explicit, never a silent no-op.
            other => return Err(Vm3Error::Unimplemented { what: inst_kind(other) }),
        }
        Ok(())
    }

    /// Call a compiled VBA procedure: evaluate the arguments in the *caller* and push a
    /// callee frame (ByVal copies the value in; ByRef true-aliases the caller's backing
    /// location so writes propagate live; an omitted optional gets the `MISSING_ARG`
    /// sentinel), then hand control to the dispatch loop. The return value is copied out
    /// when the frame returns (see `do_return`). Mirrors vm2's `call_proc`.
    fn call_proc(
        &mut self,
        dst: Option<OxPlace>,
        proc: FuncId,
        args: &[OxArg],
    ) -> Result<(), Vm3Error> {
        let program = self.program;
        let callee = program
            .funcs
            .get(proc.0)
            .ok_or_else(|| Vm3Error::Malformed(format!("call to unknown proc {}", proc.0)))?;
        // Resolve the destination + ByRef backings in the caller, before pushing.
        let dst_loc = dst.map(|p| self.resolve(&p));
        let mut locals = vec![Variant::empty(); callee.locals.len()];
        let mut aliases = HashMap::new();
        for (i, arg) in args.iter().enumerate() {
            match arg {
                OxArg::ByVal(op) => {
                    let v = self.operand(op)?;
                    if let Some(slot) = locals.get_mut(i) {
                        *slot = v;
                    }
                }
                OxArg::ByRef(place) => {
                    aliases.insert(i, self.resolve(place));
                }
                OxArg::Omitted => {
                    if let Some(slot) = locals.get_mut(i) {
                        *slot = Variant::from_error_code(MISSING_ARG);
                    }
                }
            }
        }
        let return_local = callee.return_local;
        let entry = callee.entry;

        self.guard_call_depth()?;
        // Push the callee and hand control to it; each procedure starts with no active
        // handler, and the caller's mode is restored from the frame when it returns. The
        // dispatch loop runs the callee and `do_return`/`propagate_fault` pops it — there
        // is no native recursion here, so the call depth is heap-bounded.
        self.frames.push(Frame {
            func: proc,
            block: entry,
            ip: 0,
            locals,
            temps: HashMap::new(),
            aliases,
            dst: dst_loc,
            return_local,
            saved_error_mode: self.error_mode,
            saved_active_error: self.active_error,
            gosub_stack: Vec::new(),
        });
        // Each procedure starts with no handler and no active error (restored on return).
        self.error_mode = ErrorMode::None;
        self.active_error = None;
        Ok(())
    }

    /// A cross-bundle call (`OxInst::CallExtern`). Resolve `import` to a native library
    /// function and run it through the same `invoke_native_lib` bridge a `CallNative`
    /// builtin uses — no frame is pushed (a `NativeBody::Library` body has no VM frame; its
    /// arguments are positional ByVal values), mirroring vm2's `call_extern` short-circuit
    /// and keeping a library function bit-identical however it is routed.
    fn call_extern(
        &mut self,
        dst: Option<OxPlace>,
        import: ImportId,
        args: &[OxArg],
    ) -> Result<(), Vm3Error> {
        let id = self.resolve_library_import(import)?;
        let argv = self.extern_args(args)?;
        let result = self.invoke_native_lib(id, &argv)?;
        if let Some(dst) = dst {
            self.store(&dst, result)?;
        }
        Ok(())
    }

    /// Resolve a cross-bundle `import` to the native library function it names.
    ///
    /// vm3 links only the synthetic `VBA` library bundle (`oxvba_bundle::vba_library_bundle`)
    /// today — the home of every built-in function (`Strings.Left`, `Math.Abs`, the
    /// `DateTime`/`Conversion`/`Information`/`FileSystem` members, …), which the binder lowers
    /// to a `CallExtern` rather than a `CallNative`. A reference to another VBA *project*
    /// needs a multi-`OxProgram` linker, which is deferred — reported as an explicit
    /// `Unimplemented`, never silently mis-run. (Built-in object *methods*
    /// — `Collection.Add`/… , `NativeBody::Method` — never arrive here; they are reached by
    /// member dispatch on a `Collection` instance, which lands with the object model.)
    fn resolve_library_import(&self, import: ImportId) -> Result<NativeImplId, Vm3Error> {
        let imp = self
            .program
            .imports
            .get(import.0)
            .ok_or_else(|| Vm3Error::Malformed(format!("CallExtern names unknown import {}", import.0)))?;
        if !imp.unit.eq_ignore_ascii_case("VBA") {
            return Err(Vm3Error::Unimplemented {
                what: "cross-project OxProgram link",
            });
        }
        let lib = oxvba_bundle::vba_library_bundle();
        let export = lib
            .exports
            .iter()
            .find(|e| e.token.matches(&imp.token))
            .ok_or_else(|| {
                Vm3Error::Malformed(format!(
                    "the VBA library bundle has no export matching import {}",
                    import.0
                ))
            })?;
        let oxvba_bundle::ExportTarget::Proc(proc) = export.target else {
            return Err(Vm3Error::Malformed(
                "a VBA library import resolved to a non-procedure export".into(),
            ));
        };
        match lib.procedures.get(proc).and_then(|p| p.native) {
            Some(oxvba_bundle::NativeBody::Library(id)) => Ok(id),
            Some(oxvba_bundle::NativeBody::Method(_)) => Err(Vm3Error::Malformed(
                "a native object method is not callable via CallExtern".into(),
            )),
            // A VBA-bodied library proc would need the multi-OxProgram linker; the synthetic
            // VBA bundle has only native bodies, so this is unreachable today.
            None => Err(Vm3Error::Unimplemented {
                what: "cross-project OxProgram link",
            }),
        }
    }

    /// Marshal a cross-bundle library call's arguments to plain values: a native library
    /// body reads positional values (a ByRef argument by its *value*), and an omitted
    /// optional is `Empty` — matching vm2's `extern_native_args`.
    fn extern_args(&self, args: &[OxArg]) -> Result<Vec<Variant>, Vm3Error> {
        args.iter()
            .map(|a| match a {
                OxArg::ByVal(op) => self.operand(op),
                OxArg::ByRef(place) => self.read(place),
                OxArg::Omitted => Ok(Variant::empty()),
            })
            .collect()
    }

    /// Build SAFEARRAY bounds from `ReDim` upper-bound operands + static lower bounds, with
    /// vm2's overflow guards: `upper < lower` → subscript out of range (9); a dimension above
    /// `u32::MAX` elements → out of memory (7), so a garbage bound raises a VBA error instead
    /// of attempting an unbounded host allocation that would abort the process.
    fn build_bounds(
        &self,
        upper_bounds: &[OxOperand],
        lower_bounds: &[i32],
    ) -> Result<Vec<SafeArrayBound>, Vm3Error> {
        let mut bounds = Vec::with_capacity(upper_bounds.len());
        for (i, upper_op) in upper_bounds.iter().enumerate() {
            let lower = lower_bounds.get(i).copied().unwrap_or(0);
            let upper_v = self.operand(upper_op)?;
            let upper = arith::int(&upper_v).map_err(arith_fault)? as i32;
            if upper < lower {
                return Err(Vm3Error::Fault(Fault::new(
                    9,
                    "array upper bound below lower bound",
                )));
            }
            let span = i64::from(upper) - i64::from(lower) + 1;
            if span > i64::from(u32::MAX) {
                return Err(Vm3Error::Fault(Fault::new(
                    7,
                    format!("array dimension too large ({span} elements)"),
                )));
            }
            bounds.push(SafeArrayBound {
                count: span as u32,
                lower,
            });
        }
        Ok(bounds)
    }

    /// Flat element index from VBA (absolute) subscript operands, C-order (first dimension
    /// outermost), bounds-checked → subscript out of range (9).
    fn flat_index(
        &self,
        indices: &[OxOperand],
        bounds: &[SafeArrayBound],
    ) -> Result<usize, Vm3Error> {
        if indices.len() != bounds.len() {
            return Err(Vm3Error::Fault(Fault::new(
                9,
                "wrong number of array subscripts",
            )));
        }
        let mut flat = 0usize;
        for (i, index_op) in indices.iter().enumerate() {
            let index_v = self.operand(index_op)?;
            let raw = arith::int(&index_v).map_err(arith_fault)? as i32;
            let bound = &bounds[i];
            let offset = i64::from(raw) - i64::from(bound.lower);
            if offset < 0 || offset >= i64::from(bound.count) {
                return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
            }
            flat = flat * bound.count as usize + offset as usize;
        }
        Ok(flat)
    }

    /// The array (SAFEARRAY) value of an operand, else type mismatch (13).
    fn array_of(&self, op: &OxOperand) -> Result<SafeArray, Vm3Error> {
        self.operand(op)?
            .as_safearray()
            .ok_or_else(|| Vm3Error::Fault(Fault::new(13, "expected an array")))
    }

    /// The 0-based dimension index for `LBound`/`UBound` from an optional dimension operand
    /// (default dimension 1), validated against the array's rank → subscript out of range (9).
    fn array_bound_index(
        &self,
        dimension: Option<&OxOperand>,
        bounds: &[SafeArrayBound],
    ) -> Result<usize, Vm3Error> {
        let dim = match dimension {
            Some(op) => {
                let v = self.operand(op)?;
                arith::int(&v).map_err(arith_fault)?
            }
            None => 1,
        };
        if dim < 1 {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        let index = (dim - 1) as usize;
        if index >= bounds.len() {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        Ok(index)
    }

    /// `ReDim [Preserve]`: build the new SAFEARRAY shaped by `upper_bounds`/`lower_bounds`,
    /// seeding each element with the declared element type's typed default — except, when
    /// `preserve`, keeping each still-in-range existing element (so a UDT array's populated
    /// records survive and only the grown tail is freshly default-seeded). The element storage
    /// matches the declared element type (typed scalars / native records, not normalized to
    /// VT_VARIANT).
    fn array_redim(
        &mut self,
        dst: &OxPlace,
        upper_bounds: &[OxOperand],
        lower_bounds: &[i32],
        element: &ArrayElementType,
        preserve: bool,
    ) -> Result<(), Vm3Error> {
        let bounds = self.build_bounds(upper_bounds, lower_bounds)?;
        let count: usize = bounds.iter().map(|b| b.count as usize).product();
        let elems: Vec<Variant> = if preserve {
            let old = self
                .read(dst)?
                .as_safearray()
                .and_then(|a| a.variant_elements())
                .unwrap_or_default();
            (0..count)
                .map(|i| {
                    old.get(i)
                        .cloned()
                        .unwrap_or_else(|| default_array_element(element))
                })
                .collect()
        } else {
            (0..count).map(|_| default_array_element(element)).collect()
        };
        let array = redim_safearray_from_elements(bounds, element, elems)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
        self.store(dst, Variant::from_safearray(array))?;
        Ok(())
    }

    /// `Let`/`Set` legality check (mirrors vm2). `Set` requires an object source (else
    /// "Object required" 424); `Let` into an `Object` target requires `Set` when the source
    /// is an object (91) and an object source otherwise (424). The strict `Set` *type* check
    /// (error 13 — a project-instance source must be the target's declared class/interface)
    /// needs the project class tables and lands with the object model (M3-5); until a project
    /// instance can exist, every Set-of-object is a COM/`Nothing` value, which vm2 also
    /// passes — so falling through to `Ok` here is behaviorally exact.
    fn validate_assignment(
        &self,
        src: &OxOperand,
        intent: AssignmentIntent,
        target_kind: AssignmentTargetKind,
        target_name: &str,
        _target_type_name: &str,
    ) -> Result<(), Vm3Error> {
        use AssignmentIntent as Intent;
        use AssignmentTargetKind as Kind;
        let value = self.operand(src)?;
        let is_object = matches!(value.vtype(), VarType::Object) || is_nothing(&value);
        match intent {
            Intent::Set if !is_object => Err(Vm3Error::Fault(Fault::new(
                424,
                format!("Object required: {target_name}"),
            ))),
            Intent::Let if target_kind == Kind::Object && is_object => Err(Vm3Error::Fault(
                Fault::new(91, format!("Object variable requires Set: {target_name}")),
            )),
            Intent::Let if target_kind == Kind::Object => Err(Vm3Error::Fault(Fault::new(
                424,
                format!("Object required: {target_name}"),
            ))),
            _ => Ok(()),
        }
    }

    /// Allocate a fresh project-class instance (`New <Class>`): a refcounted IUnknown with a
    /// unique identity, then run its `Class_Initialize` (if any). Mirrors vm2's `Op::NewObject`
    /// (bundle id is always 0 — vm3 runs one program). The instance's `has_terminate` flag is
    /// what later parks it for `Class_Terminate` when its last reference drops.
    fn new_project_instance(&mut self, class_idx: usize) -> Result<Variant, Vm3Error> {
        let descriptor = *self
            .class_descriptors
            .get(class_idx)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown class {class_idx}")))?;
        let program = self.program;
        let class = program
            .classes
            .get(class_idx)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown class {class_idx}")))?;
        let has_terminate = class.terminate.is_some();
        let initialize = class.initialize;
        let instance_id = self.next_instance_id;
        self.next_instance_id += 1;
        let object =
            ObjectRef::from_project_instance(instance_id, class_idx as i32, 0, has_terminate, descriptor);
        let value = Variant::from_object_ref(object.clone());
        if let Some(init) = initialize {
            self.run_proc_with_me(init, Variant::from_object_ref(object), false)?;
        }
        Ok(value)
    }

    /// A `VB_PredeclaredId` auto-instance (`Class1.Foo` with no explicit `New`): allocate the
    /// singleton + run `Class_Initialize` on first access, then reuse the cached instance.
    /// Mirrors vm2's `predeclared_instance`.
    fn predeclared_instance(&mut self, class_idx: usize) -> Result<Variant, Vm3Error> {
        if let Some(existing) = self.predeclared_singletons.get(&class_idx) {
            return Ok(existing.clone());
        }
        let descriptor = *self
            .class_descriptors
            .get(class_idx)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown class {class_idx}")))?;
        let program = self.program;
        let class = program
            .classes
            .get(class_idx)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown class {class_idx}")))?;
        let has_terminate = class.terminate.is_some();
        let initialize = class.initialize;
        let instance_id = self.next_instance_id;
        self.next_instance_id += 1;
        let object =
            ObjectRef::from_project_instance(instance_id, class_idx as i32, 0, has_terminate, descriptor);
        let value = Variant::from_object_ref(object);
        self.predeclared_singletons.insert(class_idx, value.clone());
        if let Some(init) = initialize {
            self.run_proc_with_me(init, value.clone(), false)?;
        }
        Ok(value)
    }

    /// Run a procedure to completion **synchronously** with `me` as its hidden first local —
    /// the lifecycle/event entry point (`Class_Initialize`/`Class_Terminate`/event handlers).
    ///
    /// vm3's dispatch is an explicit loop, so this pushes the callee frame and drives a NESTED
    /// `run_loop(base)` that returns when this frame returns. `run_loop` breaks at
    /// `frames.len() == base + 1` on a normal `Return` *without* popping, and leaves the
    /// faulting frame in place on an uncaught fault — so afterwards we restore the caller's
    /// error state (saved on the frame we pushed) and truncate back to `base` either way.
    /// `suppress` (used for `Class_Terminate`) swallows a raised VBA fault; a structural
    /// `Malformed` always propagates.
    fn run_proc_with_me(
        &mut self,
        proc: FuncId,
        me: Variant,
        suppress: bool,
    ) -> Result<(), Vm3Error> {
        self.guard_call_depth()?;
        let base = self.frames.len();
        let mut frame = self.new_frame(proc);
        if let Some(slot) = frame.locals.get_mut(0) {
            *slot = me;
        }
        frame.saved_error_mode = self.error_mode;
        frame.saved_active_error = self.active_error;
        self.frames.push(frame);
        self.error_mode = ErrorMode::None;
        self.active_error = None;
        let result = self.run_loop(base);
        if let Some(fr) = self.frames.get(base) {
            self.error_mode = fr.saved_error_mode;
            self.active_error = fr.saved_active_error;
        }
        self.frames.truncate(base);
        // Truncating released the lifecycle frame's object locals (and any an uncaught fault
        // left parked as it unwound) — run their `Class_Terminate`s now, the nested-epilogue /
        // fault-path drain that mirrors vm2 (re-entrant drains fold via the `draining` guard).
        self.maybe_drain();
        match result {
            Err(Vm3Error::Fault(_)) if suppress => Ok(()),
            other => other,
        }
    }

    /// Run any parked `Class_Terminate`s to a fixpoint. `Release` (an object's last reference
    /// dropping — a frame pop or a slot overwrite) parks a `has_terminate` instance on the
    /// shared `oxvba_runtime` termination queue; this dequeues and runs each `Class_Terminate`
    /// (with errors suppressed), pinned to statement boundaries / proc epilogue — exactly
    /// vm2's `maybe_drain`. The `draining` guard makes a re-entrant release (a Terminate that
    /// drops another object) fold into the same loop rather than nest.
    fn maybe_drain(&mut self) {
        if self.draining {
            return;
        }
        self.draining = true;
        while oxvba_runtime::has_pending_terminations() {
            for (instance_id, _bundle_id, route_key) in oxvba_runtime::take_pending_terminations() {
                let terminate = self
                    .program
                    .classes
                    .get(route_key as usize)
                    .and_then(|c| c.terminate);
                if let (Some(proc), Some(object)) = (
                    terminate,
                    oxvba_runtime::retained_parked_termination_object(instance_id),
                ) {
                    // A fault in `Class_Terminate` is swallowed (suppress); a `Malformed`
                    // defect would still surface — drop it here to match vm2's `let _ = …`.
                    let _ = self.run_proc_with_me(proc, Variant::from_object_ref(object), true);
                }
                oxvba_runtime::finish_pending_termination(instance_id);
                // WithEvents subscription teardown for the terminated instance lands with the
                // event model (M3-6).
            }
        }
        self.draining = false;
    }

    /// `TypeOf <object> Is <Type>`: for a project instance, match the bare type name against
    /// the instance's class name or any `Implements`ed interface; for a foreign/COM object,
    /// delegate to the host (unreachable until `CreateObject` lands in M3-8, but mirrors vm2).
    fn type_of_is(&self, object: &OxOperand, type_name: &str) -> Result<bool, Vm3Error> {
        let v = self.operand(object)?;
        let obj = variant_to_object(&v)?;
        let bare = type_name.rsplit('.').next().unwrap_or(type_name);
        if obj.is_project_instance() {
            return Ok(self
                .program
                .classes
                .get(obj.route_key() as usize)
                .is_some_and(|class| {
                    class.name.eq_ignore_ascii_case(bare)
                        || class
                            .implements
                            .iter()
                            .any(|i| i.eq_ignore_ascii_case(bare))
                }));
        }
        if let Ok(Some(name)) = self.host.com().object_type_name(obj.clone())
            && (name.eq_ignore_ascii_case(type_name) || name.eq_ignore_ascii_case(bare))
        {
            return Ok(true);
        }
        Ok(false)
    }

    /// The class name of a project instance (else the host's COM name) — the
    /// `TypeName`-of-object resolution mirroring vm2's `object_type_name`.
    fn object_type_name(&self, object: &ObjectRef) -> Option<String> {
        if object.is_project_instance() {
            return self
                .program
                .classes
                .get(object.route_key() as usize)
                .map(|c| c.name.clone());
        }
        self.host.com().object_type_name(object.clone()).ok().flatten()
    }

    /// Invoke a base-library built-in. Most builtins are the pure shared
    /// `oxvba_lib::invoke`, but a few are host/bundle-aware and the pure body would
    /// return a generically-wrong value: `TypeName` of an object yields the literal
    /// `"Object"` from the pure body, so resolve the real class/COM name here, where the
    /// host COM facet is in reach — never let the generic `"Object"` leak as a
    /// silently-wrong result.
    ///
    /// **This method is the intended `builtin_invoke` boundary of the future
    /// `RuntimeImports` ABI** (plan: M4). The Cranelift JIT does not re-implement builtins
    /// or this object-name special-case — it lowers `CallNative` to a `builtin_invoke`
    /// `extern "C"` shim that recovers `&mut Vm3` from its `ctx` and calls *this* method,
    /// so the interpreter and compiled code share one implementation and cannot drift.
    /// Keep its shape `(ctx, id, &[Variant]) -> Result<Variant, _>` ABI-friendly.
    fn invoke_native_lib(&mut self, id: NativeImplId, argv: &[Variant]) -> Result<Variant, Vm3Error> {
        if id == NativeImplId::TypeName
            && let Some(object) = argv.first().and_then(|a| a.as_object_ref())
            && let Some(name) = self.object_type_name(&object)
        {
            // A project instance resolves its class name from the program's class table; a COM
            // object is named by the host. Only if neither names it do we fall through to the
            // pure body (which yields the generic "Object"), exactly as vm2 does.
            return Ok(Variant::from_string(name));
        }
        oxvba_lib::invoke(id, argv, self.host, &mut self.lib).map_err(|e| Vm3Error::Fault(Fault::from_lib(e)))
    }

    /// Marshal a native built-in's arguments to plain values — a built-in reads the
    /// *value* of a ByRef argument (matching vm2's `native_args`), and an omitted one is
    /// `Empty`.
    fn native_args(&self, args: &[OxCallArg]) -> Result<Vec<Variant>, Vm3Error> {
        args.iter()
            .map(|a| match a {
                OxCallArg::Operand(op) => self.operand(op),
                OxCallArg::ByRef(place) => self.read(place),
                OxCallArg::Omitted => Ok(Variant::empty()),
                OxCallArg::Named { value, .. } => self.operand(value),
                OxCallArg::Const(n) => Ok(Variant::from_i32(*n)),
            })
            .collect()
    }

    /// Bound runaway recursion at vm2's frame ceiling, raising error 28 ("Out of stack
    /// space") as a fault, not a panic. The dispatch loop holds frames on the heap (no
    /// native recursion), so the same ceiling vm2 uses is reachable without overflow.
    fn guard_call_depth(&self) -> Result<(), Vm3Error> {
        const MAX_FRAMES: usize = 50_000;
        if self.frames.len() >= MAX_FRAMES {
            return Err(Vm3Error::Fault(Fault {
                code: 28,
                message: "Out of stack space".into(),
                source: None,
            }));
        }
        Ok(())
    }

    /// Store the result of a fallible kernel op, raising its fault on error.
    fn store_arith(
        &mut self,
        dst: &OxPlace,
        out: Result<Variant, ArithError>,
    ) -> Result<(), Vm3Error> {
        let v = out.map_err(|e| Vm3Error::Fault(Fault::from_arith(e)))?;
        self.store(dst, v)
    }

    /// Resolve an [`OxPlace`] against the top frame to a concrete frame-stack [`Loc`],
    /// following a ByRef parameter's alias to its caller-side backing.
    fn resolve(&self, place: &OxPlace) -> Loc {
        let top = self.frames.len() - 1;
        match place {
            OxPlace::Global(g) => Loc::Global(g.0),
            OxPlace::Local(l) => self.frames[top]
                .aliases
                .get(&l.0)
                .copied()
                .unwrap_or(Loc::Local(top, l.0)),
            OxPlace::Temp(t) => Loc::Temp(top, t.0),
        }
    }

    /// Read a resolved location. Local/Global are dense, program-sized tables, so an
    /// out-of-range index is a structural defect (`Malformed`), never a silent default;
    /// `Temp` absence is the SSA write-before-read contract (sparse map → `Empty`).
    fn read_loc(&self, loc: Loc) -> Result<Variant, Vm3Error> {
        match loc {
            Loc::Global(g) => self
                .globals
                .get(g)
                .cloned()
                .ok_or_else(|| Vm3Error::Malformed(format!("global {g} out of range"))),
            Loc::Local(fi, li) => self
                .frames
                .get(fi)
                .and_then(|f| f.locals.get(li))
                .cloned()
                .ok_or_else(|| Vm3Error::Malformed(format!("local [{fi}][{li}] out of range"))),
            Loc::Temp(fi, ti) => Ok(self
                .frames
                .get(fi)
                .and_then(|f| f.temps.get(&ti))
                .cloned()
                .unwrap_or_else(Variant::empty)),
        }
    }

    /// Write a resolved location (same dense/sparse contract as [`Self::read_loc`]).
    fn write_loc(&mut self, loc: Loc, v: Variant) -> Result<(), Vm3Error> {
        match loc {
            Loc::Global(g) => {
                *self
                    .globals
                    .get_mut(g)
                    .ok_or_else(|| Vm3Error::Malformed(format!("global {g} out of range")))? = v;
            }
            Loc::Local(fi, li) => {
                *self
                    .frames
                    .get_mut(fi)
                    .and_then(|f| f.locals.get_mut(li))
                    .ok_or_else(|| Vm3Error::Malformed(format!("local [{fi}][{li}] out of range")))? =
                    v;
            }
            Loc::Temp(fi, ti) => {
                if let Some(f) = self.frames.get_mut(fi) {
                    f.temps.insert(ti, v);
                } else {
                    return Err(Vm3Error::Malformed(format!("temp frame {fi} out of range")));
                }
            }
        }
        Ok(())
    }

    fn store(&mut self, place: &OxPlace, v: Variant) -> Result<(), Vm3Error> {
        let loc = self.resolve(place);
        self.write_loc(loc, v)
    }

    fn read(&self, place: &OxPlace) -> Result<Variant, Vm3Error> {
        self.read_loc(self.resolve(place))
    }

    fn operand(&self, op: &OxOperand) -> Result<Variant, Vm3Error> {
        match op {
            OxOperand::Const(c) => Ok(const_variant(c)),
            OxOperand::Use(p) => self.read(p),
        }
    }
}

/// The default VBA message for a run-time error code — used as `Err.Description` for a
/// raised error whose Description is not otherwise supplied (mirrors vm2's table; a
/// richer mapping rides with M2-c-2). Unmapped codes get the application-defined text.
fn default_error_message(code: i32) -> String {
    match code {
        3 => "Return without GoSub",
        5 => "Invalid procedure call or argument",
        6 => "Overflow",
        9 => "Subscript out of range",
        11 => "Division by zero",
        13 => "Type mismatch",
        20 => "Resume without error",
        28 => "Out of stack space",
        91 => "Object variable or With block variable not set",
        424 => "Object required",
        _ => "Application-defined or object-defined error",
    }
    .to_string()
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

/// Wrap an `arith` coercion error as a routed vm3 fault (it carries its own VBA code).
fn arith_fault(e: ArithError) -> Vm3Error {
    Vm3Error::Fault(Fault::from_arith(e))
}

/// VBA `Nothing`/empty test (mirrors vm2): a null object reference, `Empty`/`Null`, or a
/// numeric zero (the literal-0-as-Nothing representation). Used by `Let`/`Set` validation.
fn is_nothing(value: &Variant) -> bool {
    match value.vtype() {
        VarType::Object => value.as_object_ref().map(|o| o.raw()).unwrap_or(0) == 0,
        VarType::Empty | VarType::Null => true,
        _ => value.as_i32() == Some(0),
    }
}

/// The raw identity (an `i32`) of an object value, or 0 for a non-object/`Nothing` — the basis
/// of the `Is` operator (`CompareObjectIs`). Mirrors vm2's `object_identity`.
fn object_identity(value: &Variant) -> i32 {
    value.as_object_ref().map(|o| o.raw()).unwrap_or(0)
}

/// Coerce a value to an object reference (mirrors vm2's `variant_to_object`): an unset object
/// reference (`Object`/`Empty`/`Null` with no instance) is "Object variable not set" (91),
/// distinct from a non-object value (424); a bare integer is a legacy compat-identity handle.
fn variant_to_object(value: &Variant) -> Result<ObjectRef, Vm3Error> {
    if let Some(object) = value.as_object_ref() {
        return Ok(object);
    }
    if matches!(
        value.vtype(),
        VarType::Object | VarType::Empty | VarType::Null
    ) {
        return Err(Vm3Error::Fault(Fault::new(
            91,
            "Object variable or With block variable not set",
        )));
    }
    if let Some(raw) = value.as_i32() {
        return Ok(ObjectRef::from_compat_identity(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ObjectRef::from_compat_identity)
            .map_err(|_| Vm3Error::Fault(Fault::new(13, "object handle exceeds i32 range")));
    }
    Err(Vm3Error::Fault(Fault::new(424, "Object required")))
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
        CoreArg, CoreBinOp, CoreCallee, CoreConst, CoreGlobal, CoreLocal, CoreParam, CorePlace,
        CoreProc, CoreProgram, CoreStmt, CoreValue, ErrorOp, ExitKind, LabelId,
        LocalId as CoreLocalId, ProcId,
    };
    use oxvba_bundle::{
        AssignmentIntent, AssignmentTargetKind, BuiltinType, NativeImplId, NumericCoerceTarget,
        NumericMode, ProcedureKind, StringCompareMode, VarTypeRef,
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
            entry: Some(ProcId(0)),
            unit_name: "T".into(),
            ..Default::default()
        }
    }

    /// A multi-proc program whose `procs[0]` is the entry (`Main`).
    fn procs_program(procs: Vec<CoreProc>) -> CoreProgram {
        CoreProgram {
            procs,
            entry: Some(ProcId(0)),
            unit_name: "T".into(),
            ..Default::default()
        }
    }

    fn proc(
        name: &str,
        kind: ProcedureKind,
        params: Vec<CoreParam>,
        locals: Vec<CoreLocal>,
        return_local: Option<CoreLocalId>,
        body: Vec<CoreStmt>,
    ) -> CoreProc {
        CoreProc { name: name.into(), kind, params, locals, return_local, body }
    }

    fn long_param(name: &str) -> CoreParam {
        CoreParam {
            name: name.into(),
            ty: VarTypeRef::Builtin(BuiltinType::Long),
            by_ref: true,
            variadic: false,
        }
    }

    /// `CorePlace::Local(i)`.
    fn lc(i: usize) -> CorePlace {
        CorePlace::Local(CoreLocalId(i))
    }
    /// `CoreValue::Load(Local(i))`.
    fn load(i: usize) -> CoreValue {
        CoreValue::Load(lc(i))
    }
    /// A `Checked(Long)` addition (the regime the binder picks for `Long` operands).
    fn long_add(l: CoreValue, r: CoreValue) -> CoreValue {
        CoreValue::Binary {
            op: CoreBinOp::Add,
            lhs: Box::new(l),
            rhs: Box::new(r),
            mode: StringCompareMode::Binary,
            num: NumericMode::Checked(NumericCoerceTarget::Long),
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

    #[test]
    fn if_else_control_flow_executes() {
        // `If <c> Then n = 5 Else n = 9` — a non-Boolean condition flows through the
        // elaboration's Truthy coercion and the Branch terminator.
        use oxvba_bundle::coreir::CoreIfArm;
        let run_if = |cond: i32| -> Option<i32> {
            let n = || CorePlace::Local(CoreLocalId(0));
            let prog = main_proc(
                vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
                vec![CoreStmt::If {
                    arms: vec![CoreIfArm {
                        condition: CoreValue::Const(CoreConst::I32(cond)),
                        body: vec![assign(n(), CoreValue::Const(CoreConst::I32(5)))],
                    }],
                    else_body: vec![assign(n(), CoreValue::Const(CoreConst::I32(9)))],
                }],
            );
            run_core(&prog).slot(0).and_then(|v| v.as_i32())
        };
        assert_eq!(run_if(1), Some(5), "a truthy condition takes the Then branch");
        assert_eq!(run_if(0), Some(9), "a falsy condition takes the Else branch");
    }

    #[test]
    fn entry_falls_back_to_the_first_proc_when_no_main() {
        // No `Sub Main` (CoreProgram.entry == None): vm3 must still run the only proc,
        // matching vm2's select_entry fallback (else nothing runs and `g` stays Empty).
        let g = CorePlace::Global(oxvba_bundle::coreir::GlobalId(0));
        let prog = CoreProgram {
            procs: vec![CoreProc {
                name: "Helper".into(), // deliberately not "Main"
                kind: ProcedureKind::Sub,
                params: Vec::new(),
                locals: Vec::new(),
                return_local: None,
                body: vec![assign(g, CoreValue::Const(CoreConst::I32(42)))],
            }],
            globals: vec![CoreGlobal {
                name: "g".into(),
                ty: VarTypeRef::Builtin(BuiltinType::Long),
                array_element: None,
            }],
            entry: None,
            unit_name: "T".into(),
            ..Default::default()
        };
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));
    }

    #[test]
    fn unassigned_boolean_condition_is_false_not_an_error() {
        // `Dim b As Boolean : If b Then n = 5 Else n = 9` — b is unassigned, so Empty at
        // runtime (not a Boolean tag). vm2's is_truthy(Empty) = False, so the Else branch
        // (n = 9); vm3 must match, not error on the non-Boolean tag. (Regression guard:
        // before the Truthy-always fix this returned Malformed.)
        use oxvba_bundle::coreir::CoreIfArm;
        let b = || CorePlace::Local(CoreLocalId(0)); // Dim b As Boolean (unassigned)
        let n = || CorePlace::Local(CoreLocalId(1)); // Dim n As Long
        let prog = main_proc(
            vec![
                local("b", VarTypeRef::Builtin(BuiltinType::Boolean)),
                local("n", VarTypeRef::Builtin(BuiltinType::Long)),
            ],
            vec![CoreStmt::If {
                arms: vec![CoreIfArm {
                    condition: CoreValue::Load(b()),
                    body: vec![assign(n(), CoreValue::Const(CoreConst::I32(5)))],
                }],
                else_body: vec![assign(n(), CoreValue::Const(CoreConst::I32(9)))],
            }],
        );
        let vm = run_core(&prog);
        assert_eq!(
            vm.slot(1).and_then(|v| v.as_i32()),
            Some(9),
            "unassigned Boolean is False -> Else branch, not an error"
        );
    }

    #[test]
    fn select_case_runs_including_a_null_selector() {
        // A Select matching a case, and one whose selector is Null (no case matches ->
        // Case Else) — the Null selector must fall through, not error (vm2 parity).
        use oxvba_bundle::coreir::{CaseClause, CoreCaseBlock};
        let run_select = |sel: CoreValue, sel_ty: VarTypeRef| -> Option<i32> {
            let s = || CorePlace::Local(CoreLocalId(0));
            let x = || CorePlace::Local(CoreLocalId(1));
            let prog = main_proc(
                vec![local("s", sel_ty), local("x", VarTypeRef::Builtin(BuiltinType::Long))],
                vec![
                    assign(s(), sel),
                    CoreStmt::Select {
                        selector: CoreValue::Load(s()),
                        cases: vec![CoreCaseBlock {
                            clauses: vec![CaseClause::Value(CoreValue::Const(CoreConst::I32(1)))],
                            body: vec![assign(x(), CoreValue::Const(CoreConst::I32(5)))],
                        }],
                        case_else: vec![assign(x(), CoreValue::Const(CoreConst::I32(9)))],
                    },
                ],
            );
            run_core(&prog).slot(1).and_then(|v| v.as_i32())
        };
        // Selector 1 matches `Case 1` -> x = 5.
        assert_eq!(
            run_select(CoreValue::Const(CoreConst::I32(1)), VarTypeRef::Builtin(BuiltinType::Long)),
            Some(5)
        );
        // A Null selector matches nothing (is_truthy(Null) = False) -> Case Else, x = 9.
        assert_eq!(
            run_select(CoreValue::Const(CoreConst::Null), VarTypeRef::Variant),
            Some(9),
            "a Null Select selector falls through to Case Else, not an error"
        );
    }

    #[test]
    fn for_loop_accumulates() {
        // `For i = 1 To 3 : s = s + i : Next` -> s = 6 (exercises the For counter-test
        // Branch + its Truthy coercion end-to-end).
        let i = || CorePlace::Local(CoreLocalId(0));
        let s = || CorePlace::Local(CoreLocalId(1));
        let prog = main_proc(
            vec![
                local("i", VarTypeRef::Builtin(BuiltinType::Long)),
                local("s", VarTypeRef::Builtin(BuiltinType::Long)),
            ],
            vec![
                assign(s(), CoreValue::Const(CoreConst::I32(0))),
                CoreStmt::ForRange {
                    var: i(),
                    start: CoreValue::Const(CoreConst::I32(1)),
                    end: CoreValue::Const(CoreConst::I32(3)),
                    step: None,
                    body: vec![assign(
                        s(),
                        CoreValue::Binary {
                            op: CoreBinOp::Add,
                            lhs: Box::new(CoreValue::Load(s())),
                            rhs: Box::new(CoreValue::Load(i())),
                            mode: StringCompareMode::Binary,
                            num: NumericMode::Widening,
                        },
                    )],
                },
            ],
        );
        let vm = run_core(&prog);
        assert_eq!(vm.slot(1).and_then(|v| v.as_i32()), Some(6));
    }

    #[test]
    fn call_proc_returns_a_function_value() {
        // Function Add(a As Long, b As Long) As Long : Add = a + b
        // Sub Main() : n = Add(10, 5)   ->  n = 15
        let add = proc(
            "Add",
            ProcedureKind::Function,
            vec![long_param("a"), long_param("b")], // LocalId 0, 1
            vec![local("Add", VarTypeRef::Builtin(BuiltinType::Long))], // the return local, LocalId 2
            Some(CoreLocalId(2)),
            vec![assign(lc(2), long_add(load(0), load(1)))],
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))], // LocalId 0
            None,
            vec![assign(
                lc(0),
                CoreValue::Call {
                    callee: CoreCallee::VbaProc { proc: ProcId(1) },
                    args: vec![
                        CoreArg::ByVal(CoreValue::Const(CoreConst::I32(10))),
                        CoreArg::ByVal(CoreValue::Const(CoreConst::I32(5))),
                    ],
                },
            )],
        );
        let prog = procs_program(vec![main, add]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(15));
    }

    #[test]
    fn call_proc_byref_mutates_the_caller() {
        // Sub Bump(ByRef x As Long) : x = x + 1
        // Sub Main() : v = 41 : Bump(v)   ->  v = 42 (true aliasing through the frame stack)
        let bump = proc(
            "Bump",
            ProcedureKind::Sub,
            vec![long_param("x")], // LocalId 0
            Vec::new(),
            None,
            vec![assign(lc(0), long_add(load(0), CoreValue::Const(CoreConst::I32(1))))],
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("v", VarTypeRef::Builtin(BuiltinType::Long))], // LocalId 0
            None,
            vec![
                assign(lc(0), CoreValue::Const(CoreConst::I32(41))),
                CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::VbaProc { proc: ProcId(1) },
                    args: vec![CoreArg::ByRef(lc(0))],
                }),
            ],
        );
        let prog = procs_program(vec![main, bump]);
        let vm = run_core(&prog);
        assert_eq!(
            vm.slot(0).and_then(|v| v.as_i32()),
            Some(42),
            "a ByRef write must propagate to the caller's backing slot"
        );
    }

    #[test]
    fn call_proc_byval_does_not_mutate_the_caller() {
        // The same Bump, but Main passes `v` ByVal -> the callee mutates a copy, v stays 41.
        let bump = proc(
            "Bump",
            ProcedureKind::Sub,
            vec![long_param("x")],
            Vec::new(),
            None,
            vec![assign(lc(0), long_add(load(0), CoreValue::Const(CoreConst::I32(1))))],
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("v", VarTypeRef::Builtin(BuiltinType::Long))],
            None,
            vec![
                assign(lc(0), CoreValue::Const(CoreConst::I32(41))),
                CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::VbaProc { proc: ProcId(1) },
                    args: vec![CoreArg::ByVal(load(0))],
                }),
            ],
        );
        let prog = procs_program(vec![main, bump]);
        let vm = run_core(&prog);
        assert_eq!(
            vm.slot(0).and_then(|v| v.as_i32()),
            Some(41),
            "a ByVal copy must NOT propagate back to the caller"
        );
    }

    #[test]
    fn call_native_builtin_invokes_the_shared_library() {
        // Sub Main() : n = Len("abc")   ->  n = 3 (through the shared `oxvba_lib::invoke`).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            None,
            vec![assign(
                lc(0),
                CoreValue::Call {
                    callee: CoreCallee::Native(NativeImplId::Len),
                    args: vec![CoreArg::ByVal(CoreValue::Const(CoreConst::Str("abc".into())))],
                },
            )],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(3));
    }

    #[test]
    fn call_native_typename_routes_through_the_veneer() {
        // Sub Main() : n = TypeName("hi")  ->  "String". Proves CallNative now goes
        // through the `invoke_native_lib` veneer (which only intercepts an *object*
        // argument, mirroring vm2): a non-object argument skips the interception and the
        // pure library body still answers correctly.
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Variant)],
            None,
            vec![assign(
                lc(0),
                CoreValue::Call {
                    callee: CoreCallee::Native(NativeImplId::TypeName),
                    args: vec![CoreArg::ByVal(CoreValue::Const(CoreConst::Str("hi".into())))],
                },
            )],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        let s = oxvba_runtime::variant_to_vba_string(&vm.slot(0).expect("n"))
            .map(|b| b.as_str())
            .unwrap_or_default();
        assert_eq!(s, "String");
    }

    #[test]
    fn recursion_is_bounded_not_a_stack_overflow() {
        // Sub Spin() : Spin()  — unbounded self-recursion must surface as VBA error 28
        // ("Out of stack space"), not a native stack overflow / panic.
        let spin = proc(
            "Spin",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            vec![CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::VbaProc { proc: ProcId(0) },
                args: Vec::new(),
            })],
        );
        let oxp: &'static OxProgram =
            Box::leak(Box::new(oxvba_oxir::elaborate::elaborate(&procs_program(vec![spin])).expect("elaborate")));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        match Vm3::run(oxp, host) {
            Err(Vm3Error::Fault(f)) => assert_eq!(f.code, 28, "deep recursion is error 28"),
            Err(other) => panic!("expected an Out-of-stack fault, got error: {other}"),
            Ok(_) => panic!("expected an Out-of-stack fault, but the run completed"),
        }
    }

    // ── M2-c: error / Resume / Err / GoSub model ────────────────────────────────

    /// Run a single-proc program and expect it to end with an uncaught fault of `code`.
    fn run_expecting_fault(prog: &CoreProgram, code: i32) {
        let oxp: &'static OxProgram =
            Box::leak(Box::new(oxvba_oxir::elaborate::elaborate(prog).expect("elaborate")));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        match Vm3::run(oxp, host) {
            Err(Vm3Error::Fault(f)) => assert_eq!(f.code, code, "expected uncaught error {code}"),
            Err(other) => panic!("expected uncaught error {code}, got error: {other}"),
            Ok(_) => panic!("expected uncaught error {code}, but the run completed"),
        }
    }

    /// `1 / 0` — a division-by-zero (error 11) expression.
    fn div_by_zero() -> CoreValue {
        CoreValue::Binary {
            op: CoreBinOp::Div,
            lhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
            rhs: Box::new(CoreValue::Const(CoreConst::I32(0))),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        }
    }

    #[test]
    fn on_error_resume_next_continues_and_reads_err() {
        // On Error Resume Next : n = 1/0 : n = Err.Number  ->  n = 11 (Resume Next skips
        // past the faulting statement; Err carries the division-by-zero code).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            None,
            vec![
                CoreStmt::Error(ErrorOp::OnErrorResumeNext),
                assign(lc(0), div_by_zero()),
                assign(lc(0), CoreValue::ErrField(ErrField::Number)),
            ],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(11));
    }

    #[test]
    fn err_raise_is_caught_by_on_error_goto() {
        // On Error GoTo H : Err.Raise 5 : Exit Sub : H: n = Err.Number  ->  n = 5
        // (proves Err.Raise routes through the statement pad so On Error catches it).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            None,
            vec![
                CoreStmt::Error(ErrorOp::OnErrorGotoLabel(LabelId(0))),
                CoreStmt::Error(ErrorOp::Raise {
                    number: CoreValue::Const(CoreConst::I32(5)),
                    source: None,
                    description: None,
                    inherit: true,
                }),
                CoreStmt::Exit(ExitKind::Proc),
                CoreStmt::Label(LabelId(0)),
                assign(lc(0), CoreValue::ErrField(ErrField::Number)),
            ],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(5));
    }

    #[test]
    fn resume_re_runs_the_faulting_statement() {
        // On Error GoTo H : k = k+1 : m = 1/(k-1) [faults when k=1] : n = 42 : Exit Sub
        // H: k = k+1 : Resume   ->  the division statement re-runs with k=2 (1/1 ok), so
        // control reaches `n = 42`. `Resume` re-enters the *faulting* statement, and the
        // handler's k-bump prevents an infinite re-fault. (`n` is a clean Long literal so
        // the assertion is unaffected by the Double division result, which lands in `m`.)
        let k = || lc(2);
        let div = CoreValue::Binary {
            op: CoreBinOp::Div,
            lhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
            rhs: Box::new(CoreValue::Binary {
                op: CoreBinOp::Sub,
                lhs: Box::new(load(2)),
                rhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
                mode: StringCompareMode::Binary,
                num: NumericMode::Widening,
            }),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        };
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![
                local("n", VarTypeRef::Builtin(BuiltinType::Long)), // 0
                local("m", VarTypeRef::Variant),                    // 1
                local("k", VarTypeRef::Builtin(BuiltinType::Long)), // 2
            ],
            None,
            vec![
                CoreStmt::Error(ErrorOp::OnErrorGotoLabel(LabelId(0))),
                assign(k(), long_add(load(2), CoreValue::Const(CoreConst::I32(1)))),
                assign(lc(1), div), // faulting statement: 1/(k-1)
                assign(lc(0), CoreValue::Const(CoreConst::I32(42))),
                CoreStmt::Exit(ExitKind::Proc),
                CoreStmt::Label(LabelId(0)),
                assign(k(), long_add(load(2), CoreValue::Const(CoreConst::I32(1)))),
                CoreStmt::Error(ErrorOp::Resume),
            ],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));
    }

    #[test]
    fn resume_without_active_error_raises_20() {
        // Resume Next with no active error -> runtime error 20 (Resume without error).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            vec![CoreStmt::Error(ErrorOp::ResumeNext)],
        );
        run_expecting_fault(&procs_program(vec![main]), 20);
    }

    #[test]
    fn return_without_gosub_raises_3() {
        // A bare Return (no GoSub on the stack) -> runtime error 3 (Return without GoSub).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            vec![CoreStmt::GoSubReturn],
        );
        run_expecting_fault(&procs_program(vec![main]), 3);
    }

    #[test]
    fn gosub_resumption_list_is_lifo() {
        // GoSub A : Exit Sub : A: GoSub B : s = s & "A" : Return : B: s = s & "B" : Return
        // -> s = "BA": B's Return pops the inner ret, A's Return pops the outer (LIFO).
        let s = || lc(0);
        let cat = |suffix: &str| {
            assign(
                s(),
                CoreValue::Binary {
                    op: CoreBinOp::Concat,
                    lhs: Box::new(load(0)),
                    rhs: Box::new(CoreValue::Const(CoreConst::Str(suffix.into()))),
                    mode: StringCompareMode::Binary,
                    num: NumericMode::Widening,
                },
            )
        };
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("s", VarTypeRef::Builtin(BuiltinType::String))],
            None,
            vec![
                CoreStmt::GoSub(LabelId(0)),
                CoreStmt::Exit(ExitKind::Proc),
                CoreStmt::Label(LabelId(0)), // A
                CoreStmt::GoSub(LabelId(1)),
                cat("A"),
                CoreStmt::GoSubReturn,
                CoreStmt::Label(LabelId(1)), // B
                cat("B"),
                CoreStmt::GoSubReturn,
            ],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        let s = oxvba_runtime::variant_to_vba_string(&vm.slot(0).expect("s"))
            .map(|b| b.as_str())
            .unwrap_or_default();
        assert_eq!(s, "BA");
    }
}
