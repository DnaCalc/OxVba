//! The statement binder: `bind_block` / `bind_stmt` lower every statement CST
//! node to `CoreStmt`s — assignment (with intent + coercion), control flow,
//! error-state, `ReDim`/`Erase`, and the file-I/O statements (→ native calls).

use oxvba_bundle::coreir::{
    CaseClause, CoreArg, CoreBinOp, CoreBound, CoreCaseBlock, CoreCallee, CoreConst, CoreIfArm,
    CorePlace, CoreStmt, CoreValue, ErrorOp, ExitKind,
};
use oxvba_bundle::native::NativeImplId;
use oxvba_bundle::{AssignmentIntent, ProjectMemberKind};
use oxvba_symbol::binding::DispatchRoute;
use oxvba_symbol::model::fold_identifier;
use oxvba_syntax::red::{ArgItem, CaseSpec};
use oxvba_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

use crate::error::BindError;
use crate::expr::comparison_binop;
use crate::types;
use crate::ProcLower;

impl<'a> ProcLower<'a> {
    pub(crate) fn bind_block(&mut self, block: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let mut out = Vec::new();
        for stmt in block.statements() {
            out.extend(self.bind_stmt(stmt)?);
        }
        Ok(out)
    }

    fn bind_stmt(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        use SyntaxKind::*;
        match node.kind() {
            AssignStmt | LetStmt => self.bind_assign(node, AssignmentIntent::Let),
            SetStmt => self.bind_assign(node, AssignmentIntent::Set),
            CallStmt => self.bind_call_stmt(node),
            IfStmt => self.bind_if(node),
            ForStmt => self.bind_for(node),
            DoStmt => self.bind_do(node),
            WhileStmt => self.bind_while(node),
            SelectStmt => self.bind_select(node),
            WithStmt => self.bind_with(node),
            ExitStmt => self.bind_exit(node),
            GoToStmt => Ok(vec![CoreStmt::Goto(self.label_ref_id(node)?)]),
            GoSubStmt => Ok(vec![CoreStmt::GoSub(self.label_ref_id(node)?)]),
            ReturnStmt => Ok(vec![CoreStmt::GoSubReturn]),
            LabelStmt => {
                let name = node
                    .first_significant_token()
                    .ok_or_else(|| BindError::Malformed("label".into()))?
                    .text;
                Ok(vec![CoreStmt::Label(self.label_id(name))])
            }
            OnErrorStmt => self.bind_on_error(node),
            ResumeStmt => self.bind_resume(node),
            ReDimStmt => self.bind_redim(node),
            EraseStmt => self.bind_erase(node),
            OpenStmt | CloseStmt | PrintStmt | WriteStmt | InputStmt | LineInputStmt => {
                self.bind_file_io(node)
            }
            // Declarations contribute no executable statement (frame already built).
            DimStmt | ConstStmt | DeclareStmt | TypeBlock | EnumBlock | OptionStmt
            | AttributeStmt | ImplementsStmt | EventDecl => Ok(Vec::new()),
            other => Err(BindError::Unsupported(format!("statement {other:?}"))),
        }
    }

    // ── Assignment ──────────────────────────────────────────

    fn bind_assign(
        &mut self,
        node: SyntaxNode<'_>,
        intent: AssignmentIntent,
    ) -> Result<Vec<CoreStmt>, BindError> {
        let target_node = node
            .assign_target()
            .ok_or_else(|| BindError::Malformed("assignment target".into()))?;
        let value_node = node
            .assign_value()
            .ok_or_else(|| BindError::Malformed("assignment value".into()))?;
        let val = self.bind_expr(value_node)?;
        // A property target (`obj.Prop = x` / bare `Prop = x`) is a setter call,
        // not a place store: Let → Property Let, Set → Property Set.
        if let Some(stmts) = self.try_property_assignment(target_node, intent, &val)? {
            return Ok(stmts);
        }
        let (place, target_ty) = self.bind_place(target_node)?;
        let value = types::coerce(val.value, &val.ty, &target_ty);
        Ok(vec![CoreStmt::Assign {
            place,
            value,
            intent,
            target_kind: types::assignment_target_kind(&target_ty),
            target_name: target_node.text().trim().to_string(),
            target_type_name: types::type_name(&target_ty),
        }])
    }

