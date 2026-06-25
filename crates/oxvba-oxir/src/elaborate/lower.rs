//! The Core IR → OxIR elaboration: structured tree to typed basic-block CFG.
//!
//! This is the spine of the pass. It builds the CFG (mirroring the proven `linearize`
//! lowering, but emitting blocks + terminators instead of flat labelled ops), recovers
//! each binding's [`OxTy`] from the type the binder recorded on the Core IR
//! ([`crate::elaborate::lower_var_type`]), and marks statement boundaries.
//!
//! # Coverage (landing incrementally)
//!
//! Covered: the scalar / control-flow / `VbaProc`-call core **and the full error /
//! control model** — `On Error` (`Resume Next` / `GoTo h` / `GoTo 0`),
//! `Resume`/`Resume Next`/`Resume <label>`, `Err.Raise`/`Error <n>`, `GoSub`/`Return`,
//! `GoTo`/labels. Constructs not yet handled return [`ElaborateError::Unimplemented`]
//! (never a silent mis-lowering): arrays, records, `With`, `For Each`, events, object
//! construction, COM dispatch, and the remaining call kinds. Each is added in a later
//! reviewed step.
//!
//! # Fault model (per-statement landing pads)
//!
//! Each VBA statement is lowered to its own start block (so it is a block-precise
//! `Resume` re-entry target) plus a **landing pad** block; the statement's fallible
//! instructions transfer to that pad via `fault_target`. The pad is an
//! [`OxTerminator::FaultDispatch`] `{ resume, resume_next }` that records the resume
//! target and dispatches on the runtime error mode (propagate / `Resume Next` / active
//! handler). `On Error GoTo <label>` sets the runtime handler via
//! [`OxInst::SetErrorHandler`]; `<label>` blocks are pre-assigned so forward `GoTo`s
//! resolve. Statements with no fallible instruction carry no fault edge (their pad is
//! then an unreachable block — collapsing those is a noted later optimization; the M1
//! gate is round-trip + verifier, not execution). Box/Unbox insertion at typed↔Variant
//! boundaries, the fault-status-carrying return, and the `Class_Terminate` drain are
//! deferred to the typed-execution (vm3) work.

use std::collections::HashMap;

use oxvba_bundle::coreir::{
    self, CoreArg, CoreBinOp, CoreConst, CoreProc, CoreProgram, CoreStmt, CoreUnOp, CoreValue,
    ErrorOp, LabelId as CoreLabelId,
};
use oxvba_bundle::NumericMode;

use crate::com::ComInterface;
use crate::elaborate::{NameResolver, lower_var_type};
use crate::ids::{BlockId, FuncId, GlobalId, LocalId, TempId};
use crate::inst::{ErrorHandler, OxBlock, OxInst, OxTerminator};
use crate::program::{OxFunc, OxGlobal, OxLocal, OxParamInfo, OxProgram};
use crate::ty::OxTy;
use crate::value::{ArithOp, CmpOp, LogicalOp, OxArg, OxConst, OxOperand, OxPlace};

/// A failure to elaborate Core IR into OxIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElaborateError {
    /// A Core IR construct the elaboration spine does not yet handle. Honest and
    /// explicit — a program using it fails to elaborate rather than mis-lowering.
    Unimplemented { what: &'static str },
    /// A structurally invalid input (should not occur for well-formed binder output).
    Malformed(String),
}

impl std::fmt::Display for ElaborateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElaborateError::Unimplemented { what } => {
                write!(f, "elaboration not yet implemented for: {what}")
            }
            ElaborateError::Malformed(m) => write!(f, "malformed Core IR: {m}"),
        }
    }
}

impl std::error::Error for ElaborateError {}

type Result<T> = std::result::Result<T, ElaborateError>;

/// Elaborate a whole compilation unit. `resolver` classifies declared type names
/// (object / UDT) into the typed identities OxIR carries; for the procedural spine
/// (no object-typed locals) any resolver is acceptable.
pub fn elaborate(program: &CoreProgram, resolver: &impl NameResolver) -> Result<OxProgram> {
    let globals = program
        .globals
        .iter()
        .map(|g| OxGlobal {
            name: g.name.clone(),
            ty: lower_var_type(&g.ty, resolver),
        })
        .collect();

    let mut funcs = Vec::with_capacity(program.procs.len());
    for proc in &program.procs {
        funcs.push(elaborate_proc(proc, resolver)?);
    }

    Ok(OxProgram {
        funcs,
        globals,
        // Project classes + the typed COM interface table are populated by the
        // object/COM de-erasure step.
        classes: Vec::new(),
        com_interfaces: Vec::<ComInterface>::new(),
        entry: program.entry.map(|p| FuncId(p.0)),
        global_initializer: program.global_initializer.map(|p| FuncId(p.0)),
        unit_name: program.unit_name.clone(),
        // Stable, name-keyed metadata is reused verbatim.
        event_routes: program.event_routes.clone(),
        external_calls: program.external_calls.clone(),
        com_class_exports: program.com_class_exports.clone(),
        exports: program.exports.clone(),
        imports: program.imports.clone(),
    })
}

fn elaborate_proc(proc: &CoreProc, resolver: &impl NameResolver) -> Result<OxFunc> {
    // The unified local index space is params first, then locals (the binder's
    // `LocalId` convention), so OxFunc.locals mirrors it 1:1.
    let mut locals: Vec<OxLocal> = Vec::with_capacity(proc.params.len() + proc.locals.len());
    for p in &proc.params {
        locals.push(OxLocal {
            name: p.name.clone(),
            ty: lower_var_type(&p.ty, resolver),
            param: Some(OxParamInfo {
                by_ref: p.by_ref,
                variadic: p.variadic,
            }),
            escaped: false,
        });
    }
    for l in &proc.locals {
        locals.push(OxLocal {
            name: l.name.clone(),
            ty: lower_var_type(&l.ty, resolver),
            param: None,
            escaped: false,
        });
    }

    // The spine recovers all binding types up front (the `locals` above, via the
    // resolver); the lowerer itself needs no resolver until object/COM construction.
    let mut lo = Lowerer::new(locals, proc.params.len());
    // Pre-assign a block to every source label so forward references resolve.
    lo.assign_labels(&proc.body);
    lo.lower_block(&proc.body)?;
    let func = lo.finish_proc(proc)?;
    Ok(func)
}

/// Loop context for `Exit Do`/`Exit For` targeting (the break block of the enclosing
/// loop, tagged with the loop kind for the nearest-matching-kind rule).
struct LoopCtx {
    is_for: bool,
    brk: BlockId,
}

