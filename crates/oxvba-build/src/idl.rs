use crate::com_descriptor::{
    ComClassDescriptor, ComEventDescriptor, ComImplementedInterfaceDescriptor,
    ComImplementedInterfaceMethodDescriptor, ComInvokeKind, ComMemberDescriptor, ComParamType,
    ComServerDescriptor, ComWireType,
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

    for interface in implemented_interfaces(descriptor) {
        idl.push_str(&generate_implemented_interface_idl(interface));
    }

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

    if class_supports_bounded_dual_interface(class) {
        idl.push_str(&format!(
            r#"    [
        object,
        uuid({iid}),
        dual,
        nonextensible,
        oleautomation,
        pointer_default(unique),
        helpstring("{help} Interface")
    ]
    interface {interface_name} : IDispatch
    {{
"#,
            iid = class.default_interface_iid,
        ));
        for member in &class.members {
            idl.push_str(&generate_dual_member_idl(member));
        }
        idl.push_str("    };\n\n");
    } else {
        idl.push_str(&format!(
            r#"    [
        uuid({iid}),
        helpstring("{help} Interface")
    ]
    dispinterface {interface_name}
    {{
        properties:
        methods:
"#,
            iid = class.default_interface_iid,
        ));
        for member in &class.members {
            idl.push_str(&generate_dispatch_member_idl(member));
        }
        idl.push_str("    };\n\n");
    }

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
"#,
        clsid = class.clsid,
    ));
    if class_supports_bounded_dual_interface(class) {
        idl.push_str(&format!("        [default] interface {interface_name};\n"));
    } else {
        idl.push_str(&format!(
            "        [default] dispinterface {interface_name};\n"
        ));
    }
    if let Some(source_name) = class.source_interface_name.as_ref() {
        idl.push_str(&format!(
            "        [default, source] dispinterface {};\n",
            sanitize_ident(source_name)
        ));
    }
    for interface in &class.implemented_interfaces {
        idl.push_str(&format!(
            "        interface {};\n",
            sanitize_ident(&interface.name)
        ));
    }
    idl.push_str("    };\n\n");
    idl
}

fn implemented_interfaces(
    descriptor: &ComServerDescriptor,
) -> Vec<&ComImplementedInterfaceDescriptor> {
    let mut interfaces = Vec::new();
    for class in &descriptor.classes {
        for interface in &class.implemented_interfaces {
            if !interfaces
                .iter()
                .any(|existing: &&ComImplementedInterfaceDescriptor| existing.iid == interface.iid)
            {
                interfaces.push(interface);
            }
        }
    }
    interfaces
}

