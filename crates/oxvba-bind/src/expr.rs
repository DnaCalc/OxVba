//! The expression binder: `bind_expr` walks every expression CST node, infers a
//! `VarTypeRef` bottom-up, and emits a `CoreValue` (plus a `CorePlace` when the
//! expression denotes an l-value).

use oxvba_bundle::NumericMode;
use oxvba_bundle::coreir::{CoreArg, CoreBinOp, CoreConst, CorePlace, CoreUnOp, CoreValue};
use oxvba_bundle::{BundleImport, ExportToken, ProjectMemberKind};
use oxvba_symbol::binding::DispatchRoute;
use oxvba_symbol::model::SymbolKind;
use oxvba_symbol::signature::{BuiltinType, VarTypeRef};
use oxvba_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::error::BindError;
use crate::types;
use crate::{Bound, ProcLower};

pub(crate) fn builtin(b: BuiltinType) -> VarTypeRef {
    VarTypeRef::Builtin(b)
}

pub(crate) fn value_bound(value: CoreValue, ty: VarTypeRef) -> Bound {
    Bound {
        value,
        ty,
        place: None,
    }
}

impl<'a> ProcLower<'a> {
    pub(crate) fn bind_expr(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        match node.kind() {
            SyntaxKind::LiteralExpr => self.bind_literal(node),
            SyntaxKind::IdentExpr => self.bind_ident(node),
            SyntaxKind::ParenExpr => {
                let inner = node
                    .paren_inner()
                    .ok_or_else(|| BindError::Malformed("empty ()".into()))?;
                self.bind_expr(inner)
            }
            SyntaxKind::UnaryExpr => self.bind_unary(node),
            SyntaxKind::BinaryExpr => self.bind_binary(node),
            SyntaxKind::IndexExpr => self.bind_index_or_call(node),
            SyntaxKind::MemberExpr => self.bind_member(node),
            SyntaxKind::NewExpr => self.bind_new(node),
            SyntaxKind::AddressOfExpr => self.bind_address_of(node),
            SyntaxKind::ErrorNode => Err(BindError::Malformed("error expression node".into())),
            other => Err(BindError::Unsupported(format!("expression {other:?}"))),
        }
    }

    fn bind_literal(&self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let tok = node
            .first_significant_token()
            .ok_or_else(|| BindError::Malformed("empty literal".into()))?;
        let (value, ty) = match tok.kind {
            SyntaxKind::IntLiteral => {
                let c = parse_int(tok.text)?;
                let ty = int_literal_type(tok.text, &c);
                (c, ty)
            }
            SyntaxKind::HexLiteral => (parse_radix(tok.text, 16)?, builtin(BuiltinType::Long)),
            SyntaxKind::OctLiteral => (parse_radix(tok.text, 8)?, builtin(BuiltinType::Long)),
            SyntaxKind::FloatLiteral => (
                CoreConst::F64(parse_float(tok.text)?.to_bits()),
                builtin(BuiltinType::Double),
            ),
            SyntaxKind::StringLiteral => (
                CoreConst::Str(unquote(tok.text)),
                builtin(BuiltinType::String),
            ),
            SyntaxKind::KwTrue => (CoreConst::Bool(true), builtin(BuiltinType::Boolean)),
            SyntaxKind::KwFalse => (CoreConst::Bool(false), builtin(BuiltinType::Boolean)),
            SyntaxKind::KwEmpty => (CoreConst::Empty, VarTypeRef::Variant),
            SyntaxKind::KwNull => (CoreConst::Null, VarTypeRef::Variant),
            SyntaxKind::KwNothing => (CoreConst::Nothing, VarTypeRef::Variant),
            SyntaxKind::DateLiteral => (
                CoreConst::Date(
                    oxvba_symbol::const_eval::date::parse_date_literal_serial_bits(tok.text)
                        .ok_or_else(|| {
                            BindError::Malformed(format!("date literal `{}`", tok.text))
                        })?,
                ),
                builtin(BuiltinType::Date),
            ),
            other => return Err(BindError::Unsupported(format!("literal {other:?}"))),
        };
        Ok(value_bound(CoreValue::Const(value), ty))
    }

