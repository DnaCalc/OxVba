//! The expression binder: `bind_expr` walks every expression CST node, infers a
//! `VarTypeRef` bottom-up, and emits a `CoreValue` (plus a `CorePlace` when the
//! expression denotes an l-value).

use oxvba_bundle::coreir::{CoreBinOp, CoreConst, CorePlace, CoreUnOp, CoreValue};
use oxvba_symbol::binding::DispatchRoute;
use oxvba_symbol::signature::{BuiltinType, VarTypeRef};
use oxvba_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::error::BindError;
use crate::types;
use crate::{Bound, ProcLower};

pub(crate) fn builtin(b: BuiltinType) -> VarTypeRef {
    VarTypeRef::Builtin(b)
}

pub(crate) fn value_bound(value: CoreValue, ty: VarTypeRef) -> Bound {
    Bound { value, ty, place: None }
}

impl<'a> ProcLower<'a> {
    pub(crate) fn bind_expr(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        match node.kind() {
            SyntaxKind::LiteralExpr => self.bind_literal(node),
            SyntaxKind::IdentExpr => self.bind_ident(node),
            SyntaxKind::ParenExpr => {
                let inner = node.paren_inner().ok_or_else(|| BindError::Malformed("empty ()".into()))?;
                self.bind_expr(inner)
            }
            SyntaxKind::UnaryExpr => self.bind_unary(node),
            SyntaxKind::BinaryExpr => self.bind_binary(node),
            SyntaxKind::IndexExpr => self.bind_index_or_call(node),
            SyntaxKind::MemberExpr => self.bind_member(node),
            SyntaxKind::NewExpr => self.bind_new(node),
            SyntaxKind::ErrorNode => Err(BindError::Malformed("error expression node".into())),
            other => Err(BindError::Unsupported(format!("expression {other:?}"))),
        }
    }

