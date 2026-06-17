use oxvba_com::{
    ComInterfaceIid, TypeLibInterfaceMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata,
    TypeLibParamType, TypeLibResolveRequest, TypeLibWireType,
};
use oxvba_symbol::manifest::ProjectReference;
use oxvba_symbol::surface::{ProjectExportSurface, SurfaceTypeKind};

use crate::identity::deterministic_uuid;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComServerDescriptor {
    pub project_name: String,
    pub libid: String,
    pub version_major: u16,
    pub version_minor: u16,
    pub classes: Vec<ComClassDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComClassDescriptor {
    pub class_name: String,
    pub description: Option<String>,
    pub prog_id: String,
    pub creatable: bool,
    pub clsid: String,
    pub default_interface_name: String,
    pub default_interface_iid: String,
    pub source_interface_name: Option<String>,
    pub source_interface_iid: Option<String>,
    pub implemented_interfaces: Vec<ComImplementedInterfaceDescriptor>,
    pub members: Vec<ComMemberDescriptor>,
    pub events: Vec<ComEventDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComImplementedInterfaceDescriptor {
    pub name: String,
    pub iid: String,
    pub methods: Vec<ComImplementedInterfaceMethodDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComImplementedInterfaceMethodDescriptor {
    pub name: String,
    pub vba_name: String,
    pub dispid: i32,
    pub vtable_slot: Option<u16>,
    pub invoke_kind: ComInvokeKind,
    pub parameter_names: Vec<String>,
    pub parameter_types: Vec<ComParamType>,
    pub parameter_wire_types: Vec<ComWireType>,
    pub parameter_optional: Vec<bool>,
    pub return_type: Option<ComParamType>,
    pub return_wire_type: Option<ComWireType>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComMemberDescriptor {
    pub name: String,
    pub dispid: i32,
    pub vtable_slot: Option<u16>,
    pub invoke_kind: ComInvokeKind,
    pub is_default: bool,
    pub parameter_names: Vec<String>,
    pub parameter_types: Vec<ComParamType>,
    pub parameter_optional: Vec<bool>,
    pub return_type: Option<ComParamType>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComEventDescriptor {
    pub name: String,
    pub dispid: i32,
    pub callback_arity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ComInvokeKind {
    PropertyGet,
    Method,
    PropertyPut,
    PropertyPutRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ComParamType {
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ComWireType {
    Automation(ComParamType),
    InterfacePointer { name: String, iid: String },
    SafeArrayVariant,
    ByRefSafeArrayVariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComNativeInterfaceRequirement {
    AutomationValue,
    ScalarByRefWriteback,
    TypedInterfacePointer,
    SafeArrayVariant,
    SafeArrayByRefWriteback,
}

impl ComServerDescriptor {
    pub fn from_surface(surface: &ProjectExportSurface) -> Self {
        Self::from_surface_with_references(surface, &[])
    }

    pub fn from_surface_with_references(
        surface: &ProjectExportSurface,
        references: &[ProjectReference],
    ) -> Self {
        let project_name = surface.project_name.clone();
        let classes = surface
            .types
            .iter()
            .filter_map(|ty| {
                let SurfaceTypeKind::Coclass {
                    prog_id, creatable, ..
                } = &ty.kind
                else {
                    return None;
                };
                let class_name = ty.name.clone();
                let default_prog_id = format!("{}.{}", surface.project_name, class_name);
                let source_interface_name =
                    (!ty.events.is_empty()).then(|| format!("_{}Events", class_name));
                let source_interface_iid = source_interface_name
                    .as_ref()
                    .map(|name| deterministic_uuid(&surface.project_name, name));
                let implemented_interfaces =
                    implemented_interface_descriptors(&ty.implements, references);
                Some(ComClassDescriptor {
                    class_name: class_name.clone(),
                    description: ty.description.clone(),
                    prog_id: prog_id.clone().unwrap_or(default_prog_id),
                    creatable: *creatable,
                    clsid: deterministic_uuid(&surface.project_name, &class_name),
                    default_interface_name: format!("I{class_name}"),
                    default_interface_iid: deterministic_uuid(
                        &surface.project_name,
                        &format!("I{class_name}"),
                    ),
                    source_interface_name,
                    source_interface_iid,
                    implemented_interfaces,
                    members: ty
                        .members
                        .iter()
                        .map(|member| ComMemberDescriptor {
                            name: member.name.clone(),
                            dispid: member.dispid,
                            vtable_slot: member.vtable_slot,
                            invoke_kind: map_invoke_kind(member.invoke_kind),
                            is_default: member.is_default,
                            parameter_names: member.parameter_names.clone(),
                            parameter_types: member
                                .parameter_types
                                .iter()
                                .copied()
                                .map(map_param_type)
                                .collect(),
                            parameter_optional: member.parameter_optional.clone(),
                            return_type: member.return_type.map(map_param_type),
                        })
                        .collect(),
                    events: ty
                        .events
                        .iter()
                        .map(|event| ComEventDescriptor {
                            name: event.name.clone(),
                            dispid: event.event_id,
                            callback_arity: event.callback_arity,
                        })
                        .collect(),
                })
            })
            .collect();
        Self {
            project_name: project_name.clone(),
            libid: deterministic_uuid(&project_name, "__typelib__"),
            version_major: 1,
            version_minor: 0,
            classes,
        }
    }

    pub fn creatable_classes(&self) -> impl Iterator<Item = &ComClassDescriptor> {
        self.classes.iter().filter(|class| class.creatable)
    }
}

impl ComImplementedInterfaceDescriptor {
    pub fn native_requirements(&self) -> Vec<ComNativeInterfaceRequirement> {
        let mut requirements = Vec::new();
        for method in &self.methods {
            for wire_type in &method.parameter_wire_types {
                push_wire_requirement(&mut requirements, wire_type);
            }
            if let Some(wire_type) = &method.return_wire_type {
                push_wire_requirement(&mut requirements, wire_type);
            }
        }
        requirements.sort();
        requirements.dedup();
        requirements
    }
}

impl ComParamType {
    pub fn is_by_ref(self) -> bool {
        matches!(
            self,
            Self::ByRefVariant
                | Self::ByRefLong
                | Self::ByRefInteger
                | Self::ByRefString
                | Self::ByRefDouble
                | Self::ByRefSingle
                | Self::ByRefCurrency
                | Self::ByRefDate
                | Self::ByRefDecimal
                | Self::ByRefObject
                | Self::ByRefByte
                | Self::ByRefBoolean
                | Self::ByRefLongLong
                | Self::ByRefLongPtr
        )
    }
}

fn push_wire_requirement(
    requirements: &mut Vec<ComNativeInterfaceRequirement>,
    wire_type: &ComWireType,
) {
    let requirement = match wire_type {
        ComWireType::Automation(param) if param.is_by_ref() => {
            ComNativeInterfaceRequirement::ScalarByRefWriteback
        }
        ComWireType::Automation(_) => ComNativeInterfaceRequirement::AutomationValue,
        ComWireType::InterfacePointer { .. } => {
            ComNativeInterfaceRequirement::TypedInterfacePointer
        }
        ComWireType::SafeArrayVariant => ComNativeInterfaceRequirement::SafeArrayVariant,
        ComWireType::ByRefSafeArrayVariant => {
            ComNativeInterfaceRequirement::SafeArrayByRefWriteback
        }
    };
    requirements.push(requirement);
}

fn implemented_interface_descriptors(
    implements: &[String],
    references: &[ProjectReference],
) -> Vec<ComImplementedInterfaceDescriptor> {
    let mut interfaces = Vec::new();
    for iface in implements {
        let (reference_qualifier, bare) = qualified_interface_name(iface);
        if let Some(interface) = resolve_referenced_interface_descriptor(
            reference_qualifier.as_deref(),
            &bare,
            references,
        ) && !interfaces
            .iter()
            .any(|existing: &ComImplementedInterfaceDescriptor| existing.iid == interface.iid)
        {
            interfaces.push(interface);
        }
    }
    interfaces
}

fn qualified_interface_name(raw: &str) -> (Option<String>, String) {
    let trimmed = raw.trim();
    if let Some((qualifier, name)) = trimmed.rsplit_once('.') {
        let qualifier = qualifier.trim();
        let name = name.trim();
        if !qualifier.is_empty() && !name.is_empty() {
            return (Some(qualifier.to_string()), name.to_string());
        }
    }
    (None, bare_interface_name(trimmed))
}

fn bare_interface_name(raw: &str) -> String {
    raw.rsplit(['.', ':'])
        .next()
        .unwrap_or(raw)
        .trim()
        .to_string()
}

fn resolve_referenced_interface_descriptor(
    reference_qualifier: Option<&str>,
    interface_name: &str,
    references: &[ProjectReference],
) -> Option<ComImplementedInterfaceDescriptor> {
    let mut candidate_names = vec![interface_name.to_string()];
    if !interface_name.starts_with('_') {
        candidate_names.push(format!("_{interface_name}"));
    }
    for reference in references {
        if let Some(reference_qualifier) = reference_qualifier
            && !project_reference_name_matches(reference, reference_qualifier)
        {
            continue;
        }
        let Some(request) = typelib_request_from_reference(reference) else {
            continue;
        };
        for candidate_name in &candidate_names {
            if let Some(metadata) =
                oxvba_com::resolve_typelib_interface_metadata(&request, candidate_name)
                && let Some(descriptor) =
                    interface_descriptor_from_typelib(metadata, interface_name)
            {
                return Some(descriptor);
            }
        }
    }
    None
}

fn project_reference_name_matches(reference: &ProjectReference, qualifier: &str) -> bool {
    match reference {
        ProjectReference::TypeLibrary { name, .. } => name.eq_ignore_ascii_case(qualifier),
        _ => false,
    }
}

fn typelib_request_from_reference(reference: &ProjectReference) -> Option<TypeLibResolveRequest> {
    match reference {
        ProjectReference::TypeLibrary {
            name,
            guid,
            version_major,
            version_minor,
            lcid,
            import_lib,
        } => Some(TypeLibResolveRequest {
            reference_name: name.clone(),
            requested_coclass: None,
            importlib_hint: import_lib.clone(),
            libid_hint: guid.clone(),
            major_version_hint: *version_major,
            minor_version_hint: *version_minor,
            lcid_hint: *lcid,
        }),
        _ => None,
    }
}

fn interface_descriptor_from_typelib(
    metadata: TypeLibInterfaceMetadata,
    vba_interface_name: &str,
) -> Option<ComImplementedInterfaceDescriptor> {
    let iid = metadata.iid.map(format_iid)?;
    let methods = metadata
        .members
        .iter()
        .filter(|member| implemented_typelib_member_is_user_contract(member))
        .map(|member| implemented_method_from_typelib_member(vba_interface_name, member))
        .collect::<Vec<_>>();
    (!methods.is_empty()).then_some(ComImplementedInterfaceDescriptor {
        name: vba_interface_name.to_string(),
        iid,
        methods,
    })
}

fn implemented_typelib_member_is_user_contract(member: &TypeLibMemberMetadata) -> bool {
    if let Some(slot) = member.vtable_slot {
        return slot >= 7;
    }
    !matches!(
        member.name.as_str(),
        "QueryInterface"
            | "AddRef"
            | "Release"
            | "GetTypeInfoCount"
            | "GetTypeInfo"
            | "GetIDsOfNames"
            | "Invoke"
    )
}

fn implemented_method_from_typelib_member(
    interface_name: &str,
    member: &TypeLibMemberMetadata,
) -> ComImplementedInterfaceMethodDescriptor {
    let parameter_types = member
        .parameter_types
        .iter()
        .copied()
        .map(map_param_type)
        .collect::<Vec<_>>();
    let parameter_wire_types = member
        .parameter_types
        .iter()
        .copied()
        .enumerate()
        .map(|(index, param_type)| {
            map_typelib_wire_type(
                member
                    .parameter_wire_types
                    .get(index)
                    .cloned()
                    .unwrap_or(TypeLibWireType::Automation(param_type)),
                member.parameter_iids.get(index).copied().flatten(),
            )
        })
        .collect::<Vec<_>>();
    ComImplementedInterfaceMethodDescriptor {
        name: member.name.clone(),
        vba_name: format!("{}_{}", interface_name, member.name),
        dispid: member.token,
        vtable_slot: member.vtable_slot,
        invoke_kind: map_invoke_kind(member.invoke_kind),
        parameter_names: member.parameter_names.clone(),
        parameter_types,
        parameter_wire_types,
        parameter_optional: member.parameter_optional.clone(),
        return_type: member.return_type.map(map_param_type),
        return_wire_type: member.return_type.map(|return_type| {
            map_typelib_wire_type(
                member
                    .return_wire_type
                    .clone()
                    .unwrap_or(TypeLibWireType::Automation(return_type)),
                None,
            )
        }),
    }
}

fn map_typelib_wire_type(wire_type: TypeLibWireType, iid: Option<ComInterfaceIid>) -> ComWireType {
    match wire_type {
        TypeLibWireType::InterfacePointer { name } => ComWireType::InterfacePointer {
            name,
            iid: iid.map(format_iid).unwrap_or_default(),
        },
        TypeLibWireType::Automation(param) => ComWireType::Automation(map_param_type(param)),
        TypeLibWireType::SafeArrayVariant => ComWireType::SafeArrayVariant,
        TypeLibWireType::ByRefSafeArrayVariant => ComWireType::ByRefSafeArrayVariant,
    }
}

fn format_iid(iid: ComInterfaceIid) -> String {
    format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        iid.data1,
        iid.data2,
        iid.data3,
        iid.data4[0],
        iid.data4[1],
        iid.data4[2],
        iid.data4[3],
        iid.data4[4],
        iid.data4[5],
        iid.data4[6],
        iid.data4[7]
    )
}

fn map_invoke_kind(kind: TypeLibMemberInvokeKind) -> ComInvokeKind {
    match kind {
        TypeLibMemberInvokeKind::PropertyGet => ComInvokeKind::PropertyGet,
        TypeLibMemberInvokeKind::Method => ComInvokeKind::Method,
        TypeLibMemberInvokeKind::PropertyPut => ComInvokeKind::PropertyPut,
        TypeLibMemberInvokeKind::PropertyPutRef => ComInvokeKind::PropertyPutRef,
    }
}

fn map_param_type(param: TypeLibParamType) -> ComParamType {
    match param {
        TypeLibParamType::Variant => ComParamType::Variant,
        TypeLibParamType::Long => ComParamType::Long,
        TypeLibParamType::Integer => ComParamType::Integer,
        TypeLibParamType::String => ComParamType::String,
        TypeLibParamType::Boolean => ComParamType::Boolean,
        TypeLibParamType::Double => ComParamType::Double,
        TypeLibParamType::Single => ComParamType::Single,
        TypeLibParamType::Currency => ComParamType::Currency,
        TypeLibParamType::Date => ComParamType::Date,
        TypeLibParamType::Decimal => ComParamType::Decimal,
        TypeLibParamType::Object => ComParamType::Object,
        TypeLibParamType::Byte => ComParamType::Byte,
        TypeLibParamType::LongLong => ComParamType::LongLong,
        TypeLibParamType::LongPtr => ComParamType::LongPtr,
        TypeLibParamType::ByRefVariant => ComParamType::ByRefVariant,
        TypeLibParamType::ByRefLong => ComParamType::ByRefLong,
        TypeLibParamType::ByRefInteger => ComParamType::ByRefInteger,
        TypeLibParamType::ByRefString => ComParamType::ByRefString,
        TypeLibParamType::ByRefDouble => ComParamType::ByRefDouble,
        TypeLibParamType::ByRefSingle => ComParamType::ByRefSingle,
        TypeLibParamType::ByRefCurrency => ComParamType::ByRefCurrency,
        TypeLibParamType::ByRefDate => ComParamType::ByRefDate,
        TypeLibParamType::ByRefDecimal => ComParamType::ByRefDecimal,
        TypeLibParamType::ByRefObject => ComParamType::ByRefObject,
        TypeLibParamType::ByRefByte => ComParamType::ByRefByte,
        TypeLibParamType::ByRefBoolean => ComParamType::ByRefBoolean,
        TypeLibParamType::ByRefLongLong => ComParamType::ByRefLongLong,
        TypeLibParamType::ByRefLongPtr => ComParamType::ByRefLongPtr,
    }
}