    fn bind_ident(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let tok = node
            .ident_name_token()
            .ok_or_else(|| BindError::Malformed("identifier without name".into()))?;
        let name = tok.text;
        if node.ident_is_me() {
            let me = self
                .me_value()
                .ok_or_else(|| BindError::Malformed("`Me` outside a class module".into()))?;
            let ty = self
                .info
                .class_name
                .as_deref()
                .map(|n| VarTypeRef::Object(n.to_string()))
                .unwrap_or(VarTypeRef::Variant);
            let place = match &me {
                CoreValue::Load(p) => Some(p.clone()),
                _ => None,
            };
            return Ok(Bound {
                value: me,
                ty,
                place,
            });
        }
        // Reading the function's own name yields the result pseudo-variable.
        if let Some(rl) = self.return_target(name) {
            let place = CorePlace::Local(rl);
            return Ok(Bound {
                value: CoreValue::Load(place.clone()),
                ty: self.info.return_type.clone(),
                place: Some(place),
            });
        }
        let binding = match self.resolve(name) {
            Some(b) => b,
            None => {
                // An unresolved bare name may still be a `VB_PredeclaredId` class
                // name (a referenced project's exposed predeclared instance is not a
                // global-namespace name, so it doesn't `resolve`).
                if let Some(bound) = self.bind_predeclared_instance(name)? {
                    return Ok(bound);
                }
                return Err(self.unresolved(name, "expression"));
            }
        };
        // A referenced project's published `Const`/`Enum` value carries its literal
        // in the route (surface-driven, no shared-symbol dependence).
        if let DispatchRoute::ConstValue(c) = &binding.route {
            return Ok(value_bound(CoreValue::Const(c.clone()), const_type(c)));
        }
        // An active-project `Const`/`Enum` member: its value is folded once in the
        // symbol layer (the published type system's single source of truth).
        if let Some(sym) = binding.symbol
            && let Some(c) = self.g.env.const_value(sym)
        {
            return Ok(value_bound(CoreValue::Const(c.clone()), const_type(c)));
        }
        // A `Const`/`Enum` member that did not fold is unresolvable (e.g. a circular
        // `Const` dependency) — a hard error, as in VBA. (Folding is non-fatal in the
        // symbol layer so one bad const can't abort a whole closure's binding; the
        // error surfaces here, at the use site.)
        if let Some(sym) = binding.symbol
            && matches!(
                self.g.env.symbols.symbol(sym).map(|s| s.kind),
                Some(SymbolKind::Const | SymbolKind::EnumMember)
            )
        {
            return Err(BindError::Unsupported(format!(
                "`{name}` is not a resolvable constant"
            )));
        }
        // A plain variable read.
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
        // A `VB_PredeclaredId` class name (which resolves as a class type/module, not
        // a plain value) → its global singleton instance.
        if let Some(bound) = self.bind_predeclared_instance(name)? {
            return Ok(bound);
        }
        // Otherwise a constant or a 0-argument call.
        self.bind_call_route(name, &binding, None)
    }

