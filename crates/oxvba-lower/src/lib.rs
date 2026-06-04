//! `oxvba-lower` — lower the front-end **bound HIR** directly to the clean
//! [`oxvba_bundle::Bundle`], using the clean execution model (per-procedure
//! frames, frame-relative locals, module globals). This is the production path
//! that will replace `BoundModule` → `emit` → legacy bytecode → `oxvba-temp-b2b`.
//!
//! It consumes only the **public HIR API** of `oxvba-compiler`
//! (`collect_type_hooks_from_source` → `TypedHirModule`, the `frontend_hir`
//! arenas/enums, and the `frontend_symbols` model) — it never reaches into the
//! legacy `BoundModule`/`emit` internals.
//!
//! ## Status: foundational slice
//! This first slice covers the core: module globals + per-procedure frame slot
//! allocation, literals, names, unary/binary operators (`Is` → object identity,
//! `Like` → the library), `Let` to a simple variable, and `If` / `Do`-`While`
//! (-`Until`) / `For` control flow. Everything else returns
//! [`LowerError::Unsupported`], to be widened construct-by-construct. The slot
//! model is the real clean one (frames are recursion-safe with true `ByRef`),
//! not the flat-global shortcut the temporary `b2b` bridge uses.

use std::collections::HashMap;

use oxvba_bundle::isa::{CallArg, NativeCallee, Op};
use oxvba_bundle::native::NativeImplId;
use oxvba_bundle::{Bundle, ProcedureDescriptor, ProcedureKind, StringCompareMode};
use oxvba_compiler::frontend_hir::{
    BoundHirModule, HirBinaryOp, HirDeclKind, HirExprId, HirExprKind, HirLiteral, HirProcedureKind,
    HirStmtId, HirStmtKind, HirUnaryOp,
};
use oxvba_compiler::frontend_symbols::{ScopeId, ScopeKind, SymbolId, SymbolModel, SymbolNamespace};
use oxvba_compiler::frontend_type_hooks::collect_type_hooks_from_source;

#[derive(Debug)]
pub enum LowerError {
    /// The HIR could not be built/typed from the source.
    Build(String),
    /// A construct not yet handled by this (incrementally widening) lowering.
    Unsupported(String),
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::Build(m) => write!(f, "HIR build error: {m}"),
            LowerError::Unsupported(m) => write!(f, "unsupported by oxvba-lower (yet): {m}"),
        }
    }
}

/// Lower a single module's source straight to a clean [`Bundle`].
pub fn lower_module(module_name: &str, source: &str) -> Result<Bundle, LowerError> {
    let typed = collect_type_hooks_from_source(module_name, source)
        .map_err(|e| LowerError::Build(e.to_string()))?;
    Lowerer::new(&typed.module).run()
}

struct Lowerer<'a> {
    module: &'a BoundHirModule,
    symbols: &'a SymbolModel,
    ops: Vec<Op>,
    statement_starts: Vec<usize>,
    /// Absolute slot for every symbol: globals in `[0, global_count)`; a
    /// procedure's params/locals/return are frame-relative, stored as
    /// `global_count + index`.
    slot_of: HashMap<SymbolId, usize>,
    global_count: usize,
    // Per-procedure scratch (reset in `begin_proc`):
    next_temp: usize,
    frame_slots: usize,
}

impl<'a> Lowerer<'a> {
    fn new(module: &'a BoundHirModule) -> Self {
        Self {
            module,
            symbols: &module.symbols,
            ops: Vec::new(),
            statement_starts: Vec::new(),
            slot_of: HashMap::new(),
            global_count: 0,
            next_temp: 0,
            frame_slots: 0,
        }
    }

