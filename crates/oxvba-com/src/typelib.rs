use oxvba_runtime::{
    RuntimeClassDescriptor, RuntimeInterfaceDescriptor, RuntimeInterfaceId,
    RuntimeMemberDescriptor, RuntimeMemberInvokeKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibResolveRequest {
    pub reference_name: String,
    pub requested_coclass: Option<String>,
    pub importlib_hint: Option<String>,
    pub libid_hint: Option<String>,
    pub major_version_hint: Option<u16>,
    pub minor_version_hint: Option<u16>,
    pub lcid_hint: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibResolvedIdentity {
    pub reference_name: String,
    pub requested_coclass: Option<String>,
    pub importlib: String,
    pub libid: Option<String>,
    pub major_version: u16,
    pub minor_version: u16,
    pub lcid: Option<u32>,
    pub cache_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibMetadataBlob {
    pub identity: TypeLibResolvedIdentity,
    pub activation_prog_id: Option<String>,
    pub member_name_to_token: Vec<(String, i32)>,
    pub members: Vec<TypeLibMemberMetadata>,
    pub events: Vec<TypeLibEventMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibParamType {
    Variant,
    Long,
    Integer,
    String,
    Boolean,
    Double,
    Single,
    Currency,
    Date,
    Decimal,
    Object,
    Byte,
    LongLong,
    LongPtr,
    ByRefVariant,
    ByRefLong,
    ByRefInteger,
    ByRefString,
    ByRefDouble,
    ByRefSingle,
    ByRefCurrency,
    ByRefDate,
    ByRefDecimal,
    ByRefObject,
    ByRefByte,
    ByRefBoolean,
    ByRefLongLong,
    ByRefLongPtr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibMemberMetadata {
    pub name: String,
    pub token: i32,
    pub requires_argument: bool,
    pub invoke_kind: TypeLibMemberInvokeKind,
    pub parameter_names: Vec<String>,
    pub is_default_member: bool,
    pub parameter_types: Vec<TypeLibParamType>,
    pub return_type: Option<TypeLibParamType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibMemberInvokeKind {
    PropertyGet,
    Method,
    PropertyPut,
    PropertyPutRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibEventDispatchPath {
    Dispatch,
    SourceInterface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibEventMetadata {
    pub name: String,
    pub token: i32,
    pub callback_arity: u8,
    pub dispatch_path: TypeLibEventDispatchPath,
    pub connection_point_iid: Option<String>,
    pub dispatch_member_id: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibCacheScope {
    Global,
    Reference,
}

pub fn runtime_class_descriptor_from_typelib_metadata(
    metadata: &TypeLibMetadataBlob,
) -> &'static RuntimeClassDescriptor {
    let members = metadata
        .members
        .iter()
        .map(|member| RuntimeMemberDescriptor {
            name: leak_typelib_runtime_descriptor_str(member.name.clone()),
            dispatch_id: member.token,
            vtable_slot: None,
            invoke_kind: runtime_invoke_kind_from_typelib_member(member.invoke_kind),
            arity: member.parameter_names.len(),
            is_default_member: member.is_default_member,
        })
        .collect::<Vec<_>>();
    let class_name = leak_typelib_runtime_descriptor_str(
        metadata
            .activation_prog_id
            .clone()
            .unwrap_or_else(|| metadata.identity.reference_name.clone()),
    );
    let interface_name = leak_typelib_runtime_descriptor_str(format!("{class_name}._Dispatch"));
    let members = Box::leak(members.into_boxed_slice());
    let interfaces = Box::leak(
        vec![RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            name: interface_name,
            members,
            dual_dispatch: false,
        }]
        .into_boxed_slice(),
    );
    Box::leak(Box::new(RuntimeClassDescriptor {
        name: class_name,
        interfaces,
    }))
}

fn runtime_invoke_kind_from_typelib_member(
    kind: TypeLibMemberInvokeKind,
) -> RuntimeMemberInvokeKind {
    match kind {
        TypeLibMemberInvokeKind::Method => RuntimeMemberInvokeKind::Method,
        TypeLibMemberInvokeKind::PropertyGet => RuntimeMemberInvokeKind::PropertyGet,
        TypeLibMemberInvokeKind::PropertyPut => RuntimeMemberInvokeKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPutRef => RuntimeMemberInvokeKind::PropertySet,
    }
}

fn leak_typelib_runtime_descriptor_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use oxvba_runtime::{RuntimeInterfaceId, RuntimeMemberInvokeKind};

    use super::*;

    fn identity() -> TypeLibResolvedIdentity {
        TypeLibResolvedIdentity {
            reference_name: "TestLib".to_string(),
            requested_coclass: Some("Widget".to_string()),
            importlib: "TestLib".to_string(),
            libid: Some("{00000000-0000-0000-0000-000000000001}".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "TestLib:1.0".to_string(),
        }
    }

    #[test]
    fn typelib_metadata_projects_to_runtime_dispatch_descriptor() {
        let metadata = TypeLibMetadataBlob {
            identity: identity(),
            activation_prog_id: Some("OxVba.TestDispatch".to_string()),
            member_name_to_token: vec![("Value".to_string(), 0)],
            members: vec![
                TypeLibMemberMetadata {
                    name: "Value".to_string(),
                    token: 0,
                    requires_argument: false,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                    parameter_names: Vec::new(),
                    is_default_member: true,
                    parameter_types: Vec::new(),
                    return_type: Some(TypeLibParamType::Long),
                },
                TypeLibMemberMetadata {
                    name: "Item".to_string(),
                    token: 5,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                    parameter_names: vec!["index".to_string()],
                    is_default_member: false,
                    parameter_types: vec![TypeLibParamType::Variant],
                    return_type: Some(TypeLibParamType::Variant),
                },
            ],
            events: Vec::new(),
        };

        let descriptor = runtime_class_descriptor_from_typelib_metadata(&metadata);
        assert_eq!(descriptor.name, "OxVba.TestDispatch");
        assert_eq!(descriptor.interfaces.len(), 1);
        let dispatch = descriptor
            .interfaces
            .iter()
            .find(|interface| interface.id == RuntimeInterfaceId::IDispatch)
            .expect("typelib projection should expose dispatch descriptor metadata");
        assert!(
            !dispatch.dual_dispatch,
            "vtable slots are not inferred from this metadata yet"
        );
        assert_eq!(dispatch.name, "OxVba.TestDispatch._Dispatch");
        assert_eq!(dispatch.members.len(), 2);
        assert_eq!(dispatch.members[0].name, "Value");
        assert_eq!(dispatch.members[0].dispatch_id, 0);
        assert_eq!(dispatch.members[0].vtable_slot, None);
        assert_eq!(
            dispatch.members[0].invoke_kind,
            RuntimeMemberInvokeKind::PropertyGet
        );
        assert_eq!(dispatch.members[0].arity, 0);
        assert!(dispatch.members[0].is_default_member);
        assert_eq!(dispatch.members[1].name, "Item");
        assert_eq!(dispatch.members[1].dispatch_id, 5);
        assert_eq!(
            dispatch.members[1].invoke_kind,
            RuntimeMemberInvokeKind::Method
        );
        assert_eq!(dispatch.members[1].arity, 1);
    }
}