    /// A `VB_PredeclaredId` class referenced by its name → its global singleton: an
    /// active-project class lowers to `CoreValue::Predeclared`; a referenced project's
    /// exposed predeclared class lowers to a cross-bundle `CoreValue::PredeclaredExtern`
    /// (registering a `Class` import). These are VBA's document/class predeclared
    /// instances — `ThisWorkbook`, `Sheet1`, `UserForm1`, … Returns `None` when `name`
    /// is not a predeclared class.
    fn bind_predeclared_instance(&mut self, name: &str) -> Result<Option<Bound>, BindError> {
        let folded = oxvba_symbol::model::fold_identifier(name);
        if let Some(&class_id) = self.g.ids.predeclared_class_of.get(&folded) {
            let class_name = self.g.ids.classes[class_id.0].name.clone();
            return Ok(Some(value_bound(
                CoreValue::Predeclared { class: class_id },
                VarTypeRef::Object(class_name),
            )));
        }
        if let Some((unit, class)) = self.g.env.resolve_extern_predeclared(name) {
            let import = self.g.intern_import(BundleImport {
                unit,
                token: ExportToken::Class {
                    name: class.clone(),
                },
            });
            return Ok(Some(value_bound(
                CoreValue::PredeclaredExtern { import },
                VarTypeRef::Object(class),
            )));
        }
        Ok(None)
    }

    fn bind_unary(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let op = node
            .unary_op_token()
            .ok_or_else(|| BindError::Malformed("unary without operator".into()))?;
        let operand_node = node
            .unary_operand()
            .ok_or_else(|| BindError::Malformed("unary without operand".into()))?;
        let operand = self.bind_expr(operand_node)?;
        match op.kind {
            SyntaxKind::Plus => Ok(operand),
            // Unary minus is fixed-typed arithmetic: `-ai` (Integer) overflows at
            // `-(-32768)`; the numeric regime rides on the op.
            SyntaxKind::Minus => {
                let ty = operand.ty.clone();
                let value = CoreValue::Unary {
                    op: CoreUnOp::Negate,
                    expr: Box::new(operand.value),
                    num: types::numeric_mode(&ty),
                };
                Ok(value_bound(value, ty))
            }
            SyntaxKind::KwNot => {
                let ty = if types::is_boolean(&operand.ty) {
                    builtin(BuiltinType::Boolean)
                } else {
                    builtin(BuiltinType::Long)
                };
                Ok(value_bound(
                    CoreValue::Unary {
                        op: CoreUnOp::Not,
                        expr: Box::new(operand.value),
                        num: NumericMode::Widening,
                    },
                    ty,
                ))
            }
            other => Err(BindError::Unsupported(format!("unary {other:?}"))),
        }
    }

    fn bind_binary(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        if node.is_typeof() {
            let operand_node = node
                .typeof_operand()
                .ok_or_else(|| BindError::Malformed("TypeOf without operand".into()))?;
            let type_node = node
                .typeof_type()
                .ok_or_else(|| BindError::Malformed("TypeOf without type".into()))?;
            let operand = self.bind_expr(operand_node)?;
            return Ok(value_bound(
                CoreValue::TypeOfIs {
                    object: Box::new(operand.value),
                    type_name: type_node.text().trim().to_string(),
                },
                builtin(BuiltinType::Boolean),
            ));
        }

        let op_tok = node
            .binary_op_token()
            .ok_or_else(|| BindError::Malformed("binary without operator".into()))?;
        let op = core_binop(op_tok.kind)
            .ok_or_else(|| BindError::Unsupported(format!("operator {:?}", op_tok.kind)))?;
        let lhs = self.bind_expr(
            node.binary_lhs()
                .ok_or_else(|| BindError::Malformed("binary lhs".into()))?,
        )?;
        let rhs = self.bind_expr(
            node.binary_rhs()
                .ok_or_else(|| BindError::Malformed("binary rhs".into()))?,
        )?;
        // `\` and `Mod` always yield an integer (`LongLong` when either side is 64-bit,
        // else `Long`); the VM rounds the operands. Every other op's result type comes
        // from the promotion lattice.
        let ty = if matches!(op, CoreBinOp::IntDiv | CoreBinOp::Mod) {
            if types::is_longlong(&lhs.ty) || types::is_longlong(&rhs.ty) {
                builtin(BuiltinType::LongLong)
            } else {
                builtin(BuiltinType::Long)
            }
        } else {
            types::result_type(op, &lhs.ty, &rhs.ty)
        };
        // The numeric regime (checked-fixed vs widening) rides on the op itself, so the
        // VM and the JIT type the arithmetic without a separate coercion node.
        let value = CoreValue::Binary {
            op,
            lhs: Box::new(lhs.value),
            rhs: Box::new(rhs.value),
            mode: self.info.compare_mode,
            num: types::numeric_mode(&ty),
        };
        Ok(value_bound(value, ty))
    }