    fn bind_literal(&self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let tok = node
            .first_significant_token()
            .ok_or_else(|| BindError::Malformed("empty literal".into()))?;
        let (value, ty) = match tok.kind {
            SyntaxKind::IntLiteral => (parse_int(tok.text)?, builtin(BuiltinType::Long)),
            SyntaxKind::HexLiteral => (parse_radix(tok.text, 16)?, builtin(BuiltinType::Long)),
            SyntaxKind::OctLiteral => (parse_radix(tok.text, 8)?, builtin(BuiltinType::Long)),
            SyntaxKind::FloatLiteral => (
                CoreConst::F64(parse_float(tok.text)?.to_bits()),
                builtin(BuiltinType::Double),
            ),
            SyntaxKind::StringLiteral => (CoreConst::Str(unquote(tok.text)), builtin(BuiltinType::String)),
            SyntaxKind::KwTrue => (CoreConst::Bool(true), builtin(BuiltinType::Boolean)),
            SyntaxKind::KwFalse => (CoreConst::Bool(false), builtin(BuiltinType::Boolean)),
            SyntaxKind::KwEmpty => (CoreConst::Empty, VarTypeRef::Variant),
            SyntaxKind::KwNull => (CoreConst::Null, VarTypeRef::Variant),
            SyntaxKind::KwNothing => (CoreConst::Nothing, VarTypeRef::Variant),
            SyntaxKind::DateLiteral => (
                CoreConst::Date(
                    crate::date::parse_date_literal_serial_bits(tok.text)
                        .ok_or_else(|| BindError::Malformed(format!("date literal `{}`", tok.text)))?,
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
            return Ok(Bound { value: me, ty, place });
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
        let binding = self
            .resolve(name)
            .ok_or_else(|| self.unresolved(name, "expression"))?;
        // A folded `Const` substitutes its literal value.
        if let Some(sym) = binding.symbol
            && let Some(c) = self.g.ids.const_of.get(&sym) {
                return Ok(value_bound(CoreValue::Const(c.clone()), const_type(c)));
            }
        // A plain variable read.
        if let DispatchRoute::Value = binding.route
            && let Some(sym) = binding.symbol
                && let Some((place, ty)) = self.place_for_symbol(sym) {
                    return Ok(Bound { value: CoreValue::Load(place.clone()), ty, place: Some(place) });
                }
        // Otherwise a constant or a 0-argument call.
        self.bind_call_route(name, &binding, None)
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
            SyntaxKind::Minus => Ok(Bound {
                ty: operand.ty.clone(),
                value: CoreValue::Unary { op: CoreUnOp::Negate, expr: Box::new(operand.value) },
                place: None,
            }),
            SyntaxKind::KwNot => {
                let ty = if types::is_boolean(&operand.ty) {
                    builtin(BuiltinType::Boolean)
                } else {
                    builtin(BuiltinType::Long)
                };
                Ok(value_bound(
                    CoreValue::Unary { op: CoreUnOp::Not, expr: Box::new(operand.value) },
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
            node.binary_lhs().ok_or_else(|| BindError::Malformed("binary lhs".into()))?,
        )?;
        let rhs = self.bind_expr(
            node.binary_rhs().ok_or_else(|| BindError::Malformed("binary rhs".into()))?,
        )?;
        // `\` and `Mod` operate on integers — coerce operands to the widest integer
        // type involved (LongLong when either side is 64-bit, else Long) so a
        // 64-bit operand isn't truncated to Long before the operation.
        let (lv, rv, ty) = if matches!(op, CoreBinOp::IntDiv | CoreBinOp::Mod) {
            if types::is_longlong(&lhs.ty) || types::is_longlong(&rhs.ty) {
                (
                    types::coerce_to_longlong(lhs.value),
                    types::coerce_to_longlong(rhs.value),
                    builtin(BuiltinType::LongLong),
                )
            } else {
                (types::coerce_to_long(lhs.value), types::coerce_to_long(rhs.value), builtin(BuiltinType::Long))
            }
        } else {
            (lhs.value, rhs.value, types::result_type(op, &lhs.ty, &rhs.ty))
        };
        Ok(value_bound(
            CoreValue::Binary { op, lhs: Box::new(lv), rhs: Box::new(rv), mode: self.info.compare_mode },
            ty,
        ))
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
                    && !matches!(binding.route, DispatchRoute::Value) {
                        return self.bind_call_route(tok.text, &binding, node.index_arg_list());
                    }
        // `obj.Member(args)` — a method/property call, or an index into a member
        // array. The member binder decides by resolving the member.
        if base.kind() == SyntaxKind::MemberExpr {
            return self.bind_member_call(base, node.index_arg_list());
        }
        // An array element read.
        let (place, ty) = self.bind_place(node)?;
        Ok(Bound { value: CoreValue::Load(place.clone()), ty, place: Some(place) })
    }

    fn bind_new(&mut self, node: SyntaxNode<'_>) -> Result<Bound, BindError> {
        let name = node
            .new_type_name()
            .ok_or_else(|| BindError::Malformed("New without a type".into()))?;
        let folded = oxvba_symbol::model::fold_identifier(&name);
        if let Some(&class_id) = self.g.ids.class_of.get(&folded) {
            return Ok(value_bound(CoreValue::New(class_id), VarTypeRef::Object(name)));
        }
        // COM coclass creation (CreateObject-style) is a separate path.
        Err(BindError::Unsupported(format!("New {name} (only project classes are creatable)")))
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

/// Fold a `Const` initializer expression to a literal value (literals, a sign, or
/// a parenthesised literal). Non-literal initializers (`Const B = A + 1`) return
/// `None` and fall back to the ordinary name path.
pub(crate) fn fold_const_literal(node: SyntaxNode<'_>) -> Option<CoreConst> {
    match node.kind() {
        SyntaxKind::LiteralExpr => {
            let tok = node.first_significant_token()?;
            match tok.kind {
                SyntaxKind::IntLiteral => parse_int(tok.text).ok(),
                SyntaxKind::HexLiteral => parse_radix(tok.text, 16).ok(),
                SyntaxKind::OctLiteral => parse_radix(tok.text, 8).ok(),
                SyntaxKind::FloatLiteral => Some(CoreConst::F64(parse_float(tok.text).ok()?.to_bits())),
                SyntaxKind::StringLiteral => Some(CoreConst::Str(unquote(tok.text))),
                SyntaxKind::KwTrue => Some(CoreConst::Bool(true)),
                SyntaxKind::KwFalse => Some(CoreConst::Bool(false)),
                SyntaxKind::DateLiteral => crate::date::parse_date_literal_serial_bits(tok.text).map(CoreConst::Date),
                _ => None,
            }
        }
        SyntaxKind::ParenExpr => fold_const_literal(node.paren_inner()?),
        SyntaxKind::UnaryExpr => {
            let inner = fold_const_literal(node.unary_operand()?)?;
            match node.unary_op_token()?.kind {
                SyntaxKind::Plus => Some(inner),
                SyntaxKind::Minus => negate_const(inner),
                _ => None,
            }
        }
        _ => None,
    }
}

fn negate_const(c: CoreConst) -> Option<CoreConst> {
    Some(match c {
        CoreConst::I32(n) => CoreConst::I32(n.checked_neg()?),
        CoreConst::I64(n) => CoreConst::I64(n.checked_neg()?),
        CoreConst::F64(bits) => CoreConst::F64((-f64::from_bits(bits)).to_bits()),
        _ => return None,
    })
}

/// The inferred type of a folded constant value.
pub(crate) fn const_type(c: &CoreConst) -> VarTypeRef {
    match c {
        CoreConst::I32(_) | CoreConst::I64(_) => builtin(BuiltinType::Long),
        CoreConst::F64(_) => builtin(BuiltinType::Double),
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
