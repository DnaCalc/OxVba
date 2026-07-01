//! The Core IR → OxIR elaboration: structured tree to typed basic-block CFG.
//!
//! This is the spine of the pass. It builds the CFG (mirroring the proven `linearize`
//! lowering, but emitting blocks + terminators instead of flat labelled ops), recovers
//! each binding's [`OxTy`] from the type the binder recorded on the Core IR
//! ([`crate::elaborate::lower_var_type`]), and marks statement boundaries.
//!
//! # Coverage
//!
//! The whole **procedural core**: scalars, all control flow, the full error/control
//! model (`On Error` `Resume Next`/`GoTo h`/`GoTo 0`, `Resume`/`Resume Next`/`Resume
//! <label>`, `Err.Raise`/`Error <n>`, `GoSub`/`Return`, `GoTo`/labels), `With`,
//! `For Each`, arrays (literal/`LBound`/`UBound`/`ReDim`/`Erase`/element access), UDT
//! records, the pointer helpers / `Err` fields / `AddressOf`, and **every call kind**
//! (`VbaProc`, base-library/`Declare` native calls, cross-bundle `ExternProc`,
//! `CallByName`, and both COM paths — see below).
//!
//! The **object surface**: `New`/predeclared (extern) instances, object field get/set,
//! `WithEvents` sink get/set, `RaiseEvent`, and `TypeOf … Is`. The project class table
//! is projected from the Core IR, and a declared project-class type name resolves a
//! local to a typed `Object(Class(_))`.
//!
//! **Typed COM**: an early-bound call ([`coreir::CoreCallee::EarlyCom`]) interns its
//! resolved member descriptor into the typed interface table ([`OxProgram::com_interfaces`])
//! and lowers to a descriptor-keyed [`OxInst::ComCallEarly`]; a late-bound call
//! ([`coreir::CoreCallee::LateDispatch`]) lowers to the dynamic by-name
//! [`OxInst::ComCallLate`]. (Precise typed-COM *receiver* identity — typing a
//! `Dim r As Excel.Range` local as `Object(ComIface(_))` rather than `Untyped` — is a
//! later refinement; each call already carries its own typed descriptor.)
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
    self, CoreArg, CoreBinOp, CoreClass, CoreConst, CoreProc, CoreProgram, CoreStmt, CoreUnOp,
    CoreValue, ErrField, ErrorOp, LabelId as CoreLabelId,
};
use oxvba_bundle::{
    AssignmentIntent, AssignmentTargetKind, NumericCoerceTarget, NumericMode, ProjectMemberKind,
};

use oxvba_com::{TypeLibInterfaceMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata};

use crate::com::{ComInterface, ComMethodRef};
use crate::elaborate::{NameResolver, ResolvedTypeName, lower_var_type};
use crate::ids::{BlockId, FuncId, GlobalId, ImportId, LocalId, TempId};
use crate::inst::{ErrorHandler, OxBlock, OxInst, OxTerminator};
use crate::program::{OxClass, OxClassMethod, OxFunc, OxGlobal, OxLocal, OxParamInfo, OxProgram};
use crate::ty::{ArrayShape, ClassId, IfaceId, ObjClass, OxTy};
use crate::value::{
    ArithOp, CmpOp, DeclarePtrWriteback, LogicalOp, OxArg, OxCallArg, OxCoerceTarget, OxConst,
    OxNativeCallee, OxOperand, OxPlace,
};

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