struct Lowerer {
    /// Blocks indexed by [`BlockId`]; `None` = reserved but not yet finalized. Every
    /// reserved id is filled exactly once, so the final vector has `id == position`.
    blocks: Vec<Option<OxBlock>>,
    /// The block currently being built.
    cur: BlockId,
    instrs: Vec<OxInst>,
    /// The current statement's landing pad — the fault target for that statement's
    /// fallible instructions. Set per statement by [`Self::lower_block`]; a
    /// control-flow statement saves/restores it around its sub-bodies so its own
    /// condition/setup faults to *its* pad while body statements fault to theirs.
    cur_fault: BlockId,
    /// The single normal-return convergence block (`Return`): the target of `Exit Sub`
    /// and of fall-through past the body.
    epilogue: BlockId,
    locals: Vec<OxLocal>,
    param_count: usize,
    next_temp: usize,
    loops: Vec<LoopCtx>,
    /// Pre-assigned block per source label, so forward `GoTo` / `On Error GoTo` /
    /// `Resume <label>` / `GoSub` references resolve.
    labels: HashMap<CoreLabelId, BlockId>,
}

impl Lowerer {
    fn new(locals: Vec<OxLocal>, param_count: usize) -> Self {
        // Entry = block 0, epilogue = block 1; both reserved up front.
        let blocks = vec![None, None];
        Self {
            blocks,
            cur: BlockId(0),
            instrs: Vec::new(),
            // Until the first statement sets its pad, faults would land on the
            // epilogue; the prologue emits no fallible instruction, so this is unused.
            cur_fault: BlockId(1),
            epilogue: BlockId(1),
            locals,
            param_count,
            next_temp: 0,
            loops: Vec::new(),
            labels: HashMap::new(),
        }
    }

    // ── Block / temp plumbing ────────────────────────────────────────────────

    fn reserve(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(None);
        id
    }

    fn new_temp(&mut self) -> TempId {
        let t = TempId(self.next_temp);
        self.next_temp += 1;
        t
    }

    fn emit(&mut self, inst: OxInst) {
        self.instrs.push(inst);
    }

    /// Finalize the current block with `term`, then continue building in `next`. A
    /// fault edge to the current statement's landing pad is attached only when the
    /// block actually contains a fallible instruction (so non-faulting blocks carry no
    /// spurious fault edge, and their pad — if unused — is a dead block).
    fn finish_to(&mut self, term: OxTerminator, next: BlockId) {
        let instrs = std::mem::take(&mut self.instrs);
        let fault_target = instrs
            .iter()
            .any(|i| i.is_fallible())
            .then_some(self.cur_fault);
        debug_assert!(self.blocks[self.cur.0].is_none(), "block finalized twice");
        self.blocks[self.cur.0] = Some(OxBlock {
            id: self.cur,
            instrs,
            fault_target,
            terminator: term,
        });
        self.cur = next;
    }

    /// Fill a statement's landing pad: a [`OxTerminator::FaultDispatch`] seeded with the
    /// statement's own start (`resume`) and the following statement's start
    /// (`resume_next`).
    fn build_pad(&mut self, pad: BlockId, resume: BlockId, resume_next: BlockId) {
        debug_assert!(self.blocks[pad.0].is_none(), "pad finalized twice");
        self.blocks[pad.0] = Some(OxBlock {
            id: pad,
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::FaultDispatch {
                resume,
                resume_next,
            },
        });
    }

    /// Pre-assign a block to every source label reachable in `body` (recursively), so
    /// forward references resolve. Runs before lowering.
    fn assign_labels(&mut self, body: &[CoreStmt]) {
        for stmt in body {
            match stmt {
                CoreStmt::Label(id) => {
                    if !self.labels.contains_key(id) {
                        let b = self.reserve();
                        self.labels.insert(*id, b);
                    }
                }
                CoreStmt::If { arms, else_body } => {
                    for arm in arms {
                        self.assign_labels(&arm.body);
                    }
                    self.assign_labels(else_body);
                }
                CoreStmt::DoLoop { body, .. }
                | CoreStmt::ForRange { body, .. }
                | CoreStmt::ForEach { body, .. }
                | CoreStmt::With { body, .. } => self.assign_labels(body),
                CoreStmt::Select {
                    cases, case_else, ..
                } => {
                    for c in cases {
                        self.assign_labels(&c.body);
                    }
                    self.assign_labels(case_else);
                }
                _ => {}
            }
        }
    }

    fn label_block(&self, id: &CoreLabelId) -> Result<BlockId> {
        self.labels
            .get(id)
            .copied()
            .ok_or_else(|| ElaborateError::Malformed(format!("reference to undefined label {id:?}")))
    }

    /// The start block of `stmt`: a labelled statement starts at its pre-assigned label
    /// block (so `GoTo` reaches it); any other statement gets a fresh block.
    fn stmt_start_block(&mut self, stmt: &CoreStmt) -> BlockId {
        match stmt {
            CoreStmt::Label(id) => match self.labels.get(id) {
                Some(&b) => b,
                None => self.reserve(),
            },
            _ => self.reserve(),
        }
    }

    fn finish_proc(mut self, proc: &CoreProc) -> Result<OxFunc> {
        // The body's final continuation falls through to the epilogue.
        let epilogue = self.epilogue;
        self.finish_to(OxTerminator::Jump(epilogue), epilogue);
        // Build the epilogue: a bare Return (the `Class_Terminate` drain + the
        // fault-status-carrying return are the vm3 work).
        debug_assert!(self.blocks[epilogue.0].is_none());
        self.blocks[epilogue.0] = Some(OxBlock {
            id: epilogue,
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::Return,
        });

        let blocks = self
            .blocks
            .into_iter()
            .map(|b| b.ok_or_else(|| ElaborateError::Malformed("unfilled reserved block".into())))
            .collect::<Result<Vec<_>>>()?;

        Ok(OxFunc {
            name: proc.name.clone(),
            kind: proc.kind,
            locals: self.locals,
            param_count: self.param_count,
            return_local: proc.return_local.map(|l| LocalId(l.0)),
            blocks,
            entry: BlockId(0),
        })
    }

    // ── Statements ───────────────────────────────────────────────────────────

    /// Lower a statement list. Each statement gets its own start block (a block-precise
    /// `Resume` target) and landing pad; statements chain into the next's start. After
    /// the call, `cur` is the continuation block following the last statement.
    fn lower_block(&mut self, stmts: &[CoreStmt]) -> Result<()> {
        if stmts.is_empty() {
            return Ok(());
        }
        // Enter the first statement's start block.
        let first = self.stmt_start_block(&stmts[0]);
        self.finish_to(OxTerminator::Jump(first), first);
        for (i, stmt) in stmts.iter().enumerate() {
            let s_start = self.cur;
            let pad = self.reserve();
            let s_next = match stmts.get(i + 1) {
                Some(next) => self.stmt_start_block(next),
                None => self.reserve(),
            };
            self.cur_fault = pad;
            self.emit(OxInst::StmtBoundary { stmt: i as u32 });
            self.lower_stmt(stmt, s_next)?;
            self.build_pad(pad, s_start, s_next);
        }
        Ok(())
    }