    pub(crate) fn bind_index_or_call(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let base = node
            .index_base()
            .ok_or_else(|| BindError::Malformed("index without base".into()))?;
        // Decide array-index vs call by resolving the base name. A name that
        // resolves to a callable is a call even when it equals the enclosing
        // function's name (a recursive call `f(args)` — VBA does not allow
        // indexing the result pseudo-variable, so `f(i)` is always a call).
        if base.kind() == SyntaxKind::IdentExpr
            && let Some(tok) = base.ident_name_token()
            && let Some(binding) = self.resolve(tok.text)
            && !matches!(binding.route, DispatchRoute::Value)
        {
            return self.bind_call_route(tok.text, &binding, node.index_arg_list());
        }
        // `obj(i)` on an Object variable is a DEFAULT-MEMBER call (`obj.Item(i)` /
        // dispid-0), not an array subscript. VBA resolves `obj(…)` for an object
        // receiver through its default member. The gate is the receiver's static
        // type being a scalar `Object` (a typed COM coclass, `Dim As Object`, the
        // built-in `Collection`, or a referenced coclass). A declared array
        // (`VarTypeRef::Array`, including `Dim arr() As Object`) and an ambiguous
        // bare `Variant` (which a `ReDim`/`ParamArray` makes an array, decided at
        // run time) are NOT objects here and fall through to the array-index path.
        //   * a typed COM receiver whose default member resolves to a ComMember →
        //     early-bound dispatch by the default member's dispid;
        //   * any other late-bound Object → late dispatch of the dispid-0 default
        //     member, named "Item" (Collection/Dictionary's default).
        if base.kind() == SyntaxKind::IdentExpr
            && let Some(tok) = base.ident_name_token()
            && let Some(sym) = self.resolve(tok.text).and_then(|b| b.symbol)
            && let ty @ VarTypeRef::Object(_) = self.symbol_type(sym)
        {
            if let Some(binding) = self.g.env.resolve_default_member(&ty) {
                match &binding.route {
                    DispatchRoute::ProjectMember { kind } => {
                        let sym = binding
                            .symbol
                            .ok_or_else(|| self.unresolved(tok.text, "default member"))?;
                        let member = self
                            .symbol_display_name(sym)
                            .unwrap_or_else(|| tok.text.to_string());
                        let kind = match kind {
                            ProjectMemberKind::Method => ProjectMemberKind::Method,
                            _ => ProjectMemberKind::PropertyGet,
                        };
                        let signature =
                            self.project_property_accessor_signature(sym, kind, &member)?;
                        let args = self.bind_proc_args(node.index_arg_list(), &signature, sym)?;
                        let ret = signature.return_type.unwrap_or(VarTypeRef::Variant);
                        let dispatch = self.interface_dispatch_name(&ty, &member);
                        let recv = CoreValue::Load(self.place_by_name(tok.text)?);
                        return Ok(value_bound(
                            self.late_member_call(&dispatch, kind, recv, args),
                            ret,
                        ));
                    }
                    DispatchRoute::ComMember {
                        dispid,
                        member_kind,
                        param_by_ref,
                        ..
                    } => {
                        let dispid = *dispid;
                        // `obj(i)` is a value-context read: dispatch the default member as a
                        // Property Get (or Method), never its Let/Set variant (a default
                        // member that shares its dispid across get/put/putref can resolve to
                        // the writer by typelib order).
                        let member_kind = match member_kind {
                            ProjectMemberKind::Method => ProjectMemberKind::Method,
                            _ => ProjectMemberKind::PropertyGet,
                        };
                        let by_ref = param_by_ref.clone();
                        let recv = CoreValue::Load(self.place_by_name(tok.text)?);
                        let args = self.bind_args_byref(node.index_arg_list(), &by_ref)?;
                        return Ok(value_bound(
                            self.early_com_call(dispid, member_kind, recv, args),
                            VarTypeRef::Variant,
                        ));
                    }
                    DispatchRoute::ExternMember {
                        member,
                        kind,
                        param_types,
                        ..
                    } => {
                        let kind = match kind {
                            ProjectMemberKind::Method => ProjectMemberKind::Method,
                            _ => ProjectMemberKind::PropertyGet,
                        };
                        let recv = CoreValue::Load(self.place_by_name(tok.text)?);
                        let args = self.bind_extern_args(node.index_arg_list(), param_types)?;
                        return Ok(value_bound(
                            self.late_member_call(member, kind, recv, args),
                            VarTypeRef::Variant,
                        ));
                    }
                    _ => {}
                }
            }
            if self.is_late_bound_receiver(&ty) {
                let recv = CoreValue::Load(self.place_by_name(tok.text)?);
                let args = self.bind_extern_args(node.index_arg_list(), &[])?;
                return Ok(value_bound(
                    self.late_member_call(
                        "Item",
                        oxvba_bundle::ProjectMemberKind::Method,
                        recv,
                        args,
                    ),
                    VarTypeRef::Variant,
                ));
            }
        }
        // `obj.Member(args)` — a method/property call, or an index into a member
        // array. The member binder decides by resolving the member.
        if base.kind() == SyntaxKind::MemberExpr {
            return self.bind_member_call(base, node.index_arg_list());
        }
        // An array element read.
        let (place, ty) = self.bind_place(node)?;
        Ok(Bound {
            value: CoreValue::Load(place.clone()),
            ty,
            place: Some(place),
        })
    }

