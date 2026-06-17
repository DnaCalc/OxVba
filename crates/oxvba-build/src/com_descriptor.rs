use oxvba_com::{TypeLibMemberInvokeKind, TypeLibParamType};
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
    pub implemented_interfaces: Vec<ComImplementedInterfaceProfile>,
    pub members: Vec<ComMemberDescriptor>,
    pub events: Vec<ComEventDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ComImplementedInterfaceProfile {
    IdtExtensibility2,
    IRtdServer,
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

impl ComServerDescriptor {
    pub fn from_surface(surface: &ProjectExportSurface) -> Self {
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
                let implemented_interfaces = implemented_interface_profiles(&ty.implements);
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

impl ComImplementedInterfaceProfile {
    pub fn vba_name(self) -> &'static str {
        match self {
            Self::IdtExtensibility2 => "IDTExtensibility2",
            Self::IRtdServer => "IRtdServer",
        }
    }
}

fn implemented_interface_profiles(implements: &[String]) -> Vec<ComImplementedInterfaceProfile> {
    let mut profiles = Vec::new();
    for iface in implements {
        if let Some(profile) = implemented_interface_profile(iface)
            && !profiles.contains(&profile)
        {
            profiles.push(profile);
        }
    }
    profiles
}

fn implemented_interface_profile(raw: &str) -> Option<ComImplementedInterfaceProfile> {
    let bare = raw
        .rsplit(['.', ':'])
        .next()
        .unwrap_or(raw)
        .trim()
        .replace('_', "");
    if bare.eq_ignore_ascii_case("IDTExtensibility2") {
        Some(ComImplementedInterfaceProfile::IdtExtensibility2)
    } else if bare.eq_ignore_ascii_case("IRtdServer") || bare.eq_ignore_ascii_case("IRTDServer") {
        Some(ComImplementedInterfaceProfile::IRtdServer)
    } else {
        None
    }
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