    /// Lower one statement so that control ends at `s_next` (the following statement's
    /// start block, or the enclosing continuation).
    fn lower_stmt(&mut self, stmt: &CoreStmt, s_next: BlockId) -> Result<()> {
        match stmt {
            CoreStmt::Assign { place, value, .. } => {
                let (src, _ty) = self.lower_value(value)?;
                let dst = self.lower_place_store(place)?;
                self.emit(OxInst::Assign { dst, value: src });
                self.finish_to(OxTerminator::Jump(s_next), s_next);
                Ok(())
            }
            CoreStmt::Eval(value) => {
                // Evaluate for effect; the result (if any) is discarded.
                let _ = self.lower_value(value)?;
                self.finish_to(OxTerminator::Jump(s_next), s_next);
                Ok(())
            }
            CoreStmt::If { arms, else_body } => self.lower_if(arms, else_body, s_next),
            CoreStmt::DoLoop {
                condition,
                until,
                post_check,
                body,
            } => self.lower_do_loop(condition, *until, *post_check, body, s_next),
            CoreStmt::ForRange {
                var,
                start,
                end,
                step,
                body,
            } => self.lower_for_range(var, start, end, step.as_ref(), body, s_next),
            CoreStmt::Select {
                selector,
                cases,
                case_else,
            } => self.lower_select(selector, cases, case_else, s_next),
            CoreStmt::Exit(kind) => {
                let target = self.exit_target(*kind)?;
                self.finish_to(OxTerminator::Jump(target), s_next);
                Ok(())
            }
            CoreStmt::Label(_) => {
                // The label's start block IS this statement's start (resolved by
                // `stmt_start_block`); it simply falls through to the next statement.
                self.finish_to(OxTerminator::Jump(s_next), s_next);
                Ok(())
            }
            CoreStmt::Goto(id) => {
                let target = self.label_block(id)?;
                self.finish_to(OxTerminator::Jump(target), s_next);
                Ok(())
            }
            CoreStmt::GoSub(id) => {
                let target = self.label_block(id)?;
                // `Return` from the subroutine resumes at the statement after this GoSub.
                self.finish_to(OxTerminator::GoSub { target, ret: s_next }, s_next);
                Ok(())
            }
            CoreStmt::GoSubReturn => {
                self.finish_to(OxTerminator::GoSubReturn, s_next);
                Ok(())
            }
            CoreStmt::Error(op) => self.lower_error(op, s_next),
            // Deferred to later reviewed steps.
            CoreStmt::ForEach { .. } => Err(unimpl("For Each")),
            CoreStmt::With { .. } => Err(unimpl("With")),
            CoreStmt::ReDim { .. } => Err(unimpl("ReDim")),
            CoreStmt::Erase { .. } => Err(unimpl("Erase")),
            CoreStmt::RaiseEvent { .. } => Err(unimpl("RaiseEvent")),
        }
    }

    fn lower_error(&mut self, op: &ErrorOp, s_next: BlockId) -> Result<()> {
        match op {
            ErrorOp::OnErrorResumeNext => {
                self.emit(OxInst::SetErrorHandler(ErrorHandler::ResumeNext));
                self.finish_to(OxTerminator::Jump(s_next), s_next);
            }
            ErrorOp::OnErrorGoto0 => {
                self.emit(OxInst::SetErrorHandler(ErrorHandler::Goto0));
                self.finish_to(OxTerminator::Jump(s_next), s_next);
            }
            ErrorOp::OnErrorGotoLabel(id) => {
                let h = self.label_block(id)?;
                self.emit(OxInst::SetErrorHandler(ErrorHandler::GotoLabel(h)));
                self.finish_to(OxTerminator::Jump(s_next), s_next);
            }
            ErrorOp::ClearErr => {
                self.emit(OxInst::ClearErr);
                self.finish_to(OxTerminator::Jump(s_next), s_next);
            }
            ErrorOp::ResumeNext => self.finish_to(OxTerminator::ResumeNext, s_next),
            ErrorOp::Resume => self.finish_to(OxTerminator::Resume, s_next),
            ErrorOp::ResumeLabel(id) => {
                let b = self.label_block(id)?;
                self.finish_to(OxTerminator::ResumeLabel(b), s_next);
            }
            ErrorOp::Raise { code } => self.finish_to(OxTerminator::Raise { code: *code }, s_next),
            ErrorOp::RaiseValue { code } => {
                let (op, _) = self.lower_value(code)?;
                self.finish_to(OxTerminator::RaiseValue(op), s_next);
            }
        }
        Ok(())
    }

    /// The break target of an `Exit` statement: the epilogue for `Exit Sub`/`Function`,
    /// else the enclosing loop's break block.
    fn exit_target(&self, kind: coreir::ExitKind) -> Result<BlockId> {
        match kind {
            coreir::ExitKind::Proc => Ok(self.epilogue),
            coreir::ExitKind::For => self.loop_break(true),
            coreir::ExitKind::Do => self.loop_break(false),
        }
    }

    fn lower_if(
        &mut self,
        arms: &[coreir::CoreIfArm],
        else_body: &[CoreStmt],
        end: BlockId,
    ) -> Result<()> {
        // Every condition (`If`/`ElseIf`) belongs to the If statement, so it faults to
        // the If's pad; the arm bodies are separate statements with their own pads.
        let if_pad = self.cur_fault;
        for arm in arms {
            self.cur_fault = if_pad;
            let (cond, _) = self.lower_value(&arm.condition)?;
            let then_blk = self.reserve();
            let next_blk = self.reserve();
            self.finish_to(
                OxTerminator::Branch {
                    cond,
                    then_blk,
                    else_blk: next_blk,
                },
                then_blk,
            );
            self.lower_block(&arm.body)?;
            self.finish_to(OxTerminator::Jump(end), next_blk);
        }
        // `cur` is the final `next_blk` (the `Else` position).
        self.lower_block(else_body)?;
        self.finish_to(OxTerminator::Jump(end), end);
        Ok(())
    }