    fn bind_new(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let name = node
            .new_type_name()
            .ok_or_else(|| BindError::Malformed("New without a type".into()))?;
        let (value, ty) = self.new_value_for_type(&name)?;
        Ok(value_bound(value, ty))
    }

    /// Resolve `New <name>` to its instantiation value + inferred object type — the
    /// resolution ladder shared by the `New` expression and `Dim x As New Foo`
    /// auto-instantiation. A project class mints in-bundle; a referenced project's
    /// coclass mints cross-bundle; a COM coclass activates via `CreateObject`.
    pub(crate) fn new_value_for_type(
        &mut self,
        name: &str,
    ) -> Result<
        (
            oxvba_bundle::coreir::CoreValue,
            oxvba_symbol::signature::VarTypeRef,
        ),
        BindError,
    > {
        let folded = oxvba_symbol::model::fold_identifier(name);
        if let Some(&class_id) = self.g.ids.class_of.get(&folded) {
            return Ok((
                CoreValue::New(class_id),
                VarTypeRef::Object(name.to_string()),
            ));
        }
        // A creatable coclass published by a *referenced project*: instantiate it in
        // that project's bundle via a cross-bundle `NewExtern` (the new instance
        // carries the target bundle's id, so later method dispatch routes there). The
        // result is typed by the bare class name so member access binds against the
        // referenced surface.
        if let Some((unit, class)) = self.g.env.resolve_extern_coclass(name) {
            let import = self.g.intern_import(oxvba_bundle::BundleImport {
                unit,
                token: oxvba_bundle::ExportToken::Class {
                    name: class.clone(),
                },
            });
            return Ok((CoreValue::NewExtern { import }, VarTypeRef::Object(class)));
        }
        // A creatable COM coclass (from a referenced typelib) instantiates via the
        // same activation path as `CreateObject("<ProgID>")`; the result is typed
        // as the coclass so member access resolves against its typelib.
        if let Some(prog_id) = self.g.env.resolve_coclass(name) {
            let args = vec![CoreArg::ByVal(CoreValue::Const(CoreConst::Str(prog_id)))];
            return Ok((
                CoreValue::Call {
                    callee: oxvba_bundle::coreir::CoreCallee::Native(
                        oxvba_bundle::native::NativeImplId::CreateObject,
                    ),
                    args,
                },
                VarTypeRef::Object(name.to_string()),
            ));
        }
        Err(BindError::Unsupported(format!(
            "New {name} (only project classes are creatable)"
        )))
    }

