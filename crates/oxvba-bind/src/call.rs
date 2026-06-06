//! The call binder: maps a resolved `DispatchRoute` to a `CoreCallee` (project
//! proc / native / `Declare` / special form), binds arguments (positional /
//! named / omitted, ByVal coercion, ByRef aliasing), and lowers member access
//! (the `Err` object for now; objects/COM arrive in a later phase).

use oxvba_bundle::coreir::{BoundWhich, CoreArg, CoreCallee, CorePlace, CoreValue, ErrField};
use oxvba_bundle::native::NativeImplId;
use oxvba_bundle::ProjectMemberKind;
use oxvba_symbol::binding::{Binding, DispatchRoute, SpecialForm};
use oxvba_symbol::model::{fold_identifier, LibraryConstValue, PredeclaredObjectId, SymbolId, SymbolImpl};
use oxvba_symbol::signature::{BuiltinType, Param, PassingMode, Signature, VarTypeRef};
use oxvba_syntax::red::ArgItem;
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::error::BindError;
use crate::expr::{builtin, value_bound};
use crate::types;
use crate::{Bound, ProcLower};

impl<'a> ProcLower<'a> {
    /// Lower a resolved name + optional argument list to a value (a call, a
    /// constant, or a special form).
    pub(crate) fn bind_call_route(
        &mut self,
        name: &str,
        binding: &Binding,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        match &binding.route {
            DispatchRoute::LibraryConst(value) => Ok(library_const(value)),
            DispatchRoute::ProjectMember { kind } => {
                self.bind_project_call(name, binding, *kind, arglist)
            }
            DispatchRoute::Native(id) => {
                let args = self.bind_args(arglist, None)?;
                Ok(value_bound(
                    CoreValue::Call { callee: CoreCallee::Native(*id), args },
                    VarTypeRef::Variant,
                ))
            }
            DispatchRoute::Declare { descriptor_id } => {
                // ByRef `Declare` params write back to the caller slot (copy-out).
                let by_ref = self.declare_param_by_ref(binding.symbol);
                let args = self.bind_args_byref(arglist, &by_ref)?;
                Ok(value_bound(
                    CoreValue::Call { callee: CoreCallee::Declare { descriptor_id: *descriptor_id }, args },
                    VarTypeRef::Variant,
                ))
            }
            DispatchRoute::SpecialForm(SpecialForm::Array) => {
                let items = match arglist {
                    Some(a) => self.bind_positional_values(a)?,
                    None => Vec::new(),
                };
                Ok(value_bound(CoreValue::ArrayLiteral(items), VarTypeRef::Variant))
            }
            // `IIf`/`Choose`/`Switch` are eager VBA library functions (every
            // argument is evaluated before the call) — lower them as native
            // calls, not as lazy/short-circuit forms.
            DispatchRoute::SpecialForm(sf @ (SpecialForm::IIf | SpecialForm::Choose | SpecialForm::Switch)) => {
                let id = match sf {
                    SpecialForm::IIf => NativeImplId::IIf,
                    SpecialForm::Choose => NativeImplId::Choose,
                    SpecialForm::Switch => NativeImplId::Switch,
                    _ => unreachable!(),
                };
                let args = self.bind_args(arglist, None)?;
                Ok(value_bound(
                    CoreValue::Call { callee: CoreCallee::Native(id), args },
                    VarTypeRef::Variant,
                ))
            }
            // `UBound`/`LBound` take an array l-value (single dimension; any 2nd
            // dimension argument is ignored — multi-dim arrays are not modeled).
            DispatchRoute::SpecialForm(sf @ (SpecialForm::UBound | SpecialForm::LBound)) => {
                let which = if matches!(sf, SpecialForm::UBound) {
                    BoundWhich::Upper
                } else {
                    BoundWhich::Lower
                };
                let first = arglist
                    .and_then(|a| a.arg_items().into_iter().next())
                    .ok_or_else(|| BindError::Malformed(format!("`{name}` requires an array argument")))?;
                let expr = match first {
                    ArgItem::Positional(e) => e,
                    _ => return Err(BindError::Malformed(format!("`{name}` array argument"))),
                };
                let (place, _) = self.bind_place(expr)?;
                Ok(value_bound(
                    CoreValue::Bound { which, array: Box::new(place) },
                    builtin(BuiltinType::Long),
                ))
            }
            DispatchRoute::SpecialForm(SpecialForm::CallByName) => self.bind_callbyname(arglist),
            DispatchRoute::ErrMember(_) => {
                Err(BindError::Unsupported(format!("`{name}` Err member in value context")))
            }
            other => Err(BindError::Unsupported(format!("call route {other:?} for `{name}`"))),
        }
    }

