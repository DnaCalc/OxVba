//! `oxvba-vm2` — the interpreter over the clean [`oxvba_bundle::Bundle`].
//!
//! This is the Phase-1 reference runtime: a slot-machine interpreter with an
//! exhaustive `match` over every [`Op`], so the instruction set is covered by
//! construction. Library built-ins dispatch into [`oxvba_lib::invoke`]; COM
//! member dispatch, `Declare Lib`, and host I/O go through the
//! [`oxvba_hal::HostServices`] facets.
//!
//! ## Execution model
//! Slots are a single flat file ([`Bundle::slot_count`]); each procedure owns
//! the range `[frame_base, frame_base + frame_slots)` and the VM snapshots/
//! restores that range across a call so recursion is safe. Module-level globals
//! live in slots outside every procedure's range and therefore persist.
//! Arguments use the copy-in / copy-out convention ([`ProcArg`]): `ByRef`
//! copies the caller slot into the parameter on entry and copies it back on
//! return. `On Error` state is per-procedure (saved/restored across calls).
//!
//! FIDELITY (documented, Phase-1): `Resume`/`Resume Next` target op granularity
//! rather than VBA statement granularity (the front-end does not yet emit
//! statement markers); `ByRef` is copy-in/copy-out rather than true aliasing;
//! the project-object lifecycle (`New`/refcount/`Class_Terminate`) and the
//! WithEvents→COM subscription bridge (which needs the project event-route
//! table) are staged — WithEvents binding state + owner enumeration are
//! implemented here, the host subscription sync is not yet wired.

mod arith;

use std::borrow::Cow;
use std::collections::HashMap;

use oxvba_bundle::{
    Bundle, CallArg, ComMemberSelector, ExternalCallWriteback, ExternalCallWritebackKind,
    NativeCallee, Op, ProcArg, ProjectMemberKind,
};
use oxvba_com::{DynamicCallArg, DynamicCallKind, DynamicCallRequest, DynamicMemberSelector, DynamicValue};
use oxvba_hal::HostServices;
use oxvba_hal::traits::DynLinkDescriptorView;
use oxvba_lib::{LibContext, LibError};
use oxvba_runtime::object_ref::ObjectRef;
use oxvba_runtime::safe_array::{SafeArray, SafeArrayBound};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{Variant, pointer_helpers};

use arith::CmpOp;

/// A VBA run-time error surfaced to the embedder (uncaught by any handler).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    pub code: i32,
    pub message: String,
}

/// An in-flight raised error (caught by `On Error` if a handler is active).
#[derive(Debug, Clone)]
struct Fault {
    code: i32,
    message: String,
}

impl Fault {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
    fn from_string(message: String) -> Self {
        Fault::new(13, message) // Type mismatch
    }
    fn from_lib(err: LibError) -> Self {
        Fault { code: err.code, message: err.message }
    }
    fn from_hal(err: oxvba_hal::HalError) -> Self {
        Fault::new(5, format!("{err:?}"))
    }
}

/// `On Error` handler state for the current procedure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorMode {
    None,
    ResumeNext,
    Goto(usize),
}

/// The `Err` object (number/description/source).
#[derive(Debug, Clone, Default)]
struct ErrObject {
    number: i32,
    description: String,
    source: String,
}

/// One activation record for a `CallProc`.
struct CallRecord {
    return_pc: usize,
    dst: Option<usize>,
    base: usize,
    len: usize,
    return_slot: Option<usize>,
    /// `(caller_slot, parameter_slot)` pairs for `ByRef` copy-out.
    byrefs: Vec<(usize, usize)>,
    /// The callee frame range as it was before the call (restored on return).
    snapshot: Vec<Variant>,
    saved_error_mode: ErrorMode,
    saved_resume_pc: usize,
}

/// `For Each` enumerator state, keyed by the iterator slot.
struct ForEachState {
    elements: Vec<Variant>,
    position: usize,
}

/// The interpreter.
pub struct Vm<'h> {
    bundle: &'h Bundle,
    host: &'h dyn HostServices,
    slots: Vec<Variant>,
    pc: usize,
    next_pc: usize,
    halted: bool,
    call_stack: Vec<CallRecord>,
    error_mode: ErrorMode,
    /// pc of the statement being protected when the active handler fired.
    resume_pc: usize,
    err: ErrObject,
    lib: LibContext,
    for_each: HashMap<usize, ForEachState>,
    withevents: HashMap<i64, Variant>,
    withevents_iters: Vec<(Vec<ObjectRef>, usize)>,
}

