//! COM typelib provider — resolves members and events of a resolved
//! `TypeLibMetadataBlob` against a typed receiver. It returns one fully-typed
//! `ComMember` route (dispid + vtable slot + name) regardless of receiver, so the
//! binder picks early vs late by the receiver's static type.

use oxvba_bundle::ProjectMemberKind;
use oxvba_com::{TypeLibMemberInvokeKind, TypeLibMemberMetadata, TypeLibMetadataBlob};

use crate::binding::{Binding, DispatchRoute, member_kind_from_invoke};
use crate::model::fold_identifier;
use crate::provider::Provider;
use crate::signature::VarTypeRef;

pub struct ComTypeLibProvider {
    blob: TypeLibMetadataBlob,
    /// Folded names this blob answers to (its reference name, requested coclass,
    /// activation ProgID, and library-level coclasses).
    type_names: Vec<String>,
    /// Folded names whose members are scoped by this blob's member list. A
    /// library-wide blob knows every coclass type name, but its flat member list
    /// must not make one coclass see another coclass's members.
    member_type_names: Vec<String>,
}

impl ComTypeLibProvider {
    pub fn new(blob: TypeLibMetadataBlob) -> Self {
        let reference = fold_identifier(&blob.identity.reference_name);
        let mut type_names = vec![reference.clone()];
        let mut member_type_names = vec![reference.clone()];
        if let Some(coclass) = &blob.identity.requested_coclass {
            let folded = fold_identifier(coclass);
            type_names.push(folded.clone());
            type_names.push(format!("{reference}.{folded}"));
            member_type_names.push(folded.clone());
            member_type_names.push(format!("{reference}.{folded}"));
        }
        if let Some(prog_id) = &blob.activation_prog_id {
            let folded = fold_identifier(prog_id);
            type_names.push(folded.clone());
            member_type_names.push(folded);
        }
        for coclass in &blob.coclass_names {
            let folded = fold_identifier(coclass);
            type_names.push(folded.clone());
            type_names.push(format!("{reference}.{folded}"));
        }
        type_names.sort();
        type_names.dedup();
        member_type_names.sort();
        member_type_names.dedup();
        Self {
            blob,
            type_names,
            member_type_names,
        }
    }

    fn owns(&self, type_name: &str) -> bool {
        let folded = fold_identifier(type_name);
        self.type_names.contains(&folded)
    }

    fn owns_members(&self, type_name: &str) -> bool {
        let folded = fold_identifier(type_name);
        self.member_type_names.contains(&folded)
    }

    /// Whether this typelib's library reference defines a coclass `<reference>.<coclass>`
    /// (e.g. `Excel.Application` for the `Excel` library). Used to resolve a `WithEvents`
    /// EVENT SOURCE by coclass name even when the blob is a library-level reference that
    /// names no single coclass. Scoped to the event-source path: member resolution stays
    /// keyed on [`owns`] so a library reference does not lump every coclass's members.
    fn owns_coclass_source(&self, type_name: &str) -> bool {
        if self.owns(type_name) {
            return true;
        }
        let folded = fold_identifier(type_name);
        let reference = fold_identifier(&self.blob.identity.reference_name);
        self.blob
            .coclass_names
            .iter()
            .any(|coclass| folded == format!("{reference}.{}", fold_identifier(coclass)))
    }

    /// The coclass name a `WithEvents` receiver type maps to (e.g. `Application` for
    /// `Excel.Application`), used to scope event enumeration to that coclass. `None` when no
    /// specific coclass can be derived — the caller then keeps the whole event set.
    fn matched_coclass(&self, type_name: &str) -> Option<String> {
        let folded = fold_identifier(type_name);
        let reference = fold_identifier(&self.blob.identity.reference_name);
        if let Some(suffix) = folded.strip_prefix(&format!("{reference}.")) {
            return Some(suffix.to_string());
        }
        if let Some(coclass) = &self.blob.identity.requested_coclass {
            return Some(fold_identifier(coclass));
        }
        if self
            .blob
            .coclass_names
            .iter()
            .any(|coclass| fold_identifier(coclass) == folded)
        {
            return Some(folded);
        }
        None
    }

    pub fn activation_prog_id(&self) -> Option<&str> {
        self.blob.activation_prog_id.as_deref()
    }

    pub fn resolve_object_root(&self, name: &str) -> Option<Binding> {
        if !self.owns(name) {
            return None;
        }
        Some(Binding::new(
            None,
            DispatchRoute::ComObjectRoot {
                type_name: name.to_string(),
                prog_id: self
                    .coclass_prog_id_for_name(name)
                    .or_else(|| self.activation_prog_id().map(str::to_string)),
            },
        ))
    }