    /// If `target` denotes a project property, lower the assignment to a Property
    /// Let/Set accessor call (`Eval(Call(LateDispatch, [receiver, value]))`) and
    /// return it; otherwise `None` (the caller does a place store).
    fn try_property_assignment(
        &mut self,
        target: SyntaxNode<'_>,
        intent: AssignmentIntent,
        val: &crate::Bound,
    ) -> Result<Option<Vec<CoreStmt>>, BindError> {
        let kind = match intent {
            AssignmentIntent::Set => ProjectMemberKind::PropertySet,
            _ => ProjectMemberKind::PropertyLet,
        };
        match target.kind() {
            SyntaxKind::IdentExpr => {
                let Some(name) = target.ident_name_token().map(|t| t.text) else {
                    return Ok(None);
                };
                if self.return_target(name).is_some() {
                    return Ok(None);
                }
                let Some(binding) = self.resolve(name) else { return Ok(None) };
                if !is_property_route(&binding.route) {
                    return Ok(None);
                }
                // A bare property name is an implicit `Me.Prop` (class member).
                let Some(recv) = self.me_value() else { return Ok(None) };
                let call = self.late_member_call(name, kind, recv, vec![CoreArg::ByVal(val.value.clone())]);
                Ok(Some(vec![CoreStmt::Eval(call)]))
            }
            SyntaxKind::MemberExpr => {
                let Some(member) = target.member_name_token().map(|t| t.text) else {
                    return Ok(None);
                };
                let recv = self.member_receiver_bound(target)?;
                let Some(binding) = self.resolve_member(&recv.ty, member, Some(kind)) else {
                    return Ok(None);
                };
                if !is_property_route(&binding.route) {
                    return Ok(None);
                }
                let call =
                    self.late_member_call(member, kind, recv.value, vec![CoreArg::ByVal(val.value.clone())]);
                Ok(Some(vec![CoreStmt::Eval(call)]))
            }
            _ => Ok(None),
        }
    }

    fn bind_call_stmt(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let callee = node
            .call_callee()
            .ok_or_else(|| BindError::Malformed("call statement callee".into()))?;
        let arglist = node.call_arg_list();
        // `Err.Raise` / `Err.Clear` are error-state statements, not value calls.
        if callee.kind() == SyntaxKind::MemberExpr {
            if let Some(recv) = callee.member_receiver() {
                if self.is_err_receiver(recv) {
                    let member = callee
                        .member_name_token()
                        .ok_or_else(|| BindError::Malformed("Err member".into()))?
                        .text;
                    return self.bind_err_statement(member, arglist);
                }
            }
        }
        let bound = self.bind_call_from_callee(callee, arglist)?;
        Ok(vec![CoreStmt::Eval(bound.value)])
    }

