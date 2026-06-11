use oxvba_runtime::{
    RUNTIME_IDISPATCH_INTERFACE_IDENTITY, RuntimeClassDescriptor, RuntimeInterfaceDescriptor,
    RuntimeInterfaceId, RuntimeMemberDescriptor, RuntimeMemberInvokeKind, RuntimeParamDescriptor,
    RuntimeValueType,
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

impl TypeLibParamType {
    /// True for the `ByRef*` variants — the parameter is passed by reference
    /// (`[out]`/`[in,out]` in IDL), so the caller's argument is written back.
    pub fn is_by_ref(&self) -> bool {
        matches!(
            self,
            TypeLibParamType::ByRefVariant
                | TypeLibParamType::ByRefLong
                | TypeLibParamType::ByRefInteger
                | TypeLibParamType::ByRefString
                | TypeLibParamType::ByRefDouble
                | TypeLibParamType::ByRefSingle
                | TypeLibParamType::ByRefCurrency
                | TypeLibParamType::ByRefDate
                | TypeLibParamType::ByRefDecimal
                | TypeLibParamType::ByRefObject
                | TypeLibParamType::ByRefByte
                | TypeLibParamType::ByRefBoolean
                | TypeLibParamType::ByRefLongLong
                | TypeLibParamType::ByRefLongPtr
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibMemberMetadata {
    pub name: String,
    pub token: i32,
    /// x64 vtable **slot index** (NOT a byte offset). The live loader divides
    /// `FUNCDESC::oVft` by 8 before storing it here, so this value can be used
    /// directly to index `(*(*this))[slot]` on x64. `None` for members without
    /// a vtable slot (pure dispinterface members).
    pub vtable_slot: Option<u16>,
    pub requires_argument: bool,
    pub invoke_kind: TypeLibMemberInvokeKind,
    pub parameter_names: Vec<String>,
    pub parameter_optional: Vec<bool>,
    pub is_default_member: bool,
    pub parameter_types: Vec<TypeLibParamType>,
    pub return_type: Option<TypeLibParamType>,
    /// True when `FUNCDESC::callconv == CC_STDCALL` (4) — the only convention
    /// the x64 vtable marshaller may call through. Fixture/catalog metadata
    /// that never drives a real vtable call leaves this `false`.
    pub callconv_is_stdcall: bool,
    /// True when the member's containing type carries `TYPEFLAG_FDUAL`, i.e. it
    /// is reachable both via `IDispatch::Invoke` and a custom-interface vtable
    /// slot. Informational; the vtable gate keys on `vtable_slot` + callconv.
    pub is_dual: bool,
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
        .map(|member| {
            let params = member
                .parameter_names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let (value_type, by_ref) = member
                        .parameter_types
                        .get(index)
                        .copied()
                        .map(runtime_value_type_from_typelib_param)
                        .unwrap_or((RuntimeValueType::Variant, false));
                    RuntimeParamDescriptor {
                        name: leak_typelib_runtime_descriptor_str(name.clone()),
                        value_type,
                        by_ref,
                        optional: member
                            .parameter_optional
                            .get(index)
                            .copied()
                            .unwrap_or(false),
                        param_array: false,
                    }
                })
                .collect::<Vec<_>>();
            RuntimeMemberDescriptor {
                name: leak_typelib_runtime_descriptor_str(member.name.clone()),
                dispatch_id: member.token,
                vtable_slot: member.vtable_slot,
                invoke_kind: runtime_invoke_kind_from_typelib_member(member.invoke_kind),
                arity: member.parameter_names.len(),
                params: Box::leak(params.into_boxed_slice()),
                return_type: member
                    .return_type
                    .map(|return_type| runtime_value_type_from_typelib_param(return_type).0),
                is_default_member: member.is_default_member,
            }
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
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: interface_name,
            members,
            dual_dispatch: metadata
                .members
                .iter()
                .any(|member| member.vtable_slot.is_some()),
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

fn runtime_value_type_from_typelib_param(param_type: TypeLibParamType) -> (RuntimeValueType, bool) {
    match param_type {
        TypeLibParamType::Variant => (RuntimeValueType::Variant, false),
        TypeLibParamType::Long => (RuntimeValueType::Long, false),
        TypeLibParamType::Integer => (RuntimeValueType::Integer, false),
        TypeLibParamType::String => (RuntimeValueType::String, false),
        TypeLibParamType::Boolean => (RuntimeValueType::Boolean, false),
        TypeLibParamType::Double => (RuntimeValueType::Double, false),
        TypeLibParamType::Single => (RuntimeValueType::Single, false),
        TypeLibParamType::Currency => (RuntimeValueType::Currency, false),
        TypeLibParamType::Date => (RuntimeValueType::Date, false),
        TypeLibParamType::Decimal => (RuntimeValueType::Decimal, false),
        TypeLibParamType::Object => (RuntimeValueType::Object, false),
        TypeLibParamType::Byte => (RuntimeValueType::Byte, false),
        TypeLibParamType::LongLong => (RuntimeValueType::LongLong, false),
        TypeLibParamType::LongPtr => (RuntimeValueType::LongPtr, false),
        TypeLibParamType::ByRefVariant => (RuntimeValueType::Variant, true),
        TypeLibParamType::ByRefLong => (RuntimeValueType::Long, true),
        TypeLibParamType::ByRefInteger => (RuntimeValueType::Integer, true),
        TypeLibParamType::ByRefString => (RuntimeValueType::String, true),
        TypeLibParamType::ByRefDouble => (RuntimeValueType::Double, true),
        TypeLibParamType::ByRefSingle => (RuntimeValueType::Single, true),
        TypeLibParamType::ByRefCurrency => (RuntimeValueType::Currency, true),
        TypeLibParamType::ByRefDate => (RuntimeValueType::Date, true),
        TypeLibParamType::ByRefDecimal => (RuntimeValueType::Decimal, true),
        TypeLibParamType::ByRefObject => (RuntimeValueType::Object, true),
        TypeLibParamType::ByRefByte => (RuntimeValueType::Byte, true),
        TypeLibParamType::ByRefBoolean => (RuntimeValueType::Boolean, true),
        TypeLibParamType::ByRefLongLong => (RuntimeValueType::LongLong, true),
        TypeLibParamType::ByRefLongPtr => (RuntimeValueType::LongPtr, true),
    }
}

fn leak_typelib_runtime_descriptor_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use oxvba_runtime::{RuntimeInterfaceId, RuntimeMemberInvokeKind, RuntimeValueType};

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
                    vtable_slot: Some(7),
                    requires_argument: false,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                    parameter_names: Vec::new(),
                    parameter_optional: Vec::new(),
                    is_default_member: true,
                    parameter_types: Vec::new(),
                    return_type: Some(TypeLibParamType::Long),
                    callconv_is_stdcall: true,
                    is_dual: true,
                },
                TypeLibMemberMetadata {
                    name: "Item".to_string(),
                    token: 5,
                    vtable_slot: None,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                    parameter_names: vec!["index".to_string()],
                    parameter_optional: vec![true],
                    is_default_member: false,
                    parameter_types: vec![TypeLibParamType::Variant],
                    return_type: Some(TypeLibParamType::Variant),
                    callconv_is_stdcall: false,
                    is_dual: true,
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
            dispatch.dual_dispatch,
            "explicit vtable slots project a dual-interface descriptor"
        );
        assert_eq!(dispatch.name, "OxVba.TestDispatch._Dispatch");
        assert_eq!(dispatch.members.len(), 2);
        assert_eq!(dispatch.members[0].name, "Value");
        assert_eq!(dispatch.members[0].dispatch_id, 0);
        assert_eq!(dispatch.members[0].vtable_slot, Some(7));
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
        assert_eq!(dispatch.members[1].params.len(), 1);
        assert_eq!(dispatch.members[1].params[0].name, "index");
        assert_eq!(
            dispatch.members[1].params[0].value_type,
            RuntimeValueType::Variant
        );
        assert!(!dispatch.members[1].params[0].by_ref);
        assert!(dispatch.members[1].params[0].optional);
        assert_eq!(
            dispatch.members[1].return_type,
            Some(RuntimeValueType::Variant)
        );
        assert_eq!(
            dispatch.members[0].return_type,
            Some(RuntimeValueType::Long)
        );
    }
}
