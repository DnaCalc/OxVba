//! The call binder: maps a resolved `DispatchRoute` to a `CoreCallee` (project
//! proc / native / `Declare` / special form), binds arguments (positional /
//! named / omitted, ByVal coercion, ByRef aliasing), and lowers member access
//! (the `Err` object for now; objects/COM arrive in a later phase).

use oxvba_bundle::coreir::{
    BoundWhich, CoreArg, CoreCallee, CoreConst, CorePlace, CoreStmt, CoreValue, ErrField, PtrKind,
    PtrWriteback, PtrWritebackKind,
};
use oxvba_bundle::native::NativeImplId;
use oxvba_bundle::{BundleImport, ExportToken, ProjectMemberKind};
use oxvba_com::TypeLibParamType;
use oxvba_symbol::binding::{Binding, DispatchRoute, SpecialForm};
use oxvba_symbol::model::{
    PredeclaredObjectId, SymbolId, SymbolImpl, SymbolNamespace, fold_identifier,
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
        match &binding.route {
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
                let args = self.bind_extern_proc_args(arglist, param_types, param_names)?;
                Ok(value_bound(
                    CoreValue::Call {
                        callee: CoreCallee::ExternProc { import },
                        args,
                    },
                    VarTypeRef::Variant,
                ))
            }
            DispatchRoute::Native(id) => {
                let args = self.bind_args(arglist, None)?;
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
                    CoreValue::ArrayLiteral(items),
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
                    .ok_or_else(|| {
                        BindError::Malformed(format!("`{name}` requires an array argument"))
                    })?;
                let expr = match first {
                    ArgItem::Positional(e, _) => e,
                    _ => return Err(BindError::Malformed(format!("`{name}` array argument"))),
                };
                let (place, _) = self.bind_place(expr)?;
                Ok(value_bound(
                    CoreValue::Bound {
                        which,
                        array: Box::new(place),
                    },
                    builtin(BuiltinType::Long),
                ))
            }
            DispatchRoute::SpecialForm(SpecialForm::CallByName) => self.bind_callbyname(arglist),
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
            Some(sig) => self.bind_proc_args(arglist, sig, sym)?,
            None => self.bind_args(arglist, None)?,
        };
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
            && let Ok((place, _)) = self.bind_place(expr)
        {
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
                    args.push(CoreArg::Named {
                        name: name.text.to_string(),
                        value: self.bind_expr(value)?.value,
                    });
                }
                ArgItem::Positional(expr, passing) => {
                    args.push(self.bind_arg_byref(
                        expr,
                        param_by_ref.get(i).copied().unwrap_or(false),
                        passing,
                    )?);
                }
            }
        }
        Ok(args)
    }

    /// Bind `Declare` arguments (ByRef-aware, like [`Self::bind_args_byref`]) and
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
                    _ => None,
                }
            } else {
                None
            };
            return Ok((kind, CoreValue::Load(place), writeback));
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
    /// (unfilled slots left `Omitted`) — exactly as a same-bundle `VbaProc` call.
    /// Without this, an out-of-order named call (`Lib.F(b:=2, a:=1)`) would pass
    /// arguments in source order.
    fn bind_extern_proc_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        param_types: &[TypeLibParamType],
        param_names: &[String],
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
                    let arg = self.bind_extern_one(expr, param_types.get(pos), passing)?;
                    if pos < n {
                        slots[pos] = Some(arg)
                    } else {
                        extra.push(arg)
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
                    } else {
                        extra.push(CoreArg::Omitted)
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
            .map(|s| s.unwrap_or(CoreArg::Omitted))
            .collect();
        args.extend(extra);
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
        let mut args = self.bind_proc_args(target.index_arg_list(), &signature, sym)?;
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
                let mut args = self.bind_proc_args(arglist, &signature, sym)?;
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
                        dispid,
                        param_by_ref,
                        ..
                    },
                ..
            }) => {
                let mut args = self.bind_args_byref(arglist, &param_by_ref)?;
                args.push(CoreArg::ByVal(rhs.clone()));
                Ok(Some(vec![CoreStmt::Eval(
                    self.early_com_call(dispid, kind, recv.value, args),
                )]))
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
                let mut args = self.bind_args(arglist, None)?;
                args.push(CoreArg::ByVal(rhs.clone()));
                Ok(Some(vec![CoreStmt::Eval(
                    self.late_member_call(member, kind, recv.value, args),
                )]))
            }
            // A project member or a plain field/method member → a place store path.
            _ => Ok(None),
        }
    }

    /// Arguments for a project `VbaProc`: named args are reordered into their
    /// positional slots by parameter name (linearize binds VbaProc args strictly
    /// positionally), with unfilled slots left `Omitted`.
    fn bind_proc_args(
        &mut self,
        arglist: Option<SyntaxNode<'_>>,
        signature: &Signature,
        proc_sym: SymbolId,
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
                        slots[pos] =
                            Some(self.bind_one_arg(expr, signature.params.get(pos), passing)?);
                    } else {
                        // Variadic-tail (ParamArray) element, or an extra positional
                        // when there is no ParamArray — bound ByVal, no signature param.
                        tail.push(self.bind_one_arg(expr, None, passing)?);
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
                    } else {
                        tail.push(CoreArg::Omitted);
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
                        Some(i) if i < fixed_count => {
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
        let mut args: Vec<CoreArg> = slots
            .into_iter()
            .enumerate()
            .map(|(i, s)| match s {
                Some(CoreArg::Omitted) | None => {
                    self.omitted_optional_arg(proc_sym, i, &signature.params[i])
                }
                Some(arg) => arg,
            })
            .collect();
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
        // `Module.Member` where `Module` is a standard module — a namespace qualifier,
        // not a value.
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

    /// `Module.Member(args)` where `Module` is a bare identifier naming a standard
    /// module (a namespace qualifier). VBA module-qualified calls — emitted by the
    /// project startup shim as `Call Module.Proc()` — must resolve the member as a
    /// qualified project member; binding the module name as a value would fail (or,
    /// when a same-named proc exists, recurse). Returns `None` when the receiver is
    /// not a module qualifier (a leading-dot `With` member, an object receiver, …).
    fn try_module_qualified(
        &mut self,
        node: SyntaxNode<'_>,
        member: &str,
        arglist: Option<SyntaxNode<'_>>,
    ) -> Result<Option<Bound>, BindError> {
        if node.member_has_leading_dot() {
            return Ok(None);
        }
        let Some(recv) = node.member_receiver() else {
            return Ok(None);
        };
        if recv.kind() != SyntaxKind::IdentExpr {
            return Ok(None);
        }
        let Some(tok) = recv.ident_name_token() else {
            return Ok(None);
        };
        if !self.is_module_qualifier(tok.text) {
            return Ok(None);
        }
        match self.g.env.resolve_qualified(&[tok.text, member]) {
            // A qualified member can be a proc (call), a `Const`/`Enum` value, or a
            // module variable (place) — lower it the same way a bare name resolves.
            Some(binding) => Ok(Some(self.finish_value_or_call(member, &binding, arglist)?)),
            None => Ok(None),
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

    /// True if `name` is a **standard** (`Procedural`) module — a free-call namespace
    /// qualifier, so `name.Member` is a module-qualified call. Class / Document / Form
    /// modules need an instance, so they are excluded.
    ///
    /// Module-ness is read from the authoritative module list, **not** from `resolve`:
    /// a `Sub`/`Function` of the same name as a module must not block `Module.Member`
    /// (a procedure isn't a value receiver), and `resolve` prioritises the Procedure
    /// namespace over Module — so the entry shim's `Main.Main` (module `Main`, sub
    /// `Main`) would otherwise mis-bind as a self-call. Only a **local/parameter
    /// variable** of the same name shadows the qualifier (`x.Member` is then member
    /// access on that variable's value).
    fn is_module_qualifier(&self, name: &str) -> bool {
        let folded = fold_identifier(name);
        let is_proc_module = self.g.env.all_modules().any(|m| {
            fold_identifier(m.module_name) == folded
                && m.module_kind == oxvba_symbol::manifest::ModuleKind::Procedural
        });
        is_proc_module && !self.resolves_to_local_value(name)
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
                    let ty = self.member_return_type(binding.symbol, kind);
                    let dispatch = self.interface_dispatch_name(&recv.ty, member);
                    Ok(value_bound(
                        self.late_member_call(&dispatch, kind, recv.value, Vec::new()),
                        ty,
                    ))
                }
                DispatchRoute::ComMember {
                    dispid,
                    member_kind,
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
                    Ok(value_bound(
                        self.early_com_call(*dispid, member_kind, recv.value, Vec::new()),
                        VarTypeRef::Variant,
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
        // `Module.Member(args)` — a qualified standard-module call (e.g. the startup
        // shim's `Call Module.Proc()`), not member access on a value.
        if let Some(bound) = self.try_module_qualified(member_node, member, arglist)? {
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
                    let indices = self.bind_positional_values(a)?;
                    let elem_ty = match field_ty {
                        VarTypeRef::Array(inner) => self.g.resolve_udt_type(*inner),
                        _ => VarTypeRef::Variant,
                    };
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
                    dispid,
                    member_kind,
                    param_by_ref,
                    ..
                } => {
                    let dispid = *dispid;
                    // A member call in VALUE context is a read: a property is fetched
                    // through Property Get, never Let/Set. A get/put/putref-sharing
                    // member (e.g. a dictionary's `Item`) can resolve to its Let/Set
                    // variant by typelib order; coerce a property kind to Get so the
                    // read dispatches PROPERTYGET, not a (write) PROPERTYPUT(REF).
                    let member_kind = match member_kind {
                        ProjectMemberKind::Method => ProjectMemberKind::Method,
                        _ => ProjectMemberKind::PropertyGet,
                    };
                    // Emit ByRef for the typelib's [out]/[in,out] params.
                    let by_ref = param_by_ref.clone();
                    let method_args = self.bind_args_byref(arglist, &by_ref)?;
                    Ok(value_bound(
                        self.early_com_call(dispid, member_kind, recv.value, method_args),
                        VarTypeRef::Variant,
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
                let method_args = self.bind_args(arglist, None)?;
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
            },
            args,
        }
    }

    /// Build an early-bound COM dispatch (`recv.member` by dispid), receiver arg0.
    pub(crate) fn early_com_call(
        &self,
        dispid: i32,
        kind: ProjectMemberKind,
        recv: CoreValue,
        mut method_args: Vec<CoreArg>,
    ) -> CoreValue {
        let mut args = vec![CoreArg::ByVal(recv)];
        args.append(&mut method_args);
        CoreValue::Call {
            callee: CoreCallee::EarlyCom {
                dispid,
                kind: Some(kind),
            },
            args,
        }
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

/// Map a published typelib parameter type to the `VarTypeRef` used for ByVal
/// argument coercion. Only the scalar value types matter (those drive a narrowing
/// `Coerce`); `ByRef*` params never coerce, and object/variant/decimal map to
/// `Variant` (no coercion node).
fn tlb_param_to_vartype(p: &TypeLibParamType) -> VarTypeRef {
    use TypeLibParamType as T;
    match p {
        T::Boolean => VarTypeRef::Builtin(BuiltinType::Boolean),
        T::Byte => VarTypeRef::Builtin(BuiltinType::Byte),
        T::Integer => VarTypeRef::Builtin(BuiltinType::Integer),
        T::Long => VarTypeRef::Builtin(BuiltinType::Long),
        T::LongLong => VarTypeRef::Builtin(BuiltinType::LongLong),
        T::LongPtr => VarTypeRef::Builtin(BuiltinType::LongPtr),
        T::Single => VarTypeRef::Builtin(BuiltinType::Single),
        T::Double => VarTypeRef::Builtin(BuiltinType::Double),
        T::Currency => VarTypeRef::Builtin(BuiltinType::Currency),
        T::Date => VarTypeRef::Builtin(BuiltinType::Date),
        T::String => VarTypeRef::Builtin(BuiltinType::String),
        _ => VarTypeRef::Variant,
    }
}

/// The `ErrField` (and its type) for an `Err.<member>` read.
pub(crate) fn err_field(member: &str) -> Option<(ErrField, VarTypeRef)> {
    match fold_identifier(member).as_str() {
        "number" => Some((ErrField::Number, builtin(BuiltinType::Long))),
        "description" => Some((ErrField::Description, builtin(BuiltinType::String))),
        "source" => Some((ErrField::Source, builtin(BuiltinType::String))),
        "lastdllerror" => Some((ErrField::LastDllError, builtin(BuiltinType::Long))),
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
