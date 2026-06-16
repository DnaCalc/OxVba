use crate::com_descriptor::{
    ComClassDescriptor, ComEventDescriptor, ComInvokeKind, ComMemberDescriptor, ComParamType,
    ComServerDescriptor,
};

pub fn generate_idl(descriptor: &ComServerDescriptor) -> String {
    let library_name = sanitize_ident(&format!("{}Lib", descriptor.project_name));
    let mut idl = String::new();
    idl.push_str(&format!(
        r#"// Auto-generated OxVBA IDL for {project}

import "oaidl.idl";
import "ocidl.idl";

[
    uuid({libid}),
    version({major}.{minor}),
    helpstring("{project} Type Library")
]
library {library_name}
{{
    importlib("stdole2.tlb");

"#,
        project = escape_helpstring(&descriptor.project_name),
        libid = descriptor.libid,
        major = descriptor.version_major,
        minor = descriptor.version_minor,
    ));

    for class in &descriptor.classes {
        idl.push_str(&generate_class_idl(class));
    }

    idl.push_str("};\n");
    idl
}

fn generate_class_idl(class: &ComClassDescriptor) -> String {
    let mut idl = String::new();
    let interface_name = sanitize_ident(&class.default_interface_name);
    let class_name = sanitize_ident(&class.class_name);
    let help = escape_helpstring(class.description.as_deref().unwrap_or(&class.class_name));

    idl.push_str(&format!(
        r#"    [
        uuid({iid}),
        dual,
        oleautomation,
        helpstring("{help} Interface")
    ]
    interface {interface_name} : IDispatch
    {{
"#,
        iid = class.default_interface_iid,
    ));
    for member in &class.members {
        idl.push_str(&generate_member_idl(member));
    }
    idl.push_str("    };\n\n");

    if let (Some(source_name), Some(source_iid)) = (
        class.source_interface_name.as_ref(),
        class.source_interface_iid.as_ref(),
    ) {
        let source_name = sanitize_ident(source_name);
        idl.push_str(&format!(
            r#"    [
        uuid({source_iid}),
        helpstring("{help} Events")
    ]
    dispinterface {source_name}
    {{
        properties:
        methods:
"#
        ));
        for event in &class.events {
            idl.push_str(&generate_event_idl(event));
        }
        idl.push_str("    };\n\n");
    }

    idl.push_str(&format!(
        r#"    [
        uuid({clsid}),
        helpstring("{help}")
    ]
    coclass {class_name}
    {{
        [default] interface {interface_name};
"#,
        clsid = class.clsid,
    ));
    if let Some(source_name) = class.source_interface_name.as_ref() {
        idl.push_str(&format!(
            "        [default, source] dispinterface {};\n",
            sanitize_ident(source_name)
        ));
    }
    idl.push_str("    };\n\n");
    idl
}

fn generate_member_idl(member: &ComMemberDescriptor) -> String {
    let name = sanitize_ident(&member.name);
    let attr = member_attributes(member);
    let mut params: Vec<String> = member
        .parameter_types
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let name = member
                .parameter_names
                .get(index)
                .filter(|name| !name.trim().is_empty())
                .map(|name| sanitize_ident(name))
                .unwrap_or_else(|| format!("arg{index}"));
            if param.is_by_ref() {
                format!("[in, out] {}* {name}", idl_type(*param))
            } else {
                format!("[in] {} {name}", idl_type(*param))
            }
        })
        .collect();

    match member.invoke_kind {
        ComInvokeKind::PropertyGet | ComInvokeKind::Method => {
            if let Some(return_type) = member.return_type {
                params.push(format!("[out, retval] {}* pRetVal", idl_type(return_type)));
            } else if member.invoke_kind == ComInvokeKind::PropertyGet {
                params.push("[out, retval] VARIANT* pRetVal".to_string());
            }
            format!("        {attr} HRESULT {name}({});\n", params.join(", "))
        }
        ComInvokeKind::PropertyPut | ComInvokeKind::PropertyPutRef => {
            format!("        {attr} HRESULT {name}({});\n", params.join(", "))
        }
    }
}

fn generate_event_idl(event: &ComEventDescriptor) -> String {
    let name = sanitize_ident(&event.name);
    let params = (0..event.callback_arity)
        .map(|index| format!("[in] VARIANT arg{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "        [id({})] void {}({});\n",
        event.dispid, name, params
    )
}

fn member_attributes(member: &ComMemberDescriptor) -> String {
    let mut attrs = vec![format!("id({})", member.dispid)];
    match member.invoke_kind {
        ComInvokeKind::PropertyGet => attrs.push("propget".to_string()),
        ComInvokeKind::PropertyPut => attrs.push("propput".to_string()),
        ComInvokeKind::PropertyPutRef => attrs.push("propputref".to_string()),
        ComInvokeKind::Method => {}
    }
    if member.is_default {
        attrs.push("defaultbind".to_string());
    }
    format!("[{}]", attrs.join(", "))
}

fn idl_type(param: ComParamType) -> &'static str {
    match param.base_type() {
        ComParamType::Long => "long",
        ComParamType::Integer => "short",
        ComParamType::String => "BSTR",
        ComParamType::Boolean => "VARIANT_BOOL",
        ComParamType::Double => "double",
        ComParamType::Single => "float",
        ComParamType::Currency => "CY",
        ComParamType::Date => "DATE",
        ComParamType::Decimal => "DECIMAL",
        ComParamType::Object => "IDispatch*",
        ComParamType::Byte => "unsigned char",
        ComParamType::LongLong | ComParamType::LongPtr => "hyper",
        _ => "VARIANT",
    }
}

impl ComParamType {
    fn is_by_ref(self) -> bool {
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

    fn base_type(self) -> Self {
        match self {
            Self::ByRefVariant => Self::Variant,
            Self::ByRefLong => Self::Long,
            Self::ByRefInteger => Self::Integer,
            Self::ByRefString => Self::String,
            Self::ByRefDouble => Self::Double,
            Self::ByRefSingle => Self::Single,
            Self::ByRefCurrency => Self::Currency,
            Self::ByRefDate => Self::Date,
            Self::ByRefDecimal => Self::Decimal,
            Self::ByRefObject => Self::Object,
            Self::ByRefByte => Self::Byte,
            Self::ByRefBoolean => Self::Boolean,
            Self::ByRefLongLong => Self::LongLong,
            Self::ByRefLongPtr => Self::LongPtr,
            other => other,
        }
    }
}

fn sanitize_ident(raw: &str) -> String {
    let mut out = String::new();
    for (index, ch) in raw.chars().enumerate() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            if index == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_OxVbaName".to_string()
    } else {
        out
    }
}

fn escape_helpstring(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}