    /// `AddressOf proc` — resolve the operand to a standard-module procedure and
    /// emit a procedure reference. VBA forbids `AddressOf` of a class member (it
    /// has no standalone address), so reject that.
    fn bind_address_of(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let operand = node
            .first_expr_child()
            .ok_or_else(|| BindError::Malformed("AddressOf without an operand".into()))?;
        let name = match operand.kind() {
            SyntaxKind::IdentExpr => operand.ident_name_token().map(|t| t.text),
            SyntaxKind::MemberExpr => operand.member_name_token().map(|t| t.text),
            _ => None,
        }
        .ok_or_else(|| BindError::Unsupported("AddressOf requires a procedure name".into()))?;
        let binding = self
            .resolve(name)
            .ok_or_else(|| self.unresolved(name, "AddressOf operand"))?;
        let proc = binding
            .symbol
            .and_then(|s| self.g.ids.proc_of.get(&s).copied())
            .ok_or_else(|| {
                BindError::Unsupported(format!("AddressOf of `{name}` (not a project procedure)"))
            })?;
        if self.g.ids.procs[proc.0].class_name.is_some() {
            return Err(BindError::Unsupported(format!(
                "AddressOf of class member `{name}` is not allowed"
            )));
        }
        Ok(value_bound(
            CoreValue::AddressOf(proc),
            builtin(BuiltinType::Long),
        ))
    }
}

// ── Literal parsing ──────────────────────────────────────────────────────────

fn parse_int(text: &str) -> Result<CoreConst, BindError> {
    // Trim any VBA type-declaration suffix, incl. `^` (LongLong) and `&` (Long).
    let digits = text.trim_end_matches(['&', '%', '@', '!', '#', '$', '^']);
    let n: i64 = digits
        .parse()
        .map_err(|_| BindError::Malformed(format!("integer literal `{text}`")))?;
    Ok(if i32::try_from(n).is_ok() {
        CoreConst::I32(n as i32)
    } else {
        CoreConst::I64(n)
    })
}

/// The static type of a decimal integer literal: an explicit type-suffix wins,
/// else the smallest integer type that holds the value (`Integer` ≤ 32767, then
/// `Long`, then `LongLong`) — VBA's literal typing. This is what makes
/// `Integer + Integer` overflow as `Integer` (the classic `30000 * 30000` gotcha)
/// rather than silently widening; the run-time payload stays `I32`/`I64` and is
/// re-tagged by the result coercion ([`types::narrow_arith`]).
fn int_literal_type(text: &str, c: &CoreConst) -> VarTypeRef {
    match text.trim().chars().next_back() {
        Some('%') => builtin(BuiltinType::Integer),
        Some('&') => builtin(BuiltinType::Long),
        Some('^') => builtin(BuiltinType::LongLong),
        _ => match c {
            CoreConst::I32(n) if (i32::from(i16::MIN)..=i32::from(i16::MAX)).contains(n) => {
                builtin(BuiltinType::Integer)
            }
            CoreConst::I64(_) => builtin(BuiltinType::LongLong),
            _ => builtin(BuiltinType::Long),
        },
    }
}

