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
                let binding = self
                    .resolve(name)
                    .ok_or_else(|| self.unresolved(name, "assignment target"))?;
                match binding.symbol.and_then(|s| self.place_for_symbol(s)) {
                    Some(place_ty) => Ok(place_ty),
                    None => Err(BindError::InvalidAssignment(format!(
                        "`{name}` is not an assignable variable"
                    ))),
                }
            }
            SyntaxKind::IndexExpr => {
                let base = node
                    .index_base()
                    .ok_or_else(|| BindError::Malformed("index base".into()))?;
                let (base_place, _ty) = self.bind_place(base)?;
                let indices = self.bind_index_values(node)?;
                let place = CorePlace::Index {
                    array: Box::new(base_place),
                    indices,
                };
                // The element type isn't tracked through the array yet → Variant.
                Ok((place, VarTypeRef::Variant))
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
        let Some(recv_node) = node.member_receiver() else {
            return Ok(None);
        };
        let Ok((base_place, VarTypeRef::Udt(udt))) = self.bind_place(recv_node) else {
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
                oxvba_syntax::red::ArgItem::Positional(expr) => {
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