    fn run(mut self) -> Result<Bundle, LowerError> {
        self.plan_globals();

        let proc_ids: Vec<_> = self
            .module
            .declarations
            .iter()
            .copied()
            .filter(|id| {
                matches!(
                    self.module.arenas.decl(*id).map(|d| &d.kind),
                    Some(HirDeclKind::Procedure { .. })
                )
            })
            .collect();
        if proc_ids.is_empty() {
            return Err(LowerError::Unsupported("module has no procedures".into()));
        }

        let mut procedures = Vec::new();
        let mut entry_pc = 0;
        let mut entry_frame_slots = 0;
        for decl_id in proc_ids {
            let decl = self.module.arenas.decl(decl_id).expect("decl");
            let HirDeclKind::Procedure { kind, params, body, .. } = &decl.kind else {
                unreachable!()
            };
            let proc_symbol = decl.symbol;
            let (kind, params, body) = (*kind, params.clone(), body.clone());

            let entry = self.ops.len();
            let return_slot = self.begin_proc(proc_symbol, kind, &params)?;
            for stmt in &body {
                self.lower_stmt(*stmt)?;
            }
            self.ops.push(Op::Return);

            let name = self.symbol_folded(proc_symbol).unwrap_or_default();
            let descriptor = ProcedureDescriptor {
                name: self.symbol_spelling(proc_symbol).unwrap_or_else(|| name.clone()),
                entry_pc: entry,
                kind: match kind {
                    HirProcedureKind::Sub => ProcedureKind::Sub,
                    HirProcedureKind::Function => ProcedureKind::Function,
                    HirProcedureKind::PropertyGet => ProcedureKind::PropertyGet,
                    HirProcedureKind::PropertyLet => ProcedureKind::PropertyLet,
                    HirProcedureKind::PropertySet => ProcedureKind::PropertySet,
                },
                param_count: params.len(),
                frame_slots: self.frame_slots,
                return_slot,
            };
            // Entry point: `Main` if present, else the first procedure.
            if entry_pc == 0 && (name == "main" || procedures.is_empty()) {
                entry_pc = entry;
                entry_frame_slots = self.frame_slots;
            }
            procedures.push(descriptor);
        }

        self.statement_starts.sort_unstable();
        self.statement_starts.dedup();

        Ok(Bundle {
            ops: self.ops,
            procedures,
            entry_pc,
            global_count: self.global_count,
            entry_frame_slots,
            statement_starts: self.statement_starts,
            external_calls: Vec::new(),
            source_map: Vec::new(),
            com_class_exports: Vec::new(),
            classes: Vec::new(),
            event_routes: Vec::new(),
        })
    }

    // ── Slot planning ────────────────────────────────────────────────────────
    fn plan_globals(&mut self) {
        let mut g = 0;
        for symbol in self.symbols.symbols() {
            if symbol.namespace == SymbolNamespace::Module {
                self.slot_of.insert(symbol.id, g);
                g += 1;
            }
        }
        self.global_count = g;
    }

    fn begin_proc(
        &mut self,
        proc_symbol: SymbolId,
        kind: HirProcedureKind,
        params: &[SymbolId],
    ) -> Result<Option<usize>, LowerError> {
        let mut index = 0usize;
        for &param in params {
            self.slot_of.insert(param, self.global_count + index);
            index += 1;
        }
        if let Some(scope) = self.proc_scope(proc_symbol) {
            // Collect this procedure's locals (and locals in nested blocks).
            let locals: Vec<SymbolId> = self
                .symbols
                .symbols()
                .iter()
                .filter(|s| {
                    s.namespace == SymbolNamespace::Local
                        && self.scope_within(s.scope, scope)
                        && !self.slot_of.contains_key(&s.id)
                })
                .map(|s| s.id)
                .collect();
            for local in locals {
                self.slot_of.insert(local, self.global_count + index);
                index += 1;
            }
        }
        // A Function/Property-Get returns via assignment to its own name.
        let return_slot = matches!(
            kind,
            HirProcedureKind::Function | HirProcedureKind::PropertyGet
        )
        .then(|| {
            let r = index;
            self.slot_of.insert(proc_symbol, self.global_count + index);
            index += 1;
            r
        });

        self.next_temp = self.global_count + index;
        self.frame_slots = index;
        Ok(return_slot)
    }

    fn proc_scope(&self, proc_symbol: SymbolId) -> Option<ScopeId> {
        let name = self.symbols.symbol(proc_symbol)?.name;
        self.symbols
            .scopes()
            .iter()
            .find(|sc| sc.kind == ScopeKind::Procedure && sc.name == Some(name))
            .map(|sc| sc.id)
    }

    fn scope_within(&self, mut scope: ScopeId, ancestor: ScopeId) -> bool {
        loop {
            if scope == ancestor {
                return true;
            }
            match self.symbols.scope(scope).ok().and_then(|s| s.parent) {
                Some(parent) => scope = parent,
                None => return false,
            }
        }
    }

    fn symbol_folded(&self, id: SymbolId) -> Option<String> {
        let name = self.symbols.symbol(id)?.name;
        self.symbols.name(name).map(|n| n.folded.clone())
    }
    fn symbol_spelling(&self, id: SymbolId) -> Option<String> {
        let name = self.symbols.symbol(id)?.name;
        self.symbols.name(name).map(|n| n.first_spelling.clone())
    }

    fn new_temp(&mut self) -> usize {
        let slot = self.next_temp;
        self.next_temp += 1;
        self.frame_slots = self.frame_slots.max(self.next_temp - self.global_count);
        slot
    }

