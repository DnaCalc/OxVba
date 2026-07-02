//! The l-value binder: `bind_place` classifies an assignable expression into a
//! `CorePlace` (local/global slot, array element, …) and reports its type.

use oxvba_bundle::coreir::{CorePlace, CoreValue};
use oxvba_symbol::binding::DispatchRoute;
use oxvba_symbol::signature::VarTypeRef;
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::ProcLower;
use crate::error::BindError;

impl<'a> ProcLower<'a> {
    pub(crate) fn bind_place(
        &mut self,
        node: SyntaxNode<'_>,
    ) -> Result<(CorePlace, VarTypeRef), BindError> {
        match node.kind() {
            SyntaxKind::IdentExpr => {
                let tok = node
                    .ident_name_token()
                    .ok_or_else(|| BindError::Malformed("place identifier".into()))?;
                let name = tok.text;
                if let Some(rl) = self.return_target(name) {
                    return Ok((CorePlace::Local(rl), self.info.return_type.clone()));
                }
                let binding = match self.resolve(name) {
                    Some(binding) => binding,
                    None => {
                        if let Some(bound) = self.bind_predeclared_instance(name)?
                            && let Some(place) = bound.place
                        {
                            return Ok((place, bound.ty));
                        }
                        return Err(self.unresolved(name, "assignment target"));
                    }
                };
                match binding.symbol.and_then(|s| self.place_for_symbol(s)) {
                    Some(place_ty) => Ok(place_ty),
                    None => {
                        if let Some(bound) = self.bind_predeclared_instance(name)?
                            && let Some(place) = bound.place
                        {
                            return Ok((place, bound.ty));
                        }
                        Err(BindError::InvalidAssignment(format!(
                            "`{name}` is not an assignable variable"
                        )))
                    }
                }
            }
            SyntaxKind::IndexExpr => {
                let base = node
                    .index_base()
                    .ok_or_else(|| BindError::Malformed("index base".into()))?;
                let (base_place, base_ty) = self.bind_place(base)?;
                let base_is_record_field = matches!(base_place, CorePlace::RecordField { .. });
                // Track the array's element type so a nested UDT-array element
                // (`o.Lines(i).Text`) can resolve its sub-fields; a non-array
                // base (e.g. a default-member index) stays Variant.
                let elem_ty = match base_ty {
                    VarTypeRef::Array(inner) => self.g.resolve_udt_type(*inner),
                    VarTypeRef::FixedArray { element, .. } => self.g.resolve_udt_type(*element),
                    _ if base_is_record_field => {
                        return Err(BindError::ExpectedArray {
                            name: base.text().trim().to_string(),
                        });
                    }
                    _ => VarTypeRef::Variant,
                };
                let indices = self.bind_index_values(node)?;
                let place = CorePlace::Index {
                    array: Box::new(base_place),
                    indices,
                };
                Ok((place, elem_ty))
            }
            SyntaxKind::ParenExpr => {
                let inner = node
                    .paren_inner()
                    .ok_or_else(|| BindError::Malformed("empty () place".into()))?;
                self.bind_place(inner)
            }
            SyntaxKind::MemberExpr => {
                // A field of a UDT value (`p.X`) is a fixed-index element of the record.
                if let Some(place) = self.udt_field_place(node)? {
                    return Ok(place);
                }
                let member = node
                    .member_name_token()
                    .ok_or_else(|| BindError::Malformed("member target without name".into()))?
                    .text;
                if let Some(binding) = self.qualified_namespace_member_binding(node, member) {
                    match binding.symbol.and_then(|s| self.place_for_symbol(s)) {
                        Some(place_ty) => return Ok(place_ty),
                        None => {
                            return Err(BindError::InvalidAssignment(format!(
                                "`{}` is not an assignable variable",
                                node.text().trim()
                            )));
                        }
                    }
                }
                let recv = self.member_receiver_bound(node)?;
                let binding = self
                    .resolve_member(&recv.ty, member, None)
                    .ok_or_else(|| self.unresolved(member, "member assignment target"))?;
                match &binding.route {
                    // A field / WithEvents field is an assignable place. (A property
                    // target is handled as a setter call upstream in `bind_assign`.)
                    DispatchRoute::Value => {
                        let sym = binding
                            .symbol
                            .ok_or_else(|| self.unresolved(member, "member field"))?;
                        self.member_place(recv.value, sym)
                    }
                    other => Err(BindError::InvalidAssignment(format!(
                        "`.{member}` is not an assignable field ({other:?})"
                    ))),
                }
            }
            other => Err(BindError::InvalidAssignment(format!(
                "{other:?} is not an l-value"
            ))),
        }
    }

