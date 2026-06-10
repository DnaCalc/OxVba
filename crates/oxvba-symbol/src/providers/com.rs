//! COM typelib provider — resolves members and events of a resolved
//! `TypeLibMetadataBlob` against a typed receiver. It returns one fully-typed
//! `ComMember` route (dispid + vtable slot + name) regardless of receiver, so the
//! binder picks early vs late by the receiver's static type.

use oxvba_bundle::ProjectMemberKind;
use oxvba_com::TypeLibMetadataBlob;

use crate::binding::{Binding, DispatchRoute, member_kind_from_invoke};
use crate::model::fold_identifier;
use crate::provider::Provider;
use crate::signature::VarTypeRef;

pub struct ComTypeLibProvider {
    blob: TypeLibMetadataBlob,
    /// Folded names this blob answers to (its reference name, requested coclass,
    /// and activation ProgID).
    type_names: Vec<String>,
}

impl ComTypeLibProvider {
    pub fn new(blob: TypeLibMetadataBlob) -> Self {
        let mut type_names = vec![fold_identifier(&blob.identity.reference_name)];
        if let Some(coclass) = &blob.identity.requested_coclass {
            type_names.push(fold_identifier(coclass));
        }
        if let Some(prog_id) = &blob.activation_prog_id {
            type_names.push(fold_identifier(prog_id));
        }
        Self { blob, type_names }
    }

    fn owns(&self, type_name: &str) -> bool {
        let folded = fold_identifier(type_name);
        self.type_names.contains(&folded)
    }

    pub fn activation_prog_id(&self) -> Option<&str> {
        self.blob.activation_prog_id.as_deref()
    }
}

impl Provider for ComTypeLibProvider {
    fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        if !self.owns(type_name) {
            return None;
        }

        if let Some(member) = self.blob.members.iter().find(|member| {
            member.name.eq_ignore_ascii_case(name)
                && want.is_none_or(|kind| member_kind_from_invoke(member.invoke_kind) == kind)
        }) {
            return Some(com_member_binding(member));
        }

        if let Some(event) = self
            .blob
            .events
            .iter()
            .find(|event| event.name.eq_ignore_ascii_case(name))
        {
            return Some(Binding::new(
                None,
                DispatchRoute::ComEvent {
                    event_name: event.name.clone(),
                    token: event.token,
                    dispatch_path: event.dispatch_path,
                    dispatch_member_id: event.dispatch_member_id,
                    connection_point_iid: event.connection_point_iid.clone(),
                    callback_arity: event.callback_arity,
                },
            ));
        }

        None
    }

    fn resolve_default_member(&self, recv: &VarTypeRef) -> Option<Binding> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        if !self.owns(type_name) {
            return None;
        }
        self.blob
            .members
            .iter()
            .find(|member| member.is_default_member)
            .map(com_member_binding)
    }

    fn resolve_coclass(&self, name: &str) -> Option<String> {
        if self.owns(name) {
            self.activation_prog_id().map(str::to_string)
        } else {
            None
        }
    }
}

fn com_member_binding(member: &oxvba_com::TypeLibMemberMetadata) -> Binding {
    Binding {
        symbol: None,
        is_default: member.is_default_member,
        route: DispatchRoute::ComMember {
            member_name: member.name.clone(),
            dispid: member.token,
            vtable_slot: member.vtable_slot,
            invoke_kind: member.invoke_kind,
            member_kind: member_kind_from_invoke(member.invoke_kind),
            is_default_member: member.is_default_member,
            param_by_ref: member
                .parameter_types
                .iter()
                .map(|t| t.is_by_ref())
                .collect(),
        },
    }
}
