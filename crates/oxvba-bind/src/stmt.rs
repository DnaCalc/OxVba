//! The statement binder: `bind_block` / `bind_stmt` lower every statement CST
//! node to `CoreStmt`s — assignment (with intent + coercion), control flow,
//! error-state, `ReDim`/`Erase`, and the file-I/O statements (→ cross-bundle
//! `ExternProc` calls into the synthetic `VBA` bundle's `FileSystem` module).

use oxvba_bundle::coreir::{
    CaseClause, CoreArg, CoreAsNew, CoreBinOp, CoreBound, CoreCallee, CoreCaseBlock, CoreConst,
    CoreIfArm, CorePlace, CoreStmt, CoreUnOp, CoreValue, ErrorOp, ExitKind, LocalId,
};
use oxvba_bundle::native::NativeImplId;
use oxvba_bundle::{
    ArrayElementType, AssignmentIntent, BundleImport, ExportToken, NumericMode, ProjectMemberKind,
};
use oxvba_symbol::binding::{Binding, DispatchRoute};
use oxvba_symbol::model::{SymbolId, SymbolKind, fold_identifier};
use oxvba_symbol::signature::{BuiltinType, VarTypeRef};
use oxvba_syntax::red::{ArgItem, CaseSpec};
use oxvba_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

