//! The call binder: maps a resolved `DispatchRoute` to a `CoreCallee` (project
//! proc / native / `Declare` / special form), binds arguments (positional /
//! named / omitted, ByVal coercion, ByRef aliasing), and lowers member access
//! (the `Err` object for now; objects/COM arrive in a later phase).

use oxvba_bundle::coreir::{
    BoundWhich, CoreArg, CoreCallee, CoreConst, CorePlace, CoreStmt, CoreValue, ErrField, PtrKind,
    PtrWriteback, PtrWritebackKind,
};
use oxvba_bundle::native::NativeImplId;
use oxvba_bundle::{BundleImport, ExportToken, ProjectMemberKind, StringCompareMode};
use oxvba_com::{OptionalParamDefault, TypeLibMemberMetadata, TypeLibParamType, TypeLibWireType};
use oxvba_symbol::binding::{Binding, DispatchRoute, SpecialForm};
use oxvba_symbol::model::{
    PredeclaredObjectId, SymbolId, SymbolImpl, SymbolKind, SymbolNamespace, fold_identifier,
};
use oxvba_symbol::signature::{BuiltinType, Param, PassingMode, Signature, VarTypeRef};
use oxvba_symbol::structural::StructuralIntrinsic;
use oxvba_syntax::red::{ArgItem, CallSitePassing};
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::error::BindError;
use crate::expr::{builtin, value_bound};
use crate::types;
use crate::{Bound, ProcLower};

/// A bound pointer-helper operand: the pin kind, the operand value the VM
/// pins, and an optional write-back target (source l-value + payload kind).
type BoundPointerOperand = (PtrKind, CoreValue, Option<(CorePlace, PtrWritebackKind)>);

impl<'a> ProcLower<'a> {
    /// Lower a resolved name + optional argument list to a value (a call, a
    /// constant, or a special form).
    pub(crate) fn bind_call_route(
        &mut self,
        name: &str,
        binding: &Binding,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        self.reject_sub_in_value_context(name, binding)?;
        self.bind_call_route_any(name, binding, arglist)
    }

    /// Lower a resolved statement-position call. Unlike value context, this may
    /// target a `Sub`, whose result is discarded by the enclosing `Eval`.
    fn bind_call_route_statement(
        &mut self,
        name: &str,
        binding: &Binding,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        self.bind_call_route_any(name, binding, arglist)
    }

    fn bind_call_route_any(
        &mut self,
        name: &str,
        binding: &Binding,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        match &binding.route {
            DispatchRoute::Value if self.binding_is_module(binding) => {
                Err(self.module_as_value_error(name))
            }
            // A folded constant literal (a referenced project's `Public Const`/`Enum`
            // member, or a `vb*` base-library constant) used in callee/index position
            // (`Call vbCrLf`, `vbReadOnly(0)`): inline the literal, ignoring any index
            // arguments — exactly as the value path does. The normal value paths in
            // `expr::bind_ident` / `finish_value_or_call` intercept `ConstValue` first;
            // this arm covers the callee-shaped sites that still route here.
            DispatchRoute::ConstValue(c) => Ok(value_bound(
                CoreValue::Const(c.clone()),
                crate::expr::const_type(c),
            )),
            DispatchRoute::ProjectMember { kind } => {
                self.bind_project_call(name, binding, *kind, arglist)
            }
            // A referenced project's hidden-module function (no receiver): register a
            // cross-bundle import and emit an `ExternProc` call. `has_receiver: true`
            // never reaches here (a coclass member needs a receiver — it arrives via
            // `bind_member_*`); reject it defensively.
            DispatchRoute::ExternMember {
                unit,
                owner,
                member,
                kind,
                param_types,
                param_names,
                param_optional,
                param_optional_defaults,
                variadic,
                has_receiver,
            } => {
                if *has_receiver {
                    return Err(BindError::Unsupported(format!(
                        "`{name}` is a referenced coclass member; call it on a receiver"
                    )));
                }
                let import = self.g.intern_import(BundleImport {
                    unit: unit.clone(),
                    token: ExportToken::ModuleFunc {
                        module: owner.clone(),
                        member: member.clone(),
                        kind: *kind,
                    },
                });
                let mut args = self.bind_extern_proc_args(
                    arglist,
                    param_types,
                    param_names,
                    param_optional,
                    param_optional_defaults,
                    *variadic,
                )?;
                self.inject_option_compare(member, &mut args);
                Ok(value_bound(
                    CoreValue::Call {
                        callee: CoreCallee::ExternProc { import },
                        args,
                    },
                    VarTypeRef::Variant,
                ))
            }
            DispatchRoute::Native(id) => {
                let args = self.bind_native_args(*id, arglist)?;
                Ok(value_bound(
                    CoreValue::Call {
                        callee: CoreCallee::Native(*id),
                        args,
                    },
                    VarTypeRef::Variant,
                ))
            }
            DispatchRoute::Declare { descriptor_id } => {
                // ByRef `Declare` params write back to the caller slot (copy-out); a
                // `StrPtr(x)`/`VarPtr(x)` pointer argument over an l-value records a
                // pointer write-back (the pinned buffer → `x` after the call). String
                // params write back even when `ByVal` (the ANSI-buffer contract).
                let by_ref = self.declare_param_by_ref(binding.symbol);
                let is_string = self.declare_param_is_string(binding.symbol);
                let (args, ptr_writebacks) =
                    self.bind_declare_args(arglist, &by_ref, &is_string)?;
                Ok(value_bound(
                    CoreValue::Call {
                        callee: CoreCallee::Declare {
                            descriptor_id: *descriptor_id,
                            ptr_writebacks,
                        },
                        args,
                    },
                    VarTypeRef::Variant,
                ))
            }
            DispatchRoute::SpecialForm(SpecialForm::Array) => {
                let items = match arglist {
                    Some(a) => self.bind_positional_values(a)?,
                    None => Vec::new(),
                };
                Ok(value_bound(
                    CoreValue::ArrayLiteral {
                        elems: items,
                        lower_bound: self.info.option_base,
                        aliases: Vec::new(),
                    },
                    VarTypeRef::Variant,
                ))
            }
            // `IIf`/`Choose`/`Switch` are eager VBA library functions (every
            // argument is evaluated before the call) — lower them as native
            // calls, not as lazy/short-circuit forms.
            DispatchRoute::SpecialForm(
                sf @ (SpecialForm::IIf | SpecialForm::Choose | SpecialForm::Switch),
            ) => {
                let id = match sf {
                    SpecialForm::IIf => NativeImplId::IIf,
                    SpecialForm::Choose => NativeImplId::Choose,
                    SpecialForm::Switch => NativeImplId::Switch,
                    _ => unreachable!(),
                };
                let args = self.bind_args(arglist, None)?;
                Ok(value_bound(
                    CoreValue::Call {
                        callee: CoreCallee::Native(id),
                        args,
                    },
                    VarTypeRef::Variant,
                ))
            }
            // `UBound`/`LBound` take an array l-value plus an optional
            // one-based dimension argument. Invalid dimensions must remain a
            // run-time error so VBA `On Error Resume Next` feature probes work.
            DispatchRoute::SpecialForm(sf @ (SpecialForm::UBound | SpecialForm::LBound)) => {
                let which = if matches!(sf, SpecialForm::UBound) {
                    BoundWhich::Upper
                } else {
                    BoundWhich::Lower
                };
                let mut items = arglist.map(|a| a.arg_items()).unwrap_or_default();
                if items.is_empty() {
                    return Err(BindError::Malformed(format!(
                        "`{name}` requires an array argument"
                    )));
                }
                if items.len() > 2 {
                    return Err(BindError::Malformed(format!(
                        "`{name}` accepts at most array and dimension arguments"
                    )));
                }
                let first = items.remove(0);
                let dimension = items
                    .into_iter()
                    .next()
                    .map(|item| match item {
                        ArgItem::Positional(e, _) => self.bind_expr(e).map(|bound| bound.value),
                        _ => Err(BindError::Malformed(format!("`{name}` dimension argument"))),
                    })
                    .transpose()?;
                let expr = match first {
                    ArgItem::Positional(e, _) => e,
                    _ => return Err(BindError::Malformed(format!("`{name}` array argument"))),
                };
                let (place, _) = self.bind_place(expr)?;
                Ok(value_bound(
                    CoreValue::Bound {
                        which,
                        array: Box::new(place),
                        dimension: dimension.map(Box::new),
                    },
                    builtin(BuiltinType::Long),
                ))
            }
            DispatchRoute::SpecialForm(SpecialForm::CallByName) => self.bind_callbyname(arglist),
            DispatchRoute::SpecialForm(SpecialForm::Erl) => {
                let items = arglist.map(|a| a.arg_items()).unwrap_or_default();
                if !items.is_empty() {
                    return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
                }
                Ok(value_bound(CoreValue::Erl, builtin(BuiltinType::Long)))
            }
            DispatchRoute::ErrMember(_) => Err(BindError::Unsupported(format!(
                "`{name}` Err member in value context"
            ))),
            // The pointer-helper intrinsics yield the address of their operand as a
            // `LongPtr`. The operand is bound as a **value** (so r-values like
            // `StrPtr("literal")` work, not just l-values); at run time the VM pins
            // the value in the pointer registry (`oxvba_runtime::pointer_helpers`) for
            // the duration of the native call it feeds — see POST_CLEANUP.md for the
            // lifetime contract. Write-back into an l-value operand is recorded
            // separately on the `Declare` call (see `bind_args_byref`). `AddressOf` (a
            // procedure pointer) is a SpecialForm, handled separately in `expr.rs`.
            DispatchRoute::Structural(
                s @ (StructuralIntrinsic::VarPtr
                | StructuralIntrinsic::StrPtr
                | StructuralIntrinsic::ObjPtr),
            ) => {
                let first = arglist
                    .and_then(|a| a.arg_items().into_iter().next())
                    .ok_or_else(|| {
                        BindError::Malformed(format!("`{name}` requires an argument"))
                    })?;
                let expr = match first {
                    ArgItem::Positional(e, _) => e,
                    _ => return Err(BindError::Malformed(format!("`{name}` argument"))),
                };
                let (kind, value, _writeback) = self.pointer_operand(*s, expr)?;
                Ok(value_bound(
                    CoreValue::Ptr {
                        kind,
                        value: Box::new(value),
                    },
                    builtin(BuiltinType::LongPtr),
                ))
            }
            other => Err(BindError::Unsupported(format!(
                "call route {other:?} for `{name}`"
            ))),
        }
    }