    fn coclass_prog_id_for_name(&self, name: &str) -> Option<String> {
        let folded = fold_identifier(name);
        let reference = fold_identifier(&self.blob.identity.reference_name);
        self.blob.coclass_names.iter().find_map(|coclass| {
            let folded_coclass = fold_identifier(coclass);
            if folded == folded_coclass || folded == format!("{reference}.{folded_coclass}") {
                Some(format!("{}.{}", self.blob.identity.reference_name, coclass))
            } else {
                None
            }
        })
    }

    fn member_by_name(
        &self,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<&TypeLibMemberMetadata> {
        match want {
            Some(kind) => self.blob.members.iter().find(|member| {
                member.name.eq_ignore_ascii_case(name)
                    && member_kind_from_invoke(member.invoke_kind) == kind
            }),
            None => self
                .blob
                .members
                .iter()
                .filter(|member| member.name.eq_ignore_ascii_case(name))
                .filter_map(|member| read_member_rank(member).map(|rank| (rank, member)))
                .min_by_key(|(rank, _)| *rank)
                .map(|(_, member)| member),
        }
    }

    fn default_member(&self, want: Option<ProjectMemberKind>) -> Option<&TypeLibMemberMetadata> {
        match want {
            Some(kind) => self.blob.members.iter().find(|member| {
                member.is_default_member && member_kind_from_invoke(member.invoke_kind) == kind
            }),
            None => self
                .blob
                .members
                .iter()
                .filter(|member| member.is_default_member)
                .filter_map(|member| read_member_rank(member).map(|rank| (rank, member)))
                .min_by_key(|(rank, _)| *rank)
                .map(|(_, member)| member),
        }
    }

    fn scoped_source_events(
        &self,
        type_name: &str,
    ) -> Option<Vec<&oxvba_com::TypeLibEventMetadata>> {
        if !self.owns_coclass_source(type_name) {
            return None;
        }
        let want = self.matched_coclass(type_name);
        Some(
            self.blob
                .events
                .iter()
                .filter(|event| match (&want, &event.coclass) {
                    (Some(want), Some(have)) => fold_identifier(have) == *want,
                    _ => true,
                })
                .collect(),
        )
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
        if self.owns_members(type_name)
            && let Some(member) = self.member_by_name(name, want)
        {
            return Some(com_member_binding(member, type_name));
        }

        if let Some(events) = self.scoped_source_events(type_name)
            && let Some(event) = events
                .into_iter()
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
        if !self.owns_members(type_name) {
            return None;
        }
        self.default_member(None)
            .map(|member| com_member_binding(member, type_name))
    }

    fn resolve_default_member_kind(
        &self,
        recv: &VarTypeRef,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        if !self.owns_members(type_name) {
            return None;
        }
        self.default_member(want)
            .map(|member| com_member_binding(member, type_name))
    }

    fn resolve_coclass(&self, name: &str) -> Option<String> {
        if !self.owns(name) {
            return None;
        }
        self.coclass_prog_id_for_name(name)
            .or_else(|| self.activation_prog_id().map(str::to_string))
    }

    fn is_known_object_type(&self, type_name: &str) -> bool {
        self.owns(type_name)
    }

    fn source_events(&self, recv: &VarTypeRef) -> Option<Vec<(String, i32)>> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        // The event `token` is the value the runtime keys subscriptions on (it is what
        // `subscribe_event` is passed), so the route built from this list routes a delivered
        // callback for that dispid back to the handler.
        //
        // A library-wide blob holds EVERY coclass's source events; scope to the coclass this
        // WithEvents field is typed as (e.g. `Excel.Worksheet` -> only Worksheet's events)
        // so the binder stops synthesizing phantom routes for unrelated coclasses (which the
        // runtime would then re-recover per subscribe and reject). Events with no recovered
        // coclass tag, or a coclass we cannot derive, are kept because the metadata gives no
        // receiver-specific basis for excluding them.
        self.scoped_source_events(type_name).map(|events| {
            events
                .into_iter()
                .map(|event| (fold_identifier(&event.name), event.token))
                .collect()
        })
    }
}

fn read_member_rank(member: &TypeLibMemberMetadata) -> Option<u8> {
    match member.invoke_kind {
        TypeLibMemberInvokeKind::PropertyGet => Some(0),
        TypeLibMemberInvokeKind::Method => Some(1),
        TypeLibMemberInvokeKind::PropertyPut | TypeLibMemberInvokeKind::PropertyPutRef => None,
    }
}

fn com_member_binding(member: &oxvba_com::TypeLibMemberMetadata, type_name: &str) -> Binding {
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
            // The declared receiver COM type — the de-erased early-bound call's
            // typed-interface grouping key — and the full canonical descriptor.
            interface_name: type_name.to_string(),
            member: Box::new(member.clone()),
        },
    }
}