/// Run a bundle from its entry point, returning the VM (for slot inspection).
pub fn run<'h>(bundle: &'h Bundle, host: &'h dyn HostServices) -> Result<Vm<'h>, VmError> {
    let mut vm = Vm::new(bundle, host);
    vm.run()?;
    Ok(vm)
}

impl<'h> Vm<'h> {
    pub fn new(bundle: &'h Bundle, host: &'h dyn HostServices) -> Self {
        Self {
            bundle,
            host,
            slots: vec![Variant::empty(); bundle.slot_count],
            pc: bundle.entry_pc,
            next_pc: bundle.entry_pc,
            halted: false,
            call_stack: Vec::new(),
            error_mode: ErrorMode::None,
            resume_pc: bundle.entry_pc,
            err: ErrObject::default(),
            lib: LibContext::default(),
            for_each: HashMap::new(),
            withevents: HashMap::new(),
            withevents_iters: Vec::new(),
        }
    }

    /// Read a slot's value (immutable view of the current state).
    pub fn slot(&self, index: usize) -> Option<&Variant> {
        self.slots.get(index)
    }

    /// Drive the instruction stream until `Halt`, a `Return` from the top
    /// frame, the end of the ops, or an uncaught error.
    pub fn run(&mut self) -> Result<(), VmError> {
        while !self.halted && self.pc < self.bundle.ops.len() {
            let op = self.bundle.ops[self.pc].clone();
            self.next_pc = self.pc + 1;
            match self.exec(&op) {
                Ok(()) => self.pc = self.next_pc,
                Err(fault) => self.dispatch_fault(fault)?,
            }
        }
        Ok(())
    }

    // ── Slot access ──────────────────────────────────────────────────────────
    fn get(&self, index: usize) -> Result<&Variant, Fault> {
        self.slots
            .get(index)
            .ok_or_else(|| Fault::new(9, format!("slot {index} out of range")))
    }
    fn cloned(&self, index: usize) -> Result<Variant, Fault> {
        self.get(index).cloned()
    }
    fn set(&mut self, index: usize, value: Variant) -> Result<(), Fault> {
        match self.slots.get_mut(index) {
            Some(target) => {
                *target = value;
                Ok(())
            }
            None => Err(Fault::new(9, format!("slot {index} out of range"))),
        }
    }

    // ── Error handling ─────────────────────────────────────────────────────────
    fn set_err(&mut self, code: i32, message: &str) {
        self.err = ErrObject {
            number: code,
            description: message.to_string(),
            source: String::new(),
        };
    }

    /// Route a raised fault: into `Resume Next`/handler-goto if a handler is
    /// active, otherwise unwind the call stack looking for one; an uncaught
    /// fault becomes a [`VmError`] out of [`run`].
    fn dispatch_fault(&mut self, fault: Fault) -> Result<(), VmError> {
        self.set_err(fault.code, &fault.message);
        let mut errored_pc = self.pc;
        loop {
            match self.error_mode {
                ErrorMode::ResumeNext => {
                    self.resume_pc = errored_pc;
                    self.pc = errored_pc + 1;
                    return Ok(());
                }
                ErrorMode::Goto(handler) => {
                    self.resume_pc = errored_pc;
                    self.pc = handler;
                    return Ok(());
                }
                ErrorMode::None => match self.call_stack.pop() {
                    Some(rec) => {
                        // Unwind one frame; the error now "occurs" at the call site.
                        errored_pc = rec.return_pc.saturating_sub(1);
                        self.unwind_frame(rec);
                    }
                    None => {
                        return Err(VmError { code: fault.code, message: fault.message });
                    }
                },
            }
        }
    }

    /// Restore the caller's frame range and error scope after an error unwind
    /// (no return value, no `ByRef` copy-out).
    fn unwind_frame(&mut self, rec: CallRecord) {
        self.slots[rec.base..rec.base + rec.len].clone_from_slice(&rec.snapshot);
        self.error_mode = rec.saved_error_mode;
        self.resume_pc = rec.saved_resume_pc;
    }

    // ── Procedure calls ──────────────────────────────────────────────────────
    fn call_proc(&mut self, proc: usize, dst: Option<usize>, args: &[ProcArg]) -> Result<(), Fault> {
        let desc = self
            .bundle
            .procedures
            .get(proc)
            .ok_or_else(|| Fault::new(5, format!("unknown procedure {proc}")))?;
        let (base, len, return_slot, entry) =
            (desc.frame_base, desc.frame_slots, desc.return_slot, desc.entry_pc);

        // Evaluate argument values first (so ByRef sources are read before any
        // parameter slot is overwritten — important when caller == callee).
        let mut values = Vec::with_capacity(args.len());
        let mut byrefs = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            match arg {
                ProcArg::ByVal(s) => values.push(self.cloned(*s)?),
                ProcArg::ByRef(s) => {
                    values.push(self.cloned(*s)?);
                    byrefs.push((*s, base + i));
                }
                ProcArg::Omitted => values.push(Variant::from_error_code(0x000A_9C04u32 as i32)),
            }
        }

