//! Linearize the [`crate::coreir`] tree to the flat [`Bundle`] machine.
//!
//! This is the structured→flat pass: it allocates slots (module globals in
//! `[0, global_count)`; each procedure's params, then locals, then temporaries
//! frame-relative as `global_count + index`), sequences nested values through
//! temporaries, and turns structured control flow + labels into jumps via
//! back-patching. It is the production replacement for the legacy
//! `BoundModule` → `emit` path. A valid, resolved `CoreProgram` always
//! linearizes; [`LinearizeError`] exists for defensiveness.

use std::collections::HashMap;

use crate::coreir::*;
use crate::isa::{CallArg, DeclarePtrWriteback, NativeCallee, Op, ProcArg};
use crate::{
    AssignmentIntent, AssignmentTargetKind, Bundle, ClassDescriptor, ClassMethod, ComMemberSelector,
    NumericMode, ProcedureDescriptor, StringCompareMode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinearizeError {
    /// The Core IR violated an invariant a resolved program should uphold.
    Malformed(String),
}

impl std::fmt::Display for LinearizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinearizeError::Malformed(m) => write!(f, "malformed Core IR: {m}"),
        }
    }
}

type Res<T> = Result<T, LinearizeError>;

/// Lowered arguments (`A` = `ProcArg`/`CallArg`) plus the `(place, temp-slot)`
/// write-backs to replay after the call — for ByRef Field/Index/WithEvents args
/// that have no slot to alias directly (copy-in temp, copy back on return).
type LoweredArgs<A> = (Vec<A>, Vec<(CorePlace, usize)>);

/// Flatten a resolved [`CoreProgram`] into a runnable [`Bundle`].
pub fn linearize(program: &CoreProgram) -> Res<Bundle> {
    Linearizer::new(program).run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopKind {
    Do,
    For,
}

struct Linearizer<'p> {
    program: &'p CoreProgram,
    ops: Vec<Op>,
    statement_starts: Vec<usize>,
    global_count: usize,
    // ── Per-procedure scratch (reset in `begin_proc`) ──
    local_slot: Vec<usize>,
    next_temp: usize,
    frame_slots: usize,
    label_pc: HashMap<LabelId, usize>,
    pending_label_patches: Vec<(usize, LabelId)>,
    loop_exits: Vec<(LoopKind, Vec<usize>)>,
    proc_exits: Vec<usize>,
    gosub_ret_slot: Option<usize>,
    gosub_return_pcs: Vec<usize>,
    gosub_return_jumps: Vec<usize>,
}

impl<'p> Linearizer<'p> {
    fn new(program: &'p CoreProgram) -> Self {
        Self {
            program,
            ops: Vec::new(),
            statement_starts: Vec::new(),
            global_count: program.globals.len(),
            local_slot: Vec::new(),
            next_temp: 0,
            frame_slots: 0,
            label_pc: HashMap::new(),
            pending_label_patches: Vec::new(),
            loop_exits: Vec::new(),
            proc_exits: Vec::new(),
            gosub_ret_slot: None,
            gosub_return_pcs: Vec::new(),
            gosub_return_jumps: Vec::new(),
        }
    }

    fn run(mut self) -> Res<Bundle> {
        let mut procedures = Vec::with_capacity(self.program.procs.len());
        for proc in &self.program.procs {
            let entry_pc = self.here();
            self.begin_proc(proc);
            for stmt in &proc.body {
                self.lower_stmt(stmt)?;
            }
            // Epilogue: the normal end + the `Exit Sub/Function` target.
            let epilogue = self.here();
            let exits = std::mem::take(&mut self.proc_exits);
            for at in exits {
                self.patch(at, epilogue);
            }
            let return_slot = proc.return_local.map(|id| self.local_slot[id.0] - self.global_count);
            self.ops.push(Op::Return);
            // GoSub trampoline (after the Return; reached only via `GoSubReturn`).
            self.emit_gosub_trampoline();
            // Resolve label references now that all labels in this proc are known.
            self.flush_label_patches()?;

            procedures.push(ProcedureDescriptor {
                name: proc.name.clone(),
                entry_pc,
                kind: proc.kind,
                param_count: proc.params.len(),
                frame_slots: self.frame_slots,
                return_slot,
            });
        }

        let entry_index = self.select_entry();
        let (entry_pc, entry_frame_slots) = procedures
            .get(entry_index)
            .map(|p| (p.entry_pc, p.frame_slots))
            .unwrap_or((0, 0));

        self.statement_starts.sort_unstable();
        self.statement_starts.dedup();

        let classes = self
            .program
            .classes
            .iter()
            .map(|class| ClassDescriptor {
                name: class.name.clone(),
                initialize: class.initialize.map(|p| p.0),
                terminate: class.terminate.map(|p| p.0),
                methods: class
                    .methods
                    .iter()
                    .map(|m| ClassMethod { name: m.name.clone(), kind: m.kind, proc: m.proc.0 })
                    .collect(),
                implements: class.implements.clone(),
            })
            .collect();

        Ok(Bundle {
            ops: self.ops,
            procedures,
            entry_pc,
            global_count: self.global_count,
            entry_frame_slots,
            statement_starts: self.statement_starts,
            external_calls: self.program.external_calls.clone(),
            source_map: Vec::new(),
            com_class_exports: self.program.com_class_exports.clone(),
            classes,
            event_routes: self.program.event_routes.clone(),
            unit_name: self.program.unit_name.clone(),
            exports: self.program.exports.clone(),
            imports: self.program.imports.clone(),
        })
    }