    fn bind_project_call(
        &mut self,
        name: &str,
        binding: &Binding,
        kind: ProjectMemberKind,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        let sym = binding
            .symbol
            .ok_or_else(|| self.unresolved(name, "project member"))?;
        let proc_id = match kind {
            ProjectMemberKind::Method => self.g.ids.proc_of.get(&sym).copied(),
            _ => self.g.ids.prop_accessor_of.get(&(sym, kind)).copied(),
        }
        .ok_or_else(|| self.unresolved(name, "project proc"))?;

        let signature = self.proc_signature_for(sym, kind);
        let args = match &signature {
            Some(sig) => self.bind_proc_args(arglist, sig)?,
            None => self.bind_args(arglist, None)?,
        };
        let ty = signature
            .and_then(|s| s.return_type)
            .unwrap_or(VarTypeRef::Variant);
        Ok(value_bound(
            CoreValue::Call { callee: CoreCallee::VbaProc { proc: proc_id }, args },
            ty,
        ))
    }

    /// Bind one positional argument against an optional parameter: ByRef aliasing
    /// of an l-value when the parameter is ByRef, else ByVal with coercion to the
    /// parameter type. A parenthesized argument `(x)` is forced ByVal (VBA forces
    /// pass-by-value when an argument is wrapped in parentheses).
    fn bind_one_arg(
        &mut self,
        expr: SyntaxNode<'_>,
        param: Option<&Param>,
    ) -> Result<CoreArg, BindError> {
        let by_ref = param.map(|p| p.mode == PassingMode::ByRef).unwrap_or(false);
        let forced_by_val = expr.kind() == SyntaxKind::ParenExpr;
        if by_ref && !forced_by_val
            && let Ok((place, _)) = self.bind_place(expr) {
                return Ok(CoreArg::ByRef(place));
            }
        let bound = self.bind_expr(expr)?;
        let value = match param {
            Some(p) if p.mode == PassingMode::ByVal => types::coerce(bound.value, &bound.ty, &p.ty),
            _ => bound.value,
        };
        Ok(CoreArg::ByVal(value))
    }

    /// Bind one argument given an explicit by-ref flag (COM/`Declare`, where the
    /// direction comes from a typelib/`Declare` param list, not a project
    /// `Signature`). ByRef l-value (not parenthesised) → `ByRef`; else `ByVal`
    /// (no coercion — the value is marshaled by the callee).
    fn bind_arg_byref(&mut self, expr: SyntaxNode<'_>, by_ref: bool) -> Result<CoreArg, BindError> {
        if by_ref
            && expr.kind() != SyntaxKind::ParenExpr
            && let Ok((place, _)) = self.bind_place(expr)
        {
            return Ok(CoreArg::ByRef(place));
        }
        Ok(CoreArg::ByVal(self.bind_expr(expr)?.value))
    }

    /// Arguments for a COM / `Declare` callee whose per-parameter by-ref directions
    /// are known as a `Vec<bool>` (positional). Named args stay `ByVal`.
    pub(crate) fn bind_args_byref(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        param_by_ref: &[bool],
    ) -> Result<Vec<CoreArg>, BindError> {
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        let mut args = Vec::with_capacity(items.len());
        for (i, item) in items.into_iter().enumerate() {
            match item {
                ArgItem::Omitted => args.push(CoreArg::Omitted),
                ArgItem::Named { name, value } => {
                    args.push(CoreArg::Named { name: name.text.to_string(), value: self.bind_expr(value)?.value });
                }
                ArgItem::Positional(expr) => {
                    args.push(self.bind_arg_byref(expr, param_by_ref.get(i).copied().unwrap_or(false))?);
                }
            }
        }
        Ok(args)
    }