        if base + len > self.slots.len() {
            return Err(Fault::new(9, "procedure frame exceeds slot file"));
        }
        let snapshot = self.slots[base..base + len].to_vec();
        for (i, value) in values.into_iter().enumerate() {
            self.slots[base + i] = value;
        }

        self.call_stack.push(CallRecord {
            return_pc: self.pc + 1,
            dst,
            base,
            len,
            return_slot,
            byrefs,
            snapshot,
            saved_error_mode: self.error_mode,
            saved_resume_pc: self.resume_pc,
        });
        self.error_mode = ErrorMode::None; // each procedure starts with no handler
        self.next_pc = entry;
        Ok(())
    }

    fn ret(&mut self) -> Result<(), Fault> {
        let Some(rec) = self.call_stack.pop() else {
            // Return from the top-level frame ends execution.
            self.halted = true;
            return Ok(());
        };
        let return_value = rec.return_slot.map(|rs| self.slots[rs].clone());
        let writebacks: Vec<(usize, Variant)> = rec
            .byrefs
            .iter()
            .map(|(caller, param)| (*caller, self.slots[*param].clone()))
            .collect();

        self.slots[rec.base..rec.base + rec.len].clone_from_slice(&rec.snapshot);
        for (caller, value) in writebacks {
            self.set(caller, value)?;
        }
        if let (Some(dst), Some(value)) = (rec.dst, return_value) {
            self.set(dst, value)?;
        }
        self.error_mode = rec.saved_error_mode;
        self.resume_pc = rec.saved_resume_pc;
        self.next_pc = rec.return_pc;
        Ok(())
    }

    // ── Native dispatch ──────────────────────────────────────────────────────
    fn native_args(&self, args: &[CallArg]) -> Result<Vec<Variant>, Fault> {
        args.iter()
            .map(|a| match a {
                CallArg::Slot(s) => self.cloned(*s),
                CallArg::Named { slot, .. } => self.cloned(*slot),
                CallArg::Omitted => Ok(Variant::empty()),
            })
            .collect()
    }

    fn arg_object(&self, arg: Option<&CallArg>) -> Result<ObjectRef, Fault> {
        let slot = match arg {
            Some(CallArg::Slot(s)) | Some(CallArg::Named { slot: s, .. }) => *s,
            _ => return Err(Fault::new(424, "Object required")),
        };
        variant_to_object(self.get(slot)?)
    }

    fn com_dispatch(
        &mut self,
        selector: &ComMemberSelector,
        kind_hint: Option<ProjectMemberKind>,
        args: &[CallArg],
    ) -> Result<Variant, Fault> {
        let object = self.arg_object(args.first())?;
        let member = match selector {
            ComMemberSelector::DispatchId(id) => DynamicMemberSelector::Token(*id),
            ComMemberSelector::Name(name) => DynamicMemberSelector::Name(name.clone()),
        };
        let mut call_args = Vec::new();
        for arg in args.iter().skip(1) {
            let (value, name) = match arg {
                CallArg::Slot(s) => (Some(self.cloned(*s)?), None),
                CallArg::Named { name, slot } => (Some(self.cloned(*slot)?), Some(name.clone())),
                CallArg::Omitted => (None, None),
            };
            call_args.push(DynamicCallArg {
                value: value.map(DynamicValue::from_variant),
                name,
            });
        }
        let request = DynamicCallRequest {
            object,
            member,
            args: call_args,
            call_kind_hint: kind_hint.map(member_kind_to_dynamic),
        };
        self.host
            .com()
            .dispatch_invoke_dynamic_variant(&request)
            .map_err(Fault::from_hal)
    }

    fn declare_call(&mut self, descriptor_id: u32, args: &[CallArg]) -> Result<Variant, Fault> {
        let descriptor = self
            .bundle
            .external_call(descriptor_id)
            .ok_or_else(|| Fault::new(5, format!("unknown Declare descriptor {descriptor_id}")))?
            .clone();
        let arg_variants = self.native_args(args)?;

        let param_type_strings: Vec<String> =
            descriptor.param_types.iter().map(|pt| format!("{pt:?}")).collect();
        let view = DynLinkDescriptorView {
            descriptor_id: descriptor.descriptor_id,
            declared_name: &descriptor.declared_name,
            library: &descriptor.library,
            alias: &descriptor.alias,
            ordinal_alias: descriptor.ordinal_alias,
            symbol: descriptor.symbol,
            marshal_lane: &descriptor.marshal_lane,
            calling_convention: &descriptor.calling_convention,
            selection_policy: &descriptor.selection_policy,
            param_count: descriptor.param_count,
            param_types: &param_type_strings,
            param_by_ref: &descriptor.param_by_ref,
            return_type: descriptor.return_type.as_ref().map(|rt| Cow::Owned(format!("{rt:?}"))),
        };
        let (ret, wb_values) = self
            .host
            .dynlink()
            .invoke_descriptor_variants(&view, &arg_variants)
            .map_err(Fault::from_hal)?;
        self.apply_writebacks(&descriptor.writebacks, &arg_variants, &wb_values)?;
        Ok(ret)
    }

    fn apply_writebacks(
        &mut self,
        writebacks: &[ExternalCallWriteback],
        arg_values: &[Variant],
        wb_values: &[Variant],
    ) -> Result<(), Fault> {
        for wb in writebacks {
            let value = match wb.kind {
                ExternalCallWritebackKind::ByRefValue => match wb_values.get(wb.arg_index) {
                    Some(v) => v.clone(),
                    None => continue,
                },
                ExternalCallWritebackKind::PointerByteArrayPayload => {
                    let ptr = arg_values
                        .get(wb.arg_index)
                        .and_then(Variant::as_i64)
                        .ok_or_else(|| Fault::new(5, "pointer writeback arg is not a LongPtr"))?;
                    pointer_helpers::read_back_byte_array_payload_variant(ptr)
                        .map_err(Fault::from_string)?
                }
                ExternalCallWritebackKind::PointerStringPayload => {
                    let ptr = arg_values
                        .get(wb.arg_index)
                        .and_then(Variant::as_i64)
                        .ok_or_else(|| Fault::new(5, "pointer writeback arg is not a LongPtr"))?;
                    pointer_helpers::read_back_string_payload_variant(ptr)
                        .map_err(Fault::from_string)?
                }
            };
            self.set(wb.source_slot, value)?;
        }
        Ok(())
    }

    // ── Arrays ───────────────────────────────────────────────────────────────
    fn build_bounds(
        &self,
        upper_bounds: &[usize],
        lower_bounds: &[i32],
    ) -> Result<Vec<SafeArrayBound>, Fault> {
        let mut bounds = Vec::with_capacity(upper_bounds.len());
        for (i, upper_slot) in upper_bounds.iter().enumerate() {
            let lower = lower_bounds.get(i).copied().unwrap_or(0);
            let upper = arith::int(self.get(*upper_slot)?).map_err(Fault::from_string)? as i32;
            if upper < lower {
                return Err(Fault::new(9, "array upper bound below lower bound"));
            }
            let count = (i64::from(upper) - i64::from(lower) + 1) as u32;
            bounds.push(SafeArrayBound { count, lower });
        }
        Ok(bounds)
    }

    /// Flat element index from VBA (absolute) indices, C-order (first dimension
    /// outermost). Bounds-checked → error 9 on a subscript out of range.
    fn flat_index(&self, indices: &[usize], bounds: &[SafeArrayBound]) -> Result<usize, Fault> {
        if indices.len() != bounds.len() {
            return Err(Fault::new(9, "wrong number of array subscripts"));
        }
        let mut flat = 0usize;
        for (i, index_slot) in indices.iter().enumerate() {
            let raw = arith::int(self.get(*index_slot)?).map_err(Fault::from_string)? as i32;
            let bound = &bounds[i];
            let offset = i64::from(raw) - i64::from(bound.lower);
            if offset < 0 || offset >= i64::from(bound.count) {
                return Err(Fault::new(9, "subscript out of range"));
            }
            flat = flat * bound.count as usize + offset as usize;
        }
        Ok(flat)
    }

    fn array_of(&self, slot: usize) -> Result<SafeArray, Fault> {
        self.get(slot)?
            .as_safearray()
            .ok_or_else(|| Fault::new(13, "expected an array"))
    }

    // ── WithEvents ───────────────────────────────────────────────────────────
    fn withevents_key(owner: &ObjectRef, binding: i64) -> i64 {
        (i64::from(owner.raw()) << 32) | (binding & 0xFFFF_FFFF)
    }
    fn withevents_owner(key: i64) -> ObjectRef {
        ObjectRef::from_compat_identity((key >> 32) as i32)
    }
    fn withevents_binding(key: i64) -> i64 {
        key & 0xFFFF_FFFF
    }

    // ── The instruction dispatch ───────────────────────────────────────────────
    fn exec(&mut self, op: &Op) -> Result<(), Fault> {
        match op {
            // ── Loads ──
            Op::LoadI32 { slot, value } => self.set(*slot, Variant::from_i32(*value))?,
            Op::LoadI64 { slot, value } => self.set(*slot, Variant::from_i64(*value))?,
            Op::LoadBool { slot, value } => self.set(*slot, Variant::from_bool(*value))?,
            Op::LoadString { slot, value } => self.set(*slot, Variant::from_string(value.clone()))?,
            Op::LoadF64 { slot, bits } => self.set(*slot, Variant::from_f64(f64::from_bits(*bits)))?,
            Op::LoadF32 { slot, bits } => self.set(*slot, Variant::from_f32(f32::from_bits(*bits)))?,
            Op::LoadCurrency { slot, scaled } => {
                self.set(*slot, Variant::from_currency_scaled_i64(*scaled))?
            }
            Op::LoadDate { slot, bits } => {
                self.set(*slot, Variant::from_date_f64(f64::from_bits(*bits)))?
            }
            Op::LoadNull { slot } => self.set(*slot, Variant::null())?,
            Op::LoadEmpty { slot } => self.set(*slot, Variant::empty())?,
            Op::LoadProjectObjectRef { dst, handle } => self.set(
                *dst,
                Variant::from_object_ref(ObjectRef::from_compat_identity(*handle as i32)),
            )?,
            Op::LoadErrNumber { slot } => self.set(*slot, Variant::from_i32(self.err.number))?,
            Op::LoadErrDescription { slot } => {
                self.set(*slot, Variant::from_string(self.err.description.clone()))?
            }
            Op::LoadErrSource { slot } => {
                self.set(*slot, Variant::from_string(self.err.source.clone()))?
            }

            // ── Arithmetic ──
            Op::AddConstI32 { slot, value } => {
                let v = arith::add(self.get(*slot)?, &Variant::from_i32(*value))
                    .map_err(Fault::from_string)?;
                self.set(*slot, v)?;
            }
            Op::SubConstI32 { slot, value } => {
                let v = arith::sub(self.get(*slot)?, &Variant::from_i32(*value))
                    .map_err(Fault::from_string)?;
                self.set(*slot, v)?;
            }
            Op::IncSlot { slot } => {
                let v = arith::add(self.get(*slot)?, &Variant::from_i32(1))
                    .map_err(Fault::from_string)?;
                self.set(*slot, v)?;
            }
            Op::Add { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::add)?,
            Op::Sub { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::sub)?,
            Op::Mul { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::mul)?,
            Op::Div { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::div)?,
            Op::IntDiv { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::int_div)?,
            Op::Mod { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::modulo)?,
            Op::Pow { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::pow)?,
            Op::Concat { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::concat)?,
            Op::Neg { dst, src } => {
                let v = arith::neg(self.get(*src)?).map_err(Fault::from_string)?;
                self.set(*dst, v)?;
            }
            Op::Copy { dst, src } => {
                let v = self.cloned(*src)?;
                self.set(*dst, v)?;
            }

            // ── Coercion ──
            Op::CoerceNumeric { slot, target } => {
                let v = arith::coerce_numeric(self.get(*slot)?, *target)
                    .map_err(Fault::from_string)?;
                self.set(*slot, v)?;
            }
            Op::CoerceFixedString { slot, len } => {
                let v = arith::coerce_fixed_string(self.get(*slot)?, *len);
                self.set(*slot, v)?;
            }
            Op::ValidateAssignment { src, intent, target_kind, target_name, .. } => {
                self.validate_assignment(*src, *intent, *target_kind, target_name)?;
            }

            // ── Comparison ──
            Op::CmpEq { dst, lhs, rhs, mode } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Eq)?,
            Op::CmpNe { dst, lhs, rhs, mode } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Ne)?,
            Op::CmpLt { dst, lhs, rhs, mode } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Lt)?,
            Op::CmpLe { dst, lhs, rhs, mode } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Le)?,
            Op::CmpGt { dst, lhs, rhs, mode } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Gt)?,
            Op::CmpGe { dst, lhs, rhs, mode } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Ge)?,
            Op::CmpObjectIs { dst, lhs, rhs } => {
                let a = object_identity(self.get(*lhs)?);
                let b = object_identity(self.get(*rhs)?);
                self.set(*dst, Variant::from_bool(a == b))?;
            }

            // ── Boolean ──
            Op::Not { dst, src } => {
                let v = arith::not(self.get(*src)?).map_err(Fault::from_string)?;
                self.set(*dst, v)?;
            }
            Op::And { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::and)?,
            Op::Or { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::or)?,

            // ── Control flow ──
            Op::Jump { target_pc } => self.next_pc = *target_pc,
            Op::JumpIfZero { cond_slot, target_pc } => {
                if !arith::is_truthy(self.get(*cond_slot)?).map_err(Fault::from_string)? {
                    self.next_pc = *target_pc;
                }
            }
            Op::CallProc { proc, dst, args, .. } => self.call_proc(*proc, *dst, args)?,
            Op::CallNative { dst, callee, args } => {
                let value = match callee {
                    NativeCallee::Builtin(id) => {
                        let argv = self.native_args(args)?;
                        oxvba_lib::invoke(*id, &argv, self.host, &mut self.lib)
                            .map_err(Fault::from_lib)?
                    }
                    NativeCallee::ComDispatch { selector, kind_hint, .. } => {
                        self.com_dispatch(selector, *kind_hint, args)?
                    }
                    NativeCallee::Declare { descriptor_id } => {
                        self.declare_call(*descriptor_id, args)?
                    }
                };
                if let Some(dst) = dst {
                    self.set(*dst, value)?;
                }
            }
            Op::Return => self.ret()?,
            Op::Halt => self.halted = true,

            // ── Error state ──
            Op::SetOnErrorResumeNext => self.error_mode = ErrorMode::ResumeNext,
            Op::SetOnErrorGoto0 => self.error_mode = ErrorMode::None,
            Op::SetOnErrorGotoLabel { target_pc } => self.error_mode = ErrorMode::Goto(*target_pc),
            Op::ResumeNext => self.next_pc = self.resume_pc + 1,
            Op::Resume => self.next_pc = self.resume_pc,
            Op::ResumeLabel { target_pc } => self.next_pc = *target_pc,
            Op::RaiseError { code } => {
                return Err(Fault::new(*code, default_error_message(*code)));
            }
            Op::ClearErr => self.err = ErrObject::default(),

            // ── Arrays / aggregates ──
            Op::ArrayLiteral { dst, values } => {
                let elems = values
                    .iter()
                    .map(|s| self.cloned(*s))
                    .collect::<Result<Vec<_>, _>>()?;
                self.set(*dst, Variant::from_safearray(SafeArray::from_variants(elems)))?;
            }
            Op::ArrayAppend { dst, array, item } => {
                let mut elems = match self.get(*array)?.as_safearray() {
                    Some(arr) => arr.variant_elements().unwrap_or_default(),
                    None => Vec::new(),
                };
                elems.push(self.cloned(*item)?);
                self.set(*dst, Variant::from_safearray(SafeArray::from_variants(elems)))?;
            }
            Op::ArrayResize { dst, upper_bounds, lower_bounds, .. } => {
                let bounds = self.build_bounds(upper_bounds, lower_bounds)?;
                let array = SafeArray::from_shape(bounds).map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_safearray(array))?;
            }
            Op::ArrayResizePreserve { dst, upper_bounds, lower_bounds, .. } => {
                let old = self
                    .get(*dst)?
                    .as_safearray()
                    .and_then(|a| a.variant_elements())
                    .unwrap_or_default();
                let bounds = self.build_bounds(upper_bounds, lower_bounds)?;
                let array = SafeArray::from_shape(bounds).map_err(Fault::from_string)?;
                let mut elems = array.variant_elements().unwrap_or_default();
                for (i, value) in old.into_iter().enumerate() {
                    if i < elems.len() {
                        elems[i] = value;
                    }
                }
                let resized = array.replace_variant_elements(elems).map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_safearray(resized))?;
            }
            Op::ArrayGet { dst, array, indices } => {
                let arr = self.array_of(*array)?;
                let bounds = arr.bounds().ok_or_else(|| Fault::new(9, "array has no bounds"))?;
                let flat = self.flat_index(indices, &bounds)?;
                let elems = arr.variant_elements().unwrap_or_default();
                let value = elems
                    .get(flat)
                    .cloned()
                    .ok_or_else(|| Fault::new(9, "subscript out of range"))?;
                self.set(*dst, value)?;
            }
            Op::ArraySet { array, indices, src } => {
                let arr = self.array_of(*array)?;
                let bounds = arr.bounds().ok_or_else(|| Fault::new(9, "array has no bounds"))?;
                let flat = self.flat_index(indices, &bounds)?;
                let mut elems = arr.variant_elements().unwrap_or_default();
                if flat >= elems.len() {
                    return Err(Fault::new(9, "subscript out of range"));
                }
                elems[flat] = self.cloned(*src)?;
                let updated = arr.replace_variant_elements(elems).map_err(Fault::from_string)?;
                self.set(*array, Variant::from_safearray(updated))?;
            }
            Op::LBound { dst, src } => {
                let arr = self.array_of(*src)?;
                let bounds = arr.bounds().ok_or_else(|| Fault::new(9, "array has no bounds"))?;
                let lower = bounds.first().map(|b| b.lower).unwrap_or(0);
                self.set(*dst, Variant::from_i32(lower))?;
            }
            Op::UBound { dst, src } => {
                let arr = self.array_of(*src)?;
                let bounds = arr.bounds().ok_or_else(|| Fault::new(9, "array has no bounds"))?;
                let upper = bounds
                    .first()
                    .map(|b| b.lower + b.count as i32 - 1)
                    .unwrap_or(-1);
                self.set(*dst, Variant::from_i32(upper))?;
            }
            Op::ForEachInit { iter, src } => {
                let elements = match self.get(*src)?.as_safearray() {
                    Some(arr) => arr.variant_elements().unwrap_or_default(),
                    None => Vec::new(),
                };
                self.for_each.insert(*iter, ForEachState { elements, position: 0 });
            }
            Op::ForEachNext { iter, item, has_value } => {
                let next = self.for_each.get_mut(iter).and_then(|state| {
                    let value = state.elements.get(state.position).cloned();
                    if value.is_some() {
                        state.position += 1;
                    }
                    value
                });
                match next {
                    Some(value) => {
                        self.set(*item, value)?;
                        self.set(*has_value, Variant::from_bool(true))?;
                    }
                    None => self.set(*has_value, Variant::from_bool(false))?,
                }
            }

            // ── Objects / WithEvents / type identity ──
            Op::WithEventsGet { dst, owner, binding } => {
                let key = self.withevents_lookup_key(*owner, *binding)?;
                let value = self
                    .withevents
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| Variant::from_i32(0));
                self.set(*dst, value)?;
            }
            Op::WithEventsSet { dst, owner, binding, value } => {
                let key = self.withevents_lookup_key(*owner, *binding)?;
                let v = self.cloned(*value)?;
                if is_nothing(&v) {
                    self.withevents.remove(&key);
                } else {
                    self.withevents.insert(key, v.clone());
                }
                self.set(*dst, v)?;
            }
            Op::WithEventsClearOwner { dst, owner } => {
                let owner_ref = variant_to_object(self.get(*owner)?)?;
                self.withevents
                    .retain(|key, _| Self::withevents_owner(*key).raw() != owner_ref.raw());
                self.set(*dst, Variant::from_i32(0))?;
            }
            Op::WithEventsFirstOwner { dst, source, binding } => {
                let source = self.cloned(*source)?;
                let binding = arith::int(self.get(*binding)?).map_err(Fault::from_string)?;
                let mut owners: Vec<ObjectRef> = Vec::new();
                if !is_nothing(&source) {
                    for (key, value) in &self.withevents {
                        if Self::withevents_binding(*key) == (binding & 0xFFFF_FFFF)
                            && object_identity(value) == object_identity(&source)
                        {
                            owners.push(Self::withevents_owner(*key));
                        }
                    }
                }
                owners.sort_unstable_by_key(ObjectRef::raw);
                match owners.first().cloned() {
                    Some(first) => {
                        self.withevents_iters.push((owners, 1));
                        self.set(*dst, Variant::from_object_ref(first))?;
                    }
                    None => self.set(*dst, Variant::from_i32(0))?,
                }
            }
            Op::WithEventsNextOwner { dst } => {
                let next = self.withevents_iters.last_mut().and_then(|(owners, pos)| {
                    let value = owners.get(*pos).cloned();
                    if value.is_some() {
                        *pos += 1;
                    }
                    value
                });
                match next {
                    Some(owner) => self.set(*dst, Variant::from_object_ref(owner))?,
                    None => {
                        self.withevents_iters.pop();
                        self.set(*dst, Variant::from_i32(0))?;
                    }
                }
            }
            Op::TypeOfIs { dst, object_slot, type_name } => {
                let matches = self.type_of_is(*object_slot, type_name)?;
                self.set(*dst, Variant::from_bool(matches))?;
            }

            // ── Pointer helpers ──
            Op::PtrStr { dst, src } => {
                let p = pointer_helpers::register_utf16_string(&arith::as_string(self.get(*src)?))
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
            Op::PtrVar { dst, src } => {
                let p = pointer_helpers::register_variant_pointer(self.get(*src)?)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
            Op::PtrVarString { dst, src } => {
                let p = pointer_helpers::register_string_variant_pointer(self.get(*src)?)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
            Op::PtrVarVariant { dst, src } => {
                let p = pointer_helpers::register_variant_var_variant_pointer(self.get(*src)?)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
            Op::PtrObj { dst, src } => {
                let p = pointer_helpers::register_object_variant_pointer(self.get(*src)?)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
        }
        Ok(())
    }

    // ── Small op helpers ───────────────────────────────────────────────────────
    fn binop(
        &mut self,
        dst: usize,
        lhs: usize,
        rhs: usize,
        f: impl Fn(&Variant, &Variant) -> Result<Variant, String>,
    ) -> Result<(), Fault> {
        let value = f(self.get(lhs)?, self.get(rhs)?).map_err(Fault::from_string)?;
        self.set(dst, value)
    }

    fn cmp(
        &mut self,
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: oxvba_bundle::StringCompareMode,
        op: CmpOp,
    ) -> Result<(), Fault> {
        let value =
            arith::compare(self.get(lhs)?, self.get(rhs)?, mode, op).map_err(Fault::from_string)?;
        self.set(dst, value)
    }

    fn validate_assignment(
        &mut self,
        src: usize,
        intent: oxvba_bundle::AssignmentIntent,
        target_kind: oxvba_bundle::AssignmentTargetKind,
        target_name: &str,
    ) -> Result<(), Fault> {
        use oxvba_bundle::{AssignmentIntent as Intent, AssignmentTargetKind as Kind};
        let value = self.get(src)?;
        let is_object = matches!(value.vtype(), VarType::Object) || is_nothing(value);
        match intent {
            Intent::Set if !is_object => {
                Err(Fault::new(424, format!("Object required: {target_name}")))
            }
            Intent::Let if target_kind == Kind::Object && value.vtype() == VarType::Object => {
                // Assigning an object with Let to an object target requires Set.
                Err(Fault::new(91, format!("Object variable requires Set: {target_name}")))
            }
            _ => Ok(()),
        }
    }

    fn withevents_lookup_key(&self, owner: usize, binding: usize) -> Result<i64, Fault> {
        let owner_ref = variant_to_object(self.get(owner)?)?;
        let binding = arith::int(self.get(binding)?).map_err(Fault::from_string)?;
        Ok(Self::withevents_key(&owner_ref, binding))
    }

    fn type_of_is(&self, object_slot: usize, type_name: &str) -> Result<bool, Fault> {
        let object = variant_to_object(self.get(object_slot)?)?;
        match self.host.com().describe_object(object) {
            Ok(Some(descriptor)) => Ok(descriptor.prog_id_name.eq_ignore_ascii_case(type_name)),
            _ => Ok(false),
        }
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

fn member_kind_to_dynamic(kind: ProjectMemberKind) -> DynamicCallKind {
    match kind {
        ProjectMemberKind::Method => DynamicCallKind::Method,
        ProjectMemberKind::PropertyGet => DynamicCallKind::PropertyGet,
        ProjectMemberKind::PropertyLet => DynamicCallKind::PropertyLet,
        ProjectMemberKind::PropertySet => DynamicCallKind::PropertySet,
    }
}

/// A Variant carrying a COM/project object handle: an object ref, or an `i32`/
/// `i64` handle (`semantics::variant_to_com_object` analog).
fn variant_to_object(value: &Variant) -> Result<ObjectRef, Fault> {
    if let Some(object) = value.as_object_ref() {
        return Ok(object);
    }
    if let Some(raw) = value.as_i32() {
        return Ok(ObjectRef::from_compat_identity(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ObjectRef::from_compat_identity)
            .map_err(|_| Fault::new(13, "object handle exceeds i32 range"));
    }
    Err(Fault::new(424, "Object required"))
}

/// Object-identity key for `Is` comparison: the object raw, or 0 for Nothing/
/// non-object values.
fn object_identity(value: &Variant) -> i32 {
    value.as_object_ref().map(|o| o.raw()).unwrap_or(0)
}

fn is_nothing(value: &Variant) -> bool {
    match value.vtype() {
        VarType::Object => value.as_object_ref().map(|o| o.raw()).unwrap_or(0) == 0,
        VarType::Empty | VarType::Null => true,
        _ => value.as_i32() == Some(0),
    }
}

fn default_error_message(code: i32) -> String {
    match code {
        6 => "Overflow".to_string(),
        9 => "Subscript out of range".to_string(),
        11 => "Division by zero".to_string(),
        13 => "Type mismatch".to_string(),
        91 => "Object variable or With block variable not set".to_string(),
        424 => "Object required".to_string(),
        _ => "Application-defined or object-defined error".to_string(),
    }
}

#[cfg(test)]
mod tests;