    fn select_entry(&self) -> usize {
        if let Some(entry) = self.program.entry {
            return entry.0;
        }
        self.program
            .procs
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case("main"))
            .unwrap_or(0)
    }

    // ── Per-procedure frame planning ──────────────────────────────────────────
    fn begin_proc(&mut self, proc: &CoreProc) {
        let count = proc.params.len() + proc.locals.len();
        self.local_slot = (0..count).map(|i| self.global_count + i).collect();
        self.frame_slots = count;
        self.next_temp = self.global_count + count;
        self.label_pc.clear();
        self.pending_label_patches.clear();
        self.loop_exits.clear();
        self.proc_exits.clear();
        self.gosub_ret_slot = None;
        self.gosub_return_pcs.clear();
        self.gosub_return_jumps.clear();
    }

    fn new_temp(&mut self) -> usize {
        let slot = self.next_temp;
        self.next_temp += 1;
        self.frame_slots = self.frame_slots.max(self.next_temp - self.global_count);
        slot
    }

    fn gosub_slot(&mut self) -> usize {
        if let Some(slot) = self.gosub_ret_slot {
            slot
        } else {
            let slot = self.new_temp();
            self.gosub_ret_slot = Some(slot);
            slot
        }
    }

    // ── Emit / patch ──────────────────────────────────────────────────────────
    fn emit(&mut self, op: Op) -> usize {
        let at = self.ops.len();
        self.ops.push(op);
        at
    }
    fn here(&self) -> usize {
        self.ops.len()
    }
    fn patch(&mut self, at: usize, target: usize) {
        match &mut self.ops[at] {
            Op::Jump { target_pc }
            | Op::JumpIfZero { target_pc, .. }
            | Op::SetOnErrorGotoLabel { target_pc }
            | Op::ResumeLabel { target_pc } => *target_pc = target,
            _ => {}
        }
    }

    fn flush_label_patches(&mut self) -> Res<()> {
        let patches = std::mem::take(&mut self.pending_label_patches);
        for (at, label) in patches {
            let target = *self
                .label_pc
                .get(&label)
                .ok_or_else(|| LinearizeError::Malformed(format!("unresolved label {label:?}")))?;
            self.patch(at, target);
        }
        Ok(())
    }

    fn emit_gosub_trampoline(&mut self) {
        if self.gosub_return_jumps.is_empty() && self.gosub_return_pcs.is_empty() {
            return;
        }
        let Some(ret_slot) = self.gosub_ret_slot else { return };
        let trampoline = self.here();
        // For each GoSub site k: if ret_slot == k, jump to the recorded return pc.
        for (site, return_pc) in self.gosub_return_pcs.clone().into_iter().enumerate() {
            let key = self.new_temp();
            self.emit(Op::LoadI32 { slot: key, value: site as i32 });
            let cond = self.new_temp();
            self.emit(Op::CmpEq {
                dst: cond,
                lhs: ret_slot,
                rhs: key,
                mode: StringCompareMode::Binary,
            });
            let jz = self.emit(Op::JumpIfZero { cond_slot: cond, target_pc: 0 });
            self.emit(Op::Jump { target_pc: return_pc });
            let next = self.here();
            self.patch(jz, next);
        }
        self.ops.push(Op::Return); // no matching site (unreachable in well-formed IR)
        let jumps = std::mem::take(&mut self.gosub_return_jumps);
        for at in jumps {
            self.patch(at, trampoline);
        }
    }

    // ── Slot / place resolution ─────────────────────────────────────────────────
    /// A simple (directly-addressable) variable slot. Errors for compound places.
    fn simple_slot(&self, place: &CorePlace) -> Res<usize> {
        match place {
            CorePlace::Local(id) => Ok(self.local_slot[id.0]),
            CorePlace::Global(id) => Ok(id.0),
            other => Err(LinearizeError::Malformed(format!("expected a simple variable, got {other:?}"))),
        }
    }

    /// Resolve the storage slot of an array place, plus a writeback place when
    /// the array is held indirectly (a field / element) and must be stored back
    /// after in-place mutation.
    fn array_target(&mut self, place: &CorePlace) -> Res<(usize, Option<CorePlace>)> {
        match place {
            CorePlace::Local(id) => Ok((self.local_slot[id.0], None)),
            CorePlace::Global(id) => Ok((id.0, None)),
            _ => {
                let slot = self.lower_place_load(place)?;
                Ok((slot, Some(place.clone())))
            }
        }
    }

    /// Materialize a `WithEvents` binding token into a fresh slot. vm2 reads the
    /// token from a slot (`self.get(binding)` in `WithEventsGet`/`Set`), so the
    /// coreir literal token must be loaded into a slot — it cannot be passed as
    /// the slot index directly.
    fn binding_token_slot(&mut self, token: i32) -> usize {
        let slot = self.new_temp();
        self.emit(Op::LoadI32 { slot, value: token });
        slot
    }

    fn lower_place_load(&mut self, place: &CorePlace) -> Res<usize> {
        match place {
            CorePlace::Local(id) => Ok(self.local_slot[id.0]),
            CorePlace::Global(id) => Ok(id.0),
            CorePlace::Field { object, field } => {
                let object = self.lower_value(object)?;
                let dst = self.new_temp();
                self.emit(Op::FieldGet { dst, object, field: *field });
                Ok(dst)
            }
            CorePlace::Index { array, indices } => {
                let index_slots = self.lower_values(indices)?;
                let (array_slot, _writeback) = self.array_target(array)?;
                let dst = self.new_temp();
                self.emit(Op::ArrayGet { dst, array: array_slot, indices: index_slots });
                Ok(dst)
            }
            CorePlace::RecordField { base, index } => {
                let (record, _writeback) = self.array_target(base)?;
                let dst = self.new_temp();
                self.emit(Op::RecordGet { dst, record, index: *index });
                Ok(dst)
            }
            CorePlace::WithEvents { owner, binding } => {
                let owner = self.lower_value(owner)?;
                let binding = self.binding_token_slot(*binding);
                let dst = self.new_temp();
                self.emit(Op::WithEventsGet { dst, owner, binding });
                Ok(dst)
            }
        }
    }

    fn lower_place_store(&mut self, place: &CorePlace, src: usize) -> Res<()> {
        match place {
            CorePlace::Local(id) => {
                let dst = self.local_slot[id.0];
                self.emit(Op::Copy { dst, src });
            }
            CorePlace::Global(id) => {
                self.emit(Op::Copy { dst: id.0, src });
            }
            CorePlace::Field { object, field } => {
                let object = self.lower_value(object)?;
                self.emit(Op::FieldSet { object, field: *field, src });
            }
            CorePlace::Index { array, indices } => {
                let index_slots = self.lower_values(indices)?;
                let (array_slot, writeback) = self.array_target(array)?;
                self.emit(Op::ArraySet { array: array_slot, indices: index_slots, src });
                if let Some(place) = writeback {
                    self.lower_place_store(&place, array_slot)?;
                }
            }
            CorePlace::RecordField { base, index } => {
                // Update the field in the record slot, then store the (copy-on-write)
                // record back through to its base place when held indirectly.
                let (record, writeback) = self.array_target(base)?;
                self.emit(Op::RecordSet { record, index: *index, src });
                if let Some(place) = writeback {
                    self.lower_place_store(&place, record)?;
                }
            }
            CorePlace::WithEvents { owner, binding } => {
                let owner = self.lower_value(owner)?;
                let binding = self.binding_token_slot(*binding);
                let dst = self.new_temp();
                self.emit(Op::WithEventsSet { dst, owner, binding, value: src });
            }
        }
        Ok(())
    }

    // ── Values ─────────────────────────────────────────────────────────────────
    fn lower_values(&mut self, values: &[CoreValue]) -> Res<Vec<usize>> {
        values.iter().map(|v| self.lower_value(v)).collect()
    }

    fn lower_value(&mut self, value: &CoreValue) -> Res<usize> {
        match value {
            CoreValue::Const(c) => {
                let slot = self.new_temp();
                let op = const_to_load(slot, c);
                self.emit(op);
                Ok(slot)
            }
            CoreValue::Load(place) => self.lower_place_load(place),
            CoreValue::Unary { op, expr, num } => {
                let src = self.lower_value(expr)?;
                let dst = self.new_temp();
                self.emit(match op {
                    CoreUnOp::Negate => Op::Neg { dst, src, mode: *num },
                    CoreUnOp::Not => Op::Not { dst, src },
                });
                Ok(dst)
            }
            CoreValue::Binary { op, lhs, rhs, mode, num } => {
                let l = self.lower_value(lhs)?;
                let r = self.lower_value(rhs)?;
                let dst = self.new_temp();
                self.emit_binary(*op, dst, l, r, *mode, *num);
                Ok(dst)
            }
            CoreValue::Call { callee, args } => {
                let dst = self.new_temp();
                self.lower_call(callee, args, Some(dst))?;
                Ok(dst)
            }
            CoreValue::New(class) => {
                let dst = self.new_temp();
                self.emit(Op::NewObject { dst, class: class.0 });
                Ok(dst)
            }
            CoreValue::NewRecord { fields } => {
                let dst = self.new_temp();
                self.emit(Op::NewRecord { dst, fields: *fields });
                Ok(dst)
            }
            CoreValue::NewExtern { import } => {
                let dst = self.new_temp();
                self.emit(Op::NewExtern { dst, import: *import });
                Ok(dst)
            }
            CoreValue::Coerce { value, to } => {
                let src = self.lower_value(value)?;
                match to {
                    CoerceTarget::ImplicitVariant(_) => Ok(src), // runtime-implicit; no op
                    CoerceTarget::Numeric(target) => {
                        let dst = self.new_temp();
                        self.emit(Op::Copy { dst, src });
                        self.emit(Op::CoerceNumeric { slot: dst, target: *target });
                        Ok(dst)
                    }
                    CoerceTarget::FixedString(len) => {
                        let dst = self.new_temp();
                        self.emit(Op::Copy { dst, src });
                        self.emit(Op::CoerceFixedString { slot: dst, len: *len });
                        Ok(dst)
                    }
                }
            }
            CoreValue::TypeOfIs { object, type_name } => {
                let object_slot = self.lower_value(object)?;
                let dst = self.new_temp();
                self.emit(Op::TypeOfIs { dst, object_slot, type_name: type_name.clone() });
                Ok(dst)
            }
            CoreValue::Ptr { kind, value } => {
                let src = self.lower_value(value)?;
                let dst = self.new_temp();
                self.emit(match kind {
                    PtrKind::Str => Op::PtrStr { dst, src },
                    PtrKind::Var => Op::PtrVar { dst, src },
                    PtrKind::VarString => Op::PtrVarString { dst, src },
                    PtrKind::VarVariant => Op::PtrVarVariant { dst, src },
                    PtrKind::Obj => Op::PtrObj { dst, src },
                });
                Ok(dst)
            }
            CoreValue::ErrField(field) => {
                let slot = self.new_temp();
                self.emit(match field {
                    ErrField::Number => Op::LoadErrNumber { slot },
                    ErrField::Description => Op::LoadErrDescription { slot },
                    ErrField::Source => Op::LoadErrSource { slot },
                });
                Ok(slot)
            }
            CoreValue::ArrayLiteral(values) => {
                let value_slots = self.lower_values(values)?;
                let dst = self.new_temp();
                self.emit(Op::ArrayLiteral { dst, values: value_slots });
                Ok(dst)
            }
            CoreValue::Bound { which, array } => {
                let (array_slot, _writeback) = self.array_target(array)?;
                let dst = self.new_temp();
                self.emit(match which {
                    BoundWhich::Lower => Op::LBound { dst, src: array_slot },
                    BoundWhich::Upper => Op::UBound { dst, src: array_slot },
                });
                Ok(dst)
            }
            CoreValue::AddressOf(proc) => {
                let dst = self.new_temp();
                self.emit(Op::LoadProcRef { dst, proc: proc.0 });
                Ok(dst)
            }
        }
    }

    fn emit_binary(
        &mut self,
        op: CoreBinOp,
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: StringCompareMode,
        num: NumericMode,
    ) {
        let op = match op {
            CoreBinOp::Add => Op::Add { dst, lhs, rhs, mode: num },
            CoreBinOp::Sub => Op::Sub { dst, lhs, rhs, mode: num },
            CoreBinOp::Mul => Op::Mul { dst, lhs, rhs, mode: num },
            CoreBinOp::Div => Op::Div { dst, lhs, rhs },
            CoreBinOp::IntDiv => Op::IntDiv { dst, lhs, rhs, mode: num },
            CoreBinOp::Mod => Op::Mod { dst, lhs, rhs, mode: num },
            CoreBinOp::Pow => Op::Pow { dst, lhs, rhs },
            CoreBinOp::Concat => Op::Concat { dst, lhs, rhs },
            CoreBinOp::Eq => Op::CmpEq { dst, lhs, rhs, mode },
            CoreBinOp::Ne => Op::CmpNe { dst, lhs, rhs, mode },
            CoreBinOp::Lt => Op::CmpLt { dst, lhs, rhs, mode },
            CoreBinOp::Le => Op::CmpLe { dst, lhs, rhs, mode },
            CoreBinOp::Gt => Op::CmpGt { dst, lhs, rhs, mode },
            CoreBinOp::Ge => Op::CmpGe { dst, lhs, rhs, mode },
            CoreBinOp::And => Op::And { dst, lhs, rhs },
            CoreBinOp::Or => Op::Or { dst, lhs, rhs },
            CoreBinOp::Xor => Op::Xor { dst, lhs, rhs },
            CoreBinOp::Eqv => Op::Eqv { dst, lhs, rhs },
            CoreBinOp::Imp => Op::Imp { dst, lhs, rhs },
            CoreBinOp::Is => Op::CmpObjectIs { dst, lhs, rhs },
            CoreBinOp::Like => Op::CallNative {
                dst: Some(dst),
                callee: NativeCallee::Builtin(crate::native::NativeImplId::Like),
                args: vec![
                    CallArg::Slot(lhs),
                    CallArg::Slot(rhs),
                    CallArg::Const(match mode {
                        StringCompareMode::Text => 1,
                        StringCompareMode::Binary => 0,
                    }),
                ],
            },
        };
        self.emit(op);
    }

    // ── Calls ────────────────────────────────────────────────────────────────
    fn build_proc_args(&mut self, args: &[CoreArg]) -> Res<LoweredArgs<ProcArg>> {
        let mut out = Vec::with_capacity(args.len());
        let mut writebacks = Vec::new();
        for arg in args {
            match arg {
                CoreArg::ByVal(v) => out.push(ProcArg::ByVal(self.lower_value(v)?)),
                CoreArg::ByRef(place) => match place {
                    CorePlace::Local(_) | CorePlace::Global(_) => {
                        let slot = self.lower_place_load(place)?;
                        out.push(ProcArg::ByRef(slot));
                    }
                    _ => {
                        // No slot to alias: copy in, alias the temp, copy back.
                        let tmp = self.lower_place_load(place)?;
                        out.push(ProcArg::ByRef(tmp));
                        writebacks.push((place.clone(), tmp));
                    }
                },
                CoreArg::Omitted => out.push(ProcArg::Omitted),
                // Named args reach a VBA proc only after the binder ordered them.
                CoreArg::Named { value, .. } => out.push(ProcArg::ByVal(self.lower_value(value)?)),
            }
        }
        Ok((out, writebacks))
    }

    /// Like [`Self::build_proc_args`], but for `CallNative` callees: a `ByRef` of a
    /// slot place lowers to `CallArg::ByRef(slot)` (the write-back target — a true
    /// alias for project-instance dispatch, a marshaled copy-out for COM/Declare);
    /// a `ByRef` of a Field/Index/WithEvents place copies in to a temp and records
    /// a write-back applied after the call.
    fn build_call_args(&mut self, args: &[CoreArg]) -> Res<LoweredArgs<CallArg>> {
        let mut out = Vec::with_capacity(args.len());
        let mut writebacks = Vec::new();
        for arg in args {
            match arg {
                CoreArg::ByVal(v) => out.push(CallArg::Slot(self.lower_value(v)?)),
                CoreArg::ByRef(place) => match place {
                    CorePlace::Local(_) | CorePlace::Global(_) => {
                        out.push(CallArg::ByRef(self.lower_place_load(place)?));
                    }
                    _ => {
                        let tmp = self.lower_place_load(place)?;
                        out.push(CallArg::ByRef(tmp));
                        writebacks.push((place.clone(), tmp));
                    }
                },
                CoreArg::Omitted => out.push(CallArg::Omitted),
                CoreArg::Named { name, value } => {
                    out.push(CallArg::Named { name: name.clone(), slot: self.lower_value(value)? });
                }
            }
        }
        Ok((out, writebacks))
    }

    /// Copy `ByRef` Field/Index/WithEvents temps back to their places after a call.
    fn emit_arg_writebacks(&mut self, writebacks: Vec<(CorePlace, usize)>) -> Res<()> {
        for (place, tmp) in writebacks {
            self.lower_place_store(&place, tmp)?;
        }
        Ok(())
    }

    fn lower_call(&mut self, callee: &CoreCallee, args: &[CoreArg], dst: Option<usize>) -> Res<()> {
        match callee {
            CoreCallee::VbaProc { proc } => {
                let (proc_args, writebacks) = self.build_proc_args(args)?;
                self.emit(Op::CallProc { proc: proc.0, dst, args: proc_args });
                self.emit_arg_writebacks(writebacks)?;
            }
            CoreCallee::ExternProc { import } => {
                // A cross-bundle call binds args exactly like a VbaProc; the linker
                // resolves `import` to the target `(bundle, proc)` at load time.
                let (proc_args, writebacks) = self.build_proc_args(args)?;
                self.emit(Op::CallExtern { import: *import, dst, args: proc_args });
                self.emit_arg_writebacks(writebacks)?;
            }
            CoreCallee::Native(id) => {
                let (args, writebacks) = self.build_call_args(args)?;
                self.emit(Op::CallNative { dst, callee: NativeCallee::Builtin(*id), args });
                self.emit_arg_writebacks(writebacks)?;
            }
            CoreCallee::EarlyCom { dispid, kind } => {
                let (args, writebacks) = self.build_call_args(args)?;
                self.emit(Op::CallNative {
                    dst,
                    callee: NativeCallee::ComDispatch {
                        selector: ComMemberSelector::DispatchId(*dispid),
                        early_bound: true,
                        kind_hint: *kind,
                    },
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
            CoreCallee::LateDispatch { name, kind } => {
                let (args, writebacks) = self.build_call_args(args)?;
                self.emit(Op::CallNative {
                    dst,
                    callee: NativeCallee::ComDispatch {
                        selector: ComMemberSelector::Name(name.clone()),
                        early_bound: false,
                        kind_hint: *kind,
                    },
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
            CoreCallee::Declare { descriptor_id, ptr_writebacks } => {
                let (args, writebacks) = self.build_call_args(args)?;
                // Resolve each pointer write-back target (a simple variable l-value)
                // to its slot; the VM reads the pinned buffer back into it post-call.
                let ptr_writebacks = ptr_writebacks
                    .iter()
                    .map(|wb| {
                        Ok(DeclarePtrWriteback {
                            arg_index: wb.arg_index,
                            target_slot: self.simple_slot(&wb.target)?,
                            kind: wb.kind,
                        })
                    })
                    .collect::<Res<Vec<_>>>()?;
                self.emit(Op::CallNative {
                    dst,
                    callee: NativeCallee::Declare { descriptor_id: *descriptor_id, ptr_writebacks },
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
            CoreCallee::DynamicByName => {
                // args = [object, name, calltype, forwarded method args…].
                let [obj_arg, name_arg, ct_arg, rest @ ..] = args else {
                    return Err(LinearizeError::Malformed(
                        "CallByName requires object, name, and calltype operands".into(),
                    ));
                };
                let object = self.lower_arg_value(obj_arg)?;
                let name = self.lower_arg_value(name_arg)?;
                let calltype = self.lower_arg_value(ct_arg)?;
                let (call_args, writebacks) = self.build_call_args(rest)?;
                self.emit(Op::CallByName { dst, object, name, calltype, args: call_args });
                self.emit_arg_writebacks(writebacks)?;
            }
        }
        Ok(())
    }

    /// Lower a call operand expected to be a plain value (CallByName's
    /// object/name/calltype), to its slot.
    fn lower_arg_value(&mut self, arg: &CoreArg) -> Res<usize> {
        match arg {
            CoreArg::ByVal(v) => self.lower_value(v),
            CoreArg::ByRef(place) => self.lower_place_load(place),
            other => Err(LinearizeError::Malformed(format!("expected a value operand, got {other:?}"))),
        }
    }

    // ── Statements ─────────────────────────────────────────────────────────────
    fn lower_block(&mut self, body: &[CoreStmt]) -> Res<()> {
        for stmt in body {
            self.lower_stmt(stmt)?;
        }
        Ok(())
    }

    fn lower_loop_condition(&mut self, condition: &CoreValue, until: bool) -> Res<usize> {
        let value = self.lower_value(condition)?;
        if until {
            let negated = self.new_temp();
            self.emit(Op::Not { dst: negated, src: value });
            Ok(negated)
        } else {
            Ok(value)
        }
    }

    fn lower_stmt(&mut self, stmt: &CoreStmt) -> Res<()> {
        let start = self.here();
        self.statement_starts.push(start);
        match stmt {
            CoreStmt::Assign { place, value, intent, target_kind, target_name, target_type_name } => {
                let src = self.lower_value(value)?;
                if *intent == AssignmentIntent::Set || *target_kind == AssignmentTargetKind::Object {
                    self.emit(Op::ValidateAssignment {
                        src,
                        intent: *intent,
                        target_kind: *target_kind,
                        target_name: target_name.clone(),
                        target_type_name: target_type_name.clone(),
                    });
                }
                self.lower_place_store(place, src)?;
            }
            CoreStmt::Eval(value) => match value {
                CoreValue::Call { callee, args } => self.lower_call(callee, args, None)?,
                other => {
                    self.lower_value(other)?;
                }
            },
            CoreStmt::If { arms, else_body } => {
                let mut end_jumps = Vec::new();
                for arm in arms {
                    let cond = self.lower_value(&arm.condition)?;
                    let jz = self.emit(Op::JumpIfZero { cond_slot: cond, target_pc: 0 });
                    self.lower_block(&arm.body)?;
                    end_jumps.push(self.emit(Op::Jump { target_pc: 0 }));
                    let next = self.here();
                    self.patch(jz, next);
                }
                self.lower_block(else_body)?;
                let end = self.here();
                for at in end_jumps {
                    self.patch(at, end);
                }
            }
            CoreStmt::DoLoop { condition, until, post_check, body } => {
                self.loop_exits.push((LoopKind::Do, Vec::new()));
                if *post_check {
                    let top = self.here();
                    self.lower_block(body)?;
                    let cont = self.lower_loop_condition(condition, *until)?;
                    let jz = self.emit(Op::JumpIfZero { cond_slot: cont, target_pc: 0 });
                    self.emit(Op::Jump { target_pc: top });
                    let after = self.here();
                    self.patch(jz, after);
                    self.close_loop(after);
                } else {
                    let top = self.here();
                    let cont = self.lower_loop_condition(condition, *until)?;
                    let jz = self.emit(Op::JumpIfZero { cond_slot: cont, target_pc: 0 });
                    self.lower_block(body)?;
                    self.emit(Op::Jump { target_pc: top });
                    let after = self.here();
                    self.patch(jz, after);
                    self.close_loop(after);
                }
            }
            CoreStmt::ForRange { var, start, end, step, body } => {
                let var_slot = self.simple_slot(var)?;
                let start_slot = self.lower_value(start)?;
                self.emit(Op::Copy { dst: var_slot, src: start_slot });
                let limit = self.new_temp();
                let end_slot = self.lower_value(end)?;
                self.emit(Op::Copy { dst: limit, src: end_slot });
                let step_slot = self.new_temp();
                match step {
                    Some(expr) => {
                        let e = self.lower_value(expr)?;
                        self.emit(Op::Copy { dst: step_slot, src: e });
                    }
                    None => {
                        self.emit(Op::LoadI32 { slot: step_slot, value: 1 });
                    }
                }
                self.loop_exits.push((LoopKind::For, Vec::new()));
                let top = self.here();
                let cond = self.new_temp();
                // FIDELITY: assumes a non-negative Step (the common case).
                self.emit(Op::CmpLe { dst: cond, lhs: var_slot, rhs: limit, mode: StringCompareMode::Binary });
                let jz = self.emit(Op::JumpIfZero { cond_slot: cond, target_pc: 0 });
                self.lower_block(body)?;
                // The loop-counter increment widens (the counter slot already holds the
                // declared numeric type; typed-overflow of a `For` counter is out of scope).
                self.emit(Op::Add { dst: var_slot, lhs: var_slot, rhs: step_slot, mode: NumericMode::Widening });
                self.emit(Op::Jump { target_pc: top });
                let after = self.here();
                self.patch(jz, after);
                self.close_loop(after);
            }
            CoreStmt::ForEach { item, source, body } => {
                let iter = self.new_temp();
                let src = self.lower_value(source)?;
                self.emit(Op::ForEachInit { iter, src });
                self.loop_exits.push((LoopKind::For, Vec::new()));
                let top = self.here();
                let item_tmp = self.new_temp();
                let has = self.new_temp();
                self.emit(Op::ForEachNext { iter, item: item_tmp, has_value: has });
                let jz = self.emit(Op::JumpIfZero { cond_slot: has, target_pc: 0 });
                self.lower_place_store(item, item_tmp)?;
                self.lower_block(body)?;
                self.emit(Op::Jump { target_pc: top });
                let after = self.here();
                self.patch(jz, after);
                self.close_loop(after);
            }
            CoreStmt::Exit(kind) => match kind {
                ExitKind::Proc => {
                    let at = self.emit(Op::Jump { target_pc: 0 });
                    self.proc_exits.push(at);
                }
                ExitKind::Do => {
                    let at = self.emit(Op::Jump { target_pc: 0 });
                    self.push_loop_exit(LoopKind::Do, at)?;
                }
                ExitKind::For => {
                    let at = self.emit(Op::Jump { target_pc: 0 });
                    self.push_loop_exit(LoopKind::For, at)?;
                }
            },
            CoreStmt::Label(label) => {
                self.label_pc.insert(*label, self.here());
            }
            CoreStmt::Goto(label) => {
                let at = self.emit(Op::Jump { target_pc: 0 });
                self.pending_label_patches.push((at, *label));
            }
            CoreStmt::GoSub(label) => {
                let ret_slot = self.gosub_slot();
                let site = self.gosub_return_pcs.len();
                self.emit(Op::LoadI32 { slot: ret_slot, value: site as i32 });
                let at = self.emit(Op::Jump { target_pc: 0 });
                self.pending_label_patches.push((at, *label));
                self.gosub_return_pcs.push(self.here());
            }
            CoreStmt::GoSubReturn => {
                let _ = self.gosub_slot();
                let at = self.emit(Op::Jump { target_pc: 0 });
                self.gosub_return_jumps.push(at);
            }
            CoreStmt::Error(op) => match op {
                ErrorOp::OnErrorResumeNext => {
                    self.emit(Op::SetOnErrorResumeNext);
                }
                ErrorOp::OnErrorGoto0 => {
                    self.emit(Op::SetOnErrorGoto0);
                }
                ErrorOp::OnErrorGotoLabel(label) => {
                    let at = self.emit(Op::SetOnErrorGotoLabel { target_pc: 0 });
                    self.pending_label_patches.push((at, *label));
                }
                ErrorOp::ResumeNext => {
                    self.emit(Op::ResumeNext);
                }
                ErrorOp::Resume => {
                    self.emit(Op::Resume);
                }
                ErrorOp::ResumeLabel(label) => {
                    let at = self.emit(Op::ResumeLabel { target_pc: 0 });
                    self.pending_label_patches.push((at, *label));
                }
                ErrorOp::ClearErr => {
                    self.emit(Op::ClearErr);
                }
                ErrorOp::Raise { code } => {
                    self.emit(Op::RaiseError { code: *code });
                }
            },
            CoreStmt::ReDim { array, bounds, element_type, preserve } => {
                let upper_bounds = bounds.iter().map(|b| self.lower_value(&b.upper)).collect::<Res<Vec<_>>>()?;
                let lower_bounds: Vec<i32> = bounds.iter().map(|b| b.lower).collect();
                let (array_slot, writeback) = self.array_target(array)?;
                self.emit(if *preserve {
                    Op::ArrayResizePreserve { dst: array_slot, upper_bounds, lower_bounds, element_type: *element_type }
                } else {
                    Op::ArrayResize { dst: array_slot, upper_bounds, lower_bounds, element_type: *element_type }
                });
                if let Some(place) = writeback {
                    self.lower_place_store(&place, array_slot)?;
                }
            }
            CoreStmt::Erase { array, .. } => {
                let empty = self.new_temp();
                self.emit(Op::LoadEmpty { slot: empty });
                self.lower_place_store(array, empty)?;
            }
            CoreStmt::RaiseEvent { source, event, args } => {
                let source_slot = self.lower_value(source)?;
                let (event_args, writebacks) = self.build_proc_args(args)?;
                self.emit(Op::RaiseEvent { source: source_slot, event: *event, args: event_args });
                for (place, tmp) in writebacks {
                    self.lower_place_store(&place, tmp)?;
                }
            }
            CoreStmt::Select { selector, cases, case_else } => {
                let scrutinee = self.lower_value(selector)?;
                let mut end_jumps = Vec::new();
                for block in cases {
                    let matched = self.lower_case_match(scrutinee, &block.clauses)?;
                    let jz = self.emit(Op::JumpIfZero { cond_slot: matched, target_pc: 0 });
                    self.lower_block(&block.body)?;
                    end_jumps.push(self.emit(Op::Jump { target_pc: 0 }));
                    let next = self.here();
                    self.patch(jz, next);
                }
                self.lower_block(case_else)?;
                let end = self.here();
                for at in end_jumps {
                    self.patch(at, end);
                }
            }
        }
        Ok(())
    }

    fn lower_case_match(&mut self, scrutinee: usize, clauses: &[CaseClause]) -> Res<usize> {
        let mut accumulator: Option<usize> = None;
        for clause in clauses {
            let m = StringCompareMode::Binary;
            let clause_bool = match clause {
                CaseClause::Value(v) => {
                    let vs = self.lower_value(v)?;
                    let dst = self.new_temp();
                    self.emit(Op::CmpEq { dst, lhs: scrutinee, rhs: vs, mode: m });
                    dst
                }
                CaseClause::Range { lo, hi } => {
                    let lo_s = self.lower_value(lo)?;
                    let ge = self.new_temp();
                    self.emit(Op::CmpGe { dst: ge, lhs: scrutinee, rhs: lo_s, mode: m });
                    let hi_s = self.lower_value(hi)?;
                    let le = self.new_temp();
                    self.emit(Op::CmpLe { dst: le, lhs: scrutinee, rhs: hi_s, mode: m });
                    let both = self.new_temp();
                    self.emit(Op::And { dst: both, lhs: ge, rhs: le });
                    both
                }
                CaseClause::Is { op, value } => {
                    let vs = self.lower_value(value)?;
                    let dst = self.new_temp();
                    // A `Case Is <op> value` is a comparison — the numeric mode is unused.
                    self.emit_binary(*op, dst, scrutinee, vs, m, NumericMode::Widening);
                    dst
                }
            };
            accumulator = Some(match accumulator {
                None => clause_bool,
                Some(acc) => {
                    let dst = self.new_temp();
                    self.emit(Op::Or { dst, lhs: acc, rhs: clause_bool });
                    dst
                }
            });
        }
        // An empty clause list never matches.
        match accumulator {
            Some(slot) => Ok(slot),
            None => {
                let slot = self.new_temp();
                self.emit(Op::LoadBool { slot, value: false });
                Ok(slot)
            }
        }
    }

    fn close_loop(&mut self, after: usize) {
        if let Some((_, exits)) = self.loop_exits.pop() {
            for at in exits {
                self.patch(at, after);
            }
        }
    }

    fn push_loop_exit(&mut self, kind: LoopKind, at: usize) -> Res<()> {
        // Prefer the nearest loop of the matching kind; otherwise the innermost loop.
        let index = self
            .loop_exits
            .iter()
            .rposition(|(k, _)| *k == kind)
            .or(self.loop_exits.len().checked_sub(1));
        match index {
            Some(i) => {
                self.loop_exits[i].1.push(at);
                Ok(())
            }
            None => Err(LinearizeError::Malformed("Exit outside any loop".into())),
        }
    }
}

fn const_to_load(slot: usize, c: &CoreConst) -> Op {
    match c {
        CoreConst::Empty => Op::LoadEmpty { slot },
        CoreConst::Null => Op::LoadNull { slot },
        CoreConst::Nothing => Op::LoadEmpty { slot }, // null object; VM treats Empty/0 as Nothing for `Is`
        CoreConst::Bool(value) => Op::LoadBool { slot, value: *value },
        CoreConst::I32(value) => Op::LoadI32 { slot, value: *value },
        CoreConst::I64(value) => Op::LoadI64 { slot, value: *value },
        CoreConst::F64(bits) => Op::LoadF64 { slot, bits: *bits },
        CoreConst::F32(bits) => Op::LoadF32 { slot, bits: *bits },
        CoreConst::Currency(scaled) => Op::LoadCurrency { slot, scaled: *scaled },
        CoreConst::Date(bits) => Op::LoadDate { slot, bits: *bits },
        CoreConst::Str(value) => Op::LoadString { slot, value: value.clone() },
    }
}