    fn lower_do_loop(
        &mut self,
        condition: &CoreValue,
        until: bool,
        post_check: bool,
        body: &[CoreStmt],
        after: BlockId,
    ) -> Result<()> {
        // The loop's condition belongs to the loop statement (faults to its pad).
        let loop_pad = self.cur_fault;
        let body_blk = self.reserve();

        if post_check {
            // `Do … Loop While/Until`: run the body, then test.
            self.finish_to(OxTerminator::Jump(body_blk), body_blk);
            self.loops.push(LoopCtx { is_for: false, brk: after });
            self.lower_block(body)?;
            self.loops.pop();
            self.cur_fault = loop_pad;
            let cond = self.lower_loop_condition(condition, until)?;
            self.finish_to(
                OxTerminator::Branch {
                    cond,
                    then_blk: body_blk,
                    else_blk: after,
                },
                after,
            );
        } else {
            // `Do While/Until … Loop`: test at the top.
            let head = self.reserve();
            self.finish_to(OxTerminator::Jump(head), head);
            self.cur_fault = loop_pad;
            let cond = self.lower_loop_condition(condition, until)?;
            self.finish_to(
                OxTerminator::Branch {
                    cond,
                    then_blk: body_blk,
                    else_blk: after,
                },
                body_blk,
            );
            self.loops.push(LoopCtx { is_for: false, brk: after });
            self.lower_block(body)?;
            self.loops.pop();
            self.finish_to(OxTerminator::Jump(head), after);
        }
        Ok(())
    }

    /// Lower a loop continuation condition, negating it for `Until`.
    fn lower_loop_condition(&mut self, condition: &CoreValue, until: bool) -> Result<OxOperand> {
        let (cond, _) = self.lower_value(condition)?;
        if until {
            let t = self.new_temp();
            self.emit(OxInst::Not {
                dst: OxPlace::Temp(t),
                src: cond,
            });
            Ok(OxOperand::temp(t))
        } else {
            Ok(cond)
        }
    }

    fn lower_for_range(
        &mut self,
        var: &coreir::CorePlace,
        start: &CoreValue,
        end: &CoreValue,
        step: Option<&CoreValue>,
        body: &[CoreStmt],
        after: BlockId,
    ) -> Result<()> {
        // The For header (bounds, step, counter test and increment) belongs to the For
        // statement, so all of it faults to the For's pad; the body statements have
        // their own pads. `cur_fault` is already the For's pad on entry.
        let for_pad = self.cur_fault;
        let var_place = self.lower_place_store(var)?;
        let var_op = self.place_as_operand(var)?;

        // counter = start
        let (start_op, _) = self.lower_value(start)?;
        self.emit(OxInst::Assign {
            dst: var_place,
            value: start_op,
        });
        // limit and step are evaluated once into temps.
        let (end_op, _) = self.lower_value(end)?;
        let limit = self.new_temp();
        self.emit(OxInst::Assign {
            dst: OxPlace::Temp(limit),
            value: end_op,
        });
        let step_t = self.new_temp();
        let step_op = match step {
            Some(s) => self.lower_value(s)?.0,
            None => OxOperand::Const(OxConst::I32(1)),
        };
        self.emit(OxInst::Assign {
            dst: OxPlace::Temp(step_t),
            value: step_op,
        });

        // step >= 0 ? (fixed at entry — the step sign never changes)
        let nonneg = self.new_temp();
        self.emit(OxInst::Compare {
            dst: OxPlace::Temp(nonneg),
            op: CmpOp::Ge,
            lhs: OxOperand::temp(step_t),
            rhs: OxOperand::Const(OxConst::I32(0)),
            mode: oxvba_bundle::StringCompareMode::Binary,
        });

        let head = self.reserve();
        let asc = self.reserve();
        let desc = self.reserve();
        let test = self.reserve();
        let body_blk = self.reserve();
        let step_blk = self.reserve();

        self.finish_to(OxTerminator::Jump(head), head);
        // head: branch on step sign to the matching comparison.
        self.finish_to(
            OxTerminator::Branch {
                cond: OxOperand::temp(nonneg),
                then_blk: asc,
                else_blk: desc,
            },
            asc,
        );
        // ascending: counter <= limit
        let cond_t = self.new_temp();
        self.emit(OxInst::Compare {
            dst: OxPlace::Temp(cond_t),
            op: CmpOp::Le,
            lhs: var_op.clone(),
            rhs: OxOperand::temp(limit),
            mode: oxvba_bundle::StringCompareMode::Binary,
        });
        self.finish_to(OxTerminator::Jump(test), desc);
        // descending: counter >= limit (writes the same cond temp)
        self.emit(OxInst::Compare {
            dst: OxPlace::Temp(cond_t),
            op: CmpOp::Ge,
            lhs: var_op.clone(),
            rhs: OxOperand::temp(limit),
            mode: oxvba_bundle::StringCompareMode::Binary,
        });
        self.finish_to(OxTerminator::Jump(test), test);
        // test: continue into the body or exit.
        self.finish_to(
            OxTerminator::Branch {
                cond: OxOperand::temp(cond_t),
                then_blk: body_blk,
                else_blk: after,
            },
            body_blk,
        );
        self.loops.push(LoopCtx {
            is_for: true,
            brk: after,
        });
        self.lower_block(body)?;
        self.loops.pop();
        self.finish_to(OxTerminator::Jump(step_blk), step_blk);
        // step: counter = counter + step (widening — typed overflow is allowed here).
        // Belongs to the For statement, so restore its pad after the body.
        self.cur_fault = for_pad;
        self.emit(OxInst::Arith {
            dst: var_place,
            op: ArithOp::Add,
            lhs: var_op,
            rhs: OxOperand::temp(step_t),
            mode: NumericMode::Widening,
        });
        self.finish_to(OxTerminator::Jump(head), after);
        Ok(())
    }

    fn lower_select(
        &mut self,
        selector: &CoreValue,
        cases: &[coreir::CoreCaseBlock],
        case_else: &[CoreStmt],
        end: BlockId,
    ) -> Result<()> {
        // The selector and every case's clause comparisons belong to the Select
        // statement (fault to its pad); the case bodies have their own pads.
        let select_pad = self.cur_fault;
        let (sel, _) = self.lower_value(selector)?;
        // Evaluate the selector once into a temp so each case can compare against it.
        let sel_t = self.new_temp();
        self.emit(OxInst::Assign {
            dst: OxPlace::Temp(sel_t),
            value: sel,
        });
        let sel_op = OxOperand::temp(sel_t);

        for block in cases {
            self.cur_fault = select_pad;
            let matched = self.lower_case_match(&sel_op, &block.clauses)?;
            let body_blk = self.reserve();
            let next_blk = self.reserve();
            self.finish_to(
                OxTerminator::Branch {
                    cond: matched,
                    then_blk: body_blk,
                    else_blk: next_blk,
                },
                body_blk,
            );
            self.lower_block(&block.body)?;
            self.finish_to(OxTerminator::Jump(end), next_blk);
        }
        self.cur_fault = select_pad;
        self.lower_block(case_else)?;
        self.finish_to(OxTerminator::Jump(end), end);
        Ok(())
    }