use crate::call::err_field;
use crate::error::BindError;
use crate::expr::comparison_binop;
use crate::types;
use crate::{LoopContext, ProcLower};

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
            LSetStmt => self.bind_lset_rset(node, NativeImplId::LSetStmt),
            RSetStmt => self.bind_lset_rset(node, NativeImplId::RSetStmt),
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
            EndStmt => Ok(vec![CoreStmt::End]),
            LabelStmt => {
                let token = node
                    .first_significant_token()
                    .ok_or_else(|| BindError::Malformed("label".into()))?;
                let name = token.text;
                let id = self.label_id(name);
                // A label may be *referenced* any number of times, but defining it
                // twice in one procedure is a VBA compile error ("Duplicate
                // declaration in current scope"). Reject it here so vm2 and vm3 agree
                // (vm3's elaboration also rejects the duplicate `LabelId`).
                if !self.defined_labels.insert(id) {
                    return Err(BindError::DuplicateLabel {
                        name: name.to_string(),
                    });
                }
                if token.kind == SyntaxKind::IntLiteral {
                    let line = name
                        .parse::<i32>()
                        .map_err(|_| BindError::Malformed(format!("line number label `{name}`")))?;
                    if let Some(slot) = self.label_lines.get_mut(id.0) {
                        *slot = Some(line);
                    }
                }
                Ok(vec![CoreStmt::Label(id)])
            }
            OnErrorStmt => self.bind_on_error(node),
            OnComputedStmt => self.bind_on_computed(node),
            ErrorStmt => self.bind_error_stmt(node),
            ResumeStmt => self.bind_resume(node),
            ReDimStmt => self.bind_redim(node),
            EraseStmt => self.bind_erase(node),
            OpenStmt => self.bind_open(node),
            CloseStmt | SeekStmt | WidthStmt | NameStmt | LockStmt => self.bind_file_io(node),
            // `Print #`/`Write #` carry an item list whose `,`/`;` separators are
            // significant, so they bind through a dedicated path that preserves every
            // field and its trailing separator.
            PrintStmt | WriteStmt => self.bind_print_write(node),
            // `Input #`/`Line Input #` READ into their targets, so each target is an
            // assignment of a read value (not a discarded ByVal arg).
            InputStmt => self.bind_input(node),
            LineInputStmt => self.bind_line_input(node),
            // `Get #n, [rec], var` reads into the target, so it lowers as an
            // assignment of the read value, not a discarded call.
            GetStmt => self.bind_get(node),
            // `Put #n, [rec], value` flags a fixed-length-string source (no prefix).
            PutStmt => self.bind_put(node),
            RaiseEventStmt => self.bind_raise_event(node),
            // Declarations contribute no executable statement (the frame is already
            // built; any fixed-size array a `Dim` declares is allocated once at proc
            // entry — `collect_fixed_array_inits` — so a `Dim` in a loop body, or after
            // its first use, behaves as VBA's hoisted declaration).
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
        let mut val = self.bind_expr(value_node)?;
        if intent == AssignmentIntent::Let
            && let Some(stmts) = self.try_mid_assignment(target_node, &val.value)?
        {
            return Ok(stmts);
        }
        // A property target (`obj.Prop = x` / bare `Prop = x`) is a setter call,
        // not a place store: Let → Property Let, Set → Property Set.
        let value_text = value_node.text();
        let value_label = value_text.trim();
        if let Some(stmts) =
            self.try_err_field_assignment(target_node, intent, val.clone(), value_label)?
        {
            return Ok(stmts);
        }
        if let Some(stmts) = self.try_property_assignment(target_node, intent, &val, value_label)? {
            return Ok(stmts);
        }
        let (place, target_ty) = self.bind_place(target_node)?;
        if matches!(target_ty, VarTypeRef::FixedArray { .. }) {
            return Err(BindError::CantAssignToArray);
        }
        if intent == AssignmentIntent::Let && !types::is_object(&target_ty) {
            val = self.bind_default_member_value_context(val, value_label)?;
        }
        // A declared scalar variable holds its declared type: coerce the value to the
        // target type on store (unconditionally — the value's static type is not a
        // reliable proxy for its run-time tag). No-op for Object/Variant/array.
        let value = types::coerce_store(val.value, &target_ty);
        Ok(vec![CoreStmt::Assign {
            place,
            value,
            intent,
            target_kind: types::assignment_target_kind(&target_ty),
            target_name: target_node.text().trim().to_string(),
            target_type_name: types::type_name(&target_ty),
        }])
    }

    fn bind_lset_rset(
        &mut self,
        node: SyntaxNode<'_>,
        native: NativeImplId,
    ) -> Result<Vec<CoreStmt>, BindError> {
        let target_node = node
            .assign_target()
            .ok_or_else(|| BindError::Malformed("LSet/RSet target".into()))?;
        let value_node = node
            .assign_value()
            .ok_or_else(|| BindError::Malformed("LSet/RSet value".into()))?;
        let (place, target_ty) = self.bind_place(target_node)?;
        let value_text = value_node.text();
        let value_label = value_text.trim();
        match (&target_ty, native) {
            (VarTypeRef::Builtin(BuiltinType::String) | VarTypeRef::FixedString(_), _) => {}
            (VarTypeRef::Udt(_), NativeImplId::LSetStmt) => {
                let target_layout = self.g.array_element_layout(&target_ty);
                if !lset_record_layout_is_byte_copyable(&target_layout) {
                    return Err(BindError::TypeMismatch);
                }
                let val = self.bind_expr(value_node)?;
                let source_ty = self.g.resolve_udt_type(val.ty.clone());
                if !matches!(source_ty, VarTypeRef::Udt(_)) {
                    return Err(BindError::TypeMismatch);
                }
                let source_layout = self.g.array_element_layout(&source_ty);
                if !lset_record_layout_is_byte_copyable(&source_layout) {
                    return Err(BindError::TypeMismatch);
                }
                return Ok(vec![CoreStmt::LSetRecord {
                    place,
                    value: val.value,
                }]);
            }
            (_, NativeImplId::LSetStmt) => return Err(BindError::LSetTargetType),
            (_, NativeImplId::RSetStmt) => return Err(BindError::RSetTargetType),
            _ => return Err(BindError::Malformed("unknown LSet/RSet lowering".into())),
        }

        let mut val = self.bind_expr(value_node)?;
        val = self.bind_default_member_value_context(val, value_label)?;
        let aligned = CoreValue::Call {
            callee: CoreCallee::Native(native),
            args: vec![
                CoreArg::ByVal(CoreValue::Load(place.clone())),
                CoreArg::ByVal(val.value),
            ],
        };
        Ok(vec![CoreStmt::Assign {
            place,
            value: types::coerce_store(aligned, &target_ty),
            intent: AssignmentIntent::Let,
            target_kind: types::assignment_target_kind(&target_ty),
            target_name: target_node.text().trim().to_string(),
            target_type_name: types::type_name(&target_ty),
        }])
    }

    fn try_err_field_assignment(
        &mut self,
        target: SyntaxNode<'_>,
        intent: AssignmentIntent,
        mut val: crate::Bound,
        value_label: &str,
    ) -> Result<Option<Vec<CoreStmt>>, BindError> {
        if target.kind() != SyntaxKind::MemberExpr {
            return Ok(None);
        }
        let Some(recv) = target.member_receiver() else {
            return Ok(None);
        };
        if !self.is_err_receiver(recv) {
            return Ok(None);
        }
        if intent == AssignmentIntent::Set {
            return Err(BindError::Unsupported(
                "Set assignment to Err property".into(),
            ));
        }
        let member = target
            .member_name_token()
            .ok_or_else(|| BindError::Malformed("Err member assignment".into()))?
            .text;
        let Some((field, target_ty)) = err_field(member) else {
            return Ok(None);
        };
        if matches!(field, oxvba_bundle::coreir::ErrField::LastDllError) {
            return Err(BindError::Unsupported(
                "Err.LastDllError is read-only".into(),
            ));
        }
        val = self.bind_default_member_value_context(val, value_label)?;
        Ok(Some(vec![CoreStmt::Error(ErrorOp::SetErrField {
            field,
            value: types::coerce_store(val.value, &target_ty),
        })]))
    }

    fn try_mid_assignment(
        &mut self,
        target: SyntaxNode<'_>,
        rhs: &CoreValue,
    ) -> Result<Option<Vec<CoreStmt>>, BindError> {
        if target.kind() != SyntaxKind::IndexExpr {
            return Ok(None);
        }
        let Some(base) = target.index_base() else {
            return Ok(None);
        };
        let name = match base.kind() {
            SyntaxKind::IdentExpr => base.ident_name_token().map(|token| token.text),
            SyntaxKind::MemberExpr => base.member_name_token().map(|token| token.text),
            _ => None,
        };
        let Some(name) = name else {
            return Ok(None);
        };
        let folded = oxvba_symbol::model::fold_identifier(name.trim_end_matches('$'));
        if folded != "mid" {
            return Ok(None);
        }
        let items = target
            .index_arg_list()
            .ok_or_else(|| BindError::Malformed("Mid assignment arguments".into()))?
            .arg_items();
        let Some((first, rest)) = items.split_first() else {
            return Err(BindError::Malformed("Mid assignment target".into()));
        };
        let oxvba_syntax::red::ArgItem::Positional(target_expr, _) = first else {
            return Err(BindError::Malformed("Mid assignment target".into()));
        };
        let (place, target_ty) = self.bind_place(*target_expr)?;
        let mut args = vec![CoreArg::ByVal(CoreValue::Load(place.clone()))];
        for item in rest {
            match item {
                oxvba_syntax::red::ArgItem::Positional(expr, _) => {
                    args.push(CoreArg::ByVal(self.bind_expr(*expr)?.value));
                }
                oxvba_syntax::red::ArgItem::Omitted => args.push(CoreArg::Omitted),
                oxvba_syntax::red::ArgItem::Named { .. } => {
                    return Err(BindError::Unsupported(
                        "named arguments in Mid assignment".into(),
                    ));
                }
            }
        }
        args.push(CoreArg::ByVal(rhs.clone()));
        Ok(Some(vec![CoreStmt::Assign {
            place,
            value: CoreValue::Call {
                callee: CoreCallee::Native(NativeImplId::MidStmt),
                args,
            },
            intent: AssignmentIntent::Let,
            target_kind: types::assignment_target_kind(&target_ty),
            target_name: target_expr.text().trim().to_string(),
            target_type_name: types::type_name(&target_ty),
        }]))
    }

    pub(crate) fn bind_default_member_value_context(
        &mut self,
        mut val: crate::Bound,
        label: &str,
    ) -> Result<crate::Bound, BindError> {
        for _ in 0..16 {
            let Some(defaulted) = self.bind_default_member_value(val.clone(), label)? else {
                return Ok(val);
            };
            val = defaulted;
        }
        Err(BindError::Unsupported(format!(
            "default member chain for `{label}` is too deep"
        )))
    }

    /// If `target` denotes a project property, lower the assignment to a Property
    /// Let/Set accessor call (`Eval(Call(LateDispatch, [receiver, value]))`) and
    /// return it; otherwise `None` (the caller does a place store).
    fn try_property_assignment(
        &mut self,
        target: SyntaxNode<'_>,
        intent: AssignmentIntent,
        val: &crate::Bound,
        value_label: &str,
    ) -> Result<Option<Vec<CoreStmt>>, BindError> {
        let kind = match intent {
            AssignmentIntent::Set => ProjectMemberKind::PropertySet,
            _ => ProjectMemberKind::PropertyLet,
        };
        let rhs = match kind {
            ProjectMemberKind::PropertyLet => {
                self.bind_default_member_value_context(val.clone(), value_label)?
                    .value
            }
            _ => val.value.clone(),
        };
        match target.kind() {
            // `Prop(index…) = rhs` — an indexed Property Let/Set.
            SyntaxKind::IndexExpr => self.bind_indexed_property_let(target, kind, &rhs),
            SyntaxKind::IdentExpr => {
                let Some(name) = target.ident_name_token().map(|t| t.text) else {
                    return Ok(None);
                };
                if self.return_target(name).is_some() {
                    return Ok(None);
                }
                let Some(binding) = self.resolve(name) else {
                    return Ok(None);
                };
                if intent == AssignmentIntent::Let
                    && matches!(&binding.route, DispatchRoute::Value)
                    && let Some(sym) = binding.symbol
                    && let ty @ VarTypeRef::Object(_) = self.symbol_type(sym)
                    && let Some(default_binding) = self.resolve_default_member_kind(&ty, Some(kind))
                {
                    return self.bind_default_member_property_let(name, &ty, default_binding, &rhs);
                }
                if !is_property_route(&binding.route) {
                    return Ok(None);
                }
                let Some(sym) = binding.symbol else {
                    return Ok(None);
                };
                let signature = self.project_property_accessor_signature(sym, kind, name)?;
                let mut args = self.bind_property_put_proc_args(None, &signature, sym)?;
                match args.last_mut() {
                    Some(slot) => *slot = CoreArg::ByVal(rhs.clone()),
                    None => args.push(CoreArg::ByVal(rhs.clone())),
                }
                let Some(proc_id) = self.g.ids.prop_accessor_of.get(&(sym, kind)).copied() else {
                    return Ok(None);
                };
                if self
                    .g
                    .ids
                    .procs
                    .get(proc_id.0)
                    .and_then(|info| info.class_name.as_deref())
                    .is_none()
                {
                    return Ok(Some(vec![CoreStmt::Eval(CoreValue::Call {
                        callee: CoreCallee::VbaProc { proc: proc_id },
                        args,
                    })]));
                }
                // A bare class property name is an implicit `Me.Prop`.
                let Some(recv) = self.me_value() else {
                    return Ok(None);
                };
                let call = self.late_member_call(name, kind, recv, args);
                Ok(Some(vec![CoreStmt::Eval(call)]))
            }
            SyntaxKind::MemberExpr => {
                let Some(member) = target.member_name_token().map(|t| t.text) else {
                    return Ok(None);
                };
                if let Some(binding) = self.qualified_namespace_member_binding(target, member) {
                    if !is_property_route(&binding.route) {
                        return Ok(None);
                    }
                    let Some(sym) = binding.symbol else {
                        return Ok(None);
                    };
                    let signature = self.project_property_accessor_signature(sym, kind, member)?;
                    let mut args = self.bind_property_put_proc_args(None, &signature, sym)?;
                    match args.last_mut() {
                        Some(slot) => *slot = CoreArg::ByVal(rhs.clone()),
                        None => args.push(CoreArg::ByVal(rhs.clone())),
                    }
                    let Some(proc_id) = self.g.ids.prop_accessor_of.get(&(sym, kind)).copied()
                    else {
                        return Ok(None);
                    };
                    return Ok(Some(vec![CoreStmt::Eval(CoreValue::Call {
                        callee: CoreCallee::VbaProc { proc: proc_id },
                        args,
                    })]));
                }
                let recv = self.member_receiver_bound(target)?;
                // Through an interface-typed target, the setter is the mangled
                // `Interface_Property` accessor on the implementing class.
                let dispatch = self.interface_dispatch_name(&recv.ty, member);
                let setter = |this: &Self, recv_value| {
                    let call = this.late_member_call(
                        &dispatch,
                        kind,
                        recv_value,
                        vec![CoreArg::ByVal(rhs.clone())],
                    );
                    Ok(Some(vec![CoreStmt::Eval(call)]))
                };
                match self.resolve_member(&recv.ty, member, Some(kind)) {
                    // A project class/interface property must have the requested
                    // accessor. The project symbol provider resolves the property
                    // group; the binder chooses Let vs Set from assignment syntax.
                    Some(Binding {
                        route: DispatchRoute::ProjectMember { .. },
                        symbol: Some(sym),
                        ..
                    }) => {
                        let signature =
                            self.project_property_accessor_signature(sym, kind, member)?;
                        let mut args = self.bind_property_put_proc_args(None, &signature, sym)?;
                        match args.last_mut() {
                            Some(slot) => *slot = CoreArg::ByVal(rhs.clone()),
                            None => args.push(CoreArg::ByVal(rhs.clone())),
                        }
                        let call = self.late_member_call(&dispatch, kind, recv.value, args);
                        Ok(Some(vec![CoreStmt::Eval(call)]))
                    }
                    // A typed COM receiver's property put/set: dispatch by dispid
                    // (the same early-bound call shape as a COM method, with the RHS
                    // as the single value argument). `kind` (Let → PropertyLet, Set →
                    // PropertySet) selects PROPERTYPUT vs PROPERTYPUTREF at the HAL.
                    Some(Binding {
                        route:
                            DispatchRoute::ComMember {
                                interface_name,
                                member: com_member,
                                ..
                            },
                        ..
                    }) => {
                        let call = self.early_com_call(
                            member,
                            kind,
                            &interface_name,
                            &com_member,
                            recv.value,
                            vec![CoreArg::ByVal(rhs.clone())],
                        );
                        Ok(Some(vec![CoreStmt::Eval(call)]))
                    }
                    // A cross-project coclass property put/set: late dispatch by name
                    // in the receiver's bundle.
                    Some(Binding {
                        route: DispatchRoute::ExternMember { member: m, .. },
                        ..
                    }) => {
                        let call = self.late_member_call(
                            &m,
                            kind,
                            recv.value,
                            vec![CoreArg::ByVal(rhs.clone())],
                        );
                        Ok(Some(vec![CoreStmt::Eval(call)]))
                    }
                    Some(binding) if is_property_route(&binding.route) => setter(self, recv.value),
                    // A field/method member → an l-value place store (handled upstream).
                    Some(_) => Ok(None),
                    // An untyped/foreign receiver → a late-bound property put.
                    None if self.is_late_bound_receiver(&recv.ty) => setter(self, recv.value),
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn bind_call_stmt(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let callee = node
            .call_callee()
            .ok_or_else(|| BindError::Malformed("call statement callee".into()))?;
        let arglist = node.call_arg_list();
        if callee
            .first_significant_token()
            .is_some_and(|t| fold_identifier(t.text) == "stop")
        {
            if arglist.as_ref().is_some_and(|a| !a.arg_items().is_empty()) {
                return Err(BindError::Malformed("Stop takes no arguments".into()));
            }
            return Ok(Vec::new());
        }
        // `Err.Raise` / `Err.Clear` are error-state statements, not value calls.
        if callee.kind() == SyntaxKind::MemberExpr
            && let Some(recv) = callee.member_receiver()
            && self.is_err_receiver(recv)
        {
            let member = callee
                .member_name_token()
                .ok_or_else(|| BindError::Malformed("Err member".into()))?
                .text;
            return self.bind_err_statement(member, arglist);
        }
        // `Debug.Print` / `Debug.Assert` are diagnostics statements; `Debug` is a
        // predeclared object that is not a value (so it cannot bind as a receiver
        // through the general member-call path).
        if callee.kind() == SyntaxKind::MemberExpr
            && let Some(recv) = callee.member_receiver()
            && self.is_debug_receiver(recv)
        {
            let member = callee
                .member_name_token()
                .ok_or_else(|| BindError::Malformed("Debug member".into()))?
                .text;
            return self.bind_debug_statement(member, arglist);
        }
        let bound = self.bind_call_from_callee(callee, arglist)?;
        Ok(vec![CoreStmt::Eval(bound.value)])
    }

    /// `Debug.Print <items>` / `Debug.Assert <cond>`. `Print` joins its arguments
    /// through the `DebugPrint` host intrinsic (the HAL diagnostics callback);
    /// `Assert` evaluates its condition for side effects but does not break into a
    /// debugger in a headless runtime (a real IDE break is deferred — see
    /// POST_CLEANUP.md).
    fn bind_debug_statement(
        &mut self,
        member: &str,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Vec<CoreStmt>, BindError> {
        match fold_identifier(member).as_str() {
            "print" => {
                let args = self.bind_args(arglist, None)?;
                Ok(vec![CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::Native(NativeImplId::DebugPrint),
                    args,
                })])
            }
            "assert" => {
                let values = match arglist {
                    Some(a) => self.bind_positional_values(a)?,
                    None => Vec::new(),
                };
                Ok(values.into_iter().map(CoreStmt::Eval).collect())
            }
            other => Err(BindError::Unsupported(format!("Debug.{other}"))),
        }
    }

    /// `RaiseEvent E(args)` inside a class — the source is the current instance
    /// (`Me`); `event` is the event's index within its declaring class.
    fn bind_raise_event(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let name_token = node
            .raise_event_name_token()
            .ok_or_else(|| BindError::Malformed("RaiseEvent without an event name".into()))?;
        let name = normalize_identifier_token(name_token.text);
        let source = self
            .me_value()
            .ok_or_else(|| BindError::Malformed("RaiseEvent outside a class module".into()))?;
        let symbol = self
            .resolve_event_in_enclosing_module(name)?
            .ok_or_else(|| self.unresolved(name, "event"))?;
        let event = self
            .g
            .ids
            .event_index_of
            .get(&symbol)
            .copied()
            .ok_or_else(|| self.unresolved(name, "event index"))?;
        // Bind against the event's declared signature so ByRef parameters (the
        // VBA default) bind ByRef and write back to the raiser, and ByVal
        // parameters bind ByVal. Without it every argument defaulted to ByVal,
        // silently dropping ByRef event-parameter write-backs.
        let signature = self.event_signature(symbol);
        let args = self.bind_args(node.raise_event_arg_list(), signature.as_ref())?;
        Ok(vec![CoreStmt::RaiseEvent {
            source,
            event,
            args,
        }])
    }

    fn resolve_event_in_enclosing_module(&self, name: &str) -> Result<Option<SymbolId>, BindError> {
        let Some(module_scope) = self
            .g
            .env
            .symbols
            .scopes()
            .iter()
            .find(|scope| scope.id == self.info.proc_scope)
            .and_then(|scope| scope.parent)
        else {
            return Ok(None);
        };
        let folded = fold_identifier(name);
        let symbols = self
            .g
            .env
            .symbols
            .symbols_in_scope(module_scope)
            .map_err(|e| BindError::Malformed(format!("{e:?}")))?;
        for sym_id in symbols {
            let Some(sym) = self.g.env.symbols.symbol(sym_id) else {
                continue;
            };
            if sym.kind != SymbolKind::Event {
                continue;
            }
            let Some(sym_name) = self.g.env.symbols.name(sym.name) else {
                continue;
            };
            if sym_name.folded == folded {
                return Ok(Some(sym_id));
            }
        }
        Ok(None)
    }

    fn bind_err_statement(
        &mut self,
        member: &str,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Vec<CoreStmt>, BindError> {
        match fold_identifier(member).as_str() {
            "raise" => {
                let op = self.err_raise_arg(arglist)?;
                Ok(vec![CoreStmt::Error(op)])
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
        arms.push(CoreIfArm {
            condition: cond,
            body,
        });
        for elif in node.if_elseif_clauses() {
            let c = self.bind_required(elif.condition_expr(), "ElseIf condition")?;
            let b = self.bind_opt_block(elif.body_block())?;
            arms.push(CoreIfArm {
                condition: c,
                body: b,
            });
        }
        let else_body = match node.if_else_clause() {
            Some(ec) => self.bind_opt_block(ec.body_block())?,
            None => Vec::new(),
        };
        Ok(vec![CoreStmt::If { arms, else_body }])
    }

    fn bind_for(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        if node.for_is_each() {
            let var_node = node
                .foreach_var()
                .ok_or_else(|| BindError::Malformed("For Each variable".into()))?;
            let (item, _ty) = self.bind_place(var_node)?;
            let source = self.bind_required(node.foreach_collection(), "For Each collection")?;
            let body = self.bind_loop_body(node.body_block(), LoopContext::For)?;
            Ok(vec![CoreStmt::ForEach { item, source, body }])
        } else {
            let counter = node
                .for_counter_token()
                .ok_or_else(|| BindError::Malformed("For counter".into()))?;
            let (var, counter_ty) = self.place_ty_by_name(counter.text)?;
            let start = types::coerce_store(
                self.bind_required(node.for_start(), "For start")?,
                &counter_ty,
            );
            let end =
                types::coerce_store(self.bind_required(node.for_end(), "For end")?, &counter_ty);
            let step = match node.for_step() {
                Some(s) => Some(types::coerce_store(self.bind_expr(s)?.value, &counter_ty)),
                None => None,
            };
            let body = self.bind_loop_body(node.body_block(), LoopContext::For)?;
            Ok(vec![CoreStmt::ForRange {
                var,
                start,
                end,
                step,
                body,
            }])
        }
    }

    fn bind_do(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let body = self.bind_loop_body(node.body_block(), LoopContext::Do)?;
        if let Some(pre) = node.do_pre_cond() {
            let condition = self.bind_expr(pre)?.value;
            Ok(vec![CoreStmt::DoLoop {
                condition,
                until: node.do_pre_is_until(),
                post_check: false,
                body,
            }])
        } else if let Some(post) = node.do_post_cond() {
            let condition = self.bind_expr(post)?.value;
            Ok(vec![CoreStmt::DoLoop {
                condition,
                until: node.do_post_is_until(),
                post_check: true,
                body,
            }])
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
        let body = self.bind_loop_body(node.body_block(), LoopContext::While)?;
        Ok(vec![CoreStmt::DoLoop {
            condition,
            until: false,
            post_check: false,
            body,
        }])
    }

    fn bind_loop_body(
        &mut self,
        body: Option<SyntaxNode<'_>>,
        kind: LoopContext,
    ) -> Result<Vec<CoreStmt>, BindError> {
        self.loop_stack.push(kind);
        let result = self.bind_opt_block(body);
        self.loop_stack.pop();
        result
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
                        clauses.push(CaseClause::Is {
                            op: cop,
                            value: self.bind_expr(value)?.value,
                        });
                    }
                    CaseSpec::Else => {}
                }
            }
            cases.push(CoreCaseBlock { clauses, body });
        }
        Ok(vec![CoreStmt::Select {
            selector,
            cases,
            case_else,
            compare_mode: self.info.compare_mode,
        }])
    }

    fn bind_with(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let obj = self.bind_required_bound(node.condition_expr(), "With object")?;
        let id = self.next_with_temp;
        self.next_with_temp += 1;
        self.with_stack.push(crate::Bound {
            value: CoreValue::WithTemp(id),
            ty: obj.ty.clone(),
            place: obj.place.clone(),
        });
        let body = self.bind_opt_block(node.body_block());
        self.with_stack.pop();
        Ok(vec![CoreStmt::With {
            id,
            receiver: obj.value,
            body: body?,
        }])
    }

    fn bind_exit(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let kind = node.child_tokens().into_iter().find_map(|t| match t.kind {
            SyntaxKind::KwFor => Some(ExitKind::For),
            SyntaxKind::KwDo => Some(ExitKind::Do),
            SyntaxKind::KwSub | SyntaxKind::KwFunction | SyntaxKind::KwProperty => {
                Some(ExitKind::Proc)
            }
            _ => None,
        });
        match kind {
            Some(ExitKind::For) if self.loop_stack.iter().rev().any(|k| *k == LoopContext::For) => {
                Ok(vec![CoreStmt::Exit(ExitKind::For)])
            }
            Some(ExitKind::For) => Err(BindError::Malformed("Exit For outside For loop".into())),
            Some(ExitKind::Do) if self.can_exit_do() => Ok(vec![CoreStmt::Exit(ExitKind::Do)]),
            Some(ExitKind::Do) => Err(BindError::Malformed("Exit Do outside Do loop".into())),
            Some(ExitKind::Proc) => Ok(vec![CoreStmt::Exit(ExitKind::Proc)]),
            None => Err(BindError::Malformed("Exit kind".into())),
        }
    }

    fn can_exit_do(&self) -> bool {
        self.loop_stack
            .iter()
            .rev()
            .find(|kind| matches!(kind, LoopContext::Do | LoopContext::While))
            .is_some_and(|kind| *kind == LoopContext::Do)
    }

    // ── Error state ─────────────────────────────────────────

    fn bind_on_error(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let toks = node.child_tokens();
        if toks.iter().any(|t| t.kind == SyntaxKind::KwResume) {
            return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorResumeNext)]);
        }
        if let Some(lref) = node.label_ref() {
            // `-1` lexes as two tokens (Minus + IntLiteral); normalize the label text so
            // `On Error GoTo -1` is recognized regardless of intervening whitespace.
            let normalized: String = lref.text().split_whitespace().collect();
            if normalized == "-1" {
                return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorGotoMinus1)]);
            }
            let name = lref.first_significant_token().map(|t| t.text).unwrap_or("");
            if name == "0" {
                return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorGoto0)]);
            }
            return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorGotoLabel(
                self.label_id(name),
            ))]);
        }
        if toks.iter().any(|t| t.text == "0") {
            return Ok(vec![CoreStmt::Error(ErrorOp::OnErrorGoto0)]);
        }
        Err(BindError::Malformed("On Error form".into()))
    }

    /// `On <selector> GoTo L1, L2, …` / `On <selector> GoSub S1, S2, …` — the computed
    /// branch. The selector is 1-based; `is_gosub` is taken from the `GoSub` keyword.
    fn bind_on_computed(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let is_gosub = node
            .child_tokens()
            .iter()
            .any(|t| t.kind == SyntaxKind::KwGoSub);
        let selector = self.bind_required(node.first_expr_child(), "On <expr> selector")?;
        let targets: Vec<oxvba_bundle::coreir::LabelId> = node
            .child_nodes()
            .iter()
            .filter(|n| n.kind() == SyntaxKind::LabelRef)
            .map(|lref| {
                let name = lref.first_significant_token().map(|t| t.text).unwrap_or("");
                self.label_id(name)
            })
            .collect();
        if targets.is_empty() {
            return Err(BindError::Malformed(
                "On <expr> GoTo/GoSub needs at least one label".into(),
            ));
        }
        Ok(vec![CoreStmt::ComputedGoto {
            selector,
            targets,
            is_gosub,
        }])
    }

    fn bind_resume(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        if node
            .child_tokens()
            .iter()
            .any(|t| t.kind == SyntaxKind::KwNext)
        {
            return Ok(vec![CoreStmt::Error(ErrorOp::ResumeNext)]);
        }
        if let Some(lref) = node.label_ref() {
            let name = lref.first_significant_token().map(|t| t.text).unwrap_or("");
            if name == "0" {
                return Ok(vec![CoreStmt::Error(ErrorOp::Resume)]);
            }
            return Ok(vec![CoreStmt::Error(ErrorOp::ResumeLabel(
                self.label_id(name),
            ))]);
        }
        Ok(vec![CoreStmt::Error(ErrorOp::Resume)])
    }

    // ── ReDim / Erase ───────────────────────────────────────

    fn bind_redim(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        // The target is parsed as a flat token path under `ReDimStmt`
        // (`Ident`/`Me` separated by `.`/`!`) followed by the `ArrayBounds` node —
        // so we accumulate the path segments for the current target and rebuild a
        // simple-name or member-array place from them.
        let preserve = node
            .child_tokens()
            .iter()
            .any(|t| t.kind == SyntaxKind::KwPreserve);
        let mut out = Vec::new();
        let mut segments: Vec<&str> = Vec::new();
        for el in node.children() {
            match el {
                SyntaxElement::Token(t)
                    if matches!(
                        t.kind,
                        SyntaxKind::Ident | SyntaxKind::BracketedIdent | SyntaxKind::KwMe
                    ) =>
                {
                    segments.push(t.text);
                }
                // `.`/`!` are path separators — keep accumulating segments.
                SyntaxElement::Token(t) if matches!(t.kind, SyntaxKind::Dot | SyntaxKind::Bang) => {
                }
                SyntaxElement::Node(n) if n.kind() == SyntaxKind::ArrayBounds => {
                    out.push(self.redim_one(&segments, n, preserve)?);
                    segments.clear();
                }
                SyntaxElement::Token(t) if t.kind == SyntaxKind::Comma => segments.clear(),
                _ => {}
            }
        }
        Ok(out)
    }

    /// The array place + element type for one ReDim target: a simple name (a
    /// local/global/Me-field) or a dotted member path (`obj.arr`).
    fn redim_target(
        &mut self,
        segments: &[&str],
        preserve: bool,
    ) -> Result<(CorePlace, oxvba_bundle::ArrayElementType), BindError> {
        match segments {
            [] => Err(BindError::Malformed("ReDim target".into())),
            [name] => self.redim_simple_target(name, preserve),
            _ => {
                let (place, ty) = self.redim_dotted_place(segments)?;
                // Only Local/Global/Field places are valid array storage — a
                // WithEvents member is not re-dimmable.
                if matches!(place, CorePlace::WithEvents { .. }) {
                    return Err(BindError::Unsupported(
                        "ReDim of a WithEvents member".into(),
                    ));
                }
                let element_type = match &ty {
                    oxvba_symbol::signature::VarTypeRef::Variant => {
                        oxvba_bundle::ArrayElementType::Variant
                    }
                    oxvba_symbol::signature::VarTypeRef::Array(inner) => {
                        self.g.array_element_layout(inner)
                    }
                    oxvba_symbol::signature::VarTypeRef::FixedArray { element, .. } => {
                        self.g.array_element_layout(element)
                    }
                    _ => {
                        return Err(BindError::ExpectedArray {
                            name: segments.join("."),
                        });
                    }
                };
                Ok((place, element_type))
            }
        }
    }

    fn redim_simple_target(
        &mut self,
        name: &str,
        preserve: bool,
    ) -> Result<(CorePlace, oxvba_bundle::ArrayElementType), BindError> {
        if preserve && self.resolve(name).is_none() {
            if self.has_ambiguous_unqualified_name(name) {
                return Err(BindError::AmbiguousName {
                    name: name.to_string(),
                });
            }
            return Err(BindError::VariableNotDefined {
                name: name.to_string(),
            });
        }
        let (place, ty) = self.place_ty_by_name(name).map_err(|err| {
            if preserve && matches!(err, BindError::Unresolved { .. }) {
                BindError::VariableNotDefined {
                    name: name.to_string(),
                }
            } else {
                err
            }
        })?;
        let element_type = match ty {
            VarTypeRef::Variant => oxvba_bundle::ArrayElementType::Variant,
            VarTypeRef::Array(inner) => self.g.array_element_layout(&inner),
            VarTypeRef::FixedArray { element, .. } => self.g.array_element_layout(&element),
            _ => {
                return Err(BindError::ExpectedArray {
                    name: name.to_string(),
                });
            }
        };
        Ok((place, element_type))
    }

    /// Rebuild a `CorePlace::Field` for a dotted ReDim target (`a.b.arr`): the
    /// leading segment resolves to a variable (or `Me`) and supplies the initial
    /// receiver value+type; each further segment is a member access, the final
    /// one being the array field.
    fn redim_dotted_place(
        &mut self,
        segments: &[&str],
    ) -> Result<(CorePlace, oxvba_symbol::signature::VarTypeRef), BindError> {
        use oxvba_symbol::signature::VarTypeRef;
        let (first, rest) = segments
            .split_first()
            .expect("dotted path has >= 2 segments");
        // Leading receiver: `Me`, or a resolved local/global/field variable.
        let (mut recv, mut ty): (CoreValue, VarTypeRef) = if fold_identifier(first) == "me" {
            let me = self
                .me_value()
                .ok_or_else(|| BindError::Malformed("`Me` outside a class module".into()))?;
            let class_ty = self
                .info
                .class_name
                .as_deref()
                .map(|n| VarTypeRef::Object(n.to_string()))
                .unwrap_or(VarTypeRef::Variant);
            (me, class_ty)
        } else {
            let binding = self
                .resolve(first)
                .ok_or_else(|| self.unresolved(first, "ReDim receiver"))?;
            let (place, ty) = binding
                .symbol
                .and_then(|s| self.place_for_symbol(s))
                .ok_or_else(|| {
                    BindError::InvalidAssignment(format!("`{first}` is not a variable"))
                })?;
            (CoreValue::Load(place), ty)
        };
        let mut place = CorePlace::Local(LocalId(0)); // overwritten on the first segment below
        for seg in rest {
            // A field of a UDT receiver (`o.Lines`) is a fixed-index record
            // element — the same `RecordField` shape `udt_field_place` builds,
            // threaded through the dotted ReDim path so a UDT array field is a
            // re-dimmable place. (The receiver is always a `Load(place)` here:
            // we only reach `Udt` after descending into a record field, or from
            // a UDT-typed leading variable.)
            if let VarTypeRef::Udt(udt) = &ty
                && let Some((index, field_ty)) =
                    self.g.env.udt_field(udt, seg).map(|(i, t)| (i, t.clone()))
            {
                let base = match &recv {
                    CoreValue::Load(p) => p.clone(),
                    _ => {
                        return Err(BindError::Unsupported(format!(
                            "ReDim of UDT field `.{seg}` on a non-place receiver"
                        )));
                    }
                };
                place = CorePlace::RecordField {
                    base: Box::new(base),
                    index,
                };
                ty = self.g.resolve_udt_type(field_ty);
                recv = CoreValue::Load(place.clone());
                continue;
            }
            let mb = self
                .resolve_member(&ty, seg, None)
                .ok_or_else(|| self.unresolved(seg, "ReDim member"))?;
            match &mb.route {
                DispatchRoute::Value => {
                    let sym = mb
                        .symbol
                        .ok_or_else(|| self.unresolved(seg, "ReDim member field"))?;
                    let (p, t) = self.member_place(recv.clone(), sym)?;
                    place = p;
                    ty = t;
                    recv = CoreValue::Load(place.clone());
                }
                other => {
                    return Err(BindError::Unsupported(format!(
                        "ReDim of `.{seg}` ({other:?})"
                    )));
                }
            }
        }
        Ok((place, ty))
    }

    fn redim_one(
        &mut self,
        segments: &[&str],
        bounds_node: SyntaxNode<'_>,
        preserve: bool,
    ) -> Result<CoreStmt, BindError> {
        let (array, element_type) = self.redim_target(segments, preserve)?;
        let bounds = self.bind_array_bounds(bounds_node)?;
        Ok(CoreStmt::ReDim {
            array,
            bounds,
            element_type,
            preserve,
            // A user `ReDim` produces a dynamic (resizable) array.
            fixed: false,
        })
    }

    /// Lower an `ArrayBounds` node to `CoreBound`s (each `lower To upper`, or
    /// `<Option Base> To upper` for a single bound).
    fn bind_array_bounds(
        &mut self,
        bounds_node: SyntaxNode<'_>,
    ) -> Result<Vec<CoreBound>, BindError> {
        let mut bounds = Vec::new();
        for b in bounds_node.children_of(SyntaxKind::Bound) {
            let exprs = b.expr_children();
            let (lower, upper) = match exprs.len() {
                0 => return Err(BindError::Malformed("empty array bound".into())),
                // A single bound (`Dim a(10)`) takes the module's `Option Base` as
                // its lower bound; an explicit `lo To hi` overrides it.
                1 => (
                    CoreValue::Const(CoreConst::I32(self.info.option_base)),
                    self.bind_expr(exprs[0])?.value,
                ),
                _ => {
                    let lo_val = self.bind_expr(exprs[0])?.value;
                    let up = self.bind_expr(exprs[1])?.value;
                    (lo_val, up)
                }
            };
            bounds.push(CoreBound { upper, lower });
        }
        Ok(bounds)
    }

    /// Collect the fixed-size array allocations for every `Dim` anywhere under `node`
    /// (recursively, including loop/`If` bodies). VBA hoists declarations, so these run
    /// once at proc entry regardless of where the `Dim` appears; a dynamic `Dim a()` is
    /// skipped (allocated by a later `ReDim`).
    pub(crate) fn collect_fixed_array_inits(
        &mut self,
        node: SyntaxNode<'_>,
    ) -> Result<Vec<CoreStmt>, BindError> {
        let mut out = Vec::new();
        self.walk_fixed_array_inits(node, &mut out)?;
        Ok(out)
    }

    fn walk_fixed_array_inits(
        &mut self,
        node: SyntaxNode<'_>,
        out: &mut Vec<CoreStmt>,
    ) -> Result<(), BindError> {
        if node.kind() == SyntaxKind::DimStmt {
            out.extend(self.bind_dim(node)?);
        }
        for child in node.child_nodes() {
            self.walk_fixed_array_inits(child, out)?;
        }
        Ok(())
    }

    /// A `Dim` declarator allocation: a *fixed-size* array (`Dim a(1 To 3)`) via the
    /// `ReDim` path, or a UDT value (`Dim p As Point`) as a default-initialized record.
    /// A dynamic array (`Dim a()`) and plain scalars need no allocation.
    ///
    /// `Static` declarators are SKIPPED here — they persist across calls, so
    /// re-allocating them in the per-call prologue would reset them. They are
    /// allocated exactly once at program entry by [`Self::bind_static_dim`].
    pub(crate) fn bind_dim(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        self.bind_dim_filtered(node, false)
    }

    pub(crate) fn function_return_default_init(&mut self) -> Result<Vec<CoreStmt>, BindError> {
        let Some(local) = self.info.return_local else {
            return Ok(Vec::new());
        };
        let Some((value, intent, target_kind, type_name)) =
            default_value_for_type(&self.info.return_type)
        else {
            return Ok(Vec::new());
        };
        Ok(vec![CoreStmt::Assign {
            place: CorePlace::Local(local),
            value,
            intent,
            target_kind,
            target_name: self.info.name.clone(),
            target_type_name: type_name,
        }])
    }

    /// The per-program-entry allocation of a `Static` declarator's array/record
    /// (the complement of [`Self::bind_dim`], which skips statics).
    pub(crate) fn bind_static_dim(
        &mut self,
        node: SyntaxNode<'_>,
    ) -> Result<Vec<CoreStmt>, BindError> {
        self.bind_dim_filtered(node, true)
    }

    fn bind_dim_filtered(
        &mut self,
        node: SyntaxNode<'_>,
        want_static: bool,
    ) -> Result<Vec<CoreStmt>, BindError> {
        let mut out = Vec::new();
        for declarator in node.declarators() {
            let Some(name) = declarator.declarator_name() else {
                continue;
            };
            if self.is_static_local(name.text) != want_static {
                continue;
            }
            // `Dim x As New Foo` is a slot property, not an eager `Set x = New Foo`.
            // Register the slot here; the VM lazily instantiates on reads and repeats
            // that behavior after `Set x = Nothing`, matching Excel/VBA.
            if declarator.is_new()
                && let Some(sym) = self.resolve(name.text).and_then(|b| b.symbol)
                && let oxvba_symbol::signature::VarTypeRef::Object(type_name) =
                    self.symbol_type(sym)
            {
                let place = self.place_by_name(name.text)?;
                out.push(CoreStmt::AsNew {
                    place,
                    binding: self.as_new_binding_for_type(&type_name)?,
                });
                continue; // an As-New object is neither an array nor a UDT value.
            }
            // A fixed-size array allocates via ReDim.
            if let Some(bounds_node) = declarator.array_bounds() {
                if !bounds_node.children_of(SyntaxKind::Bound).is_empty() {
                    let (array, element_type) = self.redim_target(&[name.text], false)?;
                    let bounds = self.bind_array_bounds(bounds_node)?;
                    out.push(CoreStmt::ReDim {
                        array,
                        bounds,
                        element_type,
                        preserve: false,
                        // A fixed-size `Dim a(1 To 3)` array: `Erase` resets it.
                        fixed: true,
                    });
                }
                continue; // a dynamic `Dim a()` stays unallocated.
            }
            // A UDT value allocates a default record (recursively, so nested
            // UDT fields are themselves records).
            out.extend(self.udt_record_init(name.text)?);
            if let Some(sym) = self.resolve(name.text).and_then(|b| b.symbol) {
                let ty = self.symbol_type(sym);
                if let Some((value, intent, target_kind, type_name)) = default_value_for_type(&ty) {
                    let place = self.place_by_name(name.text)?;
                    out.push(CoreStmt::Assign {
                        place,
                        value,
                        intent,
                        target_kind,
                        target_name: name.text.to_string(),
                        target_type_name: type_name,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Whether `name` resolves to a procedure-local declared `Static` (or a local
    /// of a `Static` procedure) — lowered to a persistent bundle global.
    fn is_static_local(&self, name: &str) -> bool {
        self.resolve(name)
            .and_then(|b| b.symbol)
            .and_then(|s| self.g.env.symbols.symbol(s))
            .map(|s| s.kind == oxvba_symbol::model::SymbolKind::StaticLocal)
            .unwrap_or(false)
    }

    fn as_new_binding_for_type(&mut self, type_name: &str) -> Result<CoreAsNew, BindError> {
        self.g.as_new_binding_for_type(type_name)
    }

    /// Default-record allocation statements if `name` is a UDT-typed variable: a
    /// fresh native record with the type's recursive field layout. UDT array
    /// fields stay unallocated until their own `ReDim`, while fixed-array fields
    /// are materialized by the existing follow-up `ReDim` statement below.
    pub(crate) fn udt_record_init(&mut self, name: &str) -> Result<Vec<CoreStmt>, BindError> {
        let Some(sym) = self.resolve(name).and_then(|b| b.symbol) else {
            return Ok(Vec::new());
        };
        let oxvba_symbol::signature::VarTypeRef::Udt(udt) = self.symbol_type(sym) else {
            return Ok(Vec::new());
        };
        let place = self.place_by_name(name)?;
        let mut out = Vec::new();
        self.emit_udt_record_init(place, &udt, name, &mut out)?;
        Ok(out)
    }

    /// Emit the default-record allocation for `place` (a value of UDT `udt`),
    /// recursing into scalar-UDT fields so the whole nested record graph is
    /// materialized. VBA forbids by-value self-referential UDTs, so the
    /// recursion terminates.
    fn emit_udt_record_init(
        &mut self,
        place: CorePlace,
        udt: &str,
        label: &str,
        out: &mut Vec<CoreStmt>,
    ) -> Result<(), BindError> {
        let oxvba_bundle::ArrayElementType::Record(fields) = self
            .g
            .array_element_layout(&VarTypeRef::Udt(udt.to_string()))
        else {
            return Ok(());
        };
        out.push(CoreStmt::Assign {
            place: place.clone(),
            value: CoreValue::NewRecord { fields },
            intent: AssignmentIntent::Let,
            target_kind: oxvba_bundle::AssignmentTargetKind::Scalar,
            target_name: label.to_string(),
            target_type_name: udt.to_string(),
        });
        // The (index, inner-UDT-name) of each scalar UDT field, collected first
        // so the env borrow is dropped before the recursive calls.
        let fields: Vec<(usize, String, VarTypeRef)> = self
            .g
            .env
            .udt_field_list(udt)
            .map(|fields| {
                fields
                    .iter()
                    .enumerate()
                    .map(|(i, (name, ty))| (i, name.clone(), self.g.resolve_udt_type(ty.clone())))
                    .collect()
            })
            .unwrap_or_default();
        for (index, field_name, ty) in fields {
            let sub = CorePlace::RecordField {
                base: Box::new(place.clone()),
                index,
            };
            match ty {
                VarTypeRef::Udt(inner) => {
                    self.emit_udt_record_init(sub, &inner, &format!("{label}.{inner}"), out)?;
                }
                VarTypeRef::Array(inner) => {
                    if let Some(bounds) = self.udt_fixed_array_field_bounds(udt, &field_name)? {
                        out.push(CoreStmt::ReDim {
                            array: sub,
                            bounds,
                            element_type: self.g.array_element_layout(&inner),
                            preserve: false,
                            // A UDT fixed-array field (`arr(1 To 3) As Long`): `Erase` resets it.
                            fixed: true,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn udt_fixed_array_field_bounds(
        &mut self,
        udt: &str,
        field: &str,
    ) -> Result<Option<Vec<CoreBound>>, BindError> {
        let udt = fold_identifier(udt);
        let field = fold_identifier(field);
        for module in self.g.env.all_modules() {
            for type_block in module.syntax.children_of(SyntaxKind::TypeBlock) {
                let Some(type_name) = type_block_name(type_block) else {
                    continue;
                };
                if fold_identifier(type_name.trim_matches(['[', ']'])) != udt {
                    continue;
                }
                for type_field in type_block.type_fields() {
                    let Some(field_name) = type_field.declarator_name() else {
                        continue;
                    };
                    if fold_identifier(field_name.text.trim_matches(['[', ']'])) == field
                        && let Some(bounds) = type_field.array_bounds()
                        && !bounds.children_of(SyntaxKind::Bound).is_empty()
                    {
                        return self.bind_array_bounds(bounds).map(Some);
                    }
                }
            }
        }
        Ok(None)
    }

    fn bind_erase(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let mut out = Vec::new();
        for target in node.expr_children() {
            let (array, ty) = self.bind_place(target)?;
            // The element type drives a fixed-array `Erase`'s typed re-default
            // (e.g. a UDT fixed-array field `arr(1 To 3) As Long`, whose declared
            // type is `FixedArray { element, bounds }`, not `Array`). `array_element`
            // unwraps both array spellings; a non-array target stays Variant.
            let element_type =
                types::array_element(&ty).unwrap_or(oxvba_bundle::ArrayElementType::Variant);
            out.push(CoreStmt::Erase {
                array,
                element_type,
            });
        }
        Ok(out)
    }

    // ── File I/O (cross-bundle `VBA.FileSystem` member calls) ────────────────
    //
    // The funny-syntax file statements keep their special PARSING (dedicated CST
    // nodes) and special ARG-SHAPING (the packed `Open` mode, the `Put`/`Get` record
    // tuples, the `Print #` item lists, …); only the CALLEE changed — from the
    // bespoke `CoreCallee::Native(File*)` route to a cross-bundle `ExternProc` call
    // into the synthetic `VBA` bundle's `FileSystem` module (a native-bodied proc).
    // The VM's `call_extern` shortcuts the `NativeBody::Library` body straight to the
    // same `oxvba-lib` function the `Native` route used (positional ByVal args via
    // `extern_native_args`), so behaviour is unchanged.

    /// Build a cross-bundle call into the synthetic `VBA` bundle for a migrated
    /// native-bodied library member `id` — interning a `ModuleFunc` import (linked to
    /// the bundle's export by the shared `(module, member)` location) and emitting a
    /// `CoreCallee::ExternProc` call carrying the caller-shaped `args`. The location is
    /// the parser-bound file statement's `library_statement_member` (`FileSystem.Open`/
    /// `Print`/`Put`/`Get`/…) when present, else the by-name `library_member`
    /// (`Strings.Len`, or `FileSystem.Seek` for the `Seek #n, pos` statement which
    /// reuses the function form's member). This is the statement-side equivalent of the
    /// `DispatchRoute::ExternMember` lowering in `call.rs` (which the by-name functions
    /// already use); the VM's `call_extern` shortcuts the `NativeBody::Library` body to
    /// the same `oxvba-lib` function the bespoke `Native` route invoked, so behaviour
    /// is unchanged — only the callee differs.
    fn vba_library_call(&self, id: NativeImplId, args: Vec<CoreArg>) -> CoreValue {
        let (module, member) = id
            .library_statement_member()
            .or_else(|| id.library_member())
            .expect("a migrated library id has a VBA bundle member");
        let import = self.g.intern_import(BundleImport {
            unit: "VBA".to_string(),
            token: ExportToken::ModuleFunc {
                module: module.to_string(),
                member: member.to_string(),
                kind: ProjectMemberKind::Method,
            },
        });
        CoreValue::Call {
            callee: CoreCallee::ExternProc { import },
            args,
        }
    }

    fn bind_file_io(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let id = match node.kind() {
            SyntaxKind::OpenStmt => NativeImplId::FileOpen,
            SyntaxKind::CloseStmt => NativeImplId::FileClose,
            SyntaxKind::PutStmt => NativeImplId::FilePut,
            SyntaxKind::SeekStmt => NativeImplId::FileSeek,
            SyntaxKind::WidthStmt => NativeImplId::FileWidth,
            SyntaxKind::NameStmt => NativeImplId::FileRename,
            // `Lock` and `Unlock` share `LockStmt`; the `Lock` keyword distinguishes.
            SyntaxKind::LockStmt => {
                if node
                    .child_tokens()
                    .iter()
                    .any(|t| t.kind == SyntaxKind::KwLock)
                {
                    NativeImplId::FileLock
                } else {
                    NativeImplId::FileUnlock
                }
            }
            other => return Err(BindError::Unsupported(format!("file I/O {other:?}"))),
        };
        let mut args = Vec::new();
        if let Some(fnum) = node.file_number()
            && let Some(ch) = fnum.first_expr_child()
        {
            args.push(CoreArg::ByVal(self.bind_expr(ch)?.value));
        }
        for e in node.expr_children() {
            args.push(CoreArg::ByVal(self.bind_expr(e)?.value));
        }
        // Bare `Close` / `Reset` (no file number) close ALL open files — the handle-0
        // convention `close_variant` understands. Without this they reached `FileClose`
        // with no handle and raised a spurious "argument not optional" (Err 5).
        if id == NativeImplId::FileClose && args.is_empty() {
            args.push(CoreArg::ByVal(CoreValue::Const(CoreConst::I32(0))));
        }
        Ok(vec![CoreStmt::Eval(self.vba_library_call(id, args))])
    }

    fn bind_print_clause_arg(
        &mut self,
        name: &str,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<CoreValue, BindError> {
        let mut items = arglist.map(|a| a.arg_items()).unwrap_or_default();
        if items.len() != 1 {
            return Err(BindError::Malformed(format!(
                "{name} requires one argument"
            )));
        }
        match items.remove(0) {
            ArgItem::Positional(expr, _) | ArgItem::Named { value: expr, .. } => {
                Ok(self.bind_expr(expr)?.value)
            }
            ArgItem::Omitted => Err(BindError::Malformed(format!("{name} argument omitted"))),
        }
    }

    fn bind_print_clause_control(
        &mut self,
        expr: SyntaxNode<'_>,
    ) -> Result<Option<(char, CoreValue)>, BindError> {
        if expr.kind() == SyntaxKind::IdentExpr
            && let Some(name) = expr
                .ident_name_token()
                .map(|token| fold_identifier(token.text))
            && name == "tab"
        {
            return Ok(Some(('z', CoreValue::Const(CoreConst::I32(0)))));
        }
        if expr.kind() != SyntaxKind::IndexExpr {
            return Ok(None);
        }
        let Some(base) = expr.index_base() else {
            return Ok(None);
        };
        if base.kind() != SyntaxKind::IdentExpr {
            return Ok(None);
        }
        let Some(name) = base
            .ident_name_token()
            .map(|token| fold_identifier(token.text))
        else {
            return Ok(None);
        };
        match name.as_str() {
            "spc" => Ok(Some((
                's',
                self.bind_print_clause_arg("Spc", expr.index_arg_list())?,
            ))),
            "tab" => {
                let items = expr
                    .index_arg_list()
                    .map(|arglist| arglist.arg_items())
                    .unwrap_or_default();
                if items.is_empty() {
                    return Ok(Some(('z', CoreValue::Const(CoreConst::I32(0)))));
                }
                Ok(Some((
                    't',
                    self.bind_print_clause_arg("Tab", expr.index_arg_list())?,
                )))
            }
            _ => Ok(None),
        }
    }

    /// `Print #n, items…` / `Write #n, items…`. The item list's `,`/`;` separators
    /// are significant (zones vs. adjacency for `Print`; trailing-separator newline
    /// suppression for both), so we walk the `PrintItemList` in source order and pack
    /// a per-item separator spec into `args[1]`: one char per item — the separator
    /// that *follows* it (`,`, `;`, or `n` for none). `Print #` additionally carries
    /// `args[2]` as a per-item kind spec (`v` value, `s` Spc, `t` Tab), followed by
    /// item values; `Write #` keeps `args[2..]` as field values.
    fn bind_print_write(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let id = match node.kind() {
            SyntaxKind::PrintStmt => NativeImplId::FilePrint,
            SyntaxKind::WriteStmt => NativeImplId::FileWrite,
            other => return Err(BindError::Unsupported(format!("print/write {other:?}"))),
        };
        let handle = match node.file_number().and_then(|f| f.first_expr_child()) {
            Some(ch) => self.bind_expr(ch)?.value,
            None => {
                return Err(BindError::Malformed(
                    "Print/Write without a file number".into(),
                ));
            }
        };
        // Default each field's following separator to 'n' (none → terminate the line);
        // a `,`/`;` token in the list overwrites the preceding field's slot.
        let mut fields: Vec<CoreValue> = Vec::new();
        let mut seps: Vec<char> = Vec::new();
        let mut kinds: Vec<char> = Vec::new();
        if let Some(list) = node.child_node(SyntaxKind::PrintItemList) {
            for element in list.children() {
                match element {
                    SyntaxElement::Node(item) if item.kind() == SyntaxKind::PrintItem => {
                        if let Some(e) = item.first_expr_child() {
                            if id == NativeImplId::FilePrint
                                && let Some((kind, value)) = self.bind_print_clause_control(e)?
                            {
                                fields.push(value);
                                kinds.push(kind);
                                seps.push('n');
                                continue;
                            }
                            fields.push(self.bind_expr(e)?.value);
                            kinds.push('v');
                            seps.push('n');
                        }
                    }
                    SyntaxElement::Token(tok) if tok.kind == SyntaxKind::Comma => {
                        if let Some(last) = seps.last_mut() {
                            *last = ',';
                        }
                    }
                    SyntaxElement::Token(tok) if tok.kind == SyntaxKind::Semicolon => {
                        if let Some(last) = seps.last_mut() {
                            *last = ';';
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut args = vec![
            CoreArg::ByVal(handle),
            CoreArg::ByVal(CoreValue::Const(CoreConst::Str(seps.into_iter().collect()))),
        ];
        if id == NativeImplId::FilePrint {
            args.push(CoreArg::ByVal(CoreValue::Const(CoreConst::Str(
                kinds.into_iter().collect(),
            ))));
        }
        args.extend(fields.into_iter().map(CoreArg::ByVal));
        Ok(vec![CoreStmt::Eval(self.vba_library_call(id, args))])
    }

    /// `Input #n, var0, var1, …` reads one delimited field per target, so each target
    /// is its own `var = FileInput(handle, 1)` assignment (the read value coerced to
    /// the target type). Reading one field per call advances the file position
    /// sequentially. Previously the targets were passed as discarded ByVal args, so
    /// nothing was ever written back.
    fn bind_input(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let handle = match node.file_number().and_then(|f| f.first_expr_child()) {
            Some(ch) => self.bind_expr(ch)?.value,
            None => return Err(BindError::Malformed("Input # without a file number".into())),
        };
        let mut stmts = Vec::new();
        for target_node in node.expr_children() {
            let (place, target_ty) = self.bind_place(target_node)?;
            // Read exactly one field per target.
            let read = self.vba_library_call(
                NativeImplId::FileInput,
                vec![
                    CoreArg::ByVal(handle.clone()),
                    CoreArg::ByVal(CoreValue::Const(CoreConst::I32(1))),
                ],
            );
            let value = types::coerce(read, &VarTypeRef::Variant, &target_ty);
            stmts.push(CoreStmt::Assign {
                place,
                value,
                intent: AssignmentIntent::Let,
                target_kind: types::assignment_target_kind(&target_ty),
                target_name: target_node.text().trim().to_string(),
                target_type_name: types::type_name(&target_ty),
            });
        }
        Ok(stmts)
    }

    /// `Line Input #n, strvar` reads a whole line into `strvar`, so it lowers as
    /// `strvar = FileLineInput(handle)` (mirroring `Get`). Previously the target was a
    /// discarded ByVal arg, so the line was never written back.
    fn bind_line_input(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let handle = match node.file_number().and_then(|f| f.first_expr_child()) {
            Some(ch) => self.bind_expr(ch)?.value,
            None => {
                return Err(BindError::Malformed(
                    "Line Input # without a file number".into(),
                ));
            }
        };
        let target_node = node
            .expr_children()
            .into_iter()
            .next()
            .ok_or_else(|| BindError::Malformed("Line Input # without a target".into()))?;
        let (place, target_ty) = self.bind_place(target_node)?;
        let read = self.vba_library_call(NativeImplId::FileLineInput, vec![CoreArg::ByVal(handle)]);
        let value = types::coerce(read, &VarTypeRef::Variant, &target_ty);
        Ok(vec![CoreStmt::Assign {
            place,
            value,
            intent: AssignmentIntent::Let,
            target_kind: types::assignment_target_kind(&target_ty),
            target_name: target_node.text().trim().to_string(),
            target_type_name: types::type_name(&target_ty),
        }])
    }

    /// `Open path For <mode> As #n [Len = reclen]` lowers to
    /// `FileOpen(path, (filenum << 16) | mode_code, reclen)`: the file number is
    /// packed into the mode word (the open contract), and the record length rides
    /// the third arg for Random-mode positioning. Omitting `For <mode>` defaults to
    /// Random, matching VBA.
    fn bind_open(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let exprs = node.expr_children(); // [path, len?]
        let path_node = exprs
            .first()
            .copied()
            .ok_or_else(|| BindError::Malformed("Open without a path".into()))?;
        let path = self.bind_expr(path_node)?.value;
        let mode_code = node
            .child_tokens()
            .iter()
            .find_map(|t| match t.kind {
                SyntaxKind::KwInput => Some(0),
                SyntaxKind::KwOutput => Some(1),
                SyntaxKind::KwAppend => Some(2),
                SyntaxKind::KwBinary => Some(3),
                SyntaxKind::KwRandom => Some(4),
                _ => None,
            })
            .unwrap_or(4);
        let fnum_node = node
            .file_number()
            .and_then(|f| f.first_expr_child())
            .ok_or_else(|| BindError::Malformed("Open without a file number".into()))?;
        let filenum = self.bind_expr(fnum_node)?.value;
        // packed = (filenum * 65536) + mode_code  (== (filenum << 16) | mode_code)
        let packed = CoreValue::Binary {
            op: CoreBinOp::Add,
            lhs: Box::new(CoreValue::Binary {
                op: CoreBinOp::Mul,
                lhs: Box::new(filenum),
                rhs: Box::new(CoreValue::Const(CoreConst::I32(65536))),
                mode: self.info.compare_mode,
                num: NumericMode::Widening,
            }),
            rhs: Box::new(CoreValue::Const(CoreConst::I32(mode_code))),
            mode: self.info.compare_mode,
            num: NumericMode::Widening,
        };
        let record_len = match exprs.get(1) {
            Some(e) => self.bind_expr(*e)?.value,
            None => CoreValue::Const(CoreConst::I32(0)),
        };
        let args = vec![
            CoreArg::ByVal(path),
            CoreArg::ByVal(packed),
            CoreArg::ByVal(record_len),
        ];
        Ok(vec![CoreStmt::Eval(
            self.vba_library_call(NativeImplId::FileOpen, args),
        )])
    }

    /// `Get #n, [rec], var` reads a record into `var`, so it lowers as
    /// `var = FileGetInto(handle, [rec])` (the read value coerced to the target).
    fn bind_get(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let handle = match node.file_number().and_then(|f| f.first_expr_child()) {
            Some(ch) => self.bind_expr(ch)?.value,
            None => return Err(BindError::Malformed("Get without a file number".into())),
        };
        // The expression children are `[record-number?, target]`; the last is the
        // target l-value and any preceding one is the record number.
        let exprs = node.expr_children();
        let (target_node, rec_nodes) = exprs
            .split_last()
            .ok_or_else(|| BindError::Malformed("Get without a target".into()))?;
        let rec = match rec_nodes.first() {
            Some(e) => self.bind_expr(*e)?.value,
            None => CoreValue::Const(CoreConst::Empty),
        };
        let (place, target_ty) = self.bind_place(*target_node)?;
        // The read native needs the target's VBA type code to fix the record size.
        let type_code = CoreValue::Const(CoreConst::I32(record_type_code(&target_ty)));
        // String-length spec: a fixed-length string passes its length N (read N
        // bytes, no prefix); a variable string passes -(Len(target)) so a Binary
        // read knows the byte count (Random uses the on-disk 2-byte prefix).
        let str_len = match &target_ty {
            oxvba_symbol::signature::VarTypeRef::FixedString(n) => {
                CoreValue::Const(CoreConst::I32(*n as i32))
            }
            oxvba_symbol::signature::VarTypeRef::Builtin(
                oxvba_symbol::signature::BuiltinType::String,
            ) => {
                let target_value = self.bind_expr(*target_node)?.value;
                // `Len` is a by-name `VBA.Strings` member; route it cross-bundle too
                // so no file-statement path uses `CoreCallee::Native` (only the
                // out-of-scope `Debug.Print` diagnostics statement still does).
                let len =
                    self.vba_library_call(NativeImplId::Len, vec![CoreArg::ByVal(target_value)]);
                CoreValue::Unary {
                    op: oxvba_bundle::coreir::CoreUnOp::Negate,
                    expr: Box::new(len),
                    num: NumericMode::Widening,
                }
            }
            _ => CoreValue::Const(CoreConst::I32(0)),
        };
        let args = vec![
            CoreArg::ByVal(handle),
            CoreArg::ByVal(rec),
            CoreArg::ByVal(type_code),
            CoreArg::ByVal(str_len),
        ];
        // `Get #n, [rec], var` reads INTO `var`; the read value is the call result and
        // the write-back is the wrapping `CoreStmt::Assign` below (NOT a ByRef arg), so
        // routing the read through `ExternProc` preserves the write-back exactly — the
        // target slot is the assignment's place, untouched by the callee change.
        let read = self.vba_library_call(NativeImplId::FileGetInto, args);
        let value = types::coerce(
            read,
            &oxvba_symbol::signature::VarTypeRef::Variant,
            &target_ty,
        );
        Ok(vec![CoreStmt::Assign {
            place,
            value,
            intent: AssignmentIntent::Let,
            target_kind: types::assignment_target_kind(&target_ty),
            target_name: target_node.text().trim().to_string(),
            target_type_name: types::type_name(&target_ty),
        }])
    }

    /// `Put #n, [rec], value` writes a record. A fixed-length-string source is
    /// flagged so the record is written raw (no length prefix).
    fn bind_put(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let handle = match node.file_number().and_then(|f| f.first_expr_child()) {
            Some(ch) => self.bind_expr(ch)?.value,
            None => return Err(BindError::Malformed("Put without a file number".into())),
        };
        // Expression children are `[record-number?, value]`; the last is the value.
        let exprs = node.expr_children();
        let (value_node, rec_nodes) = exprs
            .split_last()
            .ok_or_else(|| BindError::Malformed("Put without a value".into()))?;
        let rec = match rec_nodes.first() {
            Some(e) => self.bind_expr(*e)?.value,
            None => CoreValue::Const(CoreConst::Empty),
        };
        let value = self.bind_expr(*value_node)?;
        let fixed = i32::from(matches!(
            value.ty,
            oxvba_symbol::signature::VarTypeRef::FixedString(_)
        ));
        let args = vec![
            CoreArg::ByVal(handle),
            CoreArg::ByVal(rec),
            CoreArg::ByVal(value.value),
            CoreArg::ByVal(CoreValue::Const(CoreConst::I32(fixed))),
        ];
        Ok(vec![CoreStmt::Eval(
            self.vba_library_call(NativeImplId::FilePut, args),
        )])
    }

    // ── Small helpers ───────────────────────────────────────
}

fn normalize_identifier_token(text: &str) -> &str {
    text.strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(text)
}

/// The VBA `VarType` discriminant for a target type, so `Get` knows the fixed
/// record size to read. Unknown/untyped targets fall back to `Variant`.
fn record_type_code(ty: &oxvba_symbol::signature::VarTypeRef) -> i32 {
    use oxvba_runtime::VarType as Vt;
    use oxvba_symbol::signature::{BuiltinType as B, VarTypeRef};
    let vt = match ty {
        VarTypeRef::Builtin(B::Byte) => Vt::Byte,
        VarTypeRef::Builtin(B::Integer) => Vt::Integer,
        VarTypeRef::Builtin(B::Boolean) => Vt::Boolean,
        VarTypeRef::Builtin(B::Long) => Vt::Long,
        VarTypeRef::Builtin(B::LongLong) | VarTypeRef::Builtin(B::LongPtr) => Vt::LongLong,
        VarTypeRef::Builtin(B::Single) => Vt::Single,
        VarTypeRef::Builtin(B::Double) => Vt::Double,
        VarTypeRef::Builtin(B::Currency) => Vt::Currency,
        VarTypeRef::Builtin(B::Date) => Vt::Date,
        VarTypeRef::Builtin(B::String) | VarTypeRef::FixedString(_) => Vt::String,
        _ => Vt::Empty,
    };
    vt as i32
}

fn default_value_for_type(
    ty: &VarTypeRef,
) -> Option<(
    CoreValue,
    AssignmentIntent,
    oxvba_bundle::AssignmentTargetKind,
    String,
)> {
    let value = match ty {
        VarTypeRef::Builtin(BuiltinType::String) => CoreValue::Const(CoreConst::Str(String::new())),
        VarTypeRef::FixedString(_) => {
            types::coerce_store(CoreValue::Const(CoreConst::Str(String::new())), ty)
        }
        VarTypeRef::Builtin(BuiltinType::Boolean) => CoreValue::Const(CoreConst::Bool(false)),
        VarTypeRef::Builtin(
            BuiltinType::Byte
            | BuiltinType::Integer
            | BuiltinType::Long
            | BuiltinType::LongLong
            | BuiltinType::LongPtr
            | BuiltinType::Single
            | BuiltinType::Double
            | BuiltinType::Currency
            | BuiltinType::Date,
        ) => types::coerce_store(CoreValue::Const(CoreConst::I32(0)), ty),
        VarTypeRef::Object(_) => CoreValue::Const(CoreConst::Nothing),
        VarTypeRef::Variant
        | VarTypeRef::Array(_)
        | VarTypeRef::FixedArray { .. }
        | VarTypeRef::Udt(_) => return None,
    };
    let intent = if matches!(ty, VarTypeRef::Object(_)) {
        AssignmentIntent::Set
    } else {
        AssignmentIntent::Let
    };
    Some((
        value,
        intent,
        types::assignment_target_kind(ty),
        types::type_name(ty),
    ))
}

impl<'a> ProcLower<'a> {
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

    fn label_ref_id(
        &mut self,
        node: SyntaxNode<'_>,
    ) -> Result<oxvba_bundle::coreir::LabelId, BindError> {
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
        self.place_ty_by_name(name).map(|(p, _)| p)
    }

    /// Resolve a bare name to a writable place and its declared type.
    pub(crate) fn place_ty_by_name(
        &mut self,
        name: &str,
    ) -> Result<(CorePlace, oxvba_symbol::signature::VarTypeRef), BindError> {
        if let Some(rl) = self.return_target(name) {
            return Ok((CorePlace::Local(rl), self.info.return_type.clone()));
        }
        let Some(binding) = self.resolve(name) else {
            return self.implicit_local(name);
        };
        binding
            .symbol
            .and_then(|s| self.place_for_symbol(s))
            .ok_or_else(|| BindError::InvalidAssignment(format!("`{name}` is not a variable")))
    }

    /// Fold a value to a constant `i32` if possible (literals, unary negation,
    /// and integer arithmetic of constants, e.g. `vbObjectError + 1` once
    /// `vbObjectError` resolves to a library constant).
    fn fold_const_i32(&self, value: &CoreValue) -> Option<i32> {
        match value {
            CoreValue::Const(CoreConst::I16(n)) => Some(i32::from(*n)),
            CoreValue::Const(CoreConst::I32(n)) => Some(*n),
            CoreValue::Const(CoreConst::I64(n)) => i32::try_from(*n).ok(),
            CoreValue::Unary {
                op: CoreUnOp::Negate,
                expr,
                ..
            } => self.fold_const_i32(expr)?.checked_neg(),
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

    fn err_raise_arg(&mut self, arglist: Option<SyntaxNode<'_>>) -> Result<ErrorOp, BindError> {
        // `Err.Raise Number, [Source], [Description], [HelpFile], [HelpContext]` —
        // resolve each argument positionally or by name.
        const PARAMS: [&str; 5] = ["number", "source", "description", "helpfile", "helpcontext"];
        let arglist = arglist.ok_or_else(|| BindError::Malformed("Err.Raise number".into()))?;
        let mut slots: [Option<SyntaxNode<'_>>; 5] = Default::default();
        let mut pos = 0usize;
        for item in arglist.arg_items() {
            match item {
                ArgItem::Positional(e, _) => {
                    let idx = pos;
                    pos += 1;
                    if idx >= PARAMS.len() {
                        return Err(BindError::Malformed("Err.Raise: too many arguments".into()));
                    }
                    slots[idx] = Some(e);
                }
                ArgItem::Named { name, value, .. } => {
                    let folded = fold_identifier(name.text);
                    let idx = PARAMS.iter().position(|p| *p == folded).ok_or_else(|| {
                        BindError::Malformed(format!("Err.Raise: unknown argument `{}`", name.text))
                    })?;
                    slots[idx] = Some(value);
                }
                // `Err.Raise 5, , "desc"` — an omitted positional still advances the slot.
                ArgItem::Omitted => {
                    pos += 1;
                }
            }
        }
        let number_node =
            slots[0].ok_or_else(|| BindError::Malformed("Err.Raise number".into()))?;
        let number_val = self.bind_expr(number_node)?.value;
        // Fold a static number into a constant so vm2's linearizer keeps its immediate
        // `RaiseError` path; a dynamic number stays an operand.
        let number = match self.fold_const_i32(&number_val) {
            Some(code) => CoreValue::Const(CoreConst::I32(code)),
            None => number_val,
        };
        let source = slots[1]
            .map(|n| self.bind_expr(n).map(|b| Box::new(b.value)))
            .transpose()?;
        let description = slots[2]
            .map(|n| self.bind_expr(n).map(|b| Box::new(b.value)))
            .transpose()?;
        let help_file = slots[3]
            .map(|n| self.bind_expr(n).map(|b| Box::new(b.value)))
            .transpose()?;
        let help_context = slots[4]
            .map(|n| self.bind_expr(n).map(|b| Box::new(b.value)))
            .transpose()?;
        Ok(ErrorOp::Raise {
            number,
            source,
            description,
            help_file,
            help_context,
            inherit: true,
        })
    }

    /// The legacy `Error <number>` statement (MS-VBAL §5.4.4.3). It raises the given
    /// run-time error, reusing the `Err.Raise` machinery — but with `inherit: false`:
    /// `Error <n>` does NOT inherit an un-cleared `Err` (oracle-confirmed), so omitted
    /// Source/Description always take their defaults.
    fn bind_error_stmt(&mut self, node: SyntaxNode<'_>) -> Result<Vec<CoreStmt>, BindError> {
        let value = self.bind_required(node.first_expr_child(), "Error statement number")?;
        let number = match self.fold_const_i32(&value) {
            Some(code) => CoreValue::Const(CoreConst::I32(code)),
            None => value,
        };
        Ok(vec![CoreStmt::Error(ErrorOp::Raise {
            number,
            source: None,
            description: None,
            help_file: None,
            help_context: None,
            inherit: false,
        })])
    }
}

/// True if a resolved route is a property accessor (Get/Let/Set) — a project
/// member, an early-bound COM member, or a cross-project extern member. A
/// property write goes through a setter call, not a place store.
fn is_property_route(route: &DispatchRoute) -> bool {
    matches!(
        route,
        DispatchRoute::ProjectMember {
            kind: ProjectMemberKind::PropertyGet
                | ProjectMemberKind::PropertyLet
                | ProjectMemberKind::PropertySet
        } | DispatchRoute::ComMember {
            member_kind: ProjectMemberKind::PropertyGet
                | ProjectMemberKind::PropertyLet
                | ProjectMemberKind::PropertySet,
            ..
        } | DispatchRoute::ExternMember {
            kind: ProjectMemberKind::PropertyGet
                | ProjectMemberKind::PropertyLet
                | ProjectMemberKind::PropertySet,
            ..
        }
    )
}

fn type_block_name(node: SyntaxNode<'_>) -> Option<&str> {
    let mut saw_type = false;
    for token in node.child_tokens() {
        if token.kind == SyntaxKind::KwType {
            saw_type = true;
            continue;
        }
        if saw_type
            && (matches!(token.kind, SyntaxKind::Ident | SyntaxKind::BracketedIdent)
                || token.kind.is_keyword())
        {
            return Some(token.text);
        }
    }
    None
}

fn lset_record_layout_is_byte_copyable(layout: &ArrayElementType) -> bool {
    match layout {
        ArrayElementType::Record(fields) => fields.iter().all(lset_record_field_is_byte_copyable),
        _ => false,
    }
}

fn lset_record_field_is_byte_copyable(field: &ArrayElementType) -> bool {
    match field {
        ArrayElementType::Variant | ArrayElementType::String => false,
        ArrayElementType::Record(fields) => fields.iter().all(lset_record_field_is_byte_copyable),
        ArrayElementType::FixedArray { element, .. } => lset_record_field_is_byte_copyable(element),
        ArrayElementType::Integer
        | ArrayElementType::Long
        | ArrayElementType::LongLong
        | ArrayElementType::LongPtr
        | ArrayElementType::Byte
        | ArrayElementType::Single
        | ArrayElementType::Double
        | ArrayElementType::Currency
        | ArrayElementType::Date
        | ArrayElementType::FixedString(_)
        | ArrayElementType::Boolean => true,
    }
}