    fn reject_sub_in_value_context(&self, name: &str, binding: &Binding) -> Result<(), BindError> {
        if self.is_statement_only_vba_library_member(binding) {
            return Err(BindError::ExpectedFunctionOrVariable {
                name: name.to_string(),
            });
        }
        if matches!(
            binding.route,
            DispatchRoute::ProjectMember {
                kind: ProjectMemberKind::Method
            }
        ) && let Some(sym) = binding.symbol
            && self
                .proc_signature_for(sym, ProjectMemberKind::Method)
                .is_some_and(|sig| sig.return_type.is_none())
        {
            return Err(BindError::ExpectedFunctionOrVariable {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    fn is_statement_only_vba_library_member(&self, binding: &Binding) -> bool {
        matches!(
            &binding.route,
            DispatchRoute::ExternMember {
                unit,
                owner,
                member,
                kind: ProjectMemberKind::Method,
                has_receiver: false,
                ..
            } if fold_identifier(unit) == "vba"
                && fold_identifier(owner) == "interaction"
                && matches!(fold_identifier(member).as_str(), "sendkeys" | "appactivate")
        )
    }

    fn bind_native_args(
        &mut self,
        id: NativeImplId,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Vec<CoreArg>, BindError> {
        let entry = oxvba_symbol::catalog::intrinsic_entry(id);
        if entry.param_names.is_empty() {
            return self.bind_args(arglist, None);
        }
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        let mut slots = vec![None; entry.param_names.len()];
        let mut extra = Vec::new();
        let mut pos = 0usize;
        let mut seen_named = false;
        for item in items {
            match item {
                ArgItem::Omitted => {
                    if seen_named {
                        return Err(BindError::Unsupported(
                            "positional argument cannot follow named argument".into(),
                        ));
                    }
                    if pos < slots.len() {
                        slots[pos] = Some(CoreArg::Omitted);
                    } else {
                        extra.push(CoreArg::Omitted);
                    }
                    pos += 1;
                }
                ArgItem::Positional(expr, passing) => {
                    if seen_named {
                        return Err(BindError::Unsupported(
                            "positional argument cannot follow named argument".into(),
                        ));
                    }
                    let arg = self.bind_one_arg(expr, None, passing)?;
                    if pos < slots.len() {
                        slots[pos] = Some(arg);
                    } else {
                        extra.push(arg);
                    }
                    pos += 1;
                }
                ArgItem::Named { name, value } => {
                    seen_named = true;
                    let folded = fold_identifier(name.text);
                    let Some(index) = entry
                        .param_names
                        .iter()
                        .position(|candidate| fold_identifier(candidate) == folded)
                    else {
                        return Err(self.unresolved(name.text, "named argument"));
                    };
                    if slots[index].is_some() {
                        return Err(BindError::Unsupported(format!(
                            "duplicate argument for parameter {}",
                            entry.param_names[index]
                        )));
                    }
                    slots[index] = Some(CoreArg::ByVal(self.bind_expr(value)?.value));
                }
            }
        }
        let mut args: Vec<CoreArg> = slots
            .into_iter()
            .map(|slot| slot.unwrap_or(CoreArg::Omitted))
            .collect();
        while matches!(args.last(), Some(CoreArg::Omitted)) {
            args.pop();
        }
        args.extend(extra);
        Ok(args)
    }

    /// Inject the module's `Option Compare` as the trailing `compare` argument of
    /// the string functions that take one (`InStr`/`InStrRev`/`StrComp`/`Replace`/
    /// `Filter`), when the call omits it and the module is `Option Compare Text`.
    /// `Binary` is the library default, so nothing is injected there. An omitted
    /// *intermediate* optional is padded with `Omitted` (the lib treats that
    /// Missing sentinel as absent), so the injected `compare` lands at the right
    /// index. `InStr` is arity-based — a 2-arg `InStr(s1,s2)` is first promoted to
    /// `InStr(1, s1, s2)` so `compare` can occupy slot 3. (These builtins route via
    /// `ExternMember` to the "VBA" library bundle, so this runs there.)
    fn inject_option_compare(&self, member: &str, args: &mut Vec<CoreArg>) {
        if self.info.compare_mode != StringCompareMode::Text {
            return;
        }
        let text = || CoreArg::ByVal(CoreValue::Const(CoreConst::I32(1)));
        match member {
            "StrComp" => set_trailing_arg(args, 2, text()),
            "Filter" => set_trailing_arg(args, 3, text()),
            "InStrRev" => set_trailing_arg(args, 3, text()),
            "Replace" => set_trailing_arg(args, 5, text()),
            "InStr" => {
                if args.len() == 2 {
                    args.insert(0, CoreArg::ByVal(CoreValue::Const(CoreConst::I32(1))));
                }
                set_trailing_arg(args, 3, text());
            }
            _ => {}
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
        let mut args = match &signature {
            Some(sig) => self.bind_proc_args(arglist, sig, sym)?,
            None => self.bind_args(arglist, None)?,
        };
        if let Some(target_class) = self
            .g
            .ids
            .procs
            .get(proc_id.0)
            .and_then(|info| info.class_name.as_deref())
            && self.info.class_name.as_deref().map(fold_identifier)
                == Some(fold_identifier(target_class))
        {
            let me = self
                .me_value()
                .ok_or_else(|| BindError::Malformed(format!("class member `{name}` without Me")))?;
            args.insert(0, CoreArg::ByVal(me));
        }
        let ty = signature
            .and_then(|s| s.return_type)
            .unwrap_or(VarTypeRef::Variant);
        Ok(value_bound(
            CoreValue::Call {
                callee: CoreCallee::VbaProc { proc: proc_id },
                args,
            },
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
        passing: CallSitePassing,
    ) -> Result<CoreArg, BindError> {
        // A call-site `ByVal`/`ByRef` overrides the callee's declared direction;
        // otherwise the declared direction (default ByRef) stands.
        let by_ref = match passing {
            CallSitePassing::ByVal => false,
            CallSitePassing::ByRef => true,
            CallSitePassing::Default => {
                param.map(|p| p.mode == PassingMode::ByRef).unwrap_or(false)
            }
        };
        // VBA forces pass-by-value when an argument is wrapped in parentheses.
        let forced_by_val = expr.kind() == SyntaxKind::ParenExpr;
        if by_ref
            && !forced_by_val
            && !self.is_object_default_member_index_expr(expr)
            && let Ok((place, place_ty)) = self.bind_place(expr)
        {
            if let Some(param) = param {
                self.ensure_byref_type_compatible(&param.ty, &place_ty)?;
            }
            return Ok(CoreArg::ByRef(place));
        }
        let bound = self.bind_expr(expr)?;
        let value = match param {
            Some(p) => {
                let expected = self.g.resolve_udt_type(p.ty.clone());
                let actual = self.g.resolve_udt_type(bound.ty.clone());
                ensure_byval_type_compatible(&expected, &actual)?;
                types::coerce(bound.value, &bound.ty, &p.ty)
            }
            None => bound.value,
        };
        Ok(CoreArg::ByVal(value))
    }

    fn paramarray_alias_place(
        &mut self,
        expr: SyntaxNode<'_>,
        passing: CallSitePassing,
    ) -> Option<CorePlace> {
        if passing == CallSitePassing::ByVal
            || expr.kind() == SyntaxKind::ParenExpr
            || self.is_object_default_member_index_expr(expr)
        {
            return None;
        }
        self.bind_place(expr).map(|(place, _)| place).ok()
    }

    fn ensure_byref_type_compatible(
        &self,
        expected: &VarTypeRef,
        actual: &VarTypeRef,
    ) -> Result<(), BindError> {
        let expected = canonical_byref_type(self.g.resolve_udt_type(expected.clone()));
        let actual = canonical_byref_type(self.g.resolve_udt_type(actual.clone()));
        if expected == actual || byref_variant_accepts_array(&expected, &actual) {
            return Ok(());
        }
        Err(BindError::ByRefTypeMismatch {
            expected: types::type_name(&expected),
            actual: types::type_name(&actual),
        })
    }

    /// Bind one argument given an explicit by-ref flag (COM/`Declare`, where the
    /// direction comes from a typelib/`Declare` param list, not a project
    /// `Signature`). ByRef l-value (not parenthesised) → `ByRef`; else `ByVal`
    /// (no coercion — the value is marshaled by the callee).
    fn bind_arg_byref(
        &mut self,
        expr: SyntaxNode<'_>,
        by_ref: bool,
        passing: CallSitePassing,
    ) -> Result<CoreArg, BindError> {
        // A call-site `ByVal`/`ByRef` overrides the typelib/`Declare` direction.
        let by_ref = match passing {
            CallSitePassing::ByVal => false,
            CallSitePassing::ByRef => true,
            CallSitePassing::Default => by_ref,
        };
        if by_ref
            && expr.kind() != SyntaxKind::ParenExpr
            && !self.is_object_default_member_index_expr(expr)
            && let Ok((place, _)) = self.bind_place(expr)
        {
            return Ok(CoreArg::ByRef(place));
        }
        Ok(CoreArg::ByVal(self.bind_expr(expr)?.value))
    }

    fn is_object_default_member_index_expr(&mut self, expr: SyntaxNode<'_>) -> bool {
        if expr.kind() != SyntaxKind::IndexExpr {
            return false;
        }
        let Some(base) = expr.index_base() else {
            return false;
        };
        self.bind_expr(base)
            .map(|bound| matches!(bound.ty, VarTypeRef::Object(_)))
            .unwrap_or(false)
    }

    /// Arguments for an early-bound COM call. Unlike a genuinely late-bound
    /// `Object` dispatch, a typed COM receiver has a typelib descriptor at bind
    /// time, so named arguments are validated and reordered into descriptor order.
    pub(crate) fn bind_com_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        member: &TypeLibMemberMetadata,
    ) -> Result<Vec<CoreArg>, BindError> {
        self.bind_com_args_inner(arglist, member, None)
    }

    /// Arguments for a COM `Property Let`/`Set` assignment. The indexed arguments
    /// bind against the visible parameters before the trailing `[propput]` value,
    /// which is supplied separately from the assignment RHS.
    fn bind_com_property_put_index_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        member: &TypeLibMemberMetadata,
    ) -> Result<Vec<CoreArg>, BindError> {
        self.bind_com_args_inner(arglist, member, Some(1))
    }

    fn bind_com_args_inner(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        member: &TypeLibMemberMetadata,
        reserved_visible_tail: Option<usize>,
    ) -> Result<Vec<CoreArg>, BindError> {
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        let visible = visible_com_param_indices(member);
        let bindable_count = visible
            .len()
            .saturating_sub(reserved_visible_tail.unwrap_or(0));
        let variadic_slot = reserved_visible_tail
            .is_none()
            .then(|| com_member_paramarray_index(member))
            .flatten()
            .and_then(|param_index| {
                visible[..bindable_count]
                    .iter()
                    .position(|&i| i == param_index)
            });
        let fixed_count = variadic_slot.unwrap_or(bindable_count);
        let mut slots: Vec<Option<CoreArg>> = (0..fixed_count).map(|_| None).collect();
        let mut variadic_tail = Vec::new();
        let mut pos = 0usize;
        let mut seen_named = false;
        for item in items {
            match item {
                ArgItem::Positional(expr, passing) => {
                    if seen_named {
                        return Err(BindError::Unsupported(
                            "positional argument cannot follow named argument".into(),
                        ));
                    }
                    if pos < fixed_count {
                        let param_index = visible[pos];
                        slots[pos] = Some(self.bind_com_one(
                            expr,
                            member.parameter_types.get(param_index),
                            passing,
                        )?);
                    } else if variadic_slot.is_some() {
                        variadic_tail.push(self.bind_com_one(
                            expr,
                            Some(&TypeLibParamType::Variant),
                            CallSitePassing::ByVal,
                        )?);
                    } else {
                        return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
                    }
                    pos += 1;
                }
                ArgItem::Omitted => {
                    if seen_named {
                        return Err(BindError::Unsupported(
                            "positional argument cannot follow named argument".into(),
                        ));
                    }
                    if pos < fixed_count {
                        slots[pos] = Some(CoreArg::Omitted);
                    } else if variadic_slot.is_some() {
                        variadic_tail.push(CoreArg::Omitted);
                    } else {
                        return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
                    }
                    pos += 1;
                }
                ArgItem::Named { name, value } => {
                    seen_named = true;
                    let folded = fold_identifier(name.text);
                    if variadic_slot.is_some_and(|slot| {
                        member
                            .parameter_names
                            .get(visible[slot])
                            .is_some_and(|p| fold_identifier(p) == folded)
                    }) {
                        return Err(BindError::Unsupported(
                            "named argument to a ParamArray parameter".into(),
                        ));
                    }
                    let Some(slot_index) = visible[..fixed_count].iter().position(|&i| {
                        member
                            .parameter_names
                            .get(i)
                            .is_some_and(|p| fold_identifier(p) == folded)
                    }) else {
                        return Err(BindError::NamedArgumentNotFound {
                            name: name.text.to_string(),
                        });
                    };
                    if slots[slot_index].is_some() {
                        let param_name = member
                            .parameter_names
                            .get(visible[slot_index])
                            .map(String::as_str)
                            .unwrap_or(name.text);
                        return Err(BindError::Unsupported(format!(
                            "duplicate argument for parameter {param_name}"
                        )));
                    }
                    let param_index = visible[slot_index];
                    slots[slot_index] = Some(self.bind_com_one(
                        value,
                        member.parameter_types.get(param_index),
                        CallSitePassing::Default,
                    )?);
                }
            }
        }

        if variadic_slot.is_some() {
            let mut args = Vec::with_capacity(fixed_count + 1);
            for (slot_index, slot) in slots.into_iter().enumerate() {
                let param_index = visible[slot_index];
                match slot {
                    Some(CoreArg::Omitted) | None
                        if !com_param_is_optional(member, param_index) =>
                    {
                        return Err(BindError::ArgumentNotOptional {
                            parameter: com_param_display_name(member, param_index),
                        });
                    }
                    Some(CoreArg::Omitted) | None => args.push(CoreArg::Omitted),
                    Some(arg) => args.push(arg),
                }
            }
            let mut elems = Vec::with_capacity(variadic_tail.len());
            let mut aliases = Vec::with_capacity(variadic_tail.len());
            for arg in variadic_tail {
                elems.push(paramarray_element(arg));
                aliases.push(None);
            }
            args.push(CoreArg::ByVal(CoreValue::ArrayLiteral {
                elems,
                lower_bound: 0,
                aliases,
            }));
            return Ok(args);
        }

        let Some(last_supplied) = slots.iter().rposition(Option::is_some) else {
            self.require_no_missing_com_required(member, &visible[..bindable_count])?;
            return Ok(Vec::new());
        };
        self.require_no_missing_com_required(member, &visible[last_supplied + 1..bindable_count])?;
        let mut args = Vec::with_capacity(last_supplied + 1);
        for (slot_index, slot) in slots.into_iter().take(last_supplied + 1).enumerate() {
            match slot {
                Some(arg) => args.push(arg),
                None => {
                    let param_index = visible[slot_index];
                    if com_param_is_optional(member, param_index) {
                        args.push(CoreArg::Omitted);
                    } else {
                        return Err(BindError::ArgumentNotOptional {
                            parameter: com_param_display_name(member, param_index),
                        });
                    }
                }
            }
        }
        Ok(args)
    }

    fn require_no_missing_com_required(
        &self,
        member: &TypeLibMemberMetadata,
        indices: &[usize],
    ) -> Result<(), BindError> {
        for &param_index in indices {
            if !com_param_is_optional(member, param_index) {
                return Err(BindError::ArgumentNotOptional {
                    parameter: com_param_display_name(member, param_index),
                });
            }
        }
        Ok(())
    }

    /// Bind `Declare` arguments (ByRef-aware, like [`Self::bind_arg_byref`]) and
    /// collect pointer-helper write-backs: a positional `StrPtr(x)` / `VarPtr(x)`
    /// over a simple-variable l-value records a [`PtrWriteback`] so the VM reads the
    /// pinned buffer back into `x` after the call (VBA's expression-shape-driven
    /// write-back). An r-value pointer operand (e.g. `StrPtr("lit")`), a compound
    /// l-value, or a kind with no buffer projection records no write-back.
    fn bind_declare_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        param_by_ref: &[bool],
        param_is_string: &[bool],
    ) -> Result<(Vec<CoreArg>, Vec<PtrWriteback>), BindError> {
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        let mut args = Vec::with_capacity(items.len());
        let mut writebacks = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            match item {
                ArgItem::Omitted => args.push(CoreArg::Omitted),
                ArgItem::Named { name, value } => args.push(CoreArg::Named {
                    name: name.text.to_string(),
                    value: self.bind_expr(value)?.value,
                }),
                ArgItem::Positional(expr, passing) => {
                    // A call-site `ByVal`/`ByRef` overrides the declared direction.
                    let by_ref = match passing {
                        CallSitePassing::ByVal => false,
                        CallSitePassing::ByRef => true,
                        CallSitePassing::Default => param_by_ref.get(i).copied().unwrap_or(false),
                    };
                    if let Some((intrinsic, operand)) = self.pointer_call(expr) {
                        let (kind, value, wb) = self.pointer_operand(intrinsic, operand)?;
                        args.push(CoreArg::ByVal(CoreValue::Ptr {
                            kind,
                            value: Box::new(value),
                        }));
                        if let Some((target, kind)) = wb {
                            writebacks.push(PtrWriteback {
                                arg_index: i,
                                target,
                                kind,
                            });
                        }
                    } else if !by_ref && param_is_string.get(i).copied().unwrap_or(false) {
                        // A call-site `ByVal` over a `ByVal As String` param suppresses
                        // the pre-sized-buffer write-back; `Default` keeps it.
                        args.push(self.bind_byval_string_arg(expr, passing)?);
                    } else {
                        // `by_ref` already folds in the override, so pass `Default`.
                        args.push(self.bind_arg_byref(expr, by_ref, CallSitePassing::Default)?);
                    }
                }
            }
        }
        Ok((args, writebacks))
    }

    /// Bind a `ByVal … As String` `Declare` argument. VBA converts a `Declare`
    /// String argument to a system-codepage ANSI buffer for the call and converts
    /// the (possibly callee-mutated) buffer back into the variable afterwards —
    /// `ByVal` notwithstanding; that is the pre-sized-buffer idiom
    /// (`s = String(255, 0): GetWindowsDirectoryA s, 255`). A String-typed,
    /// non-parenthesised l-value therefore binds ByRef so the marshaled-back value
    /// reaches the variable; anything else (a literal, an expression, `(s)`, or a
    /// non-String l-value, whose conversion temp VBA also discards) binds ByVal
    /// with no write-back.
    fn bind_byval_string_arg(
        &mut self,
        expr: SyntaxNode<'_>,
        passing: CallSitePassing,
    ) -> Result<CoreArg, BindError> {
        // An explicit call-site `ByVal` opts out of the pre-sized-buffer
        // write-back: the caller wants a pure value, not the marshaled-back buffer.
        if passing != CallSitePassing::ByVal
            && expr.kind() != SyntaxKind::ParenExpr
            && let Ok((place, ty)) = self.bind_place(expr)
            && matches!(ty, VarTypeRef::Builtin(BuiltinType::String))
        {
            return Ok(CoreArg::ByRef(place));
        }
        Ok(CoreArg::ByVal(self.bind_expr(expr)?.value))
    }

    /// If `expr` is a `StrPtr`/`VarPtr`/`ObjPtr` call, return the intrinsic and its
    /// single operand expression; otherwise `None`.
    fn pointer_call<'b>(
        &self,
        expr: SyntaxNode<'b>,
    ) -> Option<(StructuralIntrinsic, SyntaxNode<'b>)> {
        if expr.kind() != SyntaxKind::IndexExpr {
            return None;
        }
        let base = expr.index_base()?;
        if base.kind() != SyntaxKind::IdentExpr {
            return None;
        }
        let intrinsic = match self.resolve(base.ident_name_token()?.text).map(|b| b.route) {
            Some(DispatchRoute::Structural(
                s @ (StructuralIntrinsic::VarPtr
                | StructuralIntrinsic::StrPtr
                | StructuralIntrinsic::ObjPtr),
            )) => s,
            _ => return None,
        };
        match expr.index_arg_list()?.arg_items().into_iter().next()? {
            ArgItem::Positional(operand, _) => Some((intrinsic, operand)),
            _ => None,
        }
    }

    /// Bind a pointer-helper operand: the `PtrKind` + the operand **value** the VM
    /// pins, plus an optional write-back target (the source l-value + payload kind)
    /// for use as a `Declare` argument. `VarPtr` of an array element points at the
    /// whole contiguous array buffer (VBA's `VarPtr(a(0))` idiom); a simple String /
    /// String-`VarPtr` / array-`VarPtr` l-value writes back; an r-value operand
    /// (e.g. `StrPtr("lit")`) or a compound target writes back nothing.
    fn pointer_operand(
        &mut self,
        intrinsic: StructuralIntrinsic,
        operand: SyntaxNode<'_>,
    ) -> Result<BoundPointerOperand, BindError> {
        if let Ok((place, ty)) = self.bind_place(operand) {
            // `VarPtr(a(i))` → a pointer to the array's contiguous storage (read and
            // write the whole buffer), keyed off the base array place.
            if matches!(intrinsic, StructuralIntrinsic::VarPtr)
                && let CorePlace::Index { array, .. } = &place
            {
                let array_place = (**array).clone();
                let writeback = matches!(array_place, CorePlace::Local(_) | CorePlace::Global(_))
                    .then(|| (array_place.clone(), PtrWritebackKind::ByteArray));
                return Ok((PtrKind::Var, CoreValue::Load(array_place), writeback));
            }
            let kind = pointer_kind(intrinsic, &ty);
            // Write-back only into a simple variable (a slot the VM stores into).
            let writeback = if matches!(place, CorePlace::Local(_) | CorePlace::Global(_)) {
                match kind {
                    PtrKind::Str | PtrKind::VarString => {
                        Some((place.clone(), PtrWritebackKind::String))
                    }
                    PtrKind::Var if matches!(ty, VarTypeRef::Array(_)) => {
                        Some((place.clone(), PtrWritebackKind::ByteArray))
                    }
                    PtrKind::Var => {
                        scalar_ptr_writeback_kind(&ty).map(|kind| (place.clone(), kind))
                    }
                    _ => None,
                }
            } else {
                None
            };
            let value = if matches!(kind, PtrKind::Var) && scalar_ptr_writeback_kind(&ty).is_some()
            {
                types::coerce_store(CoreValue::Load(place), &ty)
            } else {
                CoreValue::Load(place)
            };
            return Ok((kind, value, writeback));
        }
        // An r-value operand (literal / expression): pin the value, no write-back.
        let operand = self.bind_expr(operand)?;
        Ok((pointer_kind(intrinsic, &operand.ty), operand.value, None))
    }

    /// Bind one positional cross-bundle extern argument against its published
    /// parameter type: a ByRef parameter aliases an l-value argument (or passes the
    /// value when it is not an l-value / is parenthesised); a ByVal parameter coerces
    /// its argument to the parameter type — exactly as a same-bundle project call.
    fn bind_extern_one(
        &mut self,
        expr: SyntaxNode<'_>,
        param: Option<&TypeLibParamType>,
        passing: CallSitePassing,
    ) -> Result<CoreArg, BindError> {
        // A call-site `ByVal`/`ByRef` overrides the typelib param direction.
        let by_ref = match passing {
            CallSitePassing::ByVal => false,
            CallSitePassing::ByRef => true,
            CallSitePassing::Default => param.is_some_and(|p| p.is_by_ref()),
        };
        if by_ref {
            if expr.kind() != SyntaxKind::ParenExpr
                && let Ok((place, _)) = self.bind_place(expr)
            {
                return Ok(CoreArg::ByRef(place));
            }
            return Ok(CoreArg::ByVal(self.bind_expr(expr)?.value));
        }
        let bound = self.bind_expr(expr)?;
        let value = match param {
            Some(p) => types::coerce(bound.value, &bound.ty, &tlb_param_to_vartype(p)),
            None => bound.value,
        };
        Ok(CoreArg::ByVal(value))
    }

    fn bind_com_one(
        &mut self,
        expr: SyntaxNode<'_>,
        param: Option<&TypeLibParamType>,
        passing: CallSitePassing,
    ) -> Result<CoreArg, BindError> {
        self.bind_extern_one(expr, param, passing)
    }

    /// Arguments for a cross-bundle **coclass member** (`LateDispatch` on a receiver):
    /// positional args bind against their published types; named args are kept
    /// verbatim for runtime name-dispatch.
    pub(crate) fn bind_extern_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        param_types: &[TypeLibParamType],
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
                    args.push(CoreArg::Named {
                        name: name.text.to_string(),
                        value: self.bind_expr(value)?.value,
                    });
                }
                ArgItem::Positional(expr, passing) => {
                    args.push(self.bind_extern_one(expr, param_types.get(i), passing)?)
                }
            }
        }
        Ok(args)
    }

    /// Arguments for a cross-bundle **free function** (`ExternProc`): the callee is
    /// positional, so named args are reordered into their declared slots by name
    /// and declared optional gaps receive the same default synthesis used for
    /// same-bundle `VbaProc` calls. Required gaps remain `Omitted` so the callee
    /// reports the call-shape error instead of receiving a fabricated value.
    /// Without this, an out-of-order named call (`Lib.F(b:=2, a:=1)`) would pass
    /// arguments in source order.
    fn bind_extern_proc_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        param_types: &[TypeLibParamType],
        param_names: &[String],
        param_optional: &[bool],
        param_optional_defaults: &[Option<CoreConst>],
        variadic: bool,
    ) -> Result<Vec<CoreArg>, BindError> {
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        let n = param_names.len();
        let mut slots: Vec<Option<CoreArg>> = (0..n).map(|_| None).collect();
        let mut extra: Vec<CoreArg> = Vec::new();
        let mut pos = 0usize;
        let mut seen_named = false;
        for item in items {
            match item {
                ArgItem::Positional(expr, passing) => {
                    if seen_named {
                        return Err(BindError::Unsupported(
                            "positional argument cannot follow named argument".into(),
                        ));
                    }
                    if pos < n {
                        let arg = self.bind_extern_one(expr, param_types.get(pos), passing)?;
                        slots[pos] = Some(arg)
                    } else if variadic {
                        extra.push(self.bind_extern_one(expr, None, passing)?)
                    } else {
                        return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
                    }
                    pos += 1;
                }
                ArgItem::Omitted => {
                    if seen_named {
                        return Err(BindError::Unsupported(
                            "positional argument cannot follow named argument".into(),
                        ));
                    }
                    if pos < n {
                        slots[pos] = Some(CoreArg::Omitted)
                    } else if variadic {
                        extra.push(CoreArg::Omitted)
                    } else {
                        return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
                    }
                    pos += 1;
                }
                ArgItem::Named { name, value } => {
                    seen_named = true;
                    let folded = fold_identifier(name.text);
                    match param_names
                        .iter()
                        .position(|p| fold_identifier(p) == folded)
                    {
                        Some(i) => {
                            if slots[i].is_some() {
                                return Err(BindError::Unsupported(format!(
                                    "duplicate argument for parameter {}",
                                    param_names[i]
                                )));
                            }
                            slots[i] = Some(self.bind_extern_one(
                                value,
                                param_types.get(i),
                                CallSitePassing::Default,
                            )?)
                        }
                        None => return Err(self.unresolved(name.text, "named argument")),
                    }
                }
            }
        }
        let mut args: Vec<CoreArg> = slots
            .into_iter()
            .enumerate()
            .map(|(i, s)| match s {
                Some(CoreArg::Omitted) | None => self.omitted_extern_optional_arg(
                    i,
                    param_types,
                    param_names,
                    param_optional,
                    param_optional_defaults,
                ),
                Some(arg) => arg,
            })
            .collect();
        while matches!(args.last(), Some(CoreArg::Omitted)) {
            args.pop();
        }
        args.extend(extra);
        Ok(args)
    }

    fn omitted_extern_optional_arg(
        &self,
        index: usize,
        param_types: &[TypeLibParamType],
        param_names: &[String],
        param_optional: &[bool],
        param_optional_defaults: &[Option<CoreConst>],
    ) -> CoreArg {
        if index >= param_names.len() || !param_optional.get(index).copied().unwrap_or(false) {
            return CoreArg::Omitted;
        }
        if let Some(Some(default)) = param_optional_defaults.get(index) {
            let ty = param_types
                .get(index)
                .map(tlb_param_to_vartype)
                .unwrap_or(VarTypeRef::Variant);
            return CoreArg::ByVal(types::coerce_store(CoreValue::Const(default.clone()), &ty));
        }
        let ty = param_types
            .get(index)
            .map(tlb_param_to_vartype)
            .unwrap_or(VarTypeRef::Variant);
        match &ty {
            VarTypeRef::Variant => CoreArg::Omitted,
            VarTypeRef::Object(_) => CoreArg::ByVal(CoreValue::Const(CoreConst::Nothing)),
            ty => CoreArg::ByVal(types::coerce_store(zero_const(ty), ty)),
        }
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
                    args.push(CoreArg::Named {
                        name: name.text.to_string(),
                        value: v,
                    });
                }
                ArgItem::Positional(expr, passing) => {
                    args.push(self.bind_one_arg(
                        expr,
                        signature.and_then(|s| s.params.get(i)),
                        passing,
                    )?);
                }
            }
        }
        Ok(args)
    }

    /// Arguments for a genuinely late-bound COM dispatch. With no static
    /// signature, VBA still passes an unparenthesized l-value argument by
    /// reference so a COM `[out]`/`[in,out]` parameter can write back to the
    /// caller. Parenthesized expressions and explicit `ByVal` stay value temps.
    pub(crate) fn bind_late_dispatch_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Vec<CoreArg>, BindError> {
        let items = match arglist {
            Some(a) => a.arg_items(),
            None => Vec::new(),
        };
        let mut args = Vec::with_capacity(items.len());
        for item in items {
            match item {
                ArgItem::Omitted => args.push(CoreArg::Omitted),
                ArgItem::Named { name, value } => {
                    let v = self.bind_expr(value)?.value;
                    args.push(CoreArg::Named {
                        name: name.text.to_string(),
                        value: v,
                    });
                }
                ArgItem::Positional(expr, passing) => {
                    args.push(self.bind_arg_byref(expr, true, passing)?);
                }
            }
        }
        Ok(args)
    }

    /// `Prop(index…) = rhs` — an indexed `Property Let`/`Set`. Lowers to a call of the
    /// accessor proc with the index arguments followed by the assigned value (the
    /// accessor's trailing parameter). Returns `None` when the base is not a bare
    /// project property (e.g. an array-element store, or a member-qualified target),
    /// so the caller falls back to a place store.
    pub(crate) fn bind_indexed_property_let(
        &mut self,
        target: SyntaxNode<'_>,
        kind: ProjectMemberKind,
        rhs: &CoreValue,
    ) -> Result<Option<Vec<CoreStmt>>, BindError> {
        let Some(base) = target.index_base() else {
            return Ok(None);
        };
        // A member-qualified indexed property put — `recv.Prop(index…) = rhs`. For a
        // COM / cross-project / late-bound receiver this is a parameterised
        // Property Let/Set: the accessor takes the index arguments followed by the
        // assigned value. (P2 `d.Item(k)=v`, P5 `d.Key(o)=n`, P6 `Set d.Item(k)=obj`.)
        if base.kind() == SyntaxKind::MemberExpr {
            return self.bind_member_indexed_property_let(target, base, kind, rhs);
        }
        if base.kind() != SyntaxKind::IdentExpr {
            return Ok(None); // any other base shape is a place store path
        }
        let Some(name) = base.ident_name_token().map(|t| t.text) else {
            return Ok(None);
        };
        let Some(binding) = self.resolve(name) else {
            return Ok(None);
        };
        if let DispatchRoute::Value = binding.route
            && let Some(sym) = binding.symbol
            && let ty @ VarTypeRef::Object(_) = self.symbol_type(sym)
            && let Some(default_binding) = self.resolve_default_member_kind(&ty, Some(kind))
        {
            return self.bind_default_member_indexed_property_let(
                name,
                &ty,
                default_binding,
                target.index_arg_list(),
                kind,
                rhs,
            );
        }
        if let DispatchRoute::Value = binding.route
            && let Some(sym) = binding.symbol
            && let ty @ VarTypeRef::Object(_) = self.symbol_type(sym)
            && self.is_late_bound_receiver(&ty)
        {
            let mut args = self.bind_args(target.index_arg_list(), None)?;
            args.push(CoreArg::ByVal(rhs.clone()));
            let recv = CoreValue::Load(self.place_by_name(name)?);
            return Ok(Some(vec![CoreStmt::Eval(
                self.late_default_member_call(kind, recv, args),
            )]));
        }
        if !matches!(
            binding.route,
            DispatchRoute::ProjectMember {
                kind: ProjectMemberKind::PropertyGet
                    | ProjectMemberKind::PropertyLet
                    | ProjectMemberKind::PropertySet
            }
        ) {
            return Ok(None); // not a property (e.g. an array → place store)
        }
        let Some(sym) = binding.symbol else {
            return Ok(None);
        };
        let Some(proc_id) = self.g.ids.prop_accessor_of.get(&(sym, kind)).copied() else {
            return Err(missing_project_property_accessor(name, kind));
        };
        // Bind the index arguments against the accessor's signature (its trailing
        // value parameter is supplied by the RHS, not the index list).
        let signature = self.project_property_accessor_signature(sym, kind, name)?;
        let mut args =
            self.bind_property_put_proc_args(target.index_arg_list(), &signature, sym)?;
        match args.last_mut() {
            Some(slot) => *slot = CoreArg::ByVal(rhs.clone()),
            None => args.push(CoreArg::ByVal(rhs.clone())),
        }
        Ok(Some(vec![CoreStmt::Eval(CoreValue::Call {
            callee: CoreCallee::VbaProc { proc: proc_id },
            args,
        })]))
    }

    /// `recv.Prop(index…) = rhs` — a member-qualified indexed Property Let/Set on a
    /// COM / cross-project / late-bound receiver. The accessor is called with the
    /// bound index arguments followed by the assigned value as its trailing
    /// parameter (the `[propput]`/`[propputref]` value). `kind` is `PropertyLet`
    /// for a `Let`/value assignment and `PropertySet` for a `Set` (the latter
    /// routes to PROPERTYPUTREF at the HAL). Returns `None` when the member is not
    /// such a property (the caller falls back to a place store).
    fn bind_member_indexed_property_let(
        &mut self,
        target: SyntaxNode<'_>,
        base: SyntaxNode<'_>,
        kind: ProjectMemberKind,
        rhs: &CoreValue,
    ) -> Result<Option<Vec<CoreStmt>>, BindError> {
        let Some(member) = base.member_name_token().map(|t| t.text) else {
            return Ok(None);
        };
        let recv = self.member_receiver_bound(base)?;
        // The index argument list lives on the enclosing IndexExpr (`target`).
        let arglist = target.index_arg_list();
        match self.resolve_member(&recv.ty, member, Some(kind)) {
            // A project class/interface property: dispatch to the resolved accessor
            // with index arguments followed by the assigned value, matching the
            // bare `Prop(index) = rhs` lowering without falling back to field/index
            // place assignment.
            Some(Binding {
                route: DispatchRoute::ProjectMember { .. },
                symbol: Some(sym),
                ..
            }) => {
                let signature = self.project_property_accessor_signature(sym, kind, member)?;
                let mut args = self.bind_property_put_proc_args(arglist, &signature, sym)?;
                match args.last_mut() {
                    Some(slot) => *slot = CoreArg::ByVal(rhs.clone()),
                    None => args.push(CoreArg::ByVal(rhs.clone())),
                }
                let dispatch = self.interface_dispatch_name(&recv.ty, member);
                Ok(Some(vec![CoreStmt::Eval(
                    self.late_member_call(&dispatch, kind, recv.value, args),
                )]))
            }
            // A typed COM receiver: dispatch the put/set by dispid with the index
            // args (ByRef per the typelib) followed by the value.
            Some(Binding {
                route:
                    DispatchRoute::ComMember {
                        interface_name,
                        member: com_member,
                        ..
                    },
                ..
            }) => {
                let mut args = self.bind_com_property_put_index_args(arglist, &com_member)?;
                args.push(CoreArg::ByVal(rhs.clone()));
                Ok(Some(vec![CoreStmt::Eval(self.early_com_call(
                    member,
                    kind,
                    &interface_name,
                    &com_member,
                    recv.value,
                    args,
                ))]))
            }
            // A cross-project coclass property: late dispatch by name, index args
            // coerced to the published param types, then the value.
            Some(Binding {
                route:
                    DispatchRoute::ExternMember {
                        member: m,
                        param_types,
                        ..
                    },
                ..
            }) => {
                let mut args = self.bind_extern_args(arglist, &param_types)?;
                args.push(CoreArg::ByVal(rhs.clone()));
                Ok(Some(vec![CoreStmt::Eval(
                    self.late_member_call(&m, kind, recv.value, args),
                )]))
            }
            // An untyped / foreign receiver: a late-bound indexed property put.
            None if self.is_late_bound_receiver(&recv.ty) => {
                let mut args = self.bind_late_dispatch_args(arglist)?;
                args.push(CoreArg::ByVal(rhs.clone()));
                Ok(Some(vec![CoreStmt::Eval(
                    self.late_member_call(member, kind, recv.value, args),
                )]))
            }
            // A project member or a plain field/method member → a place store path.
            _ => Ok(None),
        }
    }

    fn bind_default_member_indexed_property_let(
        &mut self,
        receiver_name: &str,
        receiver_ty: &VarTypeRef,
        default_binding: Binding,
        arglist: Option<SyntaxNode<'_>>,
        kind: ProjectMemberKind,
        rhs: &CoreValue,
    ) -> Result<Option<Vec<CoreStmt>>, BindError> {
        let recv = CoreValue::Load(self.place_by_name(receiver_name)?);
        match &default_binding.route {
            DispatchRoute::ProjectMember { .. } => {
                let sym = default_binding
                    .symbol
                    .ok_or_else(|| self.unresolved(receiver_name, "default member"))?;
                let member = self
                    .symbol_display_name(sym)
                    .unwrap_or_else(|| receiver_name.to_string());
                let signature = self.project_property_accessor_signature(sym, kind, &member)?;
                let mut args = self.bind_property_put_proc_args(arglist, &signature, sym)?;
                match args.last_mut() {
                    Some(slot) => *slot = CoreArg::ByVal(rhs.clone()),
                    None => args.push(CoreArg::ByVal(rhs.clone())),
                }
                let dispatch = self.interface_dispatch_name(receiver_ty, &member);
                Ok(Some(vec![CoreStmt::Eval(
                    self.late_member_call(&dispatch, kind, recv, args),
                )]))
            }
            DispatchRoute::ComMember { member_name, .. } => {
                let writer = self
                    .resolve_member(receiver_ty, member_name, Some(kind))
                    .unwrap_or_else(|| default_binding.clone());
                if let DispatchRoute::ComMember {
                    interface_name,
                    member: com_member,
                    ..
                } = writer.route
                {
                    let mut args = self.bind_com_property_put_index_args(arglist, &com_member)?;
                    args.push(CoreArg::ByVal(rhs.clone()));
                    Ok(Some(vec![CoreStmt::Eval(self.early_com_call(
                        member_name,
                        kind,
                        &interface_name,
                        &com_member,
                        recv,
                        args,
                    ))]))
                } else {
                    Ok(None)
                }
            }
            DispatchRoute::ExternMember { member, .. } => {
                let writer = self
                    .resolve_member(receiver_ty, member, Some(kind))
                    .unwrap_or_else(|| default_binding.clone());
                if let DispatchRoute::ExternMember {
                    member,
                    param_types,
                    ..
                } = writer.route
                {
                    let mut args = self.bind_extern_args(arglist, &param_types)?;
                    args.push(CoreArg::ByVal(rhs.clone()));
                    Ok(Some(vec![CoreStmt::Eval(
                        self.late_member_call(&member, kind, recv, args),
                    )]))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// A bare object in a value/Let context (`r = obj`) reads the receiver's
    /// authoritative default member. Keep this context-specific: object contexts
    /// such as `Set other = obj` must see the receiver object itself.
    pub(crate) fn bind_default_member_value(
        &mut self,
        receiver: Bound,
        receiver_label: &str,
    ) -> Result<Option<Bound>, BindError> {
        let ty = receiver.ty.clone();
        let VarTypeRef::Object(_) = ty else {
            return Ok(None);
        };
        let Some(default_binding) = self.resolve_default_member(&ty) else {
            return Ok(None);
        };
        self.bind_resolved_default_member_value(
            &ty,
            receiver.value,
            default_binding,
            receiver_label,
        )
    }

    fn bind_resolved_default_member_value(
        &mut self,
        receiver_ty: &VarTypeRef,
        receiver_value: CoreValue,
        default_binding: Binding,
        receiver_label: &str,
    ) -> Result<Option<Bound>, BindError> {
        match &default_binding.route {
            DispatchRoute::ProjectMember { kind } => {
                let sym = default_binding
                    .symbol
                    .ok_or_else(|| self.unresolved(receiver_label, "default member"))?;
                let member = self
                    .symbol_display_name(sym)
                    .unwrap_or_else(|| receiver_label.to_string());
                let kind = match kind {
                    ProjectMemberKind::Method => ProjectMemberKind::Method,
                    _ => ProjectMemberKind::PropertyGet,
                };
                let signature = self.project_property_accessor_signature(sym, kind, &member)?;
                let ret = signature.return_type.unwrap_or(VarTypeRef::Variant);
                let dispatch = self.interface_dispatch_name(receiver_ty, &member);
                Ok(Some(value_bound(
                    self.late_member_call(&dispatch, kind, receiver_value, Vec::new()),
                    ret,
                )))
            }
            DispatchRoute::ComMember {
                member_kind,
                interface_name,
                member: com_member,
                ..
            } => {
                let kind = match member_kind {
                    ProjectMemberKind::Method => ProjectMemberKind::Method,
                    _ => ProjectMemberKind::PropertyGet,
                };
                let args = self.bind_com_args(None, com_member)?;
                Ok(Some(value_bound(
                    self.early_com_call(
                        receiver_label,
                        kind,
                        interface_name,
                        com_member,
                        receiver_value,
                        args,
                    ),
                    com_member_return_type(com_member),
                )))
            }
            DispatchRoute::ExternMember { member, kind, .. } => {
                let kind = match kind {
                    ProjectMemberKind::Method => ProjectMemberKind::Method,
                    _ => ProjectMemberKind::PropertyGet,
                };
                Ok(Some(value_bound(
                    self.late_member_call(member, kind, receiver_value, Vec::new()),
                    VarTypeRef::Variant,
                )))
            }
            _ => Ok(None),
        }
    }

    /// `obj = rhs` in Let context, where `obj` is an object variable with a default
    /// member, is a default-member Property Let. `Set obj = rhs` remains ordinary
    /// object-reference assignment and deliberately does not use this path.
    pub(crate) fn bind_default_member_property_let(
        &mut self,
        receiver_name: &str,
        receiver_ty: &VarTypeRef,
        default_binding: Binding,
        rhs: &CoreValue,
    ) -> Result<Option<Vec<CoreStmt>>, BindError> {
        let recv = CoreValue::Load(self.place_by_name(receiver_name)?);
        match &default_binding.route {
            DispatchRoute::ProjectMember { .. } => {
                let sym = default_binding
                    .symbol
                    .ok_or_else(|| self.unresolved(receiver_name, "default member"))?;
                let member = self
                    .symbol_display_name(sym)
                    .unwrap_or_else(|| receiver_name.to_string());
                let signature = self.project_property_accessor_signature(
                    sym,
                    ProjectMemberKind::PropertyLet,
                    &member,
                )?;
                let mut args = self.bind_property_put_proc_args(None, &signature, sym)?;
                match args.last_mut() {
                    Some(slot) => *slot = CoreArg::ByVal(rhs.clone()),
                    None => args.push(CoreArg::ByVal(rhs.clone())),
                }
                let dispatch = self.interface_dispatch_name(receiver_ty, &member);
                Ok(Some(vec![CoreStmt::Eval(self.late_member_call(
                    &dispatch,
                    ProjectMemberKind::PropertyLet,
                    recv,
                    args,
                ))]))
            }
            DispatchRoute::ComMember { member_name, .. } => {
                let writer = self
                    .resolve_member(
                        receiver_ty,
                        member_name,
                        Some(ProjectMemberKind::PropertyLet),
                    )
                    .unwrap_or_else(|| default_binding.clone());
                if let DispatchRoute::ComMember {
                    interface_name,
                    member: com_member,
                    ..
                } = writer.route
                {
                    let mut args = self.bind_com_property_put_index_args(None, &com_member)?;
                    args.push(CoreArg::ByVal(rhs.clone()));
                    Ok(Some(vec![CoreStmt::Eval(self.early_com_call(
                        member_name,
                        ProjectMemberKind::PropertyLet,
                        &interface_name,
                        &com_member,
                        recv,
                        args,
                    ))]))
                } else {
                    Ok(None)
                }
            }
            DispatchRoute::ExternMember { member, .. } => {
                let writer = self
                    .resolve_member(receiver_ty, member, Some(ProjectMemberKind::PropertyLet))
                    .unwrap_or_else(|| default_binding.clone());
                if let DispatchRoute::ExternMember {
                    member,
                    param_types,
                    ..
                } = writer.route
                {
                    let mut args = self.bind_extern_args(None, &param_types)?;
                    args.push(CoreArg::ByVal(rhs.clone()));
                    Ok(Some(vec![CoreStmt::Eval(self.late_member_call(
                        &member,
                        ProjectMemberKind::PropertyLet,
                        recv,
                        args,
                    ))]))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Arguments for a project `VbaProc`: named args are reordered into their
    /// positional slots by parameter name (linearize binds VbaProc args strictly
    /// positionally), with unfilled slots left `Omitted`.
    pub(crate) fn bind_proc_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        signature: &Signature,
        proc_sym: SymbolId,
    ) -> Result<Vec<CoreArg>, BindError> {
        self.bind_proc_args_inner(arglist, signature, proc_sym, None)
    }

    pub(crate) fn bind_property_put_proc_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        signature: &Signature,
        proc_sym: SymbolId,
    ) -> Result<Vec<CoreArg>, BindError> {
        let n = signature.params.len();
        if n == 0 {
            return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
        }
        self.bind_proc_args_inner(arglist, signature, proc_sym, Some(n - 1))
    }

    fn bind_proc_args_inner(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        signature: &Signature,
        proc_sym: SymbolId,
        reserved_trailing_value_index: Option<usize>,
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
        let bindable_fixed_count = reserved_trailing_value_index
            .filter(|&i| i < fixed_count)
            .unwrap_or(fixed_count);
        let mut slots: Vec<Option<CoreArg>> = (0..fixed_count).map(|_| None).collect();
        let mut tail: Vec<(CoreArg, Option<CorePlace>)> = Vec::new();
        let mut pos = 0usize;
        let mut seen_named = false;
        for item in items {
            match item {
                ArgItem::Positional(expr, passing) => {
                    if seen_named {
                        return Err(BindError::Unsupported(
                            "positional argument cannot follow named argument".into(),
                        ));
                    }
                    if pos < bindable_fixed_count {
                        slots[pos] =
                            Some(self.bind_one_arg(expr, signature.params.get(pos), passing)?);
                    } else if variadic_index.is_some() {
                        // Variadic-tail (ParamArray) element — bound ByVal, no
                        // fixed signature param.
                        let alias = self.paramarray_alias_place(expr, passing);
                        tail.push((self.bind_one_arg(expr, None, passing)?, alias));
                    } else {
                        return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
                    }
                    pos += 1;
                }
                ArgItem::Omitted => {
                    if seen_named {
                        return Err(BindError::Unsupported(
                            "positional argument cannot follow named argument".into(),
                        ));
                    }
                    if pos < bindable_fixed_count {
                        slots[pos] = Some(CoreArg::Omitted);
                    } else if variadic_index.is_some() {
                        tail.push((CoreArg::Omitted, None));
                    } else {
                        return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
                    }
                    pos += 1;
                }
                ArgItem::Named { name, value } => {
                    seen_named = true;
                    let folded = fold_identifier(name.text);
                    match signature
                        .params
                        .iter()
                        .position(|p| fold_identifier(&p.name) == folded)
                    {
                        Some(i) if variadic_index == Some(i) => {
                            return Err(BindError::Unsupported(
                                "named argument to a ParamArray parameter".into(),
                            ));
                        }
                        Some(i) if Some(i) == reserved_trailing_value_index => {
                            return Err(
                                BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment,
                            );
                        }
                        Some(i) if i < bindable_fixed_count => {
                            if slots[i].is_some() {
                                return Err(BindError::Unsupported(format!(
                                    "duplicate argument for parameter {}",
                                    signature.params[i].name
                                )));
                            }
                            // A named arg has no call-site `ByVal`/`ByRef` modifier.
                            slots[i] = Some(self.bind_one_arg(
                                value,
                                signature.params.get(i),
                                CallSitePassing::Default,
                            )?);
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
        // An omitted optional slot (trailing, or an explicit `,`) binds the parameter's
        // default — its folded default expression, else the declared-type zero, else
        // `Missing` (a Variant optional with no default).
        let mut args: Vec<CoreArg> = Vec::with_capacity(fixed_count);
        for (i, slot) in slots.into_iter().enumerate() {
            let param = &signature.params[i];
            if Some(i) == reserved_trailing_value_index {
                if slot.is_some() {
                    return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
                }
                args.push(CoreArg::Omitted);
                continue;
            }
            match slot {
                Some(CoreArg::Omitted) | None if !param.optional => {
                    return Err(BindError::ArgumentNotOptional {
                        parameter: param.name.clone(),
                    });
                }
                Some(CoreArg::Omitted) | None => {
                    args.push(self.omitted_optional_arg(proc_sym, i, param));
                }
                Some(arg) => args.push(arg),
            }
        }
        match variadic_index {
            // Box the variadic tail into one fresh 0-based array for the ParamArray
            // slot (empty tail → an empty array, UBound -1). Doing this in the binder
            // — which has the signature — keeps the call vector one-arg-per-param, so
            // free procs and methods need no downstream variadic handling.
            Some(_) => {
                let mut elems = Vec::with_capacity(tail.len());
                let mut aliases = Vec::with_capacity(tail.len());
                for (arg, alias) in tail {
                    elems.push(paramarray_element(arg));
                    aliases.push(alias);
                }
                // A `ParamArray` slot is always 0-based, regardless of `Option Base`.
                args.push(CoreArg::ByVal(CoreValue::ArrayLiteral {
                    elems,
                    lower_bound: 0,
                    aliases,
                }));
            }
            None => args.extend(tail.into_iter().map(|(arg, _)| arg)),
        }
        Ok(args)
    }

    /// The argument for an omitted optional parameter: its folded default expression
    /// (coerced to the declared type), else the declared-type zero value, else
    /// `Missing` (a `Variant` optional with no default — an `Object` yields `Nothing`).
    fn omitted_optional_arg(&self, proc_sym: SymbolId, index: usize, param: &Param) -> CoreArg {
        if let Some(c) = self.g.env.optional_default(proc_sym, index) {
            return CoreArg::ByVal(types::coerce_store(CoreValue::Const(c.clone()), &param.ty));
        }
        if !param.optional {
            return CoreArg::Omitted;
        }
        match &param.ty {
            VarTypeRef::Variant => CoreArg::Omitted, // the `Missing` marker
            VarTypeRef::Object(_) => CoreArg::ByVal(CoreValue::Const(CoreConst::Nothing)),
            ty => CoreArg::ByVal(types::coerce_store(zero_const(ty), ty)),
        }
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
                ArgItem::Positional(expr, _) => CoreArg::ByVal(self.bind_expr(expr)?.value),
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
            CoreValue::Call {
                callee: CoreCallee::DynamicByName,
                args,
            },
            VarTypeRef::Variant,
        ))
    }

    /// The declared signature of an `Event` symbol (whose `imp` is a plain
    /// `Signature`, like a method), for binding `RaiseEvent` arguments with the
    /// event's ByRef/ByVal parameter directions.
    pub(crate) fn event_signature(&self, sym: SymbolId) -> Option<Signature> {
        match &self.g.env.symbols.symbol(sym)?.imp {
            SymbolImpl::Signature(id) => self.g.env.signatures.get(*id).cloned(),
            _ => None,
        }
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

    pub(crate) fn project_property_accessor_signature(
        &self,
        sym: SymbolId,
        kind: ProjectMemberKind,
        member: &str,
    ) -> Result<Signature, BindError> {
        self.proc_signature_for(sym, kind)
            .ok_or_else(|| missing_project_property_accessor(member, kind))
    }

    pub(crate) fn symbol_display_name(&self, sym: SymbolId) -> Option<String> {
        let name = self.g.env.symbols.symbol(sym)?.name;
        self.g
            .env
            .symbols
            .name(name)
            .map(|n| n.first_spelling.clone())
    }

    /// The per-parameter by-ref flags of a `Declare` symbol (empty if unknown).
    fn declare_param_by_ref(&self, sym: Option<SymbolId>) -> Vec<bool> {
        match sym
            .and_then(|s| self.g.env.symbols.symbol(s))
            .map(|s| &s.imp)
        {
            Some(SymbolImpl::Declare(d)) => d.param_by_ref.clone(),
            _ => Vec::new(),
        }
    }

    /// The per-parameter `As String`-ness of a `Declare` symbol (empty if
    /// unknown) — String params marshal through an ANSI buffer that writes back
    /// even when declared `ByVal` (see [`Self::bind_byval_string_arg`]).
    fn declare_param_is_string(&self, sym: Option<SymbolId>) -> Vec<bool> {
        match sym
            .and_then(|s| self.g.env.symbols.symbol(s))
            .map(|s| &s.imp)
        {
            Some(SymbolImpl::Declare(d)) => d
                .param_types
                .iter()
                .map(|t| matches!(t, oxvba_bundle::DeclareParamType::String))
                .collect(),
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
            && let Some((field, ty)) = err_field(member)
        {
            return Ok(value_bound(CoreValue::ErrField(field), ty));
        }
        // `Module.Member` / `Enum.Member` where the receiver is a namespace
        // qualifier, not a value.
        if let Some(bound) = self.try_module_qualified(node, member, None)? {
            return Ok(bound);
        }
        // A field read of a UDT value (`p.X`) — load the record's fixed-index element.
        if let Some((place, ty)) = self.udt_field_place(node)? {
            return Ok(Bound {
                value: CoreValue::Load(place.clone()),
                ty,
                place: Some(place),
            });
        }
        let recv = self.member_receiver_bound(node)?;
        self.bind_member_value(recv, member)
    }

    /// `Module.Member(args)` or `Enum.Member` where the receiver is a bare
    /// namespace qualifier. Module-qualified calls — emitted by the project
    /// startup shim as `Call Module.Proc()` — and enum-qualified constants both
    /// resolve the member as a qualified project/library name. Binding the
    /// qualifier as a value would fail, or recurse when a procedure has the same
    /// name as its module. Returns `None` when the receiver is not a qualifier (a
    /// leading-dot `With` member, an object receiver, …).
    pub(crate) fn try_module_qualified(
        &mut self,
        node: SyntaxNode<'_>,
        member: &str,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Option<Bound>, BindError> {
        if node.member_has_leading_dot() {
            return Ok(None);
        }
        let Some(parts) = self.qualified_namespace_member_parts(node, member) else {
            return Ok(None);
        };
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        match self.g.env.resolve_qualified(&part_refs) {
            // A qualified member can be a proc (call), a `Const`/`Enum` value, or a
            // module variable (place) — lower it the same way a bare name resolves.
            Some(binding) => Ok(Some(self.finish_value_or_call(member, &binding, arglist)?)),
            None if self.is_namespace_qualifier(&parts[0]) => {
                Err(self.unresolved(member, "member"))
            }
            None => Ok(None),
        }
    }

    fn try_module_qualified_statement(
        &mut self,
        node: SyntaxNode<'_>,
        member: &str,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Option<Bound>, BindError> {
        if node.member_has_leading_dot() {
            return Ok(None);
        }
        let Some(parts) = self.qualified_namespace_member_parts(node, member) else {
            return Ok(None);
        };
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        match self.g.env.resolve_qualified(&part_refs) {
            Some(binding) => Ok(Some(
                self.finish_value_or_call_statement(member, &binding, arglist)?,
            )),
            None if self.is_namespace_qualifier(&parts[0]) => {
                Err(self.unresolved(member, "member"))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn qualified_namespace_member_binding(
        &self,
        node: SyntaxNode<'_>,
        member: &str,
    ) -> Option<Binding> {
        let parts = self.qualified_namespace_member_parts(node, member)?;
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        self.g.env.resolve_qualified(&part_refs)
    }

    fn qualified_namespace_member_parts(
        &self,
        node: SyntaxNode<'_>,
        member: &str,
    ) -> Option<Vec<String>> {
        if node.member_has_leading_dot() {
            return None;
        }
        let recv = node.member_receiver()?;
        let mut parts = Vec::new();
        collect_qualified_member_parts(recv, &mut parts)?;
        if parts.is_empty() || self.resolves_to_local_value(&parts[0]) {
            return None;
        }
        if parts.len() > 1 && self.qualified_receiver_is_value(&parts) {
            return None;
        }
        let mut candidate = parts.clone();
        candidate.push(member.to_string());
        let candidate_refs = candidate.iter().map(String::as_str).collect::<Vec<_>>();
        if !self.is_namespace_qualifier(&parts[0])
            && !self
                .g
                .env
                .resolve_qualified(&candidate_refs)
                .is_some_and(|binding| self.is_cross_surface_namespace_binding(&binding))
        {
            return None;
        }
        parts.push(member.to_string());
        Some(parts)
    }

    fn qualified_receiver_is_value(&self, parts: &[String]) -> bool {
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        let Some(binding) = self.g.env.resolve_qualified(&part_refs) else {
            return false;
        };
        match binding.route {
            DispatchRoute::Value => !self.binding_is_module(&binding),
            DispatchRoute::ConstValue(_)
            | DispatchRoute::PredeclaredObject(_)
            | DispatchRoute::ComObjectRoot { .. } => true,
            DispatchRoute::ProjectMember { kind } => kind == ProjectMemberKind::PropertyGet,
            DispatchRoute::ExternMember {
                kind, has_receiver, ..
            } => !has_receiver && kind == ProjectMemberKind::PropertyGet,
            _ => false,
        }
    }

    fn is_cross_surface_namespace_binding(&self, binding: &Binding) -> bool {
        match &binding.route {
            DispatchRoute::ConstValue(_) => true,
            DispatchRoute::ExternMember { has_receiver, .. } => !*has_receiver,
            _ => false,
        }
    }

    /// Lower an already-resolved name `binding` to a value or call, mirroring the
    /// tail of `bind_ident`: a referenced `ConstValue` or a folded `Const`/`Enum`
    /// member → its literal; a plain variable (`Value` route with a place) → a load;
    /// otherwise a call. Used by the qualified-member path so `Mod1.SomeConst` /
    /// `Mod1.gVar` bind correctly (not only `Mod1.Proc()`).
    fn finish_value_or_call(
        &mut self,
        name: &str,
        binding: &Binding,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        if let DispatchRoute::ConstValue(c) = &binding.route {
            return Ok(value_bound(
                CoreValue::Const(c.clone()),
                crate::expr::const_type(c),
            ));
        }
        if let Some(sym) = binding.symbol
            && let Some(c) = self.g.env.const_value(sym)
        {
            return Ok(value_bound(
                CoreValue::Const(c.clone()),
                crate::expr::const_type(c),
            ));
        }
        if let DispatchRoute::Value = binding.route
            && let Some(sym) = binding.symbol
            && let Some((place, ty)) = self.place_for_symbol(sym)
        {
            return Ok(Bound {
                value: CoreValue::Load(place.clone()),
                ty,
                place: Some(place),
            });
        }
        self.bind_call_route(name, binding, arglist)
    }

    fn finish_value_or_call_statement(
        &mut self,
        name: &str,
        binding: &Binding,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        if let DispatchRoute::ConstValue(c) = &binding.route {
            return Ok(value_bound(
                CoreValue::Const(c.clone()),
                crate::expr::const_type(c),
            ));
        }
        if let Some(sym) = binding.symbol
            && let Some(c) = self.g.env.const_value(sym)
        {
            return Ok(value_bound(
                CoreValue::Const(c.clone()),
                crate::expr::const_type(c),
            ));
        }
        if let DispatchRoute::Value = binding.route
            && let Some(sym) = binding.symbol
            && let Some((place, ty)) = self.place_for_symbol(sym)
        {
            return Ok(Bound {
                value: CoreValue::Load(place.clone()),
                ty,
                place: Some(place),
            });
        }
        self.bind_call_route_statement(name, binding, arglist)
    }

    /// True if `name` is a bare namespace qualifier for `name.Member`: either a
    /// **standard** (`Procedural`) module, the built-in `VBA` library namespace, or
    /// an enum type. Class / Document / Form modules need an instance, so they are
    /// excluded.
    ///
    /// Module-ness is read from the authoritative module list, **not** from `resolve`:
    /// a `Sub`/`Function` of the same name as a module must not block `Module.Member`
    /// (a procedure isn't a value receiver), and `resolve` prioritises the Procedure
    /// namespace over Module — so the entry shim's `Main.Main` (module `Main`, sub
    /// `Main`) would otherwise mis-bind as a self-call. Only a **local/parameter
    /// variable** of the same name shadows the qualifier (`x.Member` is then member
    /// access on that variable's value).
    fn is_namespace_qualifier(&self, name: &str) -> bool {
        let folded = fold_identifier(name);
        let is_vba_namespace = folded == "vba";
        let is_project_name = self.g.env.is_project_name(name);
        let is_proc_module = self.g.env.all_modules().any(|m| {
            fold_identifier(m.module_name) == folded
                && m.module_kind == oxvba_symbol::manifest::ModuleKind::Procedural
        });
        let is_enum = self
            .resolve(name)
            .and_then(|b| b.symbol)
            .and_then(|s| self.g.env.symbols.symbol(s))
            .is_some_and(|s| s.kind == SymbolKind::Enum);
        (is_vba_namespace || is_project_name || is_proc_module || is_enum)
            && !self.resolves_to_local_value(name)
    }

    /// True if `name` resolves to a local or parameter variable in the current scope
    /// (which shadows a same-named module qualifier).
    fn resolves_to_local_value(&self, name: &str) -> bool {
        self.resolve(name)
            .and_then(|b| b.symbol)
            .and_then(|s| self.g.env.symbols.symbol(s))
            .is_some_and(|s| {
                matches!(
                    s.namespace,
                    SymbolNamespace::Local | SymbolNamespace::Parameter
                )
            })
    }

    /// Lower `recv.member` (already-bound receiver, no args) to a value.
    fn bind_member_value(&mut self, recv: Bound, member: &str) -> Result<Bound, BindError> {
        match self.resolve_member(&recv.ty, member, None) {
            Some(binding) => match &binding.route {
                DispatchRoute::Value => {
                    let sym = binding
                        .symbol
                        .ok_or_else(|| self.unresolved(member, "member field"))?;
                    let (place, ty) = self.member_place(recv.value, sym)?;
                    Ok(Bound {
                        value: CoreValue::Load(place.clone()),
                        ty,
                        place: Some(place),
                    })
                }
                DispatchRoute::ProjectMember { kind } => {
                    let kind = *kind;
                    let signature = binding
                        .symbol
                        .and_then(|s| self.proc_signature_for(s, kind));
                    if kind == ProjectMemberKind::Method
                        && signature
                            .as_ref()
                            .is_some_and(|sig| sig.return_type.is_none())
                    {
                        return Err(BindError::ExpectedFunctionOrVariable {
                            name: member.to_string(),
                        });
                    }
                    let ty = signature
                        .and_then(|sig| sig.return_type)
                        .unwrap_or(VarTypeRef::Variant);
                    let dispatch = self.interface_dispatch_name(&recv.ty, member);
                    Ok(value_bound(
                        self.late_member_call(&dispatch, kind, recv.value, Vec::new()),
                        ty,
                    ))
                }
                DispatchRoute::ComMember {
                    member_kind,
                    interface_name,
                    member: com_member,
                    ..
                } => {
                    // A bare member read is a Property Get (or a parameterless
                    // method), never Let/Set: a get/put-sharing member can resolve
                    // to its Let/Set variant by typelib order, so coerce a property
                    // kind to Get for the read.
                    let member_kind = match member_kind {
                        ProjectMemberKind::Method => ProjectMemberKind::Method,
                        _ => ProjectMemberKind::PropertyGet,
                    };
                    let args = self.bind_com_args(None, com_member)?;
                    Ok(value_bound(
                        self.early_com_call(
                            member,
                            member_kind,
                            interface_name,
                            com_member,
                            recv.value,
                            args,
                        ),
                        com_member_return_type(com_member),
                    ))
                }
                // A referenced coclass member: dispatch by name on the receiver,
                // whose `bundle_id` selects the class table in the object's bundle.
                DispatchRoute::ExternMember {
                    member: m, kind, ..
                } => {
                    let kind = match kind {
                        ProjectMemberKind::Method => ProjectMemberKind::Method,
                        _ => ProjectMemberKind::PropertyGet,
                    };
                    Ok(value_bound(
                        self.late_member_call(m, kind, recv.value, Vec::new()),
                        VarTypeRef::Variant,
                    ))
                }
                other => Err(BindError::Unsupported(format!(
                    ".{member} ({other:?} pending)"
                ))),
            },
            // No declared member on an untyped/foreign receiver → late binding.
            None if self.is_late_bound_receiver(&recv.ty) => Ok(value_bound(
                self.late_member_call(
                    member,
                    ProjectMemberKind::PropertyGet,
                    recv.value,
                    Vec::new(),
                ),
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
        self.bind_member_call_inner(member_node, arglist, false)
    }

    fn bind_member_call_statement(
        &mut self,
        member_node: SyntaxNode<'_>,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        self.bind_member_call_inner(member_node, arglist, true)
    }

    fn bind_member_call_inner(
        &mut self,
        member_node: SyntaxNode<'_>,
        arglist: Option<SyntaxNode<'_>>,
        allow_statement_only_sub: bool,
    ) -> Result<Bound, BindError> {
        let member = member_node
            .member_name_token()
            .ok_or_else(|| BindError::Malformed("member call without name".into()))?
            .text;
        // `Module.Member(args)` — a qualified standard-module call (e.g. the startup
        // shim's `Call Module.Proc()`), not member access on a value.
        let qualified = if allow_statement_only_sub {
            self.try_module_qualified_statement(member_node, member, arglist)?
        } else {
            self.try_module_qualified(member_node, member, arglist)?
        };
        if let Some(bound) = qualified {
            return Ok(bound);
        }
        let recv = self.member_receiver_bound(member_node)?;
        // A field of a UDT receiver (`o.Lines(i)` / `o.field`): a fixed-index
        // record element, optionally indexed when the field is an array. This
        // is the value/read counterpart of `udt_field_place`, and is what makes
        // nested UDT arrays (`o.Lines(i).Text`) resolve — the receiver of the
        // outer `.Text` is the typed element of the inner array index.
        if let VarTypeRef::Udt(udt) = &recv.ty
            && let Some((index, field_ty)) = self
                .g
                .env
                .udt_field(udt, member)
                .map(|(i, t)| (i, t.clone()))
        {
            let base = match &recv.value {
                CoreValue::Load(p) => p.clone(),
                _ => {
                    return Err(BindError::Unsupported(format!(
                        "UDT field `.{member}` on a non-place receiver"
                    )));
                }
            };
            let field_place = CorePlace::RecordField {
                base: Box::new(base),
                index,
            };
            let field_ty = self.g.resolve_udt_type(field_ty);
            return match arglist {
                Some(a) => {
                    let elem_ty = match field_ty {
                        VarTypeRef::Array(inner) => self.g.resolve_udt_type(*inner),
                        VarTypeRef::FixedArray { element, .. } => self.g.resolve_udt_type(*element),
                        _ => {
                            return Err(BindError::ExpectedArray {
                                name: format!(".{member}"),
                            });
                        }
                    };
                    let indices = self.bind_positional_values(a)?;
                    let place = CorePlace::Index {
                        array: Box::new(field_place),
                        indices,
                    };
                    Ok(Bound {
                        value: CoreValue::Load(place.clone()),
                        ty: elem_ty,
                        place: Some(place),
                    })
                }
                None => Ok(Bound {
                    value: CoreValue::Load(field_place.clone()),
                    ty: field_ty,
                    place: Some(field_place),
                }),
            };
        }
        match self.resolve_member(&recv.ty, member, None) {
            Some(binding) => match &binding.route {
                DispatchRoute::ProjectMember { kind } => {
                    let kind = *kind;
                    let member_sym = binding.symbol;
                    // Use the method's signature so ByRef params alias the caller
                    // (and named args reorder into positional slots for dispatch).
                    let signature = member_sym.and_then(|s| self.proc_signature_for(s, kind));
                    let method_args = match (&signature, member_sym) {
                        (Some(sig), Some(s)) => self.bind_proc_args(arglist, sig, s)?,
                        _ => self.bind_args(arglist, None)?,
                    };
                    if !allow_statement_only_sub
                        && kind == ProjectMemberKind::Method
                        && signature
                            .as_ref()
                            .is_some_and(|sig| sig.return_type.is_none())
                    {
                        return Err(BindError::ExpectedFunctionOrVariable {
                            name: member.to_string(),
                        });
                    }
                    let ty = signature
                        .and_then(|s| s.return_type)
                        .unwrap_or(VarTypeRef::Variant);
                    let dispatch = self.interface_dispatch_name(&recv.ty, member);
                    Ok(value_bound(
                        self.late_member_call(&dispatch, kind, recv.value, method_args),
                        ty,
                    ))
                }
                DispatchRoute::Value => {
                    // `recv.field(i)` — index into a member array.
                    let sym = binding
                        .symbol
                        .ok_or_else(|| self.unresolved(member, "member array"))?;
                    let (field_place, _ty) = self.member_place(recv.value, sym)?;
                    let indices = match arglist {
                        Some(a) => self.bind_positional_values(a)?,
                        None => Vec::new(),
                    };
                    let place = CorePlace::Index {
                        array: Box::new(field_place),
                        indices,
                    };
                    Ok(Bound {
                        value: CoreValue::Load(place.clone()),
                        ty: VarTypeRef::Variant,
                        place: Some(place),
                    })
                }
                DispatchRoute::ComMember {
                    member_kind,
                    interface_name,
                    member: com_member,
                    ..
                } => {
                    // A member call in VALUE context is a read: a property is fetched
                    // through Property Get, never Let/Set. A get/put/putref-sharing
                    // member (e.g. a dictionary's `Item`) can resolve to its Let/Set
                    // variant by typelib order; coerce a property kind to Get so the
                    // read dispatches PROPERTYGET, not a (write) PROPERTYPUT(REF).
                    let member_kind = match member_kind {
                        ProjectMemberKind::Method => ProjectMemberKind::Method,
                        _ => ProjectMemberKind::PropertyGet,
                    };
                    // Emit descriptor-ordered args; typed COM named args are
                    // validated/reordered here, while ByRef still aliases from the
                    // typelib's [out]/[in,out] parameter type.
                    let method_args = match self.bind_com_args(arglist, com_member) {
                        Ok(args) => args,
                        Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment)
                            if arglist.is_some()
                                && member_kind == ProjectMemberKind::PropertyGet
                                && visible_com_param_indices(com_member).is_empty() =>
                        {
                            return self.bind_com_property_get_default_member_call(
                                member,
                                interface_name,
                                com_member,
                                recv.value,
                                arglist,
                            );
                        }
                        Err(err) => return Err(err),
                    };
                    Ok(value_bound(
                        self.early_com_call(
                            member,
                            member_kind,
                            interface_name,
                            com_member,
                            recv.value,
                            method_args,
                        ),
                        com_member_return_type(com_member),
                    ))
                }
                // A referenced coclass member call: coerce args to the published
                // param types, dispatch by name in the object's bundle.
                DispatchRoute::ExternMember {
                    member: m,
                    kind,
                    param_types,
                    ..
                } => {
                    // A value-context member call is a read: coerce a property
                    // Let/Set kind to Get (a get/put-sharing member could resolve
                    // to its writer variant by surface order).
                    let kind = match kind {
                        ProjectMemberKind::Method => ProjectMemberKind::Method,
                        _ => ProjectMemberKind::PropertyGet,
                    };
                    let method_args = self.bind_extern_args(arglist, param_types)?;
                    Ok(value_bound(
                        self.late_member_call(m, kind, recv.value, method_args),
                        VarTypeRef::Variant,
                    ))
                }
                other => Err(BindError::Unsupported(format!(
                    ".{member}(...) ({other:?} pending)"
                ))),
            },
            None if self.is_late_bound_receiver(&recv.ty) => {
                let method_args = self.bind_late_dispatch_args(arglist)?;
                Ok(value_bound(
                    self.late_member_call(
                        member,
                        ProjectMemberKind::Method,
                        recv.value,
                        method_args,
                    ),
                    VarTypeRef::Variant,
                ))
            }
            None => Err(self.unresolved(member, "member call")),
        }
    }

    fn bind_com_property_get_default_member_call(
        &mut self,
        property_name: &str,
        interface_name: &str,
        property_member: &TypeLibMemberMetadata,
        recv: CoreValue,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Bound, BindError> {
        let property_args = self.bind_com_args(None, property_member)?;
        let property_value = self.early_com_call(
            property_name,
            ProjectMemberKind::PropertyGet,
            interface_name,
            property_member,
            recv,
            property_args,
        );
        let property_ty = com_member_return_type(property_member);
        let Some(default_binding) = self.resolve_default_member(&property_ty) else {
            return Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment);
        };
        match default_binding.route {
            DispatchRoute::ComMember {
                member_name,
                member_kind,
                interface_name,
                member: default_member,
                ..
            } => {
                let member_kind = match member_kind {
                    ProjectMemberKind::Method => ProjectMemberKind::Method,
                    _ => ProjectMemberKind::PropertyGet,
                };
                let args = self.bind_com_args(arglist, &default_member)?;
                Ok(value_bound(
                    self.early_com_call(
                        &member_name,
                        member_kind,
                        &interface_name,
                        &default_member,
                        property_value,
                        args,
                    ),
                    com_member_return_type(&default_member),
                ))
            }
            _ => Err(BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment),
        }
    }

    /// The `CorePlace` for an instance field / WithEvents field member symbol.
    pub(crate) fn member_place(
        &self,
        recv: CoreValue,
        sym: SymbolId,
    ) -> Result<(CorePlace, VarTypeRef), BindError> {
        if let Some(&field) = self.g.ids.field_token_of.get(&sym) {
            return Ok((
                CorePlace::Field {
                    object: Box::new(recv),
                    field,
                },
                self.symbol_type(sym),
            ));
        }
        if let Some(&binding) = self.g.ids.withevents_binding_of.get(&sym) {
            return Ok((
                CorePlace::WithEvents {
                    owner: Box::new(recv),
                    binding,
                },
                self.symbol_type(sym),
            ));
        }
        Err(BindError::Unsupported(
            "member field without an instance token".into(),
        ))
    }

    /// The member name to dispatch through. When the static receiver type is a
    /// project interface (some class `Implements` it — in **this** project or a
    /// **referenced** one), the call resolves to the mangled `Interface_Member`
    /// implementation — identical across every implementing class, so runtime name
    /// dispatch stays polymorphic (and routes by name in the object's own bundle).
    pub(crate) fn interface_dispatch_name(&self, recv_ty: &VarTypeRef, member: &str) -> String {
        if let VarTypeRef::Object(name) = recv_ty
            && self.is_project_interface(name)
        {
            // Mangle with the bare interface name (a referenced type may be dotted).
            let bare = name.rsplit('.').next().unwrap_or(name);
            return format!("{bare}_{member}");
        }
        member.to_string()
    }

    /// True if `name` is an interface type of the active project or any referenced
    /// project (a class some class `Implements`). A referenced type may be written
    /// project-qualified (`Lib.IShape`); only the trailing type name is matched.
    fn is_project_interface(&self, name: &str) -> bool {
        let bare = name.rsplit('.').next().unwrap_or(name);
        let folded = fold_identifier(bare);
        if self.g.ids.interfaces.contains(&folded) {
            return true;
        }
        self.g
            .env
            .export_surfaces()
            .iter()
            .any(|s| s.interfaces.iter().any(|i| fold_identifier(i) == folded))
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
            callee: CoreCallee::LateDispatch {
                name: name.to_string(),
                kind: Some(kind),
                default_member: false,
            },
            args,
        }
    }

    /// Build a default-member dispatch (`recv(args)`), receiver as `args[0]`.
    pub(crate) fn late_default_member_call(
        &self,
        kind: ProjectMemberKind,
        recv: CoreValue,
        mut method_args: Vec<CoreArg>,
    ) -> CoreValue {
        let mut args = vec![CoreArg::ByVal(recv)];
        args.append(&mut method_args);
        CoreValue::Call {
            callee: CoreCallee::LateDispatch {
                name: String::new(),
                kind: Some(kind),
                default_member: true,
            },
            args,
        }
    }

    /// Build an early-bound COM dispatch (`recv.member`), receiver arg0. `name`/`kind`
    /// are the **call-site** selector name + dispatch accessor (a value read coerces
    /// `kind` to `PropertyGet`; a default-member access passes the receiver label as
    /// `name`); `interface_name` is the declared receiver COM type; `member` is the
    /// full canonical typed descriptor the de-erased `EarlyCom` carries into OxIR (the
    /// dispid is `member.token`). Cloned once here so the ~8 call sites can pass the
    /// route's field by reference regardless of whether the route is owned or borrowed.
    pub(crate) fn early_com_call(
        &self,
        name: &str,
        kind: ProjectMemberKind,
        interface_name: &str,
        member: &TypeLibMemberMetadata,
        recv: CoreValue,
        mut method_args: Vec<CoreArg>,
    ) -> CoreValue {
        let mut args = vec![CoreArg::ByVal(recv)];
        args.append(&mut method_args);
        CoreValue::Call {
            callee: CoreCallee::EarlyCom {
                name: name.to_string(),
                kind: Some(kind),
                interface_name: interface_name.to_string(),
                member: Box::new(member.clone()),
            },
            args,
        }
    }

    /// True if a receiver should fall back to late binding when a member doesn't
    /// resolve: an untyped `Variant`, or a plain `Object`. A missing member on a
    /// known project, referenced-project, host, or COM type stays an error.
    pub(crate) fn is_late_bound_receiver(&self, ty: &VarTypeRef) -> bool {
        match ty {
            VarTypeRef::Variant => true,
            VarTypeRef::Object(name) => {
                let folded = fold_identifier(name);
                folded == "object"
                    || (!self.g.ids.class_of.contains_key(&folded)
                        && !self.g.env.is_known_object_type(name))
            }
            _ => false,
        }
    }

    /// The declared return type of a project member (for inference); `Variant`
    /// when unknown.
    pub(crate) fn member_return_type(
        &self,
        sym: Option<SymbolId>,
        kind: ProjectMemberKind,
    ) -> VarTypeRef {
        let ty = sym
            .and_then(|s| self.proc_signature_for(s, kind))
            .and_then(|s| s.return_type)
            .unwrap_or(VarTypeRef::Variant);
        self.g.resolve_udt_type(ty)
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

    /// True if `recv` denotes the predeclared `Debug` object.
    pub(crate) fn is_debug_receiver(&self, recv: SyntaxNode<'_>) -> bool {
        recv.kind() == SyntaxKind::IdentExpr
            && recv.ident_name_token().is_some_and(|t| {
                matches!(
                    self.resolve(t.text).map(|b| b.route),
                    Some(DispatchRoute::PredeclaredObject(PredeclaredObjectId::Debug))
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
                self.bind_call_route_statement(name, &binding, arglist)
            }
            // `Call Foo(a, b)` — the whole `Foo(a, b)` is the callee (an IndexExpr).
            SyntaxKind::IndexExpr => self.bind_index_callee_statement(callee),
            // `obj.Method` / `.Method` in statement position (no parenthesised args).
            SyntaxKind::MemberExpr => self.bind_member_call_statement(callee, arglist),
            other => Err(BindError::Unsupported(format!("call statement {other:?}"))),
        }
    }

    fn bind_index_callee_statement(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let base = node
            .index_base()
            .ok_or_else(|| BindError::Malformed("call index without base".into()))?;
        if base.kind() == SyntaxKind::IdentExpr
            && let Some(tok) = base.ident_name_token()
            && let Some(binding) = self.resolve_suffixed(base, tok.text)
            && !matches!(binding.route, DispatchRoute::Value)
        {
            return self.bind_call_route_statement(tok.text, &binding, node.index_arg_list());
        }
        if base.kind() == SyntaxKind::MemberExpr {
            return self.bind_member_call_statement(base, node.index_arg_list());
        }
        self.bind_index_or_call(node)
    }
}

/// Canonicalize storage-equivalent declarations before enforcing exact ByRef
/// type matching.
fn canonical_byref_type(ty: VarTypeRef) -> VarTypeRef {
    match ty {
        VarTypeRef::FixedString(_) => VarTypeRef::Builtin(BuiltinType::String),
        VarTypeRef::Array(inner) => VarTypeRef::Array(Box::new(canonical_byref_type(*inner))),
        VarTypeRef::FixedArray { element, .. } => {
            VarTypeRef::Array(Box::new(canonical_byref_type(*element)))
        }
        other => other,
    }
}

fn byref_variant_accepts_array(expected: &VarTypeRef, actual: &VarTypeRef) -> bool {
    matches!(expected, VarTypeRef::Variant)
        && matches!(actual, VarTypeRef::Array(_) | VarTypeRef::FixedArray { .. })
}

fn ensure_byval_type_compatible(
    expected: &VarTypeRef,
    actual: &VarTypeRef,
) -> Result<(), BindError> {
    if matches!(expected, VarTypeRef::Object(_))
        && !matches!(actual, VarTypeRef::Object(_) | VarTypeRef::Variant)
    {
        return Err(BindError::TypeMismatch);
    }
    Ok(())
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

/// Place `val` at the native-call argument `index`, unless the caller already
/// supplied a real value there. Intermediate slots are padded with
/// `CoreArg::Omitted` — the lib's optional readers treat that Missing sentinel as
/// absent — so `val` lands exactly at `index` and omitted middles still default.
fn set_trailing_arg(args: &mut Vec<CoreArg>, index: usize, val: CoreArg) {
    while args.len() <= index {
        args.push(CoreArg::Omitted);
    }
    if matches!(args[index], CoreArg::Omitted) {
        args[index] = val;
    }
}

/// The zero/empty constant seed for a declared scalar type's default (an omitted
/// `Optional … As <type>` with no explicit default). The caller coerces it to `ty`
/// (`I32(0)` → `Long(0)`/`Boolean(False)`/`Currency(0)`/`Date(0)`; `""` for strings).
fn zero_const(ty: &VarTypeRef) -> CoreValue {
    let c = match ty {
        VarTypeRef::Builtin(BuiltinType::String) | VarTypeRef::FixedString(_) => {
            CoreConst::Str(String::new())
        }
        _ => CoreConst::I32(0),
    };
    CoreValue::Const(c)
}

/// The `PtrKind` for a pointer-helper over an operand of static type `ty`. `StrPtr`
/// and `ObjPtr` are fixed; `VarPtr` distinguishes a String/fixed-string variable
/// (the BSTR cell) and a Variant variable (the VARIANT cell) from scalar/array
/// storage. Shared by the value binding (L2) and the Declare write-back detection (L3).
fn pointer_kind(intrinsic: StructuralIntrinsic, ty: &VarTypeRef) -> PtrKind {
    match intrinsic {
        StructuralIntrinsic::StrPtr => PtrKind::Str,
        StructuralIntrinsic::ObjPtr => PtrKind::Obj,
        StructuralIntrinsic::VarPtr => match ty {
            VarTypeRef::Builtin(BuiltinType::String) | VarTypeRef::FixedString(_) => {
                PtrKind::VarString
            }
            VarTypeRef::Variant => PtrKind::VarVariant,
            _ => PtrKind::Var,
        },
        // Not a pointer helper — callers restrict to the three above; default to
        // scalar storage defensively.
        _ => PtrKind::Var,
    }
}

fn scalar_ptr_writeback_kind(ty: &VarTypeRef) -> Option<PtrWritebackKind> {
    match ty {
        VarTypeRef::Builtin(BuiltinType::Boolean) => Some(PtrWritebackKind::Boolean),
        VarTypeRef::Builtin(BuiltinType::Byte) => Some(PtrWritebackKind::Byte),
        VarTypeRef::Builtin(BuiltinType::Integer) => Some(PtrWritebackKind::Integer),
        VarTypeRef::Builtin(BuiltinType::Long) => Some(PtrWritebackKind::Long),
        VarTypeRef::Builtin(BuiltinType::LongLong) => Some(PtrWritebackKind::LongLong),
        VarTypeRef::Builtin(BuiltinType::LongPtr) => Some(PtrWritebackKind::LongPtr),
        VarTypeRef::Builtin(BuiltinType::Single) => Some(PtrWritebackKind::Single),
        VarTypeRef::Builtin(BuiltinType::Double) => Some(PtrWritebackKind::Double),
        VarTypeRef::Builtin(BuiltinType::Currency) => Some(PtrWritebackKind::Currency),
        VarTypeRef::Builtin(BuiltinType::Date) => Some(PtrWritebackKind::Date),
        _ => None,
    }
}

/// Map a published typelib Automation type to the binder's `VarTypeRef`.
/// Scalars drive narrowing `Coerce` nodes for ByVal arguments; generic COM
/// object returns stay `Object`, which remains true late binding unless wire
/// metadata gives a specific interface name.
fn tlb_param_to_vartype(p: &TypeLibParamType) -> VarTypeRef {
    use TypeLibParamType as T;
    match p {
        T::Boolean | T::ByRefBoolean => VarTypeRef::Builtin(BuiltinType::Boolean),
        T::Byte | T::ByRefByte => VarTypeRef::Builtin(BuiltinType::Byte),
        T::Integer | T::ByRefInteger => VarTypeRef::Builtin(BuiltinType::Integer),
        T::Long | T::ByRefLong => VarTypeRef::Builtin(BuiltinType::Long),
        T::LongLong | T::ByRefLongLong => VarTypeRef::Builtin(BuiltinType::LongLong),
        T::LongPtr | T::ByRefLongPtr => VarTypeRef::Builtin(BuiltinType::LongPtr),
        T::Single | T::ByRefSingle => VarTypeRef::Builtin(BuiltinType::Single),
        T::Double | T::ByRefDouble => VarTypeRef::Builtin(BuiltinType::Double),
        T::Currency | T::ByRefCurrency => VarTypeRef::Builtin(BuiltinType::Currency),
        T::Date | T::ByRefDate => VarTypeRef::Builtin(BuiltinType::Date),
        T::String | T::ByRefString => VarTypeRef::Builtin(BuiltinType::String),
        T::Object | T::ByRefObject => VarTypeRef::Object("Object".to_string()),
        _ => VarTypeRef::Variant,
    }
}

pub(crate) fn com_member_return_type(member: &TypeLibMemberMetadata) -> VarTypeRef {
    if let Some(TypeLibWireType::InterfacePointer { name }) = &member.return_wire_type
        && !name.is_empty()
    {
        return VarTypeRef::Object(name.clone());
    }
    member
        .return_type
        .as_ref()
        .map(tlb_param_to_vartype)
        .unwrap_or(VarTypeRef::Variant)
}

fn visible_com_param_indices(member: &TypeLibMemberMetadata) -> Vec<usize> {
    (0..member.parameter_types.len())
        .filter(|&i| !com_param_is_lcid(member, i))
        .collect()
}

fn com_member_paramarray_index(member: &TypeLibMemberMetadata) -> Option<usize> {
    member
        .parameter_optional_defaults
        .len()
        .checked_sub(1)
        .filter(|&index| {
            matches!(
                member.parameter_optional_defaults.get(index),
                Some(OptionalParamDefault::ParamArray)
            )
        })
}

fn com_param_is_lcid(member: &TypeLibMemberMetadata, index: usize) -> bool {
    member.parameter_optional_defaults.len() == member.parameter_types.len()
        && matches!(
            member.parameter_optional_defaults.get(index),
            Some(OptionalParamDefault::Lcid)
        )
}

fn com_param_is_optional(member: &TypeLibMemberMetadata, index: usize) -> bool {
    if member
        .parameter_optional
        .get(index)
        .copied()
        .unwrap_or(false)
    {
        return true;
    }
    if member.parameter_optional_defaults.len() != member.parameter_types.len() {
        return false;
    }
    matches!(
        member.parameter_optional_defaults.get(index),
        Some(
            OptionalParamDefault::HasDefault(_)
                | OptionalParamDefault::OptionalVariant
                | OptionalParamDefault::OptionalNoDefault
                | OptionalParamDefault::ParamArray
        )
    )
}

fn com_param_display_name(member: &TypeLibMemberMetadata, index: usize) -> String {
    member
        .parameter_names
        .get(index)
        .filter(|name| !name.is_empty())
        .cloned()
        .unwrap_or_else(|| format!("arg{}", index + 1))
}

/// The `ErrField` (and its type) for an `Err.<member>` read.
pub(crate) fn err_field(member: &str) -> Option<(ErrField, VarTypeRef)> {
    match fold_identifier(member).as_str() {
        "number" => Some((ErrField::Number, builtin(BuiltinType::Long))),
        "description" => Some((ErrField::Description, builtin(BuiltinType::String))),
        "source" => Some((ErrField::Source, builtin(BuiltinType::String))),
        "helpfile" => Some((ErrField::HelpFile, builtin(BuiltinType::String))),
        "helpcontext" => Some((ErrField::HelpContext, builtin(BuiltinType::Long))),
        "lastdllerror" => Some((ErrField::LastDllError, builtin(BuiltinType::Long))),
        _ => None,
    }
}

fn collect_qualified_member_parts(node: SyntaxNode<'_>, out: &mut Vec<String>) -> Option<()> {
    match node.kind() {
        SyntaxKind::IdentExpr => {
            out.push(node.ident_name_token()?.text.to_string());
            Some(())
        }
        SyntaxKind::MemberExpr if !node.member_has_leading_dot() => {
            collect_qualified_member_parts(node.member_receiver()?, out)?;
            out.push(node.member_name_token()?.text.to_string());
            Some(())
        }
        _ => None,
    }
}

fn missing_project_property_accessor(member: &str, kind: ProjectMemberKind) -> BindError {
    let accessor = match kind {
        ProjectMemberKind::PropertyGet => "Property Get",
        ProjectMemberKind::PropertyLet => "Property Let",
        ProjectMemberKind::PropertySet => "Property Set",
        ProjectMemberKind::Method => "method",
    };
    BindError::InvalidAssignment(format!("property `{member}` has no {accessor} accessor"))
}