    /// Lower one case's clause list to a single Boolean-ish operand (the clauses are
    /// OR-ed together).
    fn lower_case_match(
        &mut self,
        sel: &OxOperand,
        clauses: &[coreir::CaseClause],
    ) -> Result<OxOperand> {
        let mut acc: Option<OxOperand> = None;
        for clause in clauses {
            let clause_bool = self.lower_case_clause(sel, clause)?;
            acc = Some(match acc {
                None => clause_bool,
                Some(prev) => {
                    let t = self.new_temp();
                    self.emit(OxInst::Logical {
                        dst: OxPlace::Temp(t),
                        op: LogicalOp::Or,
                        lhs: prev,
                        rhs: clause_bool,
                    });
                    OxOperand::temp(t)
                }
            });
        }
        Ok(acc.unwrap_or(OxOperand::Const(OxConst::Bool(false))))
    }

    fn lower_case_clause(
        &mut self,
        sel: &OxOperand,
        clause: &coreir::CaseClause,
    ) -> Result<OxOperand> {
        let mode = oxvba_bundle::StringCompareMode::Binary;
        match clause {
            coreir::CaseClause::Value(v) => {
                let (val, _) = self.lower_value(v)?;
                let t = self.new_temp();
                self.emit(OxInst::Compare {
                    dst: OxPlace::Temp(t),
                    op: CmpOp::Eq,
                    lhs: sel.clone(),
                    rhs: val,
                    mode,
                });
                Ok(OxOperand::temp(t))
            }
            coreir::CaseClause::Range { lo, hi } => {
                let (lo_op, _) = self.lower_value(lo)?;
                let ge = self.new_temp();
                self.emit(OxInst::Compare {
                    dst: OxPlace::Temp(ge),
                    op: CmpOp::Ge,
                    lhs: sel.clone(),
                    rhs: lo_op,
                    mode,
                });
                let (hi_op, _) = self.lower_value(hi)?;
                let le = self.new_temp();
                self.emit(OxInst::Compare {
                    dst: OxPlace::Temp(le),
                    op: CmpOp::Le,
                    lhs: sel.clone(),
                    rhs: hi_op,
                    mode,
                });
                let both = self.new_temp();
                self.emit(OxInst::Logical {
                    dst: OxPlace::Temp(both),
                    op: LogicalOp::And,
                    lhs: OxOperand::temp(ge),
                    rhs: OxOperand::temp(le),
                });
                Ok(OxOperand::temp(both))
            }
            coreir::CaseClause::Is { op, value } => {
                let (val, _) = self.lower_value(value)?;
                let cmp = bin_cmp(*op).ok_or(ElaborateError::Malformed(
                    "Case Is with a non-comparison operator".into(),
                ))?;
                let t = self.new_temp();
                self.emit(OxInst::Compare {
                    dst: OxPlace::Temp(t),
                    op: cmp,
                    lhs: sel.clone(),
                    rhs: val,
                    mode,
                });
                Ok(OxOperand::temp(t))
            }
        }
    }

    /// The break target for `Exit For`/`Exit Do`: the nearest loop of the matching
    /// kind, else the innermost loop (mirrors `linearize`'s rule).
    fn loop_break(&self, want_for: bool) -> Result<BlockId> {
        self.loops
            .iter()
            .rev()
            .find(|l| l.is_for == want_for)
            .or_else(|| self.loops.last())
            .map(|l| l.brk)
            .ok_or(ElaborateError::Malformed("Exit outside any loop".into()))
    }

    // ── Values ───────────────────────────────────────────────────────────────

    /// Lower an expression, emitting its instructions and returning `(operand, type)`.
    fn lower_value(&mut self, value: &CoreValue) -> Result<(OxOperand, OxTy)> {
        match value {
            CoreValue::Const(c) => Ok((OxOperand::Const(lower_const(c)), const_type(c))),
            CoreValue::Load(place) => self.lower_place_load(place),
            CoreValue::Unary { op, expr, num } => self.lower_unary(*op, expr, *num),
            CoreValue::Binary {
                op,
                lhs,
                rhs,
                mode,
                num,
            } => self.lower_binary(*op, lhs, rhs, *mode, *num),
            CoreValue::Coerce { value, to } => self.lower_coerce(value, to),
            CoreValue::Call { callee, args } => self.lower_call(callee, args),
            // Deferred to later reviewed steps.
            CoreValue::WithTemp(_) => Err(unimpl("With receiver temp")),
            CoreValue::New(_) | CoreValue::NewExtern { .. } => Err(unimpl("New <class>")),
            CoreValue::NewRecord { .. } => Err(unimpl("UDT record value")),
            CoreValue::TypeOfIs { .. } => Err(unimpl("TypeOf … Is")),
            CoreValue::Ptr { .. } => Err(unimpl("VarPtr/StrPtr/ObjPtr")),
            CoreValue::ErrField(_) => Err(unimpl("Err.Number/.Description/…")),
            CoreValue::ArrayLiteral(_) => Err(unimpl("array literal")),
            CoreValue::Bound { .. } => Err(unimpl("LBound/UBound")),
            CoreValue::AddressOf(_) => Err(unimpl("AddressOf")),
            CoreValue::Predeclared { .. } | CoreValue::PredeclaredExtern { .. } => {
                Err(unimpl("predeclared (VB_PredeclaredId) instance"))
            }
        }
    }

    fn lower_unary(&mut self, op: CoreUnOp, expr: &CoreValue, num: NumericMode) -> Result<(OxOperand, OxTy)> {
        let (src, src_ty) = self.lower_value(expr)?;
        let t = self.new_temp();
        let (inst, ty) = match op {
            CoreUnOp::Negate => (
                OxInst::Neg {
                    dst: OxPlace::Temp(t),
                    src,
                    mode: num,
                },
                numeric_result_type(num),
            ),
            // `Not` is bitwise on integers — it preserves the operand's type.
            CoreUnOp::Not => (
                OxInst::Not {
                    dst: OxPlace::Temp(t),
                    src,
                },
                src_ty,
            ),
        };
        self.emit(inst);
        Ok((OxOperand::temp(t), ty))
    }