    fn bind_err_statement(
        &mut self,
        member: &str,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Vec<CoreStmt>, BindError> {
        match fold_identifier(member).as_str() {
            "raise" => {
                let code = self.const_i32_arg(arglist)?;
                Ok(vec![CoreStmt::Error(ErrorOp::Raise { code })])
            }
            "clear" => Ok(vec![CoreStmt::Error(ErrorOp::ClearErr)]),
            other => Err(BindError::Unsupported(format!("Err.{other}"))),
        }
    }

    // ── Control flow ────────────────────────────────────────

    fn bind_if(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let mut arms = Vec::new();
        let cond = self.bind_required(node.condition_expr(), "If condition")?;
        let body = self.bind_opt_block(node.if_then_block())?;
        arms.push(CoreIfArm { condition: cond, body });
        for elif in node.if_elseif_clauses() {
            let c = self.bind_required(elif.condition_expr(), "ElseIf condition")?;
            let b = self.bind_opt_block(elif.body_block())?;
            arms.push(CoreIfArm { condition: c, body: b });
        }
        let else_body = match node.if_else_clause() {
            Some(ec) => self.bind_opt_block(ec.body_block())?,
            None => Vec::new(),
        };
        Ok(vec![CoreStmt::If { arms, else_body }])
    }

    fn bind_for(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let body = self.bind_opt_block(node.body_block())?;
        if node.for_is_each() {
            let var_node = node
                .foreach_var()
                .ok_or_else(|| BindError::Malformed("For Each variable".into()))?;
            let (item, _ty) = self.bind_place(var_node)?;
            let source = self.bind_required(node.foreach_collection(), "For Each collection")?;
            Ok(vec![CoreStmt::ForEach { item, source, body }])
        } else {
            let counter = node
                .for_counter_token()
                .ok_or_else(|| BindError::Malformed("For counter".into()))?;
            let var = self.place_by_name(counter.text)?;
            let start = self.bind_required(node.for_start(), "For start")?;
            let end = self.bind_required(node.for_end(), "For end")?;
            let step = match node.for_step() {
                Some(s) => Some(self.bind_expr(s)?.value),
                None => None,
            };
            Ok(vec![CoreStmt::ForRange { var, start, end, step, body }])
        }
    }

    fn bind_do(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let body = self.bind_opt_block(node.body_block())?;
        if let Some(pre) = node.do_pre_cond() {
            let condition = self.bind_expr(pre)?.value;
            Ok(vec![CoreStmt::DoLoop { condition, until: node.do_pre_is_until(), post_check: false, body }])
        } else if let Some(post) = node.do_post_cond() {
            let condition = self.bind_expr(post)?.value;
            Ok(vec![CoreStmt::DoLoop { condition, until: node.do_post_is_until(), post_check: true, body }])
        } else {
            // `Do … Loop` — unconditional; loop while True until an `Exit Do`.
            Ok(vec![CoreStmt::DoLoop {
                condition: CoreValue::Const(CoreConst::Bool(true)),
                until: false,
                post_check: false,
                body,
            }])
        }
    }

    fn bind_while(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let condition = self.bind_required(node.condition_expr(), "While condition")?;
        let body = self.bind_opt_block(node.body_block())?;
        Ok(vec![CoreStmt::DoLoop { condition, until: false, post_check: false, body }])
    }

    fn bind_select(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let selector = self.bind_required(node.select_scrutinee(), "Select selector")?;
        let mut cases = Vec::new();
        let mut case_else = Vec::new();
        for clause in node.select_case_clauses() {
            let specs = clause.case_specs();
            let body = self.bind_opt_block(clause.body_block())?;
            if specs.iter().any(|s| matches!(s, CaseSpec::Else)) {
                case_else = body;
                continue;
            }
            let mut clauses = Vec::new();
            for spec in specs {
                match spec {
                    CaseSpec::Value(e) => clauses.push(CaseClause::Value(self.bind_expr(e)?.value)),
                    CaseSpec::Range { lo, hi } => clauses.push(CaseClause::Range {
                        lo: self.bind_expr(lo)?.value,
                        hi: self.bind_expr(hi)?.value,
                    }),
                    CaseSpec::Is { op, value } => {
                        let cop = comparison_binop(op)
                            .ok_or_else(|| BindError::Unsupported("Case Is operator".into()))?;
                        clauses.push(CaseClause::Is { op: cop, value: self.bind_expr(value)?.value });
                    }
                    CaseSpec::Else => {}
                }
            }
            cases.push(CoreCaseBlock { clauses, body });
        }
        Ok(vec![CoreStmt::Select { selector, cases, case_else }])
    }

    fn bind_with(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let obj = self.bind_required_bound(node.condition_expr(), "With object")?;
        self.with_stack.push(obj);
        let body = self.bind_opt_block(node.body_block());
        self.with_stack.pop();
        body
    }

    fn bind_exit(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let kind = node.child_tokens().into_iter().find_map(|t| match t.kind {
            SyntaxKind::KwFor => Some(ExitKind::For),
            SyntaxKind::KwDo => Some(ExitKind::Do),
            SyntaxKind::KwSub | SyntaxKind::KwFunction | SyntaxKind::KwProperty => Some(ExitKind::Proc),
            _ => None,
        });
        kind.map(|k| vec![CoreStmt::Exit(k)])
            .ok_or_else(|| BindError::Malformed("Exit kind".into()))
    }

    // ── Error state ─────────────────────────────────────────

    fn bind_on_error(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let toks = node.child_tokens();
        if toks.iter().any(|t| t.kind == SyntaxKind::KwResume) {
            return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorResumeNext)]);
        }
        if let Some(lref) = node.label_ref() {
            let name = lref.first_significant_token().map(|t| t.text).unwrap_or("");
            if name == "0" {
                return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorGoto0)]);
            }
            return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorGotoLabel(self.label_id(name)))]);
        }
        if toks.iter().any(|t| t.text == "0") {
            return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorGoto0)]);
        }
        Err(BindError::Malformed("On Error form".into()))
    }

    fn bind_resume(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        if node.child_tokens().iter().any(|t| t.kind == SyntaxKind::KwNext) {
            return Ok(vec![CoreStmt::Error(ErrorOp::ResumeNext)]);
        }
        if let Some(lref) = node.label_ref() {
            let name = lref.first_significant_token().map(|t| t.text).unwrap_or("");
            return Ok(vec![CoreStmt::Error(ErrorOp::ResumeLabel(self.label_id(name)))]);
        }
        Ok(vec![CoreStmt::Error(ErrorOp::Resume)])
    }

    // ── ReDim / Erase ───────────────────────────────────────

    fn bind_redim(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        // A member-array target (`ReDim obj.arr(…)`) is parsed as a dotted name
        // path; we only model simple-name arrays, so reject rather than silently
        // re-dimming the receiver.
        if node.child_tokens().iter().any(|t| matches!(t.kind, SyntaxKind::Dot | SyntaxKind::Bang)) {
            return Err(BindError::Unsupported(
                "ReDim of a member array (object support pending)".into(),
            ));
        }
        let preserve = node.child_tokens().iter().any(|t| t.kind == SyntaxKind::KwPreserve);
        let mut out = Vec::new();
        let mut pending: Option<&str> = None;
        for el in node.children() {
            match el {
                SyntaxElement::Token(t)
                    if matches!(t.kind, SyntaxKind::Ident | SyntaxKind::BracketedIdent) =>
                {
                    if pending.is_none() {
                        pending = Some(t.text);
                    }
                }
                SyntaxElement::Node(n) if n.kind() == SyntaxKind::ArrayBounds => {
                    let name = pending
                        .take()
                        .ok_or_else(|| BindError::Malformed("ReDim target".into()))?;
                    out.push(self.redim_one(name, n, preserve)?);
                }
                SyntaxElement::Token(t) if t.kind == SyntaxKind::Comma => pending = None,
                _ => {}
            }
        }
        Ok(out)
    }

    fn redim_one(
        &mut self,
        name: &str,
        bounds_node: SyntaxNode<'_>,
        preserve: bool,
    ) -> Result<CoreStmt, BindError> {
        let array = self.place_by_name(name)?;
        let element_type = self.array_element_for_name(name);
        let mut bounds = Vec::new();
        for b in bounds_node.children_of(SyntaxKind::Bound) {
            let exprs = b.expr_children();
            let (lower, upper) = match exprs.len() {
                0 => return Err(BindError::Malformed("empty ReDim bound".into())),
                1 => (0, self.bind_expr(exprs[0])?.value),
                _ => {
                    let lo_val = self.bind_expr(exprs[0])?.value;
                    let lo = self.const_i32(&lo_val)?;
                    let up = self.bind_expr(exprs[1])?.value;
                    (lo, up)
                }
            };
            bounds.push(CoreBound { upper, lower });
        }
        Ok(CoreStmt::ReDim { array, bounds, element_type, preserve })
    }

    fn bind_erase(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let mut out = Vec::new();
        for target in node.expr_children() {
            let (array, ty) = self.bind_place(target)?;
            let element_type = match &ty {
                oxvba_symbol::signature::VarTypeRef::Array(inner) => types::array_element_of(inner),
                _ => oxvba_bundle::ArrayElementType::Variant,
            };
            out.push(CoreStmt::Erase { array, element_type });
        }
        Ok(out)
    }

    // ── File I/O (best-effort native lowering) ──────────────

    fn bind_file_io(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let id = match node.kind() {
            SyntaxKind::OpenStmt => NativeImplId::FileOpen,
            SyntaxKind::CloseStmt => NativeImplId::FileClose,
            SyntaxKind::PrintStmt => NativeImplId::FilePrint,
            SyntaxKind::WriteStmt => NativeImplId::FileWrite,
            SyntaxKind::InputStmt => NativeImplId::FileInput,
            SyntaxKind::LineInputStmt => NativeImplId::FileLineInput,
            other => return Err(BindError::Unsupported(format!("file I/O {other:?}"))),
        };
        let mut args = Vec::new();
        if let Some(fnum) = node.file_number() {
            if let Some(ch) = fnum.first_expr_child() {
                args.push(CoreArg::ByVal(self.bind_expr(ch)?.value));
            }
        }
        // Print/Write data is nested in a PrintItemList of PrintItems, not direct
        // expr children; descend so the values aren't dropped. Other file
        // statements (Input/Line Input) carry their lvalue targets directly.
        if let Some(list) = node.child_node(SyntaxKind::PrintItemList) {
            for item in list.children_of(SyntaxKind::PrintItem) {
                if let Some(e) = item.first_expr_child() {
                    args.push(CoreArg::ByVal(self.bind_expr(e)?.value));
                }
            }
        } else {
            for e in node.expr_children() {
                args.push(CoreArg::ByVal(self.bind_expr(e)?.value));
            }
        }
        Ok(vec![CoreStmt::Eval(CoreValue::Call { callee: CoreCallee::Native(id), args })])
    }

    // ── Small helpers ───────────────────────────────────────

    fn bind_required(
        &mut self,
        node: Option<SyntaxNode<'_>>,
        what: &str,
    ) -> Result<CoreValue, BindError> {
        Ok(self.bind_required_bound(node, what)?.value)
    }

    fn bind_required_bound(
        &mut self,
        node: Option<SyntaxNode<'_>>,
        what: &str,
    ) -> Result<crate::Bound, BindError> {
        let node = node.ok_or_else(|| BindError::Malformed(format!("missing {what}")))?;
        self.bind_expr(node)
    }

    fn bind_opt_block(
        &mut self,
        block: Option<SyntaxNode<'_>>,
    ) -> Result<Vec<CoreStmt>, BindError> {
        match block {
            Some(b) => self.bind_block(b),
            None => Ok(Vec::new()),
        }
    }

    fn label_ref_id(&mut self, node: SyntaxNode<'_>) -> Result<oxvba_bundle::coreir::LabelId, BindError> {
        let lref = node
            .label_ref()
            .ok_or_else(|| BindError::Malformed("label reference".into()))?;
        let name = lref
            .first_significant_token()
            .ok_or_else(|| BindError::Malformed("empty label reference".into()))?
            .text;
        Ok(self.label_id(name))
    }

    /// Resolve a bare name to a writable place (loop counters, ReDim targets).
    pub(crate) fn place_by_name(&mut self, name: &str) -> Result<CorePlace, BindError> {
        if let Some(rl) = self.return_target(name) {
            return Ok(CorePlace::Local(rl));
        }
        let binding = self
            .resolve(name)
            .ok_or_else(|| self.unresolved(name, "place"))?;
        binding
            .symbol
            .and_then(|s| self.place_for_symbol(s))
            .map(|(p, _)| p)
            .ok_or_else(|| BindError::InvalidAssignment(format!("`{name}` is not a variable")))
    }

    fn array_element_for_name(&self, name: &str) -> oxvba_bundle::ArrayElementType {
        if let Some(sym) = self.resolve(name).and_then(|b| b.symbol) {
            if let oxvba_symbol::signature::VarTypeRef::Array(inner) = self.symbol_type(sym) {
                return types::array_element_of(&inner);
            }
        }
        oxvba_bundle::ArrayElementType::Variant
    }

    /// Fold a value to a constant `i32` if possible (literals + integer
    /// arithmetic of constants, e.g. `vbObjectError + 1` once `vbObjectError`
    /// resolves to a library constant).
    fn fold_const_i32(&self, value: &CoreValue) -> Option<i32> {
        match value {
            CoreValue::Const(CoreConst::I32(n)) => Some(*n),
            CoreValue::Const(CoreConst::I64(n)) => i32::try_from(*n).ok(),
            CoreValue::Binary { op, lhs, rhs, .. } => {
                let a = self.fold_const_i32(lhs)?;
                let b = self.fold_const_i32(rhs)?;
                match op {
                    CoreBinOp::Add => a.checked_add(b),
                    CoreBinOp::Sub => a.checked_sub(b),
                    CoreBinOp::Mul => a.checked_mul(b),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn const_i32(&self, value: &CoreValue) -> Result<i32, BindError> {
        self.fold_const_i32(value)
            .ok_or_else(|| BindError::Unsupported("ReDim bound must be a constant".into()))
    }

    fn const_i32_arg(&mut self, arglist: Option<SyntaxNode<'_>>) -> Result<i32, BindError> {
        let arglist = arglist.ok_or_else(|| BindError::Malformed("Err.Raise number".into()))?;
        let first = arglist
            .arg_items()
            .into_iter()
            .next()
            .ok_or_else(|| BindError::Malformed("Err.Raise number".into()))?;
        let expr = match first {
            ArgItem::Positional(e) => e,
            _ => return Err(BindError::Malformed("Err.Raise number".into())),
        };
        let value = self.bind_expr(expr)?.value;
        self.fold_const_i32(&value)
            .ok_or_else(|| BindError::Unsupported("Err.Raise requires a constant error number".into()))
    }
}

/// True if a resolved route is a project property accessor (Get/Let/Set).
fn is_property_route(route: &DispatchRoute) -> bool {
    matches!(
        route,
        DispatchRoute::ProjectMember {
            kind: ProjectMemberKind::PropertyGet
                | ProjectMemberKind::PropertyLet
                | ProjectMemberKind::PropertySet
        }
    )
}