fn generate_implemented_interface_idl(interface: &ComImplementedInterfaceDescriptor) -> String {
    let interface_name = sanitize_ident(&interface.name);
    let mut idl = format!(
        r#"    [
        object,
        uuid({iid}),
        dual,
        oleautomation,
        pointer_default(unique),
        helpstring("{help} Interface")
    ]
    interface {interface_name} : IDispatch
    {{
"#,
        iid = interface.iid,
        help = escape_helpstring(&interface.name),
    );
    for method in &interface.methods {
        idl.push_str(&generate_implemented_member_idl(method));
    }
    idl.push_str("    };\n\n");
    idl
}

fn generate_implemented_member_idl(method: &ComImplementedInterfaceMethodDescriptor) -> String {
    let name = sanitize_ident(&method.name);
    let attr = implemented_member_attributes(method);
    let mut params: Vec<String> = method
        .parameter_wire_types
        .iter()
        .enumerate()
        .map(|(index, wire_type)| {
            let name = method
                .parameter_names
                .get(index)
                .filter(|name| !name.trim().is_empty())
                .map(|name| sanitize_ident(name))
                .unwrap_or_else(|| format!("arg{index}"));
            implemented_idl_param(wire_type, &name)
        })
        .collect();
    if matches!(
        method.invoke_kind,
        ComInvokeKind::Method | ComInvokeKind::PropertyGet
    ) && let Some(return_wire_type) = &method.return_wire_type
    {
        params.push(implemented_idl_retval(return_wire_type));
    }
    format!("        {attr} HRESULT {name}({});\n", params.join(", "))
}

fn implemented_member_attributes(method: &ComImplementedInterfaceMethodDescriptor) -> String {
    let mut attrs = vec![format!("id({})", method.dispid)];
    match method.invoke_kind {
        ComInvokeKind::PropertyGet => attrs.push("propget".to_string()),
        ComInvokeKind::PropertyPut => attrs.push("propput".to_string()),
        ComInvokeKind::PropertyPutRef => attrs.push("propputref".to_string()),
        ComInvokeKind::Method => {}
    }
    format!("[{}]", attrs.join(", "))
}

fn implemented_idl_param(wire_type: &ComWireType, name: &str) -> String {
    match wire_type {
        ComWireType::Automation(param) if param.is_by_ref() => {
            format!("[in, out] {}* {name}", idl_type(*param))
        }
        ComWireType::Automation(param) => format!("[in] {} {name}", idl_type(*param)),
        ComWireType::InterfacePointer { .. } => format!("[in] IDispatch* {name}"),
        ComWireType::SafeArrayVariant => format!("[in] SAFEARRAY(VARIANT)* {name}"),
        ComWireType::ByRefSafeArrayVariant => {
            format!("[in, out] SAFEARRAY(VARIANT)** {name}")
        }
    }
}

fn implemented_idl_retval(wire_type: &ComWireType) -> String {
    match wire_type {
        ComWireType::Automation(param) => {
            format!("[out, retval] {}* result", idl_type(*param))
        }
        ComWireType::InterfacePointer { .. } => "[out, retval] IDispatch** result".to_string(),
        ComWireType::SafeArrayVariant | ComWireType::ByRefSafeArrayVariant => {
            "[out, retval] SAFEARRAY(VARIANT)** result".to_string()
        }
    }
}

fn class_supports_bounded_dual_interface(class: &ComClassDescriptor) -> bool {
    class_supports_bounded_dual_scalar_methods(class)
        || class_supports_bounded_dual_long_property(class)
        || class_supports_bounded_dual_object_return_methods(class)
        || class_supports_bounded_dual_object_argument_methods(class)
}

fn class_supports_bounded_dual_scalar_methods(class: &ComClassDescriptor) -> bool {
    !class.members.is_empty()
        && class.members.len() <= 3
        && class.members.iter().enumerate().all(|(index, member)| {
            member.vtable_slot == Some(7 + index as u16)
                && member_supports_bounded_dual_scalar_method(member)
        })
}

fn class_supports_bounded_dual_long_property(class: &ComClassDescriptor) -> bool {
    if class.members.len() != 2 {
        return false;
    }
    let get = &class.members[0];
    let put = &class.members[1];
    get.vtable_slot == Some(7)
        && put.vtable_slot == Some(8)
        && get.name.eq_ignore_ascii_case(&put.name)
        && get.dispid == put.dispid
        && member_supports_bounded_dual_long_property_get(get)
        && member_supports_bounded_dual_long_property_put(put)
}

fn class_supports_bounded_dual_object_return_methods(class: &ComClassDescriptor) -> bool {
    !class.members.is_empty()
        && class.members.len() <= 2
        && class.members.iter().enumerate().all(|(index, member)| {
            member.vtable_slot == Some(7 + index as u16)
                && if index == 0 {
                    member_supports_bounded_dual_object_return(member)
                } else {
                    member_supports_bounded_dual_slot8_long_noarg(member)
                }
        })
}

fn class_supports_bounded_dual_object_argument_methods(class: &ComClassDescriptor) -> bool {
    if class.members.len() != 2 {
        return false;
    }
    let ping = &class.members[0];
    let echo = &class.members[1];
    ping.vtable_slot == Some(7)
        && echo.vtable_slot == Some(8)
        && member_supports_bounded_dual_slot7_long_noarg(ping)
        && member_supports_bounded_dual_slot8_object_arg_long(echo)
}

fn member_supports_bounded_dual_scalar_method(member: &ComMemberDescriptor) -> bool {
    if member.invoke_kind != ComInvokeKind::Method
        || member.parameter_optional.iter().any(|optional| *optional)
    {
        return false;
    }
    matches!(
        (
            member.vtable_slot,
            member.return_type,
            member.parameter_types.as_slice()
        ),
        (Some(7), Some(ComParamType::Long), [])
            | (
                Some(8),
                Some(ComParamType::Long),
                [ComParamType::Long, ComParamType::Long],
            )
            | (
                Some(9),
                Some(ComParamType::Double),
                [ComParamType::Double, ComParamType::Double],
            )
    )
}

fn member_supports_bounded_dual_slot7_long_noarg(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(7)
        && member.return_type == Some(ComParamType::Long)
        && member.parameter_types.is_empty()
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_object_return(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(7)
        && member.return_type == Some(ComParamType::Object)
        && member.parameter_types.is_empty()
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_slot8_object_arg_long(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(8)
        && member.return_type == Some(ComParamType::Long)
        && member.parameter_types.as_slice() == [ComParamType::Object]
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_slot8_long_noarg(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(8)
        && member.return_type == Some(ComParamType::Long)
        && member.parameter_types.is_empty()
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_long_property_get(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::PropertyGet
        && member.vtable_slot == Some(7)
        && member.return_type == Some(ComParamType::Long)
        && member.parameter_types.is_empty()
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_long_property_put(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::PropertyPut
        && member.vtable_slot == Some(8)
        && member.return_type.is_none()
        && member.parameter_types.as_slice() == [ComParamType::Long]
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn generate_dual_member_idl(member: &ComMemberDescriptor) -> String {
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
            format!("[in] {} {name}", idl_type(*param))
        })
        .collect();
    if matches!(
        member.invoke_kind,
        ComInvokeKind::Method | ComInvokeKind::PropertyGet
    ) {
        let return_type = member.return_type.map(idl_type).unwrap_or("void");
        params.push(format!("[out, retval] {return_type}* result"));
    }
    format!("        {attr} HRESULT {name}({});\n", params.join(", "))
}

fn generate_dispatch_member_idl(member: &ComMemberDescriptor) -> String {
    let name = sanitize_ident(&member.name);
    let attr = member_attributes(member);
    let params: Vec<String> = member
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
            let return_type = member.return_type.map(idl_type).unwrap_or(
                if member.invoke_kind == ComInvokeKind::PropertyGet {
                    "VARIANT"
                } else {
                    "void"
                },
            );
            format!(
                "        {attr} {return_type} {name}({});\n",
                params.join(", ")
            )
        }
        ComInvokeKind::PropertyPut | ComInvokeKind::PropertyPutRef => {
            format!("        {attr} void {name}({});\n", params.join(", "))
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