    /// Arguments for native / late-bound / `Declare` callees: positional in order,
    /// named preserved verbatim (the callee resolves names itself), omitted kept.
    pub(crate) fn bind_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        signature: Option<&Signature>,
    ) -> Result<Vec<CoreArg>, BindError> {
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        let mut args = Vec::with_capacity(items.len());
        for (i, item) in items.into_iter().enumerate() {
            match item {
                ArgItem::Omitted => args.push(CoreArg::Omitted),
                ArgItem::Named { name, value } => {
                    let v = self.bind_expr(value)?.value;
                    args.push(CoreArg::Named { name: name.text.to_string(), value: v });
                }
                ArgItem::Positional(expr) => {
                    args.push(self.bind_one_arg(expr, signature.and_then(|s| s.params.get(i)))?);
                }
            }
        }
        Ok(args)
    }

    /// Arguments for a project `VbaProc`: named args are reordered into their
    /// positional slots by parameter name (linearize binds VbaProc args strictly
    /// positionally), with unfilled slots left `Omitted`.
    fn bind_proc_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        signature: &Signature,
    ) -> Result<Vec<CoreArg>, BindError> {
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        let n = signature.params.len();
        // The fixed parameters precede a trailing `ParamArray` (if any). Positional
        // args at/after that index form the variadic tail, which linearize boxes
        // into one array for the ParamArray slot; the ParamArray's own fixed slot is
        // not emitted (so `args` is `fixed… ++ tail`).
        let variadic_index = signature.params.iter().position(|p| p.param_array);
        let fixed_count = variadic_index.unwrap_or(n);
        let mut slots: Vec<Option<CoreArg>> = (0..fixed_count).map(|_| None).collect();
        let mut tail: Vec<CoreArg> = Vec::new();
        let mut pos = 0usize;
        for item in items {
            match item {
                ArgItem::Positional(expr) => {
                    if pos < fixed_count {
                        slots[pos] = Some(self.bind_one_arg(expr, signature.params.get(pos))?);
                    } else {
                        // Variadic-tail (ParamArray) element, or an extra positional
                        // when there is no ParamArray — bound ByVal, no signature param.
                        tail.push(self.bind_one_arg(expr, None)?);
                    }
                    pos += 1;
                }
                ArgItem::Omitted => {
                    if pos < fixed_count {
                        slots[pos] = Some(CoreArg::Omitted);
                    } else {
                        tail.push(CoreArg::Omitted);
                    }
                    pos += 1;
                }
                ArgItem::Named { name, value } => {
                    let folded = fold_identifier(name.text);
                    match signature.params.iter().position(|p| fold_identifier(&p.name) == folded) {
                        Some(i) if variadic_index == Some(i) => {
                            return Err(BindError::Unsupported(
                                "named argument to a ParamArray parameter".into(),
                            ));
                        }
                        Some(i) if i < fixed_count => {
                            slots[i] = Some(self.bind_one_arg(value, signature.params.get(i))?);
                        }
                        Some(_) => {
                            return Err(BindError::Unsupported(
                                "named argument past the parameter list".into(),
                            ));
                        }
                        None => return Err(self.unresolved(name.text, "named argument")),
                    }
                }
            }
        }
        let mut args: Vec<CoreArg> = slots.into_iter().map(|s| s.unwrap_or(CoreArg::Omitted)).collect();
        match variadic_index {
            // Box the variadic tail into one fresh 0-based array for the ParamArray
            // slot (empty tail → an empty array, UBound -1). Doing this in the binder
            // — which has the signature — keeps the call vector one-arg-per-param, so
            // free procs and methods need no downstream variadic handling.
            Some(_) => {
                let elems: Vec<CoreValue> = tail.into_iter().map(paramarray_element).collect();
                args.push(CoreArg::ByVal(CoreValue::ArrayLiteral(elems)));
            }
            None => args.extend(tail),
        }
        Ok(args)
    }

    /// `CallByName(Object, ProcName$, CallType, [Args…])` — dispatch by a runtime
    /// member name. Object/ProcName/CallType are forwarded as the first three
    /// operands; the remaining arguments are passed by value (no static callee
    /// signature is known at the call site).
    fn bind_callbyname(&mut self, arglist: Option<SyntaxNode<'_>>) -> Result<Bound, BindError> {
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        if items.len() < 3 {
            return Err(BindError::Malformed(
                "CallByName requires Object, ProcName, and CallType".into(),
            ));
        }
        let mut args: Vec<CoreArg> = Vec::with_capacity(items.len());
        for (i, item) in items.into_iter().enumerate() {
            let arg = match item {
                ArgItem::Positional(expr) => CoreArg::ByVal(self.bind_expr(expr)?.value),
                ArgItem::Named { name, value } if i >= 3 => CoreArg::Named {
                    name: name.text.to_string(),
                    value: self.bind_expr(value)?.value,
                },
                ArgItem::Omitted if i >= 3 => CoreArg::Omitted,
                _ => {
                    return Err(BindError::Malformed(
                        "CallByName Object/ProcName/CallType must be positional".into(),
                    ));
                }
            };
            args.push(arg);
        }
        Ok(value_bound(
            CoreValue::Call { callee: CoreCallee::DynamicByName, args },
            VarTypeRef::Variant,
        ))
    }

    fn proc_signature_for(&self, sym: SymbolId, kind: ProjectMemberKind) -> Option<Signature> {
        let imp = &self.g.env.symbols.symbol(sym)?.imp;
        let sig_id = match (imp, kind) {
            (SymbolImpl::Signature(id), ProjectMemberKind::Method) => Some(*id),
            (SymbolImpl::Property(group), ProjectMemberKind::PropertyGet) => group.get,
            (SymbolImpl::Property(group), ProjectMemberKind::PropertyLet) => group.let_,
            (SymbolImpl::Property(group), ProjectMemberKind::PropertySet) => group.set,
            _ => None,
        }?;
        self.g.env.signatures.get(sig_id).cloned()
    }

    /// The per-parameter by-ref flags of a `Declare` symbol (empty if unknown).
    fn declare_param_by_ref(&self, sym: Option<SymbolId>) -> Vec<bool> {
        match sym.and_then(|s| self.g.env.symbols.symbol(s)).map(|s| &s.imp) {
            Some(SymbolImpl::Declare(d)) => d.param_by_ref.clone(),
            _ => Vec::new(),
        }
    }

    // ── Member access ───────────────────────────────────────

    /// The receiver of a `MemberExpr` as a bound value: an explicit `obj` (or
    /// `Me`), or the active `With` block's object for a leading-dot member.
    pub(crate) fn member_receiver_bound(
        &mut self,
        node: SyntaxNode<'_>,
    ) -> Result<Bound, BindError> {
        if node.member_has_leading_dot() {
            return self
                .with_stack
                .last()
                .cloned()
                .ok_or_else(|| BindError::Malformed("leading '.' outside a With block".into()));
        }
        let recv = node
            .member_receiver()
            .ok_or_else(|| BindError::Malformed("member without receiver".into()))?;
        self.bind_expr(recv)
    }

    /// A member read `recv.member` (no argument list): an instance field load, or
    /// a property-get / parameterless-method call (receiver passed as `args[0]`).
    pub(crate) fn bind_member(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let member = node
            .member_name_token()
            .ok_or_else(|| BindError::Malformed("member without name".into()))?
            .text;
        // `Err.Number` / `Err.Description` / `Err.Source` are error-state reads.
        if let Some(recv) = node.member_receiver()
            && self.is_err_receiver(recv)
                && let Some((field, ty)) = err_field(member) {
                    return Ok(value_bound(CoreValue::ErrField(field), ty));
                }
        let recv = self.member_receiver_bound(node)?;
        self.bind_member_value(recv, member)
    }

    /// Lower `recv.member` (already-bound receiver, no args) to a value.
    fn bind_member_value(&mut self, recv: Bound, member: &str) -> Result<Bound, BindError> {
        match self.resolve_member(&recv.ty, member, None) {
            Some(binding) => match &binding.route {
                DispatchRoute::Value => {
                    let sym = binding.symbol.ok_or_else(|| self.unresolved(member, "member field"))?;
                    let (place, ty) = self.member_place(recv.value, sym)?;
                    Ok(Bound { value: CoreValue::Load(place.clone()), ty, place: Some(place) })
                }
                DispatchRoute::ProjectMember { kind } => {
                    let kind = *kind;
                    let ty = self.member_return_type(binding.symbol, kind);
                    Ok(value_bound(self.late_member_call(member, kind, recv.value, Vec::new()), ty))
                }
                DispatchRoute::ComMember { dispid, member_kind, .. } => Ok(value_bound(
                    self.early_com_call(*dispid, *member_kind, recv.value, Vec::new()),
                    VarTypeRef::Variant,
                )),
                other => Err(BindError::Unsupported(format!(".{member} ({other:?} pending)"))),
            },
            // No declared member on an untyped/foreign receiver → late binding.
            None if self.is_late_bound_receiver(&recv.ty) => Ok(value_bound(
                self.late_member_call(member, ProjectMemberKind::Method, recv.value, Vec::new()),
                VarTypeRef::Variant,
            )),
            None => Err(self.unresolved(member, "member")),
        }
    }

    /// A member call `recv.member(args)` — a method/property call, or an index
    /// into a member array (`recv.arr(i)`), decided by resolving the member.
    pub(crate) fn bind_member_call(
        &mut self,
        member_node: SyntaxNode<'_>,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        let member = member_node
            .member_name_token()
            .ok_or_else(|| BindError::Malformed("member call without name".into()))?
            .text;
        let recv = self.member_receiver_bound(member_node)?;
        match self.resolve_member(&recv.ty, member, None) {
            Some(binding) => match &binding.route {
                DispatchRoute::ProjectMember { kind } => {
                    let kind = *kind;
                    // Use the method's signature so ByRef params alias the caller
                    // (and named args reorder into positional slots for dispatch).
                    let signature = binding.symbol.and_then(|s| self.proc_signature_for(s, kind));
                    let method_args = match &signature {
                        Some(sig) => self.bind_proc_args(arglist, sig)?,
                        None => self.bind_args(arglist, None)?,
                    };
                    let ty = signature.and_then(|s| s.return_type).unwrap_or(VarTypeRef::Variant);
                    Ok(value_bound(self.late_member_call(member, kind, recv.value, method_args), ty))
                }
                DispatchRoute::Value => {
                    // `recv.field(i)` — index into a member array.
                    let sym = binding.symbol.ok_or_else(|| self.unresolved(member, "member array"))?;
                    let (field_place, _ty) = self.member_place(recv.value, sym)?;
                    let indices = match arglist {
                        Some(a) => self.bind_positional_values(a)?,
                        None => Vec::new(),
                    };
                    let place = CorePlace::Index { array: Box::new(field_place), indices };
                    Ok(Bound { value: CoreValue::Load(place.clone()), ty: VarTypeRef::Variant, place: Some(place) })
                }
                DispatchRoute::ComMember { dispid, member_kind, param_by_ref, .. } => {
                    let (dispid, member_kind) = (*dispid, *member_kind);
                    // Emit ByRef for the typelib's [out]/[in,out] params.
                    let by_ref = param_by_ref.clone();
                    let method_args = self.bind_args_byref(arglist, &by_ref)?;
                    Ok(value_bound(
                        self.early_com_call(dispid, member_kind, recv.value, method_args),
                        VarTypeRef::Variant,
                    ))
                }
                other => Err(BindError::Unsupported(format!(".{member}(...) ({other:?} pending)"))),
            },
            None if self.is_late_bound_receiver(&recv.ty) => {
                let method_args = self.bind_args(arglist, None)?;
                Ok(value_bound(
                    self.late_member_call(member, ProjectMemberKind::Method, recv.value, method_args),
                    VarTypeRef::Variant,
                ))
            }
            None => Err(self.unresolved(member, "member call")),
        }
    }

    /// The `CorePlace` for an instance field / WithEvents field member symbol.
    pub(crate) fn member_place(
        &self,
        recv: CoreValue,
        sym: SymbolId,
    ) -> Result<(CorePlace, VarTypeRef), BindError> {
        if let Some(&field) = self.g.ids.field_token_of.get(&sym) {
            return Ok((CorePlace::Field { object: Box::new(recv), field }, self.symbol_type(sym)));
        }
        if let Some(&binding) = self.g.ids.withevents_binding_of.get(&sym) {
            return Ok((CorePlace::WithEvents { owner: Box::new(recv), binding }, self.symbol_type(sym)));
        }
        Err(BindError::Unsupported("member field without an instance token".into()))
    }

    /// Build a by-name member dispatch (`recv.name(args)`), receiver as `args[0]`.
    pub(crate) fn late_member_call(
        &self,
        name: &str,
        kind: ProjectMemberKind,
        recv: CoreValue,
        mut method_args: Vec<CoreArg>,
    ) -> CoreValue {
        let mut args = vec![CoreArg::ByVal(recv)];
        args.append(&mut method_args);
        CoreValue::Call {
            callee: CoreCallee::LateDispatch { name: name.to_string(), kind: Some(kind) },
            args,
        }
    }

    /// Build an early-bound COM dispatch (`recv.member` by dispid), receiver arg0.
    fn early_com_call(
        &self,
        dispid: i32,
        kind: ProjectMemberKind,
        recv: CoreValue,
        mut method_args: Vec<CoreArg>,
    ) -> CoreValue {
        let mut args = vec![CoreArg::ByVal(recv)];
        args.append(&mut method_args);
        CoreValue::Call { callee: CoreCallee::EarlyCom { dispid, kind: Some(kind) }, args }
    }

    /// True if a receiver should fall back to late binding when a member doesn't
    /// resolve: an untyped `Variant`, or an `Object` that isn't a project class
    /// (a foreign/COM object). A missing member on a *known* project class stays
    /// an error.
    pub(crate) fn is_late_bound_receiver(&self, ty: &VarTypeRef) -> bool {
        match ty {
            VarTypeRef::Variant => true,
            VarTypeRef::Object(name) => !self.g.ids.class_of.contains_key(&fold_identifier(name)),
            _ => false,
        }
    }

    /// The declared return type of a project member (for inference); `Variant`
    /// when unknown.
    fn member_return_type(&self, sym: Option<SymbolId>, kind: ProjectMemberKind) -> VarTypeRef {
        sym.and_then(|s| self.proc_signature_for(s, kind))
            .and_then(|s| s.return_type)
            .unwrap_or(VarTypeRef::Variant)
    }

    /// True if `recv` denotes the predeclared `Err` object.
    pub(crate) fn is_err_receiver(&self, recv: SyntaxNode<'_>) -> bool {
        recv.kind() == SyntaxKind::IdentExpr
            && recv.ident_name_token().is_some_and(|t| {
                matches!(
                    self.resolve(t.text).map(|b| b.route),
                    Some(DispatchRoute::PredeclaredObject(PredeclaredObjectId::Err))
                )
            })
    }

    /// Lower a statement-position callee (`Inc x`, `Call Foo(a)`, `obj.M`) to its
    /// call value.
    pub(crate) fn bind_call_from_callee(
        &mut self,
        callee: SyntaxNode<'_>,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        match callee.kind() {
            SyntaxKind::IdentExpr => {
                let name = callee
                    .ident_name_token()
                    .ok_or_else(|| BindError::Malformed("call name".into()))?
                    .text;
                let binding = self
                    .resolve(name)
                    .ok_or_else(|| self.unresolved(name, "call statement"))?;
                self.bind_call_route(name, &binding, arglist)
            }
            // `Call Foo(a, b)` — the whole `Foo(a, b)` is the callee (an IndexExpr).
            SyntaxKind::IndexExpr => self.bind_index_or_call(callee),
            // `obj.Method` / `.Method` in statement position (no parenthesised args).
            SyntaxKind::MemberExpr => self.bind_member_call(callee, arglist),
            other => Err(BindError::Unsupported(format!("call statement {other:?}"))),
        }
    }
}