    fn lower_binary(
        &mut self,
        op: CoreBinOp,
        lhs: &CoreValue,
        rhs: &CoreValue,
        mode: oxvba_bundle::StringCompareMode,
        num: NumericMode,
    ) -> Result<(OxOperand, OxTy)> {
        let (l, l_ty) = self.lower_value(lhs)?;
        let (r, r_ty) = self.lower_value(rhs)?;
        let t = self.new_temp();
        let dst = OxPlace::Temp(t);

        let ty = match op {
            CoreBinOp::Add | CoreBinOp::Sub | CoreBinOp::Mul | CoreBinOp::IntDiv | CoreBinOp::Mod => {
                let aop = arith_op(op).expect("arith op");
                self.emit(OxInst::Arith {
                    dst,
                    op: aop,
                    lhs: l,
                    rhs: r,
                    mode: num,
                });
                numeric_result_type(num)
            }
            CoreBinOp::Div => {
                self.emit(OxInst::Div { dst, lhs: l, rhs: r });
                OxTy::Double
            }
            CoreBinOp::Pow => {
                self.emit(OxInst::Pow { dst, lhs: l, rhs: r });
                OxTy::Double
            }
            CoreBinOp::Concat => {
                self.emit(OxInst::Concat { dst, lhs: l, rhs: r });
                OxTy::Str
            }
            CoreBinOp::Eq
            | CoreBinOp::Ne
            | CoreBinOp::Lt
            | CoreBinOp::Le
            | CoreBinOp::Gt
            | CoreBinOp::Ge => {
                let cop = bin_cmp(op).expect("cmp op");
                self.emit(OxInst::Compare {
                    dst,
                    op: cop,
                    lhs: l,
                    rhs: r,
                    mode,
                });
                // A comparison yields Boolean for non-Variant operands, but can yield
                // Null (a Variant) when an operand could be Null.
                if l_ty.is_variant() || r_ty.is_variant() {
                    OxTy::Variant
                } else {
                    OxTy::Bool
                }
            }
            CoreBinOp::And
            | CoreBinOp::Or
            | CoreBinOp::Xor
            | CoreBinOp::Eqv
            | CoreBinOp::Imp => {
                let lop = logical_op(op).expect("logical op");
                self.emit(OxInst::Logical {
                    dst,
                    op: lop,
                    lhs: l,
                    rhs: r,
                });
                // Bitwise-or-logical result type follows the numeric/bool lattice;
                // conservatively Variant (refined when the typed lattice lands).
                OxTy::Variant
            }
            CoreBinOp::Is => {
                self.emit(OxInst::CompareObjectIs { dst, lhs: l, rhs: r });
                OxTy::Bool
            }
            CoreBinOp::Like => return Err(unimpl("Like")),
        };
        Ok((OxOperand::temp(t), ty))
    }

    fn lower_coerce(&mut self, value: &CoreValue, to: &coreir::CoerceTarget) -> Result<(OxOperand, OxTy)> {
        use crate::value::OxCoerceTarget;
        let (src, _) = self.lower_value(value)?;
        let (target, ty) = match to {
            coreir::CoerceTarget::Numeric(n) => {
                (OxCoerceTarget::Numeric(*n), numeric_coerce_type(*n))
            }
            coreir::CoerceTarget::String => (OxCoerceTarget::Str, OxTy::Str),
            coreir::CoerceTarget::FixedString(len) => {
                (OxCoerceTarget::FixedStr(*len as u32), OxTy::FixedStr(*len as u32))
            }
            // An implicit Variant widening keeps the value; OxIR models it as the
            // identity-typed ImplicitVariant coercion.
            coreir::CoerceTarget::ImplicitVariant(_) => {
                (OxCoerceTarget::ImplicitVariant, OxTy::Variant)
            }
        };
        let t = self.new_temp();
        self.emit(OxInst::Coerce {
            dst: OxPlace::Temp(t),
            src,
            target,
        });
        Ok((OxOperand::temp(t), ty))
    }

    fn lower_call(&mut self, callee: &coreir::CoreCallee, args: &[CoreArg]) -> Result<(OxOperand, OxTy)> {
        match callee {
            coreir::CoreCallee::VbaProc { proc } => {
                let oargs = self.lower_proc_args(args)?;
                let dst_t = self.new_temp();
                self.emit(OxInst::CallProc {
                    dst: Some(OxPlace::Temp(dst_t)),
                    proc: FuncId(proc.0),
                    args: oargs,
                });
                // The proc's return type is recovered when the callee table is typed;
                // conservatively Variant for now.
                Ok((OxOperand::temp(dst_t), OxTy::Variant))
            }
            coreir::CoreCallee::Native(_) => Err(unimpl("base-library / builtin call")),
            coreir::CoreCallee::Declare { .. } => Err(unimpl("Declare Lib call")),
            coreir::CoreCallee::EarlyCom { .. } => Err(unimpl("early-bound COM call")),
            coreir::CoreCallee::LateDispatch { .. } => Err(unimpl("late-bound COM call")),
            coreir::CoreCallee::DynamicByName => Err(unimpl("CallByName")),
            coreir::CoreCallee::ExternProc { .. } => Err(unimpl("cross-bundle call")),
        }
    }

    fn lower_proc_args(&mut self, args: &[CoreArg]) -> Result<Vec<OxArg>> {
        let mut out = Vec::with_capacity(args.len());
        for arg in args {
            out.push(match arg {
                CoreArg::ByVal(v) => OxArg::ByVal(self.lower_value(v)?.0),
                CoreArg::ByRef(place) => OxArg::ByRef(self.simple_place(place)?),
                CoreArg::Omitted => OxArg::Omitted,
                CoreArg::Named { .. } => return Err(unimpl("named argument")),
            });
        }
        Ok(out)
    }

    // ── Places ───────────────────────────────────────────────────────────────

    /// Read a place into an operand, returning `(operand, type)`.
    fn lower_place_load(&mut self, place: &coreir::CorePlace) -> Result<(OxOperand, OxTy)> {
        match place {
            coreir::CorePlace::Local(l) => {
                let ty = self.locals[l.0].ty.clone();
                Ok((OxOperand::local(LocalId(l.0)), ty))
            }
            coreir::CorePlace::Global(g) => {
                // The lowerer does not yet hold the global type table, so a global load
                // is conservatively typed `Variant` (sound; threading the global types
                // into the lowerer is a noted refinement).
                Ok((OxOperand::Use(OxPlace::Global(GlobalId(g.0))), OxTy::Variant))
            }
            coreir::CorePlace::Field { .. } => Err(unimpl("object field access")),
            coreir::CorePlace::Index { .. } => Err(unimpl("array element access")),
            coreir::CorePlace::RecordField { .. } => Err(unimpl("UDT field access")),
            coreir::CorePlace::WithEvents { .. } => Err(unimpl("WithEvents sink access")),
        }
    }

    /// The destination place for a store (only simple variable places in the spine).
    fn lower_place_store(&mut self, place: &coreir::CorePlace) -> Result<OxPlace> {
        self.simple_place(place)
    }