/// Elaborate a whole compilation unit. The resolver that classifies declared type
/// names into typed identities is built from the program itself (its project-class
/// table), so callers supply only the program.
pub fn elaborate(program: &CoreProgram) -> Result<OxProgram> {
    let resolver = ProgramResolver::new(program);

    let globals = program
        .globals
        .iter()
        .map(|g| OxGlobal {
            name: g.name.clone(),
            ty: lower_var_type(&g.ty, &resolver),
        })
        .collect();

    // The typed COM interface table is built as early-bound calls are lowered: each
    // call's resolved member is interned into it, and the call names the member by a
    // stable `ComMethodRef`.
    let mut com = ComInterner::default();

    let mut funcs = Vec::with_capacity(program.procs.len());
    for proc in &program.procs {
        funcs.push(elaborate_proc(proc, &resolver, &mut com)?);
    }

    let classes = program.classes.iter().map(lower_class).collect();

    Ok(OxProgram {
        funcs,
        globals,
        classes,
        com_interfaces: com.interfaces,
        // Resolve the entry the same way `linearize`'s `select_entry` does, so vm3/JIT
        // run the same proc vm2 runs (and expose the same entry frame): the recorded
        // entry, else a case-insensitive `Main`, else the first proc (`None` only for a
        // proc-less unit). Keeps `OxProgram.entry` the single source of truth.
        entry: program
            .entry
            .map(|p| FuncId(p.0))
            .or_else(|| {
                program
                    .procs
                    .iter()
                    .position(|p| p.name.eq_ignore_ascii_case("main"))
                    .map(FuncId)
            })
            .or_else(|| (!program.procs.is_empty()).then_some(FuncId(0))),
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

/// Lower a Core IR project class to its OxIR form (the index is its [`ClassId`]; the
/// lifecycle hooks and late-bound member table map across 1:1).
fn lower_class(c: &CoreClass) -> OxClass {
    OxClass {
        name: c.name.clone(),
        initialize: c.initialize.map(|p| FuncId(p.0)),
        terminate: c.terminate.map(|p| FuncId(p.0)),
        methods: c
            .methods
            .iter()
            .map(|m| OxClassMethod {
                name: m.name.clone(),
                kind: m.kind,
                proc: FuncId(m.proc.0),
                is_default_member: m.is_default_member,
            })
            .collect(),
        implements: c.implements.clone(),
    }
}

/// A [`NameResolver`] built from a [`CoreProgram`]: a declared type name that matches a
/// project class (case-insensitively, by full name) is a typed `Class` instance;
/// everything else — referenced COM coclasses, UDT records, `Enum`s — is the
/// conservative [`ResolvedTypeName::Untyped`]. Precise COM-interface, record-layout and
/// enum typing are later de-erasure steps (a cross-project-qualified class name like
/// `Lib.Widget` is correctly *not* a class of this unit and stays untyped here, dispatched
/// through the cross-bundle import path).
struct ProgramResolver {
    classes: HashMap<String, ClassId>,
}

impl ProgramResolver {
    fn new(program: &CoreProgram) -> Self {
        let classes = program
            .classes
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.to_ascii_lowercase(), ClassId(i)))
            .collect();
        Self { classes }
    }
}

impl NameResolver for ProgramResolver {
    fn resolve_type_name(&self, name: &str) -> ResolvedTypeName {
        match self.classes.get(&name.to_ascii_lowercase()) {
            Some(&id) => ResolvedTypeName::Class(id),
            None => ResolvedTypeName::Untyped,
        }
    }
}

/// Builds the program's typed COM interface table ([`OxProgram::com_interfaces`]) as
/// early-bound calls are lowered. Every call's resolved member descriptor is interned —
/// deduplicated within its declared receiver interface — so the table holds each used
/// member exactly once and the call site names it by a stable [`ComMethodRef`].
/// Interfaces are grouped by the declared receiver type name (case-insensitively, e.g.
/// `Excel.Range`); a member is identified within its interface by its dispid + accessor,
/// so a get and a put that share a dispid are distinct entries. (Grouping by the
/// per-member `interface_iid` and reconciling alias names is a later refinement; the
/// per-member IID is preserved on each descriptor regardless.)
#[derive(Default)]
struct ComInterner {
    interfaces: Vec<ComInterface>,
    iface_of: HashMap<String, usize>,
    method_of: HashMap<(String, i32, TypeLibMemberInvokeKind), ComMethodRef>,
}

impl ComInterner {
    fn intern(&mut self, interface_name: &str, member: &TypeLibMemberMetadata) -> ComMethodRef {
        let folded = interface_name.to_ascii_lowercase();
        let key = (folded.clone(), member.token, member.invoke_kind);
        if let Some(&existing) = self.method_of.get(&key) {
            return existing;
        }
        let iface = *self.iface_of.entry(folded).or_insert_with(|| {
            let idx = self.interfaces.len();
            self.interfaces
                .push(ComInterface::Com(TypeLibInterfaceMetadata {
                    name: interface_name.to_string(),
                    iid: member.interface_iid,
                    members: Vec::new(),
                }));
            idx
        });
        let member_pos = match &mut self.interfaces[iface] {
            ComInterface::Com(meta) => {
                let pos = meta.members.len();
                meta.members.push(member.clone());
                pos
            }
            // The interner only ever creates `Com` entries.
            ComInterface::Project(_) => unreachable!("interner builds only COM interfaces"),
        };
        let mref = ComMethodRef {
            iface: IfaceId(iface),
            member: member_pos,
        };
        self.method_of.insert(key, mref);
        mref
    }
}

/// Map a call-site member kind to the COM invoke kind (the inverse of the binder's
/// `member_kind_from_invoke`); `None` (an unkinded call) is a `Method`.
fn invoke_kind_from_member_kind(kind: Option<ProjectMemberKind>) -> TypeLibMemberInvokeKind {
    match kind {
        Some(ProjectMemberKind::PropertyGet) => TypeLibMemberInvokeKind::PropertyGet,
        Some(ProjectMemberKind::PropertyLet) => TypeLibMemberInvokeKind::PropertyPut,
        Some(ProjectMemberKind::PropertySet) => TypeLibMemberInvokeKind::PropertyPutRef,
        Some(ProjectMemberKind::Method) | None => TypeLibMemberInvokeKind::Method,
    }
}

fn elaborate_proc(
    proc: &CoreProc,
    resolver: &impl NameResolver,
    com: &mut ComInterner,
) -> Result<OxFunc> {
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

    // The binding types are recovered up front (the `locals` above, via the resolver);
    // the lowerer carries the COM interner so early-bound calls intern their members.
    let mut lo = Lowerer::new(locals, proc.params.len(), proc.label_lines.clone(), com);
    // Pre-assign a block to every source label so forward references resolve.
    lo.assign_labels(&proc.body)?;
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

/// A copy-out for a compound `ByRef` argument: the materialized `temp` (aliased ByRef for the
/// call) is stored back into the compound `place` it was copied from once the call returns —
/// but ONLY if it actually changed (`temp != original`), mirroring vm2's `VariantChanged`
/// guard so an unchanged compound `ByRef` param never clobbers an out-of-band mutation of the
/// same place (or needlessly re-runs a `WithEvents` (un)subscribe).
struct ArgWriteback {
    place: coreir::CorePlace,
    temp: OxPlace,
    /// The copied-in snapshot, captured at copy-in time; the write-back runs only when
    /// `temp != original`.
    original: OxPlace,
}

struct Lowerer<'a> {
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
    /// Numeric line metadata parallel to Core label ids.
    label_lines: Vec<Option<i32>>,
    /// The temp holding each in-scope `With` receiver (by the binder's `With` id), so
    /// `WithTemp(id)` references read it.
    with_temps: HashMap<usize, TempId>,
    /// The program-level typed-COM interface-table builder (shared across procs); an
    /// early-bound call interns its resolved member here and names it by a `ComMethodRef`.
    com: &'a mut ComInterner,
}

impl<'a> Lowerer<'a> {
    fn new(
        locals: Vec<OxLocal>,
        param_count: usize,
        label_lines: Vec<Option<i32>>,
        com: &'a mut ComInterner,
    ) -> Self {
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
            label_lines,
            with_temps: HashMap::new(),
            com,
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
        // A block needs a fault pad if it can raise: either a fallible instruction OR a
        // fallible terminator (`Raise`/`RaiseValue`). Without the terminator clause, a
        // bare `Err.Raise`/`Error n` statement (whose block holds only `StmtBoundary` and
        // the raise terminator) would carry no `fault_target`, so `On Error` could not
        // catch it.
        let fault_target = (instrs.iter().any(|i| i.is_fallible()) || term.is_fallible())
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

    /// Pre-assign a block to every source label *defined* in `body` (recursively), so
    /// forward references resolve. Runs before lowering. A label defined twice is
    /// malformed (a VBA compile error), so it is rejected here rather than silently
    /// overwriting a block during lowering.
    fn assign_labels(&mut self, body: &[CoreStmt]) -> Result<()> {
        for stmt in body {
            match stmt {
                CoreStmt::Label(id) => {
                    if self.labels.contains_key(id) {
                        return Err(ElaborateError::Malformed(format!(
                            "label {id:?} defined more than once"
                        )));
                    }
                    let b = self.reserve();
                    self.labels.insert(*id, b);
                }
                CoreStmt::If { arms, else_body } => {
                    for arm in arms {
                        self.assign_labels(&arm.body)?;
                    }
                    self.assign_labels(else_body)?;
                }
                CoreStmt::DoLoop { body, .. }
                | CoreStmt::ForRange { body, .. }
                | CoreStmt::ForEach { body, .. }
                | CoreStmt::With { body, .. } => self.assign_labels(body)?,
                CoreStmt::Select {
                    cases, case_else, ..
                } => {
                    for c in cases {
                        self.assign_labels(&c.body)?;
                    }
                    self.assign_labels(case_else)?;
                }
                _ => {}
            }
        }
        Ok(())
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
            CoreStmt::Assign {
                place,
                value,
                intent,
                target_kind,
                target_name,
                target_type_name,
            } => {
                let (src, src_ty) = self.lower_value(value)?;
                // A `Set` or object-typed assignment carries a run-time legality check
                // (e.g. error 424 "Object required"), matching the linearize lowering.
                if *intent == AssignmentIntent::Set
                    || *target_kind == AssignmentTargetKind::Object
                    || matches!(src_ty, OxTy::Object(_))
                {
                    self.emit(OxInst::ValidateAssignment {
                        src: src.clone(),
                        intent: *intent,
                        target_kind: *target_kind,
                        target_name: target_name.clone(),
                        target_type_name: target_type_name.clone(),
                    });
                }
                self.store_to_place(place, src)?;
                self.finish_to(OxTerminator::Jump(s_next), s_next);
                Ok(())
            }
            CoreStmt::Eval(value) => {
                // Evaluate for effect; the result (if any) is discarded. A call in
                // statement position is lowered with no result destination.
                match value {
                    CoreValue::Call { callee, args } => {
                        self.lower_call_into(None, callee, args)?;
                    }
                    other => {
                        let _ = self.lower_value(other)?;
                    }
                }
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
                compare_mode,
            } => self.lower_select(selector, cases, case_else, *compare_mode, s_next),
            CoreStmt::Exit(kind) => {
                let target = self.exit_target(*kind)?;
                self.finish_to(OxTerminator::Jump(target), s_next);
                Ok(())
            }
            CoreStmt::Label(id) => {
                // The label's start block IS this statement's start (resolved by
                // `stmt_start_block`); it simply falls through to the next statement.
                if let Some(Some(line)) = self.label_lines.get(id.0) {
                    self.emit(OxInst::SetLineNumber { line: *line });
                }
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
            CoreStmt::ComputedGoto {
                selector,
                targets,
                is_gosub,
            } => self.lower_computed_goto(selector, targets, *is_gosub, s_next),
            CoreStmt::GoSubReturn => {
                self.finish_to(OxTerminator::GoSubReturn, s_next);
                Ok(())
            }
            CoreStmt::End => {
                self.finish_to(OxTerminator::Halt, s_next);
                Ok(())
            }
            CoreStmt::Error(op) => self.lower_error(op, s_next),
            CoreStmt::ReDim {
                array,
                bounds,
                element_type,
                preserve,
                fixed,
            } => {
                let mut upper_bounds = Vec::with_capacity(bounds.len());
                let mut lower_bounds = Vec::with_capacity(bounds.len());
                for b in bounds {
                    upper_bounds.push(self.lower_value(&b.upper)?.0);
                    lower_bounds.push(self.lower_value(&b.lower)?.0);
                }
                // A simple target is resized in place; a COMPOUND target (e.g. a member
                // array `ReDim b.arr(n)`) builds the new array into a temp, which is then
                // written back into the nested base (materialize-and-write-back).
                let compound = !Self::is_simple_place(array);
                let dst = if compound {
                    // `Preserve` needs the base's current array as the temp's starting
                    // value; a plain `ReDim` overwrites it, but seeding the temp keeps the
                    // redim's element type / preserve logic uniform with the simple path.
                    let cur = self.place_as_operand(array)?;
                    let t = self.new_temp();
                    self.emit(OxInst::Assign {
                        dst: OxPlace::Temp(t),
                        value: cur,
                    });
                    OxPlace::Temp(t)
                } else {
                    self.simple_place(array)?
                };
                self.emit(OxInst::ArrayRedim {
                    dst,
                    upper_bounds,
                    lower_bounds,
                    element: element_type.clone(),
                    preserve: *preserve,
                    fixed: *fixed,
                });
                if compound {
                    self.store_to_place(array, OxOperand::Use(dst))?;
                }
                self.finish_to(OxTerminator::Jump(s_next), s_next);
                Ok(())
            }
            CoreStmt::Erase {
                array,
                element_type,
            } => {
                // A simple target is erased in place; a COMPOUND target (e.g. a member
                // array `Erase b.arr`) materializes the base's current array into a temp,
                // erases that, then writes it back into the nested base — the same
                // materialize-and-write-back the ReDim arm uses, preserving the
                // element-type-aware erase for both dynamic and fixed member arrays.
                let compound = !Self::is_simple_place(array);
                let arr = if compound {
                    let cur = self.place_as_operand(array)?;
                    let t = self.new_temp();
                    self.emit(OxInst::Assign {
                        dst: OxPlace::Temp(t),
                        value: cur,
                    });
                    OxPlace::Temp(t)
                } else {
                    self.simple_place(array)?
                };
                self.emit(OxInst::ArrayErase {
                    array: arr,
                    element: element_type.clone(),
                });
                if compound {
                    self.store_to_place(array, OxOperand::Use(arr))?;
                }
                self.finish_to(OxTerminator::Jump(s_next), s_next);
                Ok(())
            }
            CoreStmt::With {
                id,
                receiver,
                body,
            } => {
                // Evaluate the receiver once into a temp the body's `WithTemp(id)` reads.
                let (recv, _) = self.lower_value(receiver)?;
                let t = self.new_temp();
                self.emit(OxInst::Assign {
                    dst: OxPlace::Temp(t),
                    value: recv,
                });
                let prev = self.with_temps.insert(*id, t);
                self.lower_block(body)?;
                self.finish_to(OxTerminator::Jump(s_next), s_next);
                // Restore any shadowed outer `With` binding.
                match prev {
                    Some(p) => {
                        self.with_temps.insert(*id, p);
                    }
                    None => {
                        self.with_temps.remove(id);
                    }
                }
                Ok(())
            }
            CoreStmt::ForEach { item, source, body } => {
                let for_pad = self.cur_fault;
                let (src, _) = self.lower_value(source)?;
                let iter = self.new_temp();
                self.emit(OxInst::ForEachInit {
                    iter: OxPlace::Temp(iter),
                    source: src,
                });
                let head = self.reserve();
                let body_blk = self.reserve();
                self.finish_to(OxTerminator::Jump(head), head);
                // head: advance the iterator, branch on whether a value was produced.
                self.cur_fault = for_pad;
                let item_t = self.new_temp();
                let has_t = self.new_temp();
                self.emit(OxInst::ForEachNext {
                    iter: OxPlace::Temp(iter),
                    item: OxPlace::Temp(item_t),
                    has_value: OxPlace::Temp(has_t),
                });
                self.finish_to(
                    OxTerminator::Branch {
                        cond: OxOperand::temp(has_t),
                        then_blk: body_blk,
                        else_blk: s_next,
                    },
                    body_blk,
                );
                // body: bind the current item, then run the loop body.
                self.cur_fault = for_pad;
                self.store_to_place(item, OxOperand::temp(item_t))?;
                self.loops.push(LoopCtx {
                    is_for: true,
                    brk: s_next,
                });
                self.lower_block(body)?;
                self.loops.pop();
                self.finish_to(OxTerminator::Jump(head), s_next);
                Ok(())
            }
            CoreStmt::RaiseEvent {
                source,
                event,
                args,
            } => {
                let (src, _) = self.lower_value(source)?;
                let (event_args, writebacks) = self.lower_proc_args(args)?;
                self.emit(OxInst::RaiseEvent {
                    source: src,
                    event: *event,
                    args: event_args,
                });
                self.emit_arg_writebacks(writebacks)?;
                self.finish_to(OxTerminator::Jump(s_next), s_next);
                Ok(())
            }
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
            ErrorOp::OnErrorGotoMinus1 => {
                self.emit(OxInst::SetErrorHandler(ErrorHandler::GotoMinus1));
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
            ErrorOp::Raise {
                number,
                source,
                description,
                help_file,
                help_context,
                inherit,
            } => {
                // Evaluate Number, Source, Description, HelpFile, and HelpContext
                // left-to-right, then raise through the statement fault pad.
                let (num_op, _) = self.lower_value(number)?;
                let src_op = match source {
                    Some(s) => Some(self.lower_value(s)?.0),
                    None => None,
                };
                let desc_op = match description {
                    Some(d) => Some(self.lower_value(d)?.0),
                    None => None,
                };
                let help_file_op = match help_file {
                    Some(h) => Some(self.lower_value(h)?.0),
                    None => None,
                };
                let help_context_op = match help_context {
                    Some(h) => Some(self.lower_value(h)?.0),
                    None => None,
                };
                self.finish_to(
                    OxTerminator::Raise {
                        number: num_op,
                        source: src_op,
                        description: desc_op,
                        help_file: help_file_op,
                        help_context: help_context_op,
                        inherit: *inherit,
                    },
                    s_next,
                );
            }
            ErrorOp::SetErrField { field, value } => {
                let (src, _) = self.lower_value(value)?;
                self.emit(OxInst::ErrFieldSet { field: *field, src });
                self.finish_to(OxTerminator::Jump(s_next), s_next);
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

    /// Reduce a conditional operand to a **pre-computed Boolean** for a
    /// [`OxTerminator::Branch`] (its documented invariant) by emitting a fallible
    /// [`OxInst::Truthy`] (the `is_truthy` rule a conditional uses).
    ///
    /// This is emitted **unconditionally** — a statically-`Bool` operand is *not*
    /// guaranteed to be a runtime Boolean-tagged value, so the static type cannot be
    /// trusted: an unassigned `Dim b As Boolean` reads as `Empty`, `Not b` of an Empty
    /// `Boolean` is a `Long`, and a `Variant` comparison / `CBool(Null)` is `Null` — all
    /// of which the strict `Branch.as_bool` would otherwise reject. `Truthy` normalizes
    /// every case to a real Boolean (or, for a non-coercible `If "abc"`, faults through
    /// the enclosing statement's pad — *not* out of the pure terminator). The redundant
    /// `Truthy` on an already-Boolean comparison result is idempotent and cheap.
    fn truthy_cond(&mut self, cond: OxOperand) -> OxOperand {
        let t = self.new_temp();
        self.emit(OxInst::Truthy {
            dst: OxPlace::Temp(t),
            src: cond,
        });
        OxOperand::temp(t)
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
            let cond = self.truthy_cond(cond);
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

    /// Lower a loop continuation condition, negating it for `Until`, and reduce it to a
    /// pre-computed Boolean for the loop's `Branch` (see [`Self::truthy_cond`]). The
    /// `Not` (for `Until`) mirrors the legacy lowering and is applied before the
    /// truthiness coercion, so vm3's branch-taken value is `is_truthy(<continuation>)` —
    /// matching vm2's `JumpIfZero` truthiness on the same continuation operand.
    fn lower_loop_condition(&mut self, condition: &CoreValue, until: bool) -> Result<OxOperand> {
        let (cond, _) = self.lower_value(condition)?;
        let cond = if until {
            let t = self.new_temp();
            self.emit(OxInst::Not {
                dst: OxPlace::Temp(t),
                src: cond,
            });
            OxOperand::temp(t)
        } else {
            cond
        };
        Ok(self.truthy_cond(cond))
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
        let var_place = self.simple_place(var)?;
        let (var_op, counter_ty) = self.lower_place_load(var)?;
        // A fixed-integer counter OVERFLOWS at the increment when it would pass the
        // type's max (`For i As Integer = ... To 32767` raises Err 6 after the body
        // runs for 32767). A `Variant` counter promotes instead (Integer→Long), and
        // float counters effectively never hit their bound — both keep widening.
        let step_mode = match counter_ty {
            OxTy::Byte => NumericMode::Checked(NumericCoerceTarget::Byte),
            OxTy::Integer => NumericMode::Checked(NumericCoerceTarget::Integer),
            OxTy::Long => NumericMode::Checked(NumericCoerceTarget::Long),
            // vm3 targets Win64, where `LongPtr` is 64-bit.
            OxTy::LongLong | OxTy::LongPtr => NumericMode::Checked(NumericCoerceTarget::LongLong),
            _ => NumericMode::Widening,
        };

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
        // head: branch on step sign to the matching comparison. The compare can be Null
        // (a `Step <Null>`), so coerce to a Boolean for the Branch invariant.
        let nonneg_b = self.truthy_cond(OxOperand::temp(nonneg));
        self.finish_to(
            OxTerminator::Branch {
                cond: nonneg_b,
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
        // test: continue into the body or exit. The counter compare can be Null (a Null
        // For bound), so coerce to a Boolean for the Branch invariant (matches vm2's
        // is_truthy loop-exit).
        let cond_b = self.truthy_cond(OxOperand::temp(cond_t));
        self.finish_to(
            OxTerminator::Branch {
                cond: cond_b,
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
        // step: counter = counter + step. A fixed-integer counter uses Checked so
        // passing the type max raises Overflow (6); a Variant/float counter widens.
        // Belongs to the For statement, so restore its pad after the body.
        self.cur_fault = for_pad;
        self.emit(OxInst::Arith {
            dst: var_place,
            op: ArithOp::Add,
            lhs: var_op,
            rhs: OxOperand::temp(step_t),
            mode: step_mode,
        });
        self.finish_to(OxTerminator::Jump(head), after);
        Ok(())
    }

    fn lower_select(
        &mut self,
        selector: &CoreValue,
        cases: &[coreir::CoreCaseBlock],
        case_else: &[CoreStmt],
        compare_mode: oxvba_bundle::StringCompareMode,
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
            let matched = self.lower_case_match(&sel_op, &block.clauses, compare_mode)?;
            // The case-match is a Compare/Logical result that can be Null (a Null
            // selector), so coerce to a Boolean for the Branch invariant (a Null match
            // is then falsy, falling through to the next case / Case Else like vm2).
            let matched = self.truthy_cond(matched);
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

    /// `On <selector> GoTo/GoSub L1, L2, …` — a 1-based computed branch lowered to a
    /// chain of equality `Branch`es (the Select-Case shape; no new terminator). The
    /// selector is evaluated once (coerced to `Long`, as VBA rounds it); `targets[k-1]`
    /// is taken when `selector == k`; `0`/out-of-range falls through to `end`, while a
    /// negative selector raises error 5. For `GoSub`, a taken target returns to `end`
    /// (the statement after `On … GoSub`).
    fn lower_computed_goto(
        &mut self,
        selector: &CoreValue,
        targets: &[CoreLabelId],
        is_gosub: bool,
        end: BlockId,
    ) -> Result<()> {
        let pad = self.cur_fault;
        let (sel_raw, _) = self.lower_value(selector)?;
        // Evaluate once into a temp, coerced to Long (VBA rounds the selector; a
        // non-numeric selector faults to this statement's pad — error 13).
        let sel_t = self.new_temp();
        self.emit(OxInst::Coerce {
            dst: OxPlace::Temp(sel_t),
            src: sel_raw,
            target: OxCoerceTarget::Numeric(oxvba_bundle::NumericCoerceTarget::Long),
        });
        let sel_op = OxOperand::temp(sel_t);

        let neg_t = self.new_temp();
        self.emit(OxInst::Compare {
            dst: OxPlace::Temp(neg_t),
            op: CmpOp::Lt,
            lhs: sel_op.clone(),
            rhs: OxOperand::Const(OxConst::I32(0)),
            mode: oxvba_bundle::StringCompareMode::Binary,
        });
        let negative = self.truthy_cond(OxOperand::temp(neg_t));
        let raise_negative = self.reserve();
        let first_selector_check = self.reserve();
        self.finish_to(
            OxTerminator::Branch {
                cond: negative,
                then_blk: raise_negative,
                else_blk: first_selector_check,
            },
            raise_negative,
        );
        self.finish_to(
            OxTerminator::Raise {
                number: OxOperand::Const(OxConst::I32(5)),
                source: None,
                description: None,
                help_file: None,
                help_context: None,
                inherit: false,
            },
            first_selector_check,
        );

        for (i, label) in targets.iter().enumerate() {
            self.cur_fault = pad;
            let k = (i + 1) as i32;
            let cmp_t = self.new_temp();
            self.emit(OxInst::Compare {
                dst: OxPlace::Temp(cmp_t),
                op: CmpOp::Eq,
                lhs: sel_op.clone(),
                rhs: OxOperand::Const(OxConst::I32(k)),
                mode: oxvba_bundle::StringCompareMode::Binary,
            });
            let matched = self.truthy_cond(OxOperand::temp(cmp_t));
            let target_block = self.label_block(label)?;
            let next_blk = self.reserve();
            if is_gosub {
                let gosub_blk = self.reserve();
                self.finish_to(
                    OxTerminator::Branch {
                        cond: matched,
                        then_blk: gosub_blk,
                        else_blk: next_blk,
                    },
                    gosub_blk,
                );
                // A taken `GoSub` returns to the statement after `On … GoSub`.
                self.finish_to(
                    OxTerminator::GoSub {
                        target: target_block,
                        ret: end,
                    },
                    next_blk,
                );
            } else {
                self.finish_to(
                    OxTerminator::Branch {
                        cond: matched,
                        then_blk: target_block,
                        else_blk: next_blk,
                    },
                    next_blk,
                );
            }
        }
        // No target matched (0 / out-of-range) — fall through to the next statement.
        self.finish_to(OxTerminator::Jump(end), end);
        Ok(())
    }

    /// Lower one case's clause list to a single Boolean-ish operand (the clauses are
    /// OR-ed together).
    fn lower_case_match(
        &mut self,
        sel: &OxOperand,
        clauses: &[coreir::CaseClause],
        compare_mode: oxvba_bundle::StringCompareMode,
    ) -> Result<OxOperand> {
        let mut acc: Option<OxOperand> = None;
        for clause in clauses {
            let clause_bool = self.lower_case_clause(sel, clause, compare_mode)?;
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
        mode: oxvba_bundle::StringCompareMode,
    ) -> Result<OxOperand> {
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
            CoreValue::WithTemp(id) => {
                let t = self.with_temps.get(id).copied().ok_or_else(|| {
                    ElaborateError::Malformed(format!("unbound With receiver temp {id}"))
                })?;
                // The receiver's static type is the object step's concern; Variant here.
                Ok((OxOperand::temp(t), OxTy::Variant))
            }
            CoreValue::Ptr { kind, value } => {
                let (src, _) = self.lower_value(value)?;
                let t = self.new_temp();
                self.emit(OxInst::Ptr {
                    dst: OxPlace::Temp(t),
                    kind: *kind,
                    src,
                });
                // VarPtr/StrPtr/ObjPtr yield a pointer-width integer.
                Ok((OxOperand::temp(t), OxTy::LongPtr))
            }
            CoreValue::ErrField(field) => {
                let t = self.new_temp();
                self.emit(OxInst::ErrFieldGet {
                    dst: OxPlace::Temp(t),
                    field: *field,
                });
                let ty = match field {
                    ErrField::Number | ErrField::HelpContext | ErrField::LastDllError => {
                        OxTy::Long
                    }
                    ErrField::Description | ErrField::Source | ErrField::HelpFile => OxTy::Str,
                };
                Ok((OxOperand::temp(t), ty))
            }
            CoreValue::Erl => {
                let t = self.new_temp();
                self.emit(OxInst::ErlGet {
                    dst: OxPlace::Temp(t),
                });
                Ok((OxOperand::temp(t), OxTy::Long))
            }
            CoreValue::AddressOf(proc) => {
                let t = self.new_temp();
                self.emit(OxInst::LoadProcRef {
                    dst: OxPlace::Temp(t),
                    proc: FuncId(proc.0),
                });
                Ok((OxOperand::temp(t), OxTy::ProcRef))
            }
            // `New <Class>` — a fresh instance of a project class; its value is a typed
            // object reference (the binder's `New`s on the active project carry a
            // resolved `ClassId`). `Class_Initialize` runs at construction (fallible).
            CoreValue::New(class) => {
                let t = self.new_temp();
                let class = ClassId(class.0);
                self.emit(OxInst::NewObject {
                    dst: OxPlace::Temp(t),
                    class,
                });
                Ok((OxOperand::temp(t), OxTy::Object(ObjClass::Class(class))))
            }
            // `New <referenced class>` — an instance of a class in another bundle. Its
            // class table is unavailable here, so the receiver is an untyped reference
            // (dispatched late / cross-bundle).
            CoreValue::NewExtern { import } => {
                let t = self.new_temp();
                self.emit(OxInst::NewExtern {
                    dst: OxPlace::Temp(t),
                    import: ImportId(*import),
                });
                Ok((OxOperand::temp(t), OxTy::Object(ObjClass::Untyped)))
            }
            CoreValue::TypeOfIs { object, type_name } => {
                let (obj, _) = self.lower_value(object)?;
                let t = self.new_temp();
                self.emit(OxInst::TypeOfIs {
                    dst: OxPlace::Temp(t),
                    object: obj,
                    type_name: type_name.clone(),
                });
                Ok((OxOperand::temp(t), OxTy::Bool))
            }
            CoreValue::ArrayLiteral { elems, lower_bound } => {
                let mut ops = Vec::with_capacity(elems.len());
                for v in elems {
                    ops.push(self.lower_value(v)?.0);
                }
                let t = self.new_temp();
                self.emit(OxInst::ArrayLiteral {
                    dst: OxPlace::Temp(t),
                    values: ops,
                    lower_bound: *lower_bound,
                });
                // `Array(…)` yields a dynamic Variant array based at `Option Base`.
                Ok((
                    OxOperand::temp(t),
                    OxTy::Array(Box::new(OxTy::Variant), ArrayShape::Dynamic),
                ))
            }
            CoreValue::Bound {
                which,
                array,
                dimension,
            } => {
                let arr = self.lower_place_load(array)?.0;
                let dim = match dimension {
                    Some(d) => Some(self.lower_value(d)?.0),
                    None => None,
                };
                let t = self.new_temp();
                self.emit(OxInst::Bound {
                    dst: OxPlace::Temp(t),
                    which: *which,
                    array: arr,
                    dimension: dim,
                });
                Ok((OxOperand::temp(t), OxTy::Long))
            }
            CoreValue::NewRecord { fields } => {
                let t = self.new_temp();
                self.emit(OxInst::NewRecord {
                    dst: OxPlace::Temp(t),
                    fields: fields.clone(),
                });
                // A UDT record value; precise `Record(layout)` typing needs the record
                // layout table (a later step), so it is conservatively `Variant` here.
                Ok((OxOperand::temp(t), OxTy::Variant))
            }
            // A `VB_PredeclaredId` class → its lazily-created global singleton instance.
            CoreValue::Predeclared { class } => {
                let t = self.new_temp();
                let class = ClassId(class.0);
                self.emit(OxInst::Predeclared {
                    dst: OxPlace::Temp(t),
                    class,
                });
                Ok((OxOperand::temp(t), OxTy::Object(ObjClass::Class(class))))
            }
            CoreValue::PredeclaredExtern { import } => {
                let t = self.new_temp();
                self.emit(OxInst::PredeclaredExtern {
                    dst: OxPlace::Temp(t),
                    import: ImportId(*import),
                });
                Ok((OxOperand::temp(t), OxTy::Object(ObjClass::Untyped)))
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
            // `Not` is logical on a Boolean (→ Boolean) and bitwise otherwise: a
            // non-integer operand is coerced to an integer, so the result is `Long`
            // (matching the binder), never the operand's own type — and `Variant` stays
            // `Variant` (a `Not Null` is `Null`).
            CoreUnOp::Not => {
                let ty = match src_ty {
                    OxTy::Bool => OxTy::Bool,
                    OxTy::Variant => OxTy::Variant,
                    _ => OxTy::Long,
                };
                (
                    OxInst::Not {
                        dst: OxPlace::Temp(t),
                        src,
                    },
                    ty,
                )
            }
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
            CoreBinOp::Like => {
                // vm2 routes `a Like b` to the bespoke native `Like` builtin with a trailing
                // compare-mode flag (Text=1, Binary=0); mirror that so vm3 runs it through
                // the same `oxvba_lib` body as every other native builtin.
                let mode_flag = match mode {
                    oxvba_bundle::StringCompareMode::Text => 1,
                    oxvba_bundle::StringCompareMode::Binary => 0,
                };
                self.emit(OxInst::CallNative {
                    dst: Some(dst),
                    callee: OxNativeCallee::Builtin(oxvba_bundle::NativeImplId::Like),
                    args: vec![
                        OxCallArg::Operand(l),
                        OxCallArg::Operand(r),
                        OxCallArg::Const(mode_flag),
                    ],
                });
                OxTy::Variant
            }
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

    /// Lower a call as an expression: emit it with a fresh result temp and return that
    /// temp. The result type is recovered when the callee tables are typed;
    /// conservatively `Variant` for now.
    fn lower_call(
        &mut self,
        callee: &coreir::CoreCallee,
        args: &[CoreArg],
    ) -> Result<(OxOperand, OxTy)> {
        let t = self.new_temp();
        self.lower_call_into(Some(OxPlace::Temp(t)), callee, args)?;
        Ok((OxOperand::temp(t), OxTy::Variant))
    }

    /// Emit a call writing its result to `dst` (`None` in statement position, so a
    /// `Sub` / discarded result allocates no temp).
    fn lower_call_into(
        &mut self,
        dst: Option<OxPlace>,
        callee: &coreir::CoreCallee,
        args: &[CoreArg],
    ) -> Result<()> {
        match callee {
            coreir::CoreCallee::VbaProc { proc } => {
                let (args, writebacks) = self.lower_proc_args(args)?;
                self.emit(OxInst::CallProc {
                    dst,
                    proc: FuncId(proc.0),
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
            coreir::CoreCallee::Native(id) => {
                let (args, writebacks) = self.lower_call_args(args)?;
                self.emit(OxInst::CallNative {
                    dst,
                    callee: OxNativeCallee::Builtin(*id),
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
            coreir::CoreCallee::Declare {
                descriptor_id,
                ptr_writebacks,
            } => {
                let (args, arg_writebacks) = self.lower_call_args(args)?;
                let mut writebacks = Vec::with_capacity(ptr_writebacks.len());
                for wb in ptr_writebacks {
                    writebacks.push(DeclarePtrWriteback {
                        arg_index: wb.arg_index,
                        target: self.simple_place(&wb.target)?,
                        kind: wb.kind,
                    });
                }
                self.emit(OxInst::CallNative {
                    dst,
                    callee: OxNativeCallee::Declare {
                        descriptor_id: *descriptor_id,
                        ptr_writebacks: writebacks,
                    },
                    args,
                });
                self.emit_arg_writebacks(arg_writebacks)?;
            }
            coreir::CoreCallee::ExternProc { import } => {
                let (args, writebacks) = self.lower_proc_args(args)?;
                self.emit(OxInst::CallExtern {
                    dst,
                    import: ImportId(*import),
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
            coreir::CoreCallee::DynamicByName => {
                // args = [object, name, calltype, forwarded args…].
                let [obj, name, calltype, rest @ ..] = args else {
                    return Err(ElaborateError::Malformed(
                        "CallByName needs object, name, and calltype operands".into(),
                    ));
                };
                let object = self.lower_arg_value(obj)?;
                let name = self.lower_arg_value(name)?;
                let calltype = self.lower_arg_value(calltype)?;
                let (args, writebacks) = self.lower_call_args(rest)?;
                self.emit(OxInst::CallByName {
                    dst,
                    object,
                    name,
                    calltype,
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
            // Early-bound, descriptor-typed COM dispatch: intern the resolved member
            // into the typed interface table and name it by a stable `ComMethodRef`.
            coreir::CoreCallee::EarlyCom {
                kind,
                interface_name,
                member,
                ..
            } => {
                let (recv, args, writebacks) = self.lower_com_receiver_and_args(args)?;
                let method = self.com.intern(interface_name, member);
                self.emit(OxInst::ComCallEarly {
                    dst,
                    method,
                    invoke_kind: invoke_kind_from_member_kind(*kind),
                    recv,
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
            // Late-bound, by-name COM dispatch (the one dynamic COM path) — Variant args.
            coreir::CoreCallee::LateDispatch { name, kind } => {
                let (recv, args, writebacks) = self.lower_com_receiver_and_args(args)?;
                self.emit(OxInst::ComCallLate {
                    dst,
                    recv,
                    name: name.clone(),
                    invoke_kind: invoke_kind_from_member_kind(*kind),
                    args,
                });
                self.emit_arg_writebacks(writebacks)?;
            }
        }
        Ok(())
    }

    /// Split a COM call's argument list into its receiver (`args[0]`, the typed
    /// interface pointer) and the lowered method arguments (`args[1..]`), plus the copy-out
    /// write-backs for any compound `ByRef` method argument. The binder always emits the
    /// receiver as the first argument of an `EarlyCom`/`LateDispatch`.
    fn lower_com_receiver_and_args(
        &mut self,
        args: &[CoreArg],
    ) -> Result<(OxOperand, Vec<OxCallArg>, Vec<ArgWriteback>)> {
        let (recv_arg, rest) = args.split_first().ok_or_else(|| {
            ElaborateError::Malformed("COM dispatch requires a receiver argument".into())
        })?;
        let recv = self.lower_arg_value(recv_arg)?;
        let (method_args, writebacks) = self.lower_call_args(rest)?;
        Ok((recv, method_args, writebacks))
    }

    /// Resolve a `ByRef` argument place into an addressable `OxPlace`.
    ///
    /// A simple base (local/global) aliases directly — no copy. A COMPOUND place
    /// (`Field`/`Index`/`RecordField`/`WithEvents`) has no directly addressable slot, so
    /// mirror vm2's linearize: copy the place's current value into a fresh temp, alias the
    /// temp ByRef, and record a write-back that stores the (possibly-mutated) temp back to
    /// the place after the call. Returns the place to alias plus an optional write-back.
    fn lower_byref_place(
        &mut self,
        place: &coreir::CorePlace,
    ) -> Result<(OxPlace, Option<ArgWriteback>)> {
        if Self::is_simple_place(place) {
            Ok((self.simple_place(place)?, None))
        } else {
            // Copy the place's current value into a fresh temp (the aliased ByRef slot), and
            // a SECOND `original` snapshot temp captured at copy-in, so the post-call
            // write-back can be change-gated (vm2's `Op::Copy` of `tmp` into `original`).
            let value = self.place_as_operand(place)?;
            let t = self.new_temp();
            self.emit(OxInst::Assign {
                dst: OxPlace::Temp(t),
                value,
            });
            let orig = self.new_temp();
            self.emit(OxInst::Assign {
                dst: OxPlace::Temp(orig),
                value: OxOperand::Use(OxPlace::Temp(t)),
            });
            Ok((
                OxPlace::Temp(t),
                Some(ArgWriteback {
                    place: place.clone(),
                    temp: OxPlace::Temp(t),
                    original: OxPlace::Temp(orig),
                }),
            ))
        }
    }

    /// Lower arguments for a compiled VBA / cross-bundle procedure call (`OxArg`), plus the
    /// copy-out write-backs for any compound `ByRef` argument (see [`Self::lower_byref_place`]).
    fn lower_proc_args(&mut self, args: &[CoreArg]) -> Result<(Vec<OxArg>, Vec<ArgWriteback>)> {
        let mut out = Vec::with_capacity(args.len());
        let mut writebacks = Vec::new();
        for arg in args {
            out.push(match arg {
                CoreArg::ByVal(v) => OxArg::ByVal(self.lower_value(v)?.0),
                CoreArg::ByRef(place) => {
                    let (p, wb) = self.lower_byref_place(place)?;
                    writebacks.extend(wb);
                    OxArg::ByRef(p)
                }
                CoreArg::Omitted => OxArg::Omitted,
                // A named argument to a project proc lowers to a positional value (the
                // binder has already resolved the position).
                CoreArg::Named { value, .. } => OxArg::ByVal(self.lower_value(value)?.0),
            });
        }
        Ok((out, writebacks))
    }

    /// Lower arguments for a native / late-bound call (`OxCallArg`, which keeps named
    /// arguments and ByRef copy-out distinct), plus the copy-out write-backs for any compound
    /// `ByRef` argument (see [`Self::lower_byref_place`]).
    fn lower_call_args(
        &mut self,
        args: &[CoreArg],
    ) -> Result<(Vec<OxCallArg>, Vec<ArgWriteback>)> {
        let mut out = Vec::with_capacity(args.len());
        let mut writebacks = Vec::new();
        for arg in args {
            out.push(match arg {
                CoreArg::ByVal(v) => OxCallArg::Operand(self.lower_value(v)?.0),
                CoreArg::ByRef(place) => {
                    let (p, wb) = self.lower_byref_place(place)?;
                    writebacks.extend(wb);
                    OxCallArg::ByRef(p)
                }
                CoreArg::Omitted => OxCallArg::Omitted,
                CoreArg::Named { name, value } => OxCallArg::Named {
                    name: name.clone(),
                    value: self.lower_value(value)?.0,
                },
            });
        }
        Ok((out, writebacks))
    }

    /// Emit the copy-out write-backs recorded by [`Self::lower_proc_args`] /
    /// [`Self::lower_call_args`]: store each materialized `ByRef` temp back into the compound
    /// place it was copied from, after the call has run — but ONLY when the value actually
    /// changed (`temp != original`), mirroring vm2's `VariantChanged` + `JumpIfZero` guard.
    /// The guard is a per-write-back block split: a `VariantChanged` test, a `Branch` to a
    /// store-block when changed (else straight to the merge), then a merge continuation.
    fn emit_arg_writebacks(&mut self, writebacks: Vec<ArgWriteback>) -> Result<()> {
        for wb in writebacks {
            let changed = self.new_temp();
            self.emit(OxInst::VariantChanged {
                dst: OxPlace::Temp(changed),
                current: OxOperand::Use(wb.temp),
                original: OxOperand::Use(wb.original),
            });
            let store_blk = self.reserve();
            let merge_blk = self.reserve();
            // `Branch` reads a pre-computed Boolean (`changed`); the store runs only when true.
            self.finish_to(
                OxTerminator::Branch {
                    cond: OxOperand::Use(OxPlace::Temp(changed)),
                    then_blk: store_blk,
                    else_blk: merge_blk,
                },
                store_blk,
            );
            self.store_to_place(&wb.place, OxOperand::Use(wb.temp))?;
            self.finish_to(OxTerminator::Jump(merge_blk), merge_blk);
        }
        Ok(())
    }

    /// Lower a call argument that must be a plain value operand (the object/name/
    /// calltype operands of `CallByName`).
    fn lower_arg_value(&mut self, arg: &CoreArg) -> Result<OxOperand> {
        match arg {
            CoreArg::ByVal(v) => Ok(self.lower_value(v)?.0),
            CoreArg::ByRef(place) => Ok(self.lower_place_load(place)?.0),
            CoreArg::Omitted | CoreArg::Named { .. } => Err(ElaborateError::Malformed(
                "expected a positional value operand".into(),
            )),
        }
    }

    // ── Places ───────────────────────────────────────────────────────────────

    /// Read a place into an operand, returning `(operand, type)`. Array-element and
    /// UDT-field reads recurse on their base (so nested reads like `m(j)(i)` and
    /// `p.q.x` work), then index/project it.
    fn lower_place_load(&mut self, place: &coreir::CorePlace) -> Result<(OxOperand, OxTy)> {
        match place {
            coreir::CorePlace::Local(l) => {
                let ty = self
                    .locals
                    .get(l.0)
                    .ok_or_else(|| {
                        ElaborateError::Malformed(format!("local index {} out of range", l.0))
                    })?
                    .ty
                    .clone();
                Ok((OxOperand::local(LocalId(l.0)), ty))
            }
            coreir::CorePlace::Global(g) => {
                // The lowerer does not yet hold the global type table, so a global load
                // is conservatively typed `Variant` (sound; threading the global types
                // into the lowerer is a noted refinement).
                Ok((OxOperand::Use(OxPlace::Global(GlobalId(g.0))), OxTy::Variant))
            }
            coreir::CorePlace::Index { array, indices } => {
                // Fuse `obj.field(i…)`: read one element of the field-held array IN PLACE
                // (vm3 `FieldArrayGet`) rather than `FieldGet` (which clones the whole
                // field array into a temp) followed by `ArrayGet`. Keeps element access
                // into a class-instance-field array O(1) instead of O(N) per access.
                if let coreir::CorePlace::Field { object, field } = array.as_ref() {
                    let obj = self.lower_value(object)?.0;
                    let indices = self.lower_indices(indices)?;
                    let t = self.new_temp();
                    self.emit(OxInst::FieldArrayGet {
                        dst: OxPlace::Temp(t),
                        object: obj,
                        field: *field,
                        indices,
                    });
                    return Ok((OxOperand::temp(t), OxTy::Variant));
                }
                // Fuse `rec.field(i…)`: read one element from the inline fixed-array
                // record field without materializing the entire field as a temporary
                // SAFEARRAY on every access.
                if let coreir::CorePlace::RecordField { base, index } = array.as_ref() {
                    let rec = self.lower_place_load(base)?.0;
                    let indices = self.lower_indices(indices)?;
                    let t = self.new_temp();
                    self.emit(OxInst::RecordArrayGet {
                        dst: OxPlace::Temp(t),
                        record: rec,
                        index: *index,
                        indices,
                    });
                    return Ok((OxOperand::temp(t), OxTy::Variant));
                }
                let (arr, arr_ty) = self.lower_place_load(array)?;
                let indices = self.lower_indices(indices)?;
                // Recover the element type from the array's static type when known.
                let elem_ty = match arr_ty {
                    OxTy::Array(elem, _) => *elem,
                    _ => OxTy::Variant,
                };
                let t = self.new_temp();
                self.emit(OxInst::ArrayGet {
                    dst: OxPlace::Temp(t),
                    array: arr,
                    indices,
                });
                Ok((OxOperand::temp(t), elem_ty))
            }
            coreir::CorePlace::RecordField { base, index } => {
                let rec = self.lower_place_load(base)?.0;
                let t = self.new_temp();
                self.emit(OxInst::RecordGet {
                    dst: OxPlace::Temp(t),
                    record: rec,
                    index: *index,
                });
                // Field typing needs the record layout table (a later step).
                Ok((OxOperand::temp(t), OxTy::Variant))
            }
            coreir::CorePlace::Field { object, field } => {
                let obj = self.lower_value(object)?.0;
                let t = self.new_temp();
                self.emit(OxInst::FieldGet {
                    dst: OxPlace::Temp(t),
                    object: obj,
                    field: *field,
                });
                // A field's static type needs the class field-type table (a later
                // step); conservatively `Variant` (a COM property accessor is dynamic).
                Ok((OxOperand::temp(t), OxTy::Variant))
            }
            coreir::CorePlace::WithEvents { owner, binding } => {
                let own = self.lower_value(owner)?.0;
                let t = self.new_temp();
                self.emit(OxInst::WithEventsGet {
                    dst: OxPlace::Temp(t),
                    owner: own,
                    binding: *binding,
                });
                // The sink holds an object reference.
                Ok((OxOperand::temp(t), OxTy::Object(ObjClass::Untyped)))
            }
        }
    }

    /// Store `value` into `place`. Simple variables become an `Assign`; array elements
    /// and UDT fields become a dedicated `ArraySet`/`RecordSet` on their base place.
    ///
    /// A one-level base (`arr(i) =`, `b.field =` where `arr`/`b` is a local or global) is
    /// the array/record place directly — vm3's `ArraySet`/`RecordSet` reads it, mutates
    /// the element, and writes the (alias-resolved) value back in place.
    ///
    /// A COMPOUND base — a nested mutable base such as `b.arr(i) =` (member array) or
    /// `o.x.y =` (nested UDT) — has no directly addressable `OxPlace`. Under VBA value
    /// semantics (UDT member arrays / fields are owned, not aliased) this is exactly
    /// materialize-and-write-back: read the base into a fresh temp, run the
    /// `ArraySet`/`RecordSet` against that temp, then **recursively store the mutated temp
    /// back into the base**. The recursion threads the write-back up through any depth of
    /// `Field`/`Index`/`RecordField` nesting.
    fn store_to_place(&mut self, place: &coreir::CorePlace, value: OxOperand) -> Result<()> {
        match place {
            coreir::CorePlace::Local(_) | coreir::CorePlace::Global(_) => {
                let dst = self.simple_place(place)?;
                self.emit(OxInst::Assign { dst, value });
            }
            coreir::CorePlace::Index { array, indices } => {
                // Fuse `obj.field(i…) = v`: mutate one element of the field-held array
                // IN PLACE (vm3 `FieldArraySet`) instead of materialising and writing the
                // whole field array back per access — O(1) field-array element writes.
                if let coreir::CorePlace::Field { object, field } = array.as_ref() {
                    let obj = self.lower_value(object)?.0;
                    let indices = self.lower_indices(indices)?;
                    self.emit(OxInst::FieldArraySet {
                        object: obj,
                        field: *field,
                        indices,
                        value,
                    });
                    return Ok(());
                }
                // Fuse `rec.field(i…) = v`: mutate the single inline fixed-array
                // element instead of materializing the whole field array and writing
                // it back. Compound record bases still use the existing materialize,
                // mutate, recursive-writeback path through `mutable_base_place`.
                if let coreir::CorePlace::RecordField { base, index } = array.as_ref() {
                    let indices = self.lower_indices(indices)?;
                    let (rec, compound) = self.mutable_base_place(base)?;
                    self.emit(OxInst::RecordArraySet {
                        record: rec,
                        index: *index,
                        indices,
                        value,
                    });
                    if compound {
                        self.store_to_place(base, OxOperand::Use(rec))?;
                    }
                    return Ok(());
                }
                let indices = self.lower_indices(indices)?;
                let (arr, compound) = self.mutable_base_place(array)?;
                self.emit(OxInst::ArraySet {
                    array: arr,
                    indices,
                    value,
                });
                // A compound base was materialized into a temp; write the mutated array
                // back into the nested base it came from.
                if compound {
                    self.store_to_place(array, OxOperand::Use(arr))?;
                }
            }
            coreir::CorePlace::RecordField { base, index } => {
                let (rec, compound) = self.mutable_base_place(base)?;
                self.emit(OxInst::RecordSet {
                    record: rec,
                    index: *index,
                    value,
                });
                // A compound base was materialized into a temp; write the mutated record
                // back into the nested base it came from.
                if compound {
                    self.store_to_place(base, OxOperand::Use(rec))?;
                }
            }
            coreir::CorePlace::Field { object, field } => {
                let obj = self.lower_value(object)?.0;
                self.emit(OxInst::FieldSet {
                    object: obj,
                    field: *field,
                    value,
                });
            }
            coreir::CorePlace::WithEvents { owner, binding } => {
                let own = self.lower_value(owner)?.0;
                // The sink store yields the previously-bound sink (released by the
                // assignment); a fresh temp receives it.
                let dst = self.new_temp();
                self.emit(OxInst::WithEventsSet {
                    dst: OxPlace::Temp(dst),
                    owner: own,
                    binding: *binding,
                    value,
                });
            }
        }
        Ok(())
    }

    fn lower_indices(&mut self, indices: &[CoreValue]) -> Result<Vec<OxOperand>> {
        let mut out = Vec::with_capacity(indices.len());
        for i in indices {
            out.push(self.lower_value(i)?.0);
        }
        Ok(out)
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

    /// True for a directly addressable place (a local or a global) — the cases
    /// [`simple_place`](Self::simple_place) accepts.
    fn is_simple_place(place: &coreir::CorePlace) -> bool {
        matches!(
            place,
            coreir::CorePlace::Local(_) | coreir::CorePlace::Global(_)
        )
    }

    /// Resolve the mutable base of an in-place array/record op into an addressable
    /// [`OxPlace`]. Returns `(place, compound)`:
    ///
    /// * a SIMPLE base (local/global) → `(that place, false)`: the op mutates it directly.
    /// * a COMPOUND base (a nested mutable base, e.g. a member array `b.arr` or a nested
    ///   field `o.x`) → `(Temp, true)`: the base's current value is materialized into a
    ///   fresh temp via the value-read path, and the `true` tells the caller it must write
    ///   the mutated temp back into the base afterwards (via `store_to_place`). This is
    ///   value-semantically identical to in-place mutation for VBA UDT member arrays and
    ///   fields, which are owned aggregates.
    fn mutable_base_place(&mut self, base: &coreir::CorePlace) -> Result<(OxPlace, bool)> {
        if Self::is_simple_place(base) {
            Ok((self.simple_place(base)?, false))
        } else {
            let value = self.place_as_operand(base)?;
            let t = self.new_temp();
            self.emit(OxInst::Assign {
                dst: OxPlace::Temp(t),
                value,
            });
            Ok((OxPlace::Temp(t), true))
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
        CoreConst::I16(n) => OxConst::I16(*n),
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
        CoreConst::I16(_) => OxTy::Integer,
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
    use crate::verify::{VerifyError, verify_program};
    use oxvba_bundle::coreir::{
        BoundWhich, CoreBound, CoreCallee, CoreClassMethod, CoreIfArm, CoreLocal, CorePlace,
        ErrField, ErrorOp, LocalId as CoreLocalId, PtrKind, ProcId as CoreProcId,
    };
    use oxvba_bundle::{
        ArrayElementType, AssignmentIntent, AssignmentTargetKind, BuiltinType, NativeImplId,
        NumericCoerceTarget, ProcedureKind, ProjectMemberKind, StringCompareMode, VarTypeRef,
    };

    fn long_local(name: &str) -> CoreLocal {
        CoreLocal {
            name: name.to_string(),
            ty: VarTypeRef::Builtin(BuiltinType::Long),
            array_element: None,
        }
    }

    fn variant_local(name: &str) -> CoreLocal {
        CoreLocal {
            name: name.to_string(),
            ty: VarTypeRef::Variant,
            array_element: None,
        }
    }

    /// A minimal typed COM member descriptor for the early-bound elaboration tests.
    fn com_member(name: &str, token: i32, invoke_kind: TypeLibMemberInvokeKind) -> TypeLibMemberMetadata {
        TypeLibMemberMetadata {
            name: name.into(),
            token,
            vtable_slot: Some(7),
            requires_argument: false,
            invoke_kind,
            parameter_names: Vec::new(),
            parameter_optional: Vec::new(),
            parameter_optional_defaults: Vec::new(),
            is_default_member: false,
            parameter_types: Vec::new(),
            parameter_wire_types: Vec::new(),
            parameter_iids: Vec::new(),
            return_type: None,
            return_wire_type: None,
            callconv_is_stdcall: true,
            is_dual: true,
            interface_iid: None,
            source_typekind: None,
            vtable_slot_bound: Some(16),
        }
    }

    /// All `ComMethodRef`s named by `ComCallEarly` instructions in the program.
    fn com_call_early_methods(oxp: &OxProgram) -> Vec<ComMethodRef> {
        oxp.funcs
            .iter()
            .flat_map(|f| &f.blocks)
            .flat_map(|b| &b.instrs)
            .filter_map(|i| match i {
                OxInst::ComCallEarly { method, .. } => Some(*method),
                _ => None,
            })
            .collect()
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
            label_lines: Vec::new(),
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
        let oxp = elaborate(&prog).expect("elaborate");
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
            compare_mode: oxvba_bundle::StringCompareMode::Binary,
        };
        let prog = program(sub("Main", vec![long_local("n")], vec![do_loop, select]));
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
    }

    #[test]
    fn every_conditional_gets_a_truthy_coercion() {
        // The Branch invariant: a conditional's `cond` must be a pre-computed Boolean,
        // and it is enforced UNCONDITIONALLY — a statically-Bool operand is not a
        // guaranteed runtime Boolean (an unassigned `Dim b As Boolean` is Empty, etc.),
        // so a `Truthy` is emitted before every conditional Branch, even a comparison.
        let n = || CorePlace::Local(CoreLocalId(0));
        let if_with = |cond| CoreStmt::If {
            arms: vec![CoreIfArm {
                condition: cond,
                body: vec![assign(n(), CoreValue::Const(CoreConst::I32(5)))],
            }],
            else_body: Vec::new(),
        };
        let has_truthy = |stmt| {
            let oxp = elaborate(&program(sub("Main", vec![long_local("n")], vec![stmt])))
                .expect("elaborate");
            assert_eq!(verify_program(&oxp), Ok(()));
            oxp.funcs[0]
                .blocks
                .iter()
                .flat_map(|b| &b.instrs)
                .any(|i| matches!(i, OxInst::Truthy { .. }))
        };
        // A bare non-Boolean literal condition.
        assert!(
            has_truthy(if_with(CoreValue::Const(CoreConst::I32(1)))),
            "a non-Boolean If condition must get a Truthy coercion"
        );
        // A comparison condition (statically Bool, but can be a Null Variant at runtime)
        // is coerced too.
        let cmp = CoreValue::Binary {
            op: CoreBinOp::Gt,
            lhs: Box::new(CoreValue::Load(n())),
            rhs: Box::new(CoreValue::Const(CoreConst::I32(0))),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        };
        assert!(
            has_truthy(if_with(cmp)),
            "a comparison condition is still coerced (its static Bool type is not a runtime guarantee)"
        );
    }

    #[test]
    fn late_bound_com_call_lowers_to_com_call_late() {
        // An untyped receiver `o.DoThing` (a `Property Let` write) lowers to a dynamic
        // by-name `ComCallLate`, its invoke kind recovered from the call-site kind.
        let prog = program(sub(
            "Main",
            vec![variant_local("o")],
            vec![CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::LateDispatch {
                    name: "DoThing".to_string(),
                    kind: Some(ProjectMemberKind::PropertyLet),
                },
                args: vec![CoreArg::ByVal(CoreValue::Load(CorePlace::Local(CoreLocalId(0))))],
            })],
        ));
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
        // No typed interface table is built for a purely late-bound program.
        assert!(oxp.com_interfaces.is_empty());
        let late = oxp.funcs[0]
            .blocks
            .iter()
            .flat_map(|b| &b.instrs)
            .find_map(|i| match i {
                OxInst::ComCallLate {
                    name, invoke_kind, ..
                } => Some((name.clone(), *invoke_kind)),
                _ => None,
            })
            .expect("a ComCallLate");
        assert_eq!(late.0, "DoThing");
        assert_eq!(late.1, TypeLibMemberInvokeKind::PropertyPut);
    }

    #[test]
    fn early_bound_com_call_lowers_typed_and_builds_the_interface_table() {
        // `Dim r As Excel.Range : x = r.Value` (read) and `r.Value = x` (write): two
        // early-bound dispatches whose resolved members (get + put share a dispid) are
        // interned as distinct entries of one interface; repeated reads reuse one entry.
        let getter = com_member("Value", 6, TypeLibMemberInvokeKind::PropertyGet);
        let setter = com_member("Value", 6, TypeLibMemberInvokeKind::PropertyPut);
        let r = || CorePlace::Local(CoreLocalId(0)); // Dim r As Excel.Range
        let x = || CorePlace::Local(CoreLocalId(1)); // Dim x
        let read = |g: &TypeLibMemberMetadata| CoreValue::Call {
            callee: CoreCallee::EarlyCom {
                name: "Value".into(),
                kind: Some(ProjectMemberKind::PropertyGet),
                interface_name: "Excel.Range".into(),
                member: Box::new(g.clone()),
            },
            args: vec![CoreArg::ByVal(CoreValue::Load(r()))],
        };
        let write = CoreValue::Call {
            callee: CoreCallee::EarlyCom {
                name: "Value".into(),
                kind: Some(ProjectMemberKind::PropertyLet),
                interface_name: "Excel.Range".into(),
                member: Box::new(setter),
            },
            args: vec![
                CoreArg::ByVal(CoreValue::Load(r())),
                CoreArg::ByVal(CoreValue::Load(x())),
            ],
        };
        let body = vec![
            assign(x(), read(&getter)),         // x = r.Value
            assign(x(), read(&getter)),         // x = r.Value (again — reuses the entry)
            CoreStmt::Eval(write),              // r.Value = x
        ];
        let locals = vec![
            CoreLocal {
                name: "r".into(),
                ty: VarTypeRef::Object("Excel.Range".into()),
                array_element: None,
            },
            variant_local("x"),
        ];
        let oxp = elaborate(&program(sub("Main", locals, body))).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
        // One interface (`Excel.Range`) holding the two distinct accessors of dispid 6.
        assert_eq!(oxp.com_interfaces.len(), 1);
        assert_eq!(oxp.com_interfaces[0].name(), "Excel.Range");
        assert_eq!(oxp.com_interfaces[0].com_members().unwrap().len(), 2);
        // Three early-bound calls; the two reads share one ComMethodRef, the write differs.
        let methods = com_call_early_methods(&oxp);
        assert_eq!(methods.len(), 3);
        assert_eq!(methods[0], methods[1], "repeated reads reuse one descriptor");
        assert_ne!(methods[0], methods[2], "get and put are distinct descriptors");
        // Each ComMethodRef resolves to its typed descriptor.
        let get_desc = oxp.com_method(methods[0]).expect("get descriptor");
        assert_eq!(get_desc.invoke_kind, TypeLibMemberInvokeKind::PropertyGet);
        let put_desc = oxp.com_method(methods[2]).expect("put descriptor");
        assert_eq!(put_desc.invoke_kind, TypeLibMemberInvokeKind::PropertyPut);
        assert_eq!(get_desc.token, 6);
        assert_eq!(put_desc.token, 6);
        // The whole typed-COM program round-trips structurally through JSON.
        let json = serde_json::to_string(&oxp).expect("serialize");
        let back: OxProgram = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(oxp, back, "typed-COM OxProgram must round-trip");
    }

    #[test]
    fn object_constructs_elaborate_and_verify() {
        // One project class `Widget`, exercising `New`, a predeclared singleton,
        // object field get/set, `TypeOf … Is`, `WithEvents` sink get/set, and
        // `RaiseEvent` — the object surface of the de-erasure.
        let w = || CorePlace::Local(CoreLocalId(0)); // Dim w As Widget
        let r = || CorePlace::Local(CoreLocalId(1)); // Dim r
        let b = CorePlace::Local(CoreLocalId(2)); // Dim b As Boolean
        let field = |f: i32| CorePlace::Field {
            object: Box::new(CoreValue::Load(w())),
            field: f,
        };
        let sink = || CorePlace::WithEvents {
            owner: Box::new(CoreValue::Load(w())),
            binding: 0,
        };
        let body = vec![
            assign(w(), CoreValue::New(coreir::ClassId(0))), // Set w = New Widget
            assign(r(), CoreValue::Load(field(5))),          // r = w.Value (field get)
            assign(field(5), CoreValue::Load(r())),          // w.Value = r (field set)
            assign(
                b.clone(),
                CoreValue::TypeOfIs {
                    object: Box::new(CoreValue::Load(w())),
                    type_name: "Widget".to_string(),
                },
            ),
            assign(w(), CoreValue::Predeclared { class: coreir::ClassId(0) }),
            assign(sink(), CoreValue::Load(w())),  // Set w.Sink = w (WithEvents set)
            assign(r(), CoreValue::Load(sink())),  // r = w.Sink (WithEvents get)
            CoreStmt::RaiseEvent {
                source: CoreValue::Load(w()),
                event: 3,
                args: Vec::new(),
            },
        ];
        let locals = vec![
            CoreLocal {
                name: "w".into(),
                ty: VarTypeRef::Object("Widget".into()),
                array_element: None,
            },
            variant_local("r"),
            CoreLocal {
                name: "b".into(),
                ty: VarTypeRef::Builtin(BuiltinType::Boolean),
                array_element: None,
            },
        ];
        let prog = CoreProgram {
            procs: vec![sub("Main", locals, body)],
            classes: vec![CoreClass {
                name: "Widget".into(),
                initialize: None,
                terminate: None,
                methods: vec![CoreClassMethod {
                    name: "Value".into(),
                    kind: ProjectMemberKind::PropertyGet,
                    proc: CoreProcId(0),
                    is_default_member: false,
                }],
                implements: Vec::new(),
            }],
            unit_name: "T".into(),
            ..Default::default()
        };
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
        // The class table is projected, and `w`'s declared type resolves to that class.
        assert_eq!(oxp.classes.len(), 1);
        assert_eq!(oxp.classes[0].name, "Widget");
        assert_eq!(oxp.classes[0].methods.len(), 1);
        assert_eq!(
            oxp.funcs[0].locals[0].ty,
            OxTy::Object(ObjClass::Class(ClassId(0)))
        );
        // Round-trips structurally through JSON.
        let json = serde_json::to_string(&oxp).expect("serialize");
        let back: OxProgram = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(oxp, back, "object-construct OxProgram must round-trip");
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
        let oxp = elaborate(&prog).expect("elaborate");
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

    /// A block ending in a `Raise` terminator must carry a `fault_target` (the statement
    /// pad) so `On Error` can catch `Err.Raise`/`Error n` — `finish_to` previously set
    /// `fault_target` only for fallible *instructions*, leaving a bare raise statement's
    /// block with none (so a raised error could not reach the handler).
    #[test]
    fn raise_terminator_block_gets_a_fault_target() {
        let body = vec![
            CoreStmt::Error(ErrorOp::OnErrorGotoLabel(coreir::LabelId(0))),
            CoreStmt::Error(ErrorOp::Raise {
                number: coreir::CoreValue::Const(coreir::CoreConst::I32(5)),
                source: None,
                description: None,
                help_file: None,
                help_context: None,
                inherit: true,
            }),
            CoreStmt::Label(coreir::LabelId(0)),
            CoreStmt::Error(ErrorOp::ResumeNext),
        ];
        let oxp = elaborate(&program(sub("Main", Vec::new(), body))).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()), "raise program must verify");
        let raise_block = oxp.funcs[0]
            .blocks
            .iter()
            .find(|b| matches!(b.terminator, OxTerminator::Raise { .. }))
            .expect("a Raise terminator block");
        assert!(
            raise_block.fault_target.is_some(),
            "a Raise terminator's block must carry a fault_target (the statement pad)"
        );
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
        let oxp = elaborate(&prog).expect("elaborate");
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

    /// `ReDim a(1 To 10) : a(1) = 5 : x = a(1) : x = UBound(a) : Erase a` exercises the
    /// array op set (ReDim / ArraySet / ArrayGet / Bound / Erase), with the element
    /// type recovered from the array's declared type.
    #[test]
    fn arrays_elaborate_and_verify() {
        let a = || CorePlace::Local(CoreLocalId(0));
        let x = CorePlace::Local(CoreLocalId(1));
        let idx1 = || vec![CoreValue::Const(CoreConst::I32(1))];
        let body = vec![
            CoreStmt::ReDim {
                array: a(),
                bounds: vec![CoreBound {
                    upper: CoreValue::Const(CoreConst::I32(10)),
                    lower: CoreValue::Const(CoreConst::I32(1)),
                }],
                element_type: ArrayElementType::Long,
                preserve: false,
                fixed: false,
            },
            assign(
                CorePlace::Index {
                    array: Box::new(a()),
                    indices: idx1(),
                },
                CoreValue::Const(CoreConst::I32(5)),
            ),
            assign(
                x.clone(),
                CoreValue::Load(CorePlace::Index {
                    array: Box::new(a()),
                    indices: idx1(),
                }),
            ),
            assign(
                x.clone(),
                CoreValue::Bound {
                    which: BoundWhich::Upper,
                    array: Box::new(a()),
                    dimension: None,
                },
            ),
            CoreStmt::Erase {
                array: a(),
                element_type: ArrayElementType::Long,
            },
        ];
        let locals = vec![
            CoreLocal {
                name: "a".to_string(),
                ty: VarTypeRef::Array(Box::new(VarTypeRef::Builtin(BuiltinType::Long))),
                array_element: Some(ArrayElementType::Long),
            },
            long_local("x"),
        ];
        let prog = program(sub("Main", locals, body));
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));

        let f = &oxp.funcs[0];
        let has = |pred: fn(&OxInst) -> bool| f.blocks.iter().any(|b| b.instrs.iter().any(pred));
        assert!(has(|i| matches!(i, OxInst::ArrayRedim { .. })), "expected ArrayRedim");
        assert!(has(|i| matches!(i, OxInst::ArraySet { .. })), "expected ArraySet");
        assert!(has(|i| matches!(i, OxInst::ArrayGet { .. })), "expected ArrayGet");
        assert!(has(|i| matches!(i, OxInst::Bound { .. })), "expected Bound");
        assert!(has(|i| matches!(i, OxInst::ArrayErase { .. })), "expected ArrayErase");
        // The declared element type is recovered: `a` is an Array(Long).
        assert_eq!(
            f.locals[0].ty,
            OxTy::Array(Box::new(OxTy::Long), ArrayShape::Dynamic)
        );
    }

    /// `p.x = 1 : y = p.x` over a UDT value exercises RecordSet / RecordGet.
    #[test]
    fn records_elaborate_and_verify() {
        let p_x = || CorePlace::RecordField {
            base: Box::new(CorePlace::Local(CoreLocalId(0))),
            index: 0,
        };
        let body = vec![
            assign(p_x(), CoreValue::Const(CoreConst::I32(1))),
            assign(CorePlace::Local(CoreLocalId(1)), CoreValue::Load(p_x())),
        ];
        let locals = vec![
            CoreLocal {
                name: "p".to_string(),
                ty: VarTypeRef::Udt("mytype".to_string()),
                array_element: None,
            },
            long_local("y"),
        ];
        let prog = program(sub("Main", locals, body));
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));

        let f = &oxp.funcs[0];
        let has = |pred: fn(&OxInst) -> bool| f.blocks.iter().any(|b| b.instrs.iter().any(pred));
        assert!(has(|i| matches!(i, OxInst::RecordSet { .. })), "expected RecordSet");
        assert!(has(|i| matches!(i, OxInst::RecordGet { .. })), "expected RecordGet");
    }

    #[test]
    fn record_array_fields_elaborate_to_fused_ops() {
        let p_arr = || CorePlace::RecordField {
            base: Box::new(CorePlace::Local(CoreLocalId(0))),
            index: 0,
        };
        let p_arr_1 = || CorePlace::Index {
            array: Box::new(p_arr()),
            indices: vec![CoreValue::Const(CoreConst::I32(1))],
        };
        let body = vec![
            assign(p_arr_1(), CoreValue::Const(CoreConst::I32(5))),
            assign(CorePlace::Local(CoreLocalId(1)), CoreValue::Load(p_arr_1())),
        ];
        let locals = vec![
            CoreLocal {
                name: "p".to_string(),
                ty: VarTypeRef::Udt("mytype".to_string()),
                array_element: None,
            },
            long_local("y"),
        ];
        let prog = program(sub("Main", locals, body));
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));

        let f = &oxp.funcs[0];
        let has = |pred: fn(&OxInst) -> bool| f.blocks.iter().any(|b| b.instrs.iter().any(pred));
        assert!(
            has(|i| matches!(i, OxInst::RecordArraySet { .. })),
            "expected RecordArraySet"
        );
        assert!(
            has(|i| matches!(i, OxInst::RecordArrayGet { .. })),
            "expected RecordArrayGet"
        );
    }

    /// A base-library call lowers to `CallNative { Builtin }`.
    #[test]
    fn native_call_elaborates() {
        let call = CoreValue::Call {
            callee: CoreCallee::Native(NativeImplId::ALL[0]),
            args: vec![CoreArg::ByVal(CoreValue::Const(CoreConst::I32(1)))],
        };
        let prog = program(sub(
            "Main",
            vec![long_local("x")],
            vec![assign(CorePlace::Local(CoreLocalId(0)), call)],
        ));
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
        assert!(
            oxp.funcs[0]
                .blocks
                .iter()
                .any(|b| b.instrs.iter().any(|i| matches!(
                    i,
                    OxInst::CallNative {
                        callee: OxNativeCallee::Builtin(_),
                        ..
                    }
                ))),
            "expected a CallNative(Builtin)"
        );
    }

    /// `With p : x = .<recv> : End With` and `For Each item In coll : x = item : Next`
    /// exercise the With-receiver temp and the For-Each iterator protocol.
    #[test]
    fn with_and_foreach_elaborate() {
        let x = || CorePlace::Local(CoreLocalId(0));
        let body = vec![
            CoreStmt::With {
                id: 0,
                receiver: CoreValue::Load(CorePlace::Local(CoreLocalId(1))),
                // A direct reference to the With receiver temp.
                body: vec![assign(x(), CoreValue::WithTemp(0))],
            },
            CoreStmt::ForEach {
                item: CorePlace::Local(CoreLocalId(3)),
                source: CoreValue::Load(CorePlace::Local(CoreLocalId(2))),
                body: vec![assign(
                    x(),
                    CoreValue::Load(CorePlace::Local(CoreLocalId(3))),
                )],
            },
        ];
        let locals = vec![
            long_local("x"),
            variant_local("p"),
            variant_local("coll"),
            variant_local("item"),
        ];
        let oxp = elaborate(&program(sub("Main", locals, body))).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
        let f = &oxp.funcs[0];
        let has = |pred: fn(&OxInst) -> bool| f.blocks.iter().any(|b| b.instrs.iter().any(pred));
        assert!(has(|i| matches!(i, OxInst::ForEachInit { .. })), "expected ForEachInit");
        assert!(has(|i| matches!(i, OxInst::ForEachNext { .. })), "expected ForEachNext");
    }

    /// `p = VarPtr(x) : n = Err.Number : Err.Source = "s" : f = AddressOf Main`
    /// lower to Ptr / ErrFieldGet / ErrFieldSet / LoadProcRef.
    #[test]
    fn pointers_errfields_addressof_elaborate() {
        let body = vec![
            assign(
                CorePlace::Local(CoreLocalId(0)),
                CoreValue::Ptr {
                    kind: PtrKind::Var,
                    value: Box::new(CoreValue::Load(CorePlace::Local(CoreLocalId(1)))),
                },
            ),
            assign(
                CorePlace::Local(CoreLocalId(2)),
                CoreValue::ErrField(ErrField::Number),
            ),
            CoreStmt::Error(ErrorOp::SetErrField {
                field: ErrField::Source,
                value: CoreValue::Const(CoreConst::Str("s".into())),
            }),
            assign(
                CorePlace::Local(CoreLocalId(3)),
                CoreValue::AddressOf(CoreProcId(0)),
            ),
        ];
        let locals = vec![
            long_local("p"),
            long_local("x"),
            long_local("n"),
            variant_local("f"),
        ];
        let oxp = elaborate(&program(sub("Main", locals, body))).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
        let f = &oxp.funcs[0];
        let has = |pred: fn(&OxInst) -> bool| f.blocks.iter().any(|b| b.instrs.iter().any(pred));
        assert!(has(|i| matches!(i, OxInst::Ptr { .. })), "expected Ptr");
        assert!(has(|i| matches!(i, OxInst::ErrFieldGet { .. })), "expected ErrFieldGet");
        assert!(has(|i| matches!(i, OxInst::ErrFieldSet { .. })), "expected ErrFieldSet");
        assert!(has(|i| matches!(i, OxInst::LoadProcRef { .. })), "expected LoadProcRef");
    }

    // ── Review-fix regressions ───────────────────────────────────────────────

    /// A `Set` / object-typed assignment emits the `ValidateAssignment` legality check.
    #[test]
    fn set_assignment_emits_validate() {
        let prog = program(sub(
            "Main",
            vec![variant_local("x")],
            vec![CoreStmt::Assign {
                place: CorePlace::Local(CoreLocalId(0)),
                value: CoreValue::Const(CoreConst::Nothing),
                intent: AssignmentIntent::Set,
                target_kind: AssignmentTargetKind::Object,
                target_name: "x".to_string(),
                target_type_name: "Object".to_string(),
            }],
        ));
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
        assert!(
            oxp.funcs[0]
                .blocks
                .iter()
                .any(|b| b.instrs.iter().any(|i| matches!(i, OxInst::ValidateAssignment { .. }))),
            "Set assignment must emit ValidateAssignment"
        );
    }

    /// A `Let` assignment of a statically object-valued source (notably `Nothing`) into
    /// a Variant still needs the legality check so runtime error 91 can be caught by
    /// `On Error Resume Next` before the store overwrites the old value.
    #[test]
    fn let_variant_nothing_assignment_emits_validate() {
        let prog = program(sub(
            "Main",
            vec![variant_local("x")],
            vec![CoreStmt::Assign {
                place: CorePlace::Local(CoreLocalId(0)),
                value: CoreValue::Const(CoreConst::Nothing),
                intent: AssignmentIntent::Let,
                target_kind: AssignmentTargetKind::Variant,
                target_name: "x".to_string(),
                target_type_name: "Variant".to_string(),
            }],
        ));
        let oxp = elaborate(&prog).expect("elaborate");
        assert_eq!(verify_program(&oxp), Ok(()));
        assert!(
            oxp.funcs[0]
                .blocks
                .iter()
                .any(|b| b.instrs.iter().any(|i| matches!(i, OxInst::ValidateAssignment { .. }))),
            "Let Variant = Nothing must emit ValidateAssignment"
        );
    }

    /// A call in statement position is lowered with no result destination.
    #[test]
    fn statement_call_has_no_result_dst() {
        let prog = program(sub(
            "Main",
            Vec::new(),
            vec![CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::VbaProc {
                    proc: CoreProcId(0),
                },
                args: Vec::new(),
            })],
        ));
        let oxp = elaborate(&prog).expect("elaborate");
        assert!(
            oxp.funcs[0]
                .blocks
                .iter()
                .any(|b| b.instrs.iter().any(|i| matches!(i, OxInst::CallProc { dst: None, .. }))),
            "a statement call must have no result destination"
        );
    }

    /// A duplicate label definition is rejected (not a debug-assert / silent overwrite).
    #[test]
    fn duplicate_label_is_rejected() {
        let prog = program(sub(
            "Main",
            Vec::new(),
            vec![
                CoreStmt::Label(coreir::LabelId(0)),
                CoreStmt::Label(coreir::LabelId(0)),
            ],
        ));
        let err = elaborate(&prog).expect_err("duplicate label");
        assert!(matches!(err, ElaborateError::Malformed(_)), "got {err:?}");
    }

    /// Reading an out-of-range local is a clean error, not a panic.
    #[test]
    fn out_of_range_local_read_is_rejected() {
        let prog = program(sub(
            "Main",
            vec![variant_local("x")],
            vec![assign(
                CorePlace::Local(CoreLocalId(0)),
                CoreValue::Load(CorePlace::Local(CoreLocalId(99))),
            )],
        ));
        let err = elaborate(&prog).expect_err("out-of-range local");
        assert!(matches!(err, ElaborateError::Malformed(_)), "got {err:?}");
    }

    /// The verifier range-checks an `AddressOf` (`LoadProcRef`) proc index.
    #[test]
    fn verifier_catches_out_of_range_addressof() {
        let prog = program(sub(
            "Main",
            vec![variant_local("f")],
            vec![assign(
                CorePlace::Local(CoreLocalId(0)),
                CoreValue::AddressOf(CoreProcId(99)),
            )],
        ));
        // Elaboration does not range-check proc ids; the verifier does.
        let oxp = elaborate(&prog).expect("elaborate");
        let errs = verify_program(&oxp).expect_err("dangling AddressOf proc");
        assert!(
            errs.iter().any(|e| matches!(e, VerifyError::BadProcRef { proc: 99, .. })),
            "expected BadProcRef, got {errs:?}"
        );
    }
}