fn parse_radix(text: &str, radix: u32) -> Result<CoreConst, BindError> {
    // `&H1F` / `&O17`, optionally with a trailing `&` Long suffix.
    let body = text
        .trim_start_matches(['&'])
        .trim_start_matches(['h', 'H', 'o', 'O'])
        .trim_end_matches(['&', '%', '^']);
    let n = i64::from_str_radix(body, radix)
        .map_err(|_| BindError::Malformed(format!("radix literal `{text}`")))?;
    Ok(if i32::try_from(n).is_ok() {
        CoreConst::I32(n as i32)
    } else {
        CoreConst::I64(n)
    })
}

fn parse_float(text: &str) -> Result<f64, BindError> {
    text.trim_end_matches(['!', '#', '@'])
        .parse()
        .map_err(|_| BindError::Malformed(format!("float literal `{text}`")))
}

fn unquote(text: &str) -> String {
    let inner = text.strip_prefix('"').unwrap_or(text);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    inner.replace("\"\"", "\"")
}

/// The inferred type of a folded constant value.
pub(crate) fn const_type(c: &CoreConst) -> VarTypeRef {
    match c {
        CoreConst::I32(_) => builtin(BuiltinType::Long),
        CoreConst::I64(_) => builtin(BuiltinType::LongLong),
        CoreConst::F64(_) => builtin(BuiltinType::Double),
        CoreConst::F32(_) => builtin(BuiltinType::Single),
        CoreConst::Currency(_) => builtin(BuiltinType::Currency),
        CoreConst::Str(_) => builtin(BuiltinType::String),
        CoreConst::Bool(_) => builtin(BuiltinType::Boolean),
        CoreConst::Date(_) => builtin(BuiltinType::Date),
        _ => VarTypeRef::Variant,
    }
}

fn core_binop(kind: SyntaxKind) -> Option<CoreBinOp> {
    Some(match kind {
        SyntaxKind::Plus => CoreBinOp::Add,
        SyntaxKind::Minus => CoreBinOp::Sub,
        SyntaxKind::Star => CoreBinOp::Mul,
        SyntaxKind::Slash => CoreBinOp::Div,
        SyntaxKind::Backslash => CoreBinOp::IntDiv,
        SyntaxKind::Caret => CoreBinOp::Pow,
        SyntaxKind::Ampersand => CoreBinOp::Concat,
        SyntaxKind::Eq => CoreBinOp::Eq,
        SyntaxKind::LtGt => CoreBinOp::Ne,
        SyntaxKind::Lt => CoreBinOp::Lt,
        SyntaxKind::LtEq => CoreBinOp::Le,
        SyntaxKind::Gt => CoreBinOp::Gt,
        SyntaxKind::GtEq => CoreBinOp::Ge,
        SyntaxKind::KwMod => CoreBinOp::Mod,
        SyntaxKind::KwAnd => CoreBinOp::And,
        SyntaxKind::KwOr => CoreBinOp::Or,
        SyntaxKind::KwXor => CoreBinOp::Xor,
        SyntaxKind::KwEqv => CoreBinOp::Eqv,
        SyntaxKind::KwImp => CoreBinOp::Imp,
        SyntaxKind::KwIs => CoreBinOp::Is,
        SyntaxKind::KwLike => CoreBinOp::Like,
        _ => return None,
    })
}

/// The comparison `CoreBinOp` for a `Case Is <op>` operator token.
pub(crate) fn comparison_binop(tok: SyntaxToken<'_>) -> Option<CoreBinOp> {
    match tok.kind {
        SyntaxKind::Eq => Some(CoreBinOp::Eq),
        SyntaxKind::LtGt => Some(CoreBinOp::Ne),
        SyntaxKind::Lt => Some(CoreBinOp::Lt),
        SyntaxKind::LtEq => Some(CoreBinOp::Le),
        SyntaxKind::Gt => Some(CoreBinOp::Gt),
        SyntaxKind::GtEq => Some(CoreBinOp::Ge),
        _ => None,
    }
}