    /// A simple (directly addressable) place: a local or a global.
    fn simple_place(&self, place: &coreir::CorePlace) -> Result<OxPlace> {
        match place {
            coreir::CorePlace::Local(l) => Ok(OxPlace::Local(LocalId(l.0))),
            coreir::CorePlace::Global(g) => Ok(OxPlace::Global(GlobalId(g.0))),
            coreir::CorePlace::Field { .. }
            | coreir::CorePlace::Index { .. }
            | coreir::CorePlace::RecordField { .. }
            | coreir::CorePlace::WithEvents { .. } => Err(unimpl("compound place")),
        }
    }

    fn place_as_operand(&mut self, place: &coreir::CorePlace) -> Result<OxOperand> {
        Ok(self.lower_place_load(place)?.0)
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

fn unimpl(what: &'static str) -> ElaborateError {
    ElaborateError::Unimplemented { what }
}

fn lower_const(c: &CoreConst) -> OxConst {
    match c {
        CoreConst::Empty => OxConst::Empty,
        CoreConst::Null => OxConst::Null,
        CoreConst::Nothing => OxConst::Nothing,
        CoreConst::Bool(b) => OxConst::Bool(*b),
        CoreConst::I32(n) => OxConst::I32(*n),
        CoreConst::I64(n) => OxConst::I64(*n),
        CoreConst::F64(bits) => OxConst::F64(*bits),
        CoreConst::F32(bits) => OxConst::F32(*bits),
        CoreConst::Currency(n) => OxConst::Currency(*n),
        CoreConst::Date(bits) => OxConst::Date(*bits),
        CoreConst::Str(s) => OxConst::Str(s.clone()),
    }
}

fn const_type(c: &CoreConst) -> OxTy {
    match c {
        // Empty / Null inhabit only Variant.
        CoreConst::Empty | CoreConst::Null => OxTy::Variant,
        // `Nothing` is a null object reference.
        CoreConst::Nothing => OxTy::Object(crate::ty::ObjClass::Untyped),
        CoreConst::Bool(_) => OxTy::Bool,
        CoreConst::I32(_) => OxTy::Long,
        CoreConst::I64(_) => OxTy::LongLong,
        CoreConst::F64(_) => OxTy::Double,
        CoreConst::F32(_) => OxTy::Single,
        CoreConst::Currency(_) => OxTy::Currency,
        CoreConst::Date(_) => OxTy::Date,
        CoreConst::Str(_) => OxTy::Str,
    }
}

/// The result type of a `Checked`/`Widening` arithmetic regime: a `Checked(t)` result
/// is the typed `t`; a `Widening` result is data-dependent → `Variant`.
fn numeric_result_type(mode: NumericMode) -> OxTy {
    match mode {
        NumericMode::Checked(t) => numeric_coerce_type(t),
        NumericMode::Widening => OxTy::Variant,
    }
}

fn numeric_coerce_type(t: oxvba_bundle::NumericCoerceTarget) -> OxTy {
    use oxvba_bundle::NumericCoerceTarget as N;
    match t {
        N::Byte => OxTy::Byte,
        N::Integer => OxTy::Integer,
        N::Long => OxTy::Long,
        N::LongLong => OxTy::LongLong,
        N::Single => OxTy::Single,
        N::Double => OxTy::Double,
        N::Currency => OxTy::Currency,
        N::Boolean => OxTy::Bool,
        N::Date => OxTy::Date,
    }
}

fn arith_op(op: CoreBinOp) -> Option<ArithOp> {
    Some(match op {
        CoreBinOp::Add => ArithOp::Add,
        CoreBinOp::Sub => ArithOp::Sub,
        CoreBinOp::Mul => ArithOp::Mul,
        CoreBinOp::IntDiv => ArithOp::IntDiv,
        CoreBinOp::Mod => ArithOp::Mod,
        _ => return None,
    })
}

fn bin_cmp(op: CoreBinOp) -> Option<CmpOp> {
    Some(match op {
        CoreBinOp::Eq => CmpOp::Eq,
        CoreBinOp::Ne => CmpOp::Ne,
        CoreBinOp::Lt => CmpOp::Lt,
        CoreBinOp::Le => CmpOp::Le,
        CoreBinOp::Gt => CmpOp::Gt,
        CoreBinOp::Ge => CmpOp::Ge,
        _ => return None,
    })
}

fn logical_op(op: CoreBinOp) -> Option<LogicalOp> {
    Some(match op {
        CoreBinOp::And => LogicalOp::And,
        CoreBinOp::Or => LogicalOp::Or,
        CoreBinOp::Xor => LogicalOp::Xor,
        CoreBinOp::Eqv => LogicalOp::Eqv,
        CoreBinOp::Imp => LogicalOp::Imp,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elaborate::ResolvedTypeName;
    use crate::verify::verify_program;
    use oxvba_bundle::coreir::{
        CoreIfArm, CoreLocal, CorePlace, ErrorOp, LocalId as CoreLocalId,
    };
    use oxvba_bundle::{
        AssignmentIntent, AssignmentTargetKind, BuiltinType, NumericCoerceTarget, ProcedureKind,
        StringCompareMode, VarTypeRef,
    };

    struct UntypedResolver;
    impl NameResolver for UntypedResolver {
        fn resolve_type_name(&self, _: &str) -> ResolvedTypeName {
            ResolvedTypeName::Untyped
        }
    }

    fn long_local(name: &str) -> CoreLocal {
        CoreLocal {
            name: name.to_string(),
            ty: VarTypeRef::Builtin(BuiltinType::Long),
            array_element: None,
        }
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

    fn sub(name: &str, locals: Vec<CoreLocal>, body: Vec<CoreStmt>) -> CoreProc {
        CoreProc {
            name: name.to_string(),
            kind: ProcedureKind::Sub,
            params: Vec::new(),
            locals,
            return_local: None,
            body,
        }
    }

    fn program(proc: CoreProc) -> CoreProgram {
        CoreProgram {
            procs: vec![proc],
            unit_name: "T".to_string(),
            ..Default::default()
        }
    }

    /// `Sub Main(): n = (10 + 5) * 2 : If n > 0 Then n = 1 : For n = 1 To 10 : Next`.
    fn scalar_proc() -> CoreProc {
        let n = CorePlace::Local(CoreLocalId(0));
        let long = NumericMode::Checked(NumericCoerceTarget::Long);
        let bin = |op, l, r, num| CoreValue::Binary {
            op,
            lhs: Box::new(l),
            rhs: Box::new(r),
            mode: StringCompareMode::Binary,
            num,
        };
        let n_eq = assign(
            n.clone(),
            bin(
                CoreBinOp::Mul,
                bin(
                    CoreBinOp::Add,
                    CoreValue::Const(CoreConst::I32(10)),
                    CoreValue::Const(CoreConst::I32(5)),
                    long,
                ),
                CoreValue::Const(CoreConst::I32(2)),
                long,
            ),
        );
        let if_stmt = CoreStmt::If {
            arms: vec![CoreIfArm {
                condition: bin(
                    CoreBinOp::Gt,
                    CoreValue::Load(n.clone()),
                    CoreValue::Const(CoreConst::I32(0)),
                    NumericMode::Widening,
                ),
                body: vec![assign(n.clone(), CoreValue::Const(CoreConst::I32(1)))],
            }],
            else_body: Vec::new(),
        };
        let for_stmt = CoreStmt::ForRange {
            var: n.clone(),
            start: CoreValue::Const(CoreConst::I32(1)),
            end: CoreValue::Const(CoreConst::I32(10)),
            step: None,
            body: Vec::new(),
        };
        sub("Main", vec![long_local("n")], vec![n_eq, if_stmt, for_stmt])
    }

    #[test]
    fn scalar_proc_elaborates_verifies_and_round_trips() {
        let prog = program(scalar_proc());
        let oxp = elaborate(&prog, &UntypedResolver).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()), "elaborated program must verify");

        let json = serde_json::to_string(&oxp).expect("serialize");
        let back: OxProgram = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(oxp, back, "elaborated program must round-trip");

        // The single function lowered to a real CFG (entry + the If/For blocks + the
        // shared exit), with the binder's `Long` type recovered onto the local.
        let f = &oxp.funcs[0];
        assert!(f.blocks.len() > 3, "expected a multi-block CFG, got {}", f.blocks.len());
        assert_eq!(f.locals[0].ty, OxTy::Long);
    }

    #[test]
    fn select_and_do_loop_elaborate_and_verify() {
        let n = CorePlace::Local(CoreLocalId(0));
        let do_loop = CoreStmt::DoLoop {
            condition: CoreValue::Binary {
                op: CoreBinOp::Lt,
                lhs: Box::new(CoreValue::Load(n.clone())),
                rhs: Box::new(CoreValue::Const(CoreConst::I32(10))),
                mode: StringCompareMode::Binary,
                num: NumericMode::Widening,
            },
            until: false,
            post_check: false,
            body: vec![CoreStmt::Exit(coreir::ExitKind::Do)],
        };
        let select = CoreStmt::Select {
            selector: CoreValue::Load(n.clone()),
            cases: vec![coreir::CoreCaseBlock {
                clauses: vec![
                    coreir::CaseClause::Value(CoreValue::Const(CoreConst::I32(1))),
                    coreir::CaseClause::Range {
                        lo: CoreValue::Const(CoreConst::I32(2)),
                        hi: CoreValue::Const(CoreConst::I32(5)),
                    },
                ],
                body: vec![assign(n.clone(), CoreValue::Const(CoreConst::I32(0)))],
            }],
            case_else: Vec::new(),
        };
        let prog = program(sub("Main", vec![long_local("n")], vec![do_loop, select]));
        let oxp = elaborate(&prog, &UntypedResolver).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
    }

    #[test]
    fn deferred_constructs_are_explicit_unimplemented() {
        // `With` is deferred to the object step — it must fail explicitly, never
        // silently mis-lower.
        let prog = program(sub(
            "Main",
            Vec::new(),
            vec![CoreStmt::With {
                id: 0,
                receiver: CoreValue::Const(CoreConst::Nothing),
                body: Vec::new(),
            }],
        ));
        let err = elaborate(&prog, &UntypedResolver).expect_err("With must be deferred");
        assert!(
            matches!(err, ElaborateError::Unimplemented { what: "With" }),
            "expected Unimplemented(With), got {err:?}"
        );
    }

    /// `On Error GoTo H : x = 1/0 : Exit Sub : H: Resume Next` exercises the full
    /// error model — `SetErrorHandler`, a faulting statement's `FaultDispatch` pad, a
    /// label block, `Exit Sub`, and a `Resume Next` terminator.
    #[test]
    fn error_handling_elaborates_and_verifies() {
        let n = CorePlace::Local(CoreLocalId(0));
        let div = CoreValue::Binary {
            op: CoreBinOp::Div,
            lhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
            rhs: Box::new(CoreValue::Const(CoreConst::I32(0))),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        };
        let body = vec![
            CoreStmt::Error(ErrorOp::OnErrorGotoLabel(coreir::LabelId(0))),
            assign(n.clone(), div),
            CoreStmt::Exit(coreir::ExitKind::Proc),
            CoreStmt::Label(coreir::LabelId(0)),
            CoreStmt::Error(ErrorOp::ResumeNext),
        ];
        let prog = program(sub("Main", vec![long_local("n")], body));
        let oxp = elaborate(&prog, &UntypedResolver).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()), "error-handling program must verify");

        // The handler-dispatch pad + Resume terminator are present.
        let f = &oxp.funcs[0];
        let has_fault_dispatch = f
            .blocks
            .iter()
            .any(|b| matches!(b.terminator, OxTerminator::FaultDispatch { .. }));
        let has_resume_next = f
            .blocks
            .iter()
            .any(|b| matches!(b.terminator, OxTerminator::ResumeNext));
        let has_set_handler = f.blocks.iter().any(|b| {
            b.instrs
                .iter()
                .any(|i| matches!(i, OxInst::SetErrorHandler(ErrorHandler::GotoLabel(_))))
        });
        assert!(has_fault_dispatch, "expected a FaultDispatch landing pad");
        assert!(has_resume_next, "expected a Resume Next terminator");
        assert!(has_set_handler, "expected On Error GoTo to set the handler");
    }

    /// `GoTo`/labels and `GoSub`/`Return` lower to the corresponding terminators.
    #[test]
    fn goto_and_gosub_elaborate_and_verify() {
        let body = vec![
            CoreStmt::GoSub(coreir::LabelId(0)),
            CoreStmt::Goto(coreir::LabelId(1)),
            CoreStmt::Label(coreir::LabelId(0)),
            CoreStmt::GoSubReturn,
            CoreStmt::Label(coreir::LabelId(1)),
        ];
        let prog = program(sub("Main", Vec::new(), body));
        let oxp = elaborate(&prog, &UntypedResolver).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));

        let f = &oxp.funcs[0];
        assert!(
            f.blocks
                .iter()
                .any(|b| matches!(b.terminator, OxTerminator::GoSub { .. })),
            "expected a GoSub terminator"
        );
        assert!(
            f.blocks
                .iter()
                .any(|b| matches!(b.terminator, OxTerminator::GoSubReturn)),
            "expected a GoSubReturn terminator"
        );
        // Round-trips with the new terminators.
        let json = serde_json::to_string(&oxp).expect("serialize");
        let back: OxProgram = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(oxp, back);
    }
}