    /// If `node` is `<udt>.<field>` (a member access on a UDT value), the field place:
    /// a fixed-index element of the record (a `SafeArray`), with the field's type.
    /// `None` when the receiver is not a UDT place (the caller falls back to object /
    /// property member resolution).
    pub(crate) fn udt_field_place(
        &mut self,
        node: SyntaxNode<'_>,
    ) -> Result<Option<(CorePlace, VarTypeRef)>, BindError> {
        let Some(member) = node.member_name_token().map(|t| t.text) else {
            return Ok(None);
        };
        // The receiver place + UDT type: an explicit `recv.field`, or — for a
        // leading-dot `.field` inside `With <udt>` — the active With receiver
        // (whose `place`/`ty` were captured when the `With` object was bound).
        let Some((base_place, udt)) = self.udt_receiver_place(node)? else {
            return Ok(None);
        };
        let Some((index, field_ty)) = self
            .g
            .env
            .udt_field(&udt, member)
            .map(|(i, t)| (i, t.clone()))
        else {
            return Ok(None);
        };
        let place = CorePlace::RecordField {
            base: Box::new(base_place),
            index,
        };
        Ok(Some((place, self.g.resolve_udt_type(field_ty))))
    }

    /// The receiver place + folded UDT name for a `MemberExpr` whose receiver is a
    /// UDT value: an explicit `recv.field` (binds `recv` as a place) or a leading-dot
    /// `.field` inside a `With <udt-value>` block (reuses the active `With` receiver's
    /// captured place). `None` when the receiver is absent or not a UDT place.
    fn udt_receiver_place(
        &mut self,
        node: SyntaxNode<'_>,
    ) -> Result<Option<(CorePlace, String)>, BindError> {
        if node.member_has_leading_dot() {
            let Some(recv) = self.with_stack.last() else {
                return Ok(None);
            };
            return match (&recv.ty, &recv.place) {
                (VarTypeRef::Udt(udt), Some(place)) => Ok(Some((place.clone(), udt.clone()))),
                _ => Ok(None),
            };
        }
        let Some(recv_node) = node.member_receiver() else {
            return Ok(None);
        };
        match self.bind_place(recv_node) {
            Ok((place, VarTypeRef::Udt(udt))) => Ok(Some((place, udt))),
            _ => Ok(None),
        }
    }

    /// Bind the index expressions of an `IndexExpr` to a list of values.
    pub(crate) fn bind_index_values(
        &mut self,
        index_node: SyntaxNode<'_>,
    ) -> Result<Vec<CoreValue>, BindError> {
        let arglist = index_node
            .index_arg_list()
            .ok_or_else(|| BindError::Malformed("index without arguments".into()))?;
        self.bind_positional_values(arglist)
    }

    /// Bind each positional argument of an `ArgList` to a value (used for array
    /// indices, `Array(...)` items, etc.). Omitted/named slots become an error.
    pub(crate) fn bind_positional_values(
        &mut self,
        arglist: SyntaxNode<'_>,
    ) -> Result<Vec<CoreValue>, BindError> {
        let mut values = Vec::new();
        for item in arglist.arg_items() {
            match item {
                oxvba_syntax::red::ArgItem::Positional(expr, _) => {
                    values.push(self.bind_expr(expr)?.value)
                }
                oxvba_syntax::red::ArgItem::Named { value, .. } => {
                    values.push(self.bind_expr(value)?.value)
                }
                oxvba_syntax::red::ArgItem::Omitted => {
                    return Err(BindError::Malformed("omitted index/element".into()));
                }
            }
        }
        Ok(values)
    }
}