    // ── Emit / patch helpers ───────────────────────────────────────────────────
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
            Op::Jump { target_pc } | Op::JumpIfZero { target_pc, .. } => *target_pc = target,
            _ => {}
        }
    }

    // ── Statements ─────────────────────────────────────────────────────────────
    fn lower_stmt(&mut self, stmt_id: HirStmtId) -> Result<(), LowerError> {
        let stmt = self
            .module
            .arenas
            .stmt(stmt_id)
            .ok_or_else(|| LowerError::Build("dangling HirStmtId".into()))?;
        self.statement_starts.push(self.here());
        match &stmt.kind {
            HirStmtKind::Empty => Ok(()),
            HirStmtKind::Let { target, value, .. } => {
                let dst = self.lower_place(*target)?;
                let src = self.lower_expr(*value)?;
                self.emit(Op::Copy { dst, src });
                Ok(())
            }
            HirStmtKind::If { condition, then_body, else_body } => {
                let cond = self.lower_expr(*condition)?;
                let jz = self.emit(Op::JumpIfZero { cond_slot: cond, target_pc: 0 });
                for s in then_body.clone() {
                    self.lower_stmt(s)?;
                }
                if else_body.is_empty() {
                    let end = self.here();
                    self.patch(jz, end);
                } else {
                    let jend = self.emit(Op::Jump { target_pc: 0 });
                    let else_pc = self.here();
                    self.patch(jz, else_pc);
                    for s in else_body.clone() {
                        self.lower_stmt(s)?;
                    }
                    let end = self.here();
                    self.patch(jend, end);
                }
                Ok(())
            }
            HirStmtKind::DoWhile { condition, body, post_check, until } => {
                let (condition, body, post_check, until) =
                    (*condition, body.clone(), *post_check, *until);
                if post_check {
                    let top = self.here();
                    for s in &body {
                        self.lower_stmt(*s)?;
                    }
                    let cont = self.lower_loop_condition(condition, until)?;
                    let jz = self.emit(Op::JumpIfZero { cond_slot: cont, target_pc: 0 });
                    self.emit(Op::Jump { target_pc: top });
                    let after = self.here();
                    self.patch(jz, after);
                } else {
                    let top = self.here();
                    let cont = self.lower_loop_condition(condition, until)?;
                    let jz = self.emit(Op::JumpIfZero { cond_slot: cont, target_pc: 0 });
                    for s in &body {
                        self.lower_stmt(*s)?;
                    }
                    self.emit(Op::Jump { target_pc: top });
                    let after = self.here();
                    self.patch(jz, after);
                }
                Ok(())
            }
            HirStmtKind::ForRange { var, start, end, step, body } => {
                let (var, start, end, step, body) =
                    (*var, *start, *end, *step, body.clone());
                let var_slot = self.symbol_slot(var)?;
                let start_slot = self.lower_expr(start)?;
                self.emit(Op::Copy { dst: var_slot, src: start_slot });
                let limit = self.new_temp();
                let end_slot = self.lower_expr(end)?;
                self.emit(Op::Copy { dst: limit, src: end_slot });
                let step_slot = self.new_temp();
                match step {
                    Some(expr) => {
                        let e = self.lower_expr(expr)?;
                        self.emit(Op::Copy { dst: step_slot, src: e });
                    }
                    None => {
                        self.emit(Op::LoadI32 { slot: step_slot, value: 1 });
                    }
                }
                let top = self.here();
                let cond = self.new_temp();
                // FIDELITY: assumes a non-negative Step (the common case).
                self.emit(Op::CmpLe {
                    dst: cond,
                    lhs: var_slot,
                    rhs: limit,
                    mode: StringCompareMode::Binary,
                });
                let jz = self.emit(Op::JumpIfZero { cond_slot: cond, target_pc: 0 });
                for s in &body {
                    self.lower_stmt(*s)?;
                }
                self.emit(Op::Add { dst: var_slot, lhs: var_slot, rhs: step_slot });
                self.emit(Op::Jump { target_pc: top });
                let after = self.here();
                self.patch(jz, after);
                Ok(())
            }
            HirStmtKind::OnErrorResumeNext => {
                self.emit(Op::SetOnErrorResumeNext);
                Ok(())
            }
            HirStmtKind::OnErrorGoto0 => {
                self.emit(Op::SetOnErrorGoto0);
                Ok(())
            }
            HirStmtKind::ResumeNext => {
                self.emit(Op::ResumeNext);
                Ok(())
            }
            HirStmtKind::Resume => {
                self.emit(Op::Resume);
                Ok(())
            }
            other => Err(LowerError::Unsupported(format!("statement {other:?}"))),
        }
    }

    /// Lower a loop's continue-condition (`Do While` keeps the value; `Do Until`
    /// negates it).
    fn lower_loop_condition(&mut self, cond: HirExprId, until: bool) -> Result<usize, LowerError> {
        let value = self.lower_expr(cond)?;
        if until {
            let negated = self.new_temp();
            self.emit(Op::Not { dst: negated, src: value });
            Ok(negated)
        } else {
            Ok(value)
        }
    }

    /// An assignable place. For this slice, only a plain variable name.
    fn lower_place(&mut self, expr_id: HirExprId) -> Result<usize, LowerError> {
        let expr = self.expr(expr_id)?;
        match &expr.kind {
            HirExprKind::Name(symbol) => self.symbol_slot(*symbol),
            other => Err(LowerError::Unsupported(format!("assignment target {other:?}"))),
        }
    }

    fn symbol_slot(&self, symbol: SymbolId) -> Result<usize, LowerError> {
        self.slot_of
            .get(&symbol)
            .copied()
            .ok_or_else(|| LowerError::Unsupported("reference to unslotted symbol".into()))
    }

    // ── Expressions (return the slot holding the value) ───────────────────────
    fn expr(&self, expr_id: HirExprId) -> Result<&'a oxvba_compiler::frontend_hir::HirExpr, LowerError> {
        self.module
            .arenas
            .expr(expr_id)
            .ok_or_else(|| LowerError::Build("dangling HirExprId".into()))
    }

    fn lower_expr(&mut self, expr_id: HirExprId) -> Result<usize, LowerError> {
        let kind = self.expr(expr_id)?.kind.clone();
        match kind {
            HirExprKind::Literal(lit) => {
                let slot = self.new_temp();
                self.emit(literal_op(slot, &lit));
                Ok(slot)
            }
            HirExprKind::Name(symbol) => self.symbol_slot(symbol),
            HirExprKind::Unary { op, expr } => {
                let src = self.lower_expr(expr)?;
                let dst = self.new_temp();
                self.emit(match op {
                    HirUnaryOp::Negate => Op::Neg { dst, src },
                    HirUnaryOp::Not => Op::Not { dst, src },
                });
                Ok(dst)
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                let l = self.lower_expr(lhs)?;
                let r = self.lower_expr(rhs)?;
                let dst = self.new_temp();
                self.emit_binary(op, dst, l, r);
                Ok(dst)
            }
            other => Err(LowerError::Unsupported(format!("expression {other:?}"))),
        }
    }

    fn emit_binary(&mut self, op: HirBinaryOp, dst: usize, lhs: usize, rhs: usize) {
        let m = StringCompareMode::Binary;
        let op = match op {
            HirBinaryOp::Add => Op::Add { dst, lhs, rhs },
            HirBinaryOp::Sub => Op::Sub { dst, lhs, rhs },
            HirBinaryOp::Mul => Op::Mul { dst, lhs, rhs },
            HirBinaryOp::Div => Op::Div { dst, lhs, rhs },
            HirBinaryOp::IntDiv => Op::IntDiv { dst, lhs, rhs },
            HirBinaryOp::Mod => Op::Mod { dst, lhs, rhs },
            HirBinaryOp::Pow => Op::Pow { dst, lhs, rhs },
            HirBinaryOp::Concat => Op::Concat { dst, lhs, rhs },
            HirBinaryOp::Eq => Op::CmpEq { dst, lhs, rhs, mode: m },
            HirBinaryOp::Ne => Op::CmpNe { dst, lhs, rhs, mode: m },
            HirBinaryOp::Lt => Op::CmpLt { dst, lhs, rhs, mode: m },
            HirBinaryOp::Le => Op::CmpLe { dst, lhs, rhs, mode: m },
            HirBinaryOp::Gt => Op::CmpGt { dst, lhs, rhs, mode: m },
            HirBinaryOp::Ge => Op::CmpGe { dst, lhs, rhs, mode: m },
            HirBinaryOp::And => Op::And { dst, lhs, rhs },
            HirBinaryOp::Or => Op::Or { dst, lhs, rhs },
            HirBinaryOp::Is => Op::CmpObjectIs { dst, lhs, rhs },
            HirBinaryOp::Like => Op::CallNative {
                dst: Some(dst),
                callee: NativeCallee::Builtin(NativeImplId::Like),
                args: vec![CallArg::Slot(lhs), CallArg::Slot(rhs)],
            },
        };
        self.emit(op);
    }
}

fn literal_op(slot: usize, lit: &HirLiteral) -> Op {
    match lit {
        HirLiteral::Empty => Op::LoadEmpty { slot },
        HirLiteral::Null => Op::LoadNull { slot },
        // Nothing is the null object; the VM treats Empty/0 as Nothing for `Is`.
        HirLiteral::Nothing => Op::LoadEmpty { slot },
        HirLiteral::Bool(value) => Op::LoadBool { slot, value: *value },
        HirLiteral::Int(value) => match i32::try_from(*value) {
            Ok(v) => Op::LoadI32 { slot, value: v },
            Err(_) => Op::LoadI64 { slot, value: *value },
        },
        HirLiteral::String(value) => Op::LoadString { slot, value: value.clone() },
    }
}

#[cfg(test)]
mod tests;