/// Convert one variadic-tail argument to its array-element value for a ParamArray
/// box. Tail args are always ByVal values or omitted slots (a Missing element →
/// `Empty`); the other variants cannot occur but are mapped sensibly.
fn paramarray_element(arg: CoreArg) -> CoreValue {
    match arg {
        CoreArg::ByVal(v) => v,
        CoreArg::Named { value, .. } => value,
        CoreArg::Omitted => CoreValue::Const(oxvba_bundle::coreir::CoreConst::Empty),
        CoreArg::ByRef(place) => CoreValue::Load(place),
    }
}

fn library_const(value: &LibraryConstValue) -> Bound {
    match value {
        LibraryConstValue::Str(s) => {
            value_bound(CoreValue::Const(oxvba_bundle::coreir::CoreConst::Str(s.clone())), builtin(BuiltinType::String))
        }
        LibraryConstValue::Int(i) => {
            value_bound(CoreValue::Const(oxvba_bundle::coreir::CoreConst::I32(*i)), builtin(BuiltinType::Long))
        }
    }
}

/// The `ErrField` (and its type) for an `Err.<member>` read.
pub(crate) fn err_field(member: &str) -> Option<(ErrField, VarTypeRef)> {
    match fold_identifier(member).as_str() {
        "number" => Some((ErrField::Number, builtin(BuiltinType::Long))),
        "description" => Some((ErrField::Description, builtin(BuiltinType::String))),
        "source" => Some((ErrField::Source, builtin(BuiltinType::String))),
        _ => None,
    }
}
