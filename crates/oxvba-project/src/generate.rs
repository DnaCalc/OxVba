//! Generate `.basproj` XML from a `ProjectManifest` and associated metadata.

#![allow(clippy::too_many_arguments)]

use std::collections::BTreeMap;

use oxvba_compiler::{ModuleKind, ProjectManifest, ReferenceKind};
use oxvba_host::TypeLibraryCatalogEntry;

use crate::model::*;

/// Serialize a parsed `BasProj` model back to canonical `.basproj` XML.
pub fn serialize_basproj_xml(basproj: &BasProj) -> String {
    let mut xml = String::new();
    let sdk = if basproj.sdk.trim().is_empty() {
        "OxVba.Sdk/0.1.0"
    } else {
        basproj.sdk.as_str()
    };
    xml.push_str(&format!("<Project Sdk=\"{}\">\n", xml_escape(sdk)));

    if has_any_properties(&basproj.properties) {
        xml.push_str("  <PropertyGroup>\n");
        push_optional_property(
            &mut xml,
            "OutputType",
            basproj.properties.output_type.map(output_type_str),
        );
        push_optional_property(
            &mut xml,
            "BuildTarget",
            basproj.properties.build_target.map(build_target_str),
        );
        push_optional_property(
            &mut xml,
            "ProjectName",
            basproj.properties.project_name.as_deref(),
        );
        push_optional_property(
            &mut xml,
            "EntryPoint",
            basproj.properties.entry_point.as_deref(),
        );
        push_optional_property(
            &mut xml,
            "RuntimeFlavor",
            basproj.properties.runtime_flavor.map(runtime_flavor_str),
        );
        push_optional_property(
            &mut xml,
            "DefaultRuntimeProfile",
            basproj.properties.default_runtime_profile.as_deref(),
        );
        push_optional_property(
            &mut xml,
            "DefaultPolicyPreset",
            basproj.properties.default_policy_preset.as_deref(),
        );
        push_optional_property(
            &mut xml,
            "DefaultRootObject",
            basproj.properties.default_root_object.as_deref(),
        );
        push_optional_property(
            &mut xml,
            "DefineConstants",
            basproj.properties.define_constants.as_deref(),
        );
        xml.push_str("  </PropertyGroup>\n");
    }

    if !basproj.modules.is_empty() {
        xml.push_str("  <ItemGroup>\n");
        for module in &basproj.modules {
            serialize_module_item(&mut xml, module);
        }
        xml.push_str("  </ItemGroup>\n");
    }

    if !basproj.project_references.is_empty()
        || !basproj.com_references.is_empty()
        || !basproj.native_references.is_empty()
    {
        xml.push_str("  <ItemGroup>\n");
        for reference in &basproj.project_references {
            serialize_project_reference_item(&mut xml, reference);
        }
        for reference in &basproj.com_references {
            serialize_com_reference_item(&mut xml, reference);
        }
        for reference in &basproj.native_references {
            serialize_native_reference_item(&mut xml, reference);
        }
        xml.push_str("  </ItemGroup>\n");
    }

    if !basproj.native_exports.is_empty() {
        xml.push_str("  <ItemGroup>\n");
        for export in &basproj.native_exports {
            serialize_native_export_item(&mut xml, export);
        }
        xml.push_str("  </ItemGroup>\n");
    }

    xml.push_str("</Project>\n");
    xml
}

/// Generate a `.basproj` XML string from a `ProjectManifest` and associated metadata.
///
/// This is the inverse of `load_basproj` — used for round-trip testing and
/// project initialization.
pub fn generate_basproj_xml(
    manifest: &ProjectManifest,
    output_type: OutputType,
    build_target: Option<BuildTarget>,
    entry_point: Option<&str>,
    runtime_flavor: Option<RuntimeFlavor>,
    default_runtime_profile: Option<&str>,
    default_policy_preset: Option<&str>,
    default_root_object: Option<&str>,
    type_library_catalog: &[TypeLibraryCatalogEntry],
    native_exports: &[NativeExportDescriptor],
    class_module_metadata: &BTreeMap<String, ClassModuleMetadata>,
) -> String {
    let mut xml = String::new();
    xml.push_str("<Project Sdk=\"OxVba.Sdk/0.1.0\">\n");

    // PropertyGroup
    xml.push_str("  <PropertyGroup>\n");
    xml.push_str(&format!(
        "    <OutputType>{}</OutputType>\n",
        output_type_str(output_type)
    ));
    if let Some(build_target) = build_target {
        xml.push_str(&format!(
            "    <BuildTarget>{}</BuildTarget>\n",
            build_target_str(build_target)
        ));
    }
    xml.push_str(&format!(
        "    <ProjectName>{}</ProjectName>\n",
        xml_escape(&manifest.project_name)
    ));
    if let Some(ep) = entry_point {
        xml.push_str(&format!(
            "    <EntryPoint>{}</EntryPoint>\n",
            xml_escape(ep)
        ));
    }
    if let Some(rf) = runtime_flavor {
        xml.push_str(&format!(
            "    <RuntimeFlavor>{}</RuntimeFlavor>\n",
            runtime_flavor_str(rf)
        ));
    }
    if let Some(profile) = default_runtime_profile {
        xml.push_str(&format!(
            "    <DefaultRuntimeProfile>{}</DefaultRuntimeProfile>\n",
            xml_escape(profile)
        ));
    }
    if let Some(preset) = default_policy_preset {
        xml.push_str(&format!(
            "    <DefaultPolicyPreset>{}</DefaultPolicyPreset>\n",
            xml_escape(preset)
        ));
    }
    if let Some(root) = default_root_object {
        xml.push_str(&format!(
            "    <DefaultRootObject>{}</DefaultRootObject>\n",
            xml_escape(root)
        ));
    }
    if !manifest.conditional_constants.is_empty() {
        let constants: Vec<String> = manifest
            .conditional_constants
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        xml.push_str(&format!(
            "    <DefineConstants>{}</DefineConstants>\n",
            constants.join(";")
        ));
    }
    xml.push_str("  </PropertyGroup>\n");

    // Module ItemGroup
    if !manifest.modules.is_empty() {
        xml.push_str("  <ItemGroup>\n");
        for module in &manifest.modules {
            let filename = module_filename(module);
            match module.module_kind {
                ModuleKind::Procedural => {
                    xml.push_str(&format!(
                        "    <Module Include=\"{}\" />\n",
                        xml_escape(&filename)
                    ));
                }
                ModuleKind::Class => {
                    let attrs = &module.attributes;
                    let extra = class_module_metadata.get(&module.module_name);
                    let has_metadata = attrs.vb_predeclared_id
                        || attrs.vb_exposed
                        || attrs.vb_global_namespace
                        || attrs.vb_creatable
                        || extra.is_some();
                    if has_metadata {
                        xml.push_str(&format!(
                            "    <ClassModule Include=\"{}\">\n",
                            xml_escape(&filename)
                        ));
                        if attrs.vb_predeclared_id {
                            xml.push_str("      <VBPredeclaredId>True</VBPredeclaredId>\n");
                        }
                        if attrs.vb_exposed {
                            xml.push_str("      <VBExposed>True</VBExposed>\n");
                        }
                        if attrs.vb_global_namespace {
                            xml.push_str("      <VBGlobalNamespace>True</VBGlobalNamespace>\n");
                        }
                        if attrs.vb_creatable {
                            xml.push_str("      <VBCreatable>True</VBCreatable>\n");
                        }
                        if let Some(meta) = extra {
                            if let Some(inst) = meta.instancing {
                                xml.push_str(&format!(
                                    "      <Instancing>{}</Instancing>\n",
                                    instancing_str(inst)
                                ));
                            }
                            if let Some(ref pid) = meta.prog_id {
                                xml.push_str(&format!(
                                    "      <ProgId>{}</ProgId>\n",
                                    xml_escape(pid)
                                ));
                            }
                            if let Some(ref desc) = meta.description {
                                xml.push_str(&format!(
                                    "      <Description>{}</Description>\n",
                                    xml_escape(desc)
                                ));
                            }
                        }
                        xml.push_str("    </ClassModule>\n");
                    } else {
                        xml.push_str(&format!(
                            "    <ClassModule Include=\"{}\" />\n",
                            xml_escape(&filename)
                        ));
                    }
                }
                ModuleKind::Document => {
                    xml.push_str(&format!(
                        "    <DocumentModule Include=\"{}\" />\n",
                        xml_escape(&filename)
                    ));
                }
                _ => {
                    // Form, Extension — emit as Module for now
                    xml.push_str(&format!(
                        "    <Module Include=\"{}\" />\n",
                        xml_escape(&filename)
                    ));
                }
            }
        }
        xml.push_str("  </ItemGroup>\n");
    }

    // Reference ItemGroup
    let has_project_refs = manifest
        .references
        .iter()
        .any(|r| r.reference_kind == ReferenceKind::Project);
    let has_com_refs = !type_library_catalog.is_empty();
    if has_project_refs || has_com_refs {
        xml.push_str("  <ItemGroup>\n");
        for reference in &manifest.references {
            if matches!(
                reference.reference_kind,
                ReferenceKind::Project | ReferenceKind::HostInjected
            ) {
                let include = format!(
                    "..\\{}\\{}.basproj",
                    reference.referenced_project_name, reference.referenced_project_name
                );
                if reference.reference_kind == ReferenceKind::HostInjected {
                    xml.push_str(&format!(
                        "    <ProjectReference Include=\"{}\">\n      <Kind>HostInjected</Kind>\n    </ProjectReference>\n",
                        xml_escape(&include)
                    ));
                } else {
                    xml.push_str(&format!(
                        "    <ProjectReference Include=\"{}\" />\n",
                        xml_escape(&include)
                    ));
                }
            }
        }
        for catalog_entry in type_library_catalog {
            xml.push_str(&format!(
                "    <COMReference Include=\"{}\">\n",
                xml_escape(&catalog_entry.library_name)
            ));
            if let Some(ref libid) = catalog_entry.libid {
                xml.push_str(&format!("      <Guid>{}</Guid>\n", xml_escape(libid)));
            }
            xml.push_str(&format!(
                "      <VersionMajor>{}</VersionMajor>\n",
                catalog_entry.major_version
            ));
            xml.push_str(&format!(
                "      <VersionMinor>{}</VersionMinor>\n",
                catalog_entry.minor_version
            ));
            if let Some(lcid) = catalog_entry.lcid {
                xml.push_str(&format!("      <Lcid>{lcid}</Lcid>\n"));
            }
            xml.push_str(&format!(
                "      <ImportLib>{}</ImportLib>\n",
                xml_escape(&catalog_entry.importlib)
            ));
            xml.push_str("    </COMReference>\n");
        }
        xml.push_str("  </ItemGroup>\n");
    }

    // NativeExport ItemGroup
    if !native_exports.is_empty() {
        xml.push_str("  <ItemGroup>\n");
        for export in native_exports {
            xml.push_str(&format!(
                "    <NativeExport Include=\"{}\">\n",
                xml_escape(&export.exported_name)
            ));
            xml.push_str(&format!(
                "      <Module>{}</Module>\n",
                xml_escape(&export.module_name)
            ));
            xml.push_str(&format!(
                "      <Procedure>{}</Procedure>\n",
                xml_escape(&export.procedure_name)
            ));
            xml.push_str(&format!(
                "      <CallingConvention>{}</CallingConvention>\n",
                calling_convention_str(export.calling_convention)
            ));
            if let Some(ord) = export.ordinal {
                xml.push_str(&format!("      <Ordinal>{ord}</Ordinal>\n"));
            }
            if let Some(ref cat) = export.category {
                xml.push_str(&format!("      <Category>{}</Category>\n", xml_escape(cat)));
            }
            if let Some(ref desc) = export.description {
                xml.push_str(&format!(
                    "      <Description>{}</Description>\n",
                    xml_escape(desc)
                ));
            }
            if let Some(ref arg_descs) = export.argument_descriptions {
                xml.push_str(&format!(
                    "      <ArgumentDescriptions>{}</ArgumentDescriptions>\n",
                    xml_escape(arg_descs)
                ));
            }
            xml.push_str("    </NativeExport>\n");
        }
        xml.push_str("  </ItemGroup>\n");
    }

    xml.push_str("</Project>\n");
    xml
}

fn output_type_str(ot: OutputType) -> &'static str {
    match ot {
        OutputType::HostModule => "HostModule",
        OutputType::Library => "Library",
        OutputType::Exe => "Exe",
        OutputType::Addin => "Addin",
        OutputType::ComServer => "ComServer",
        OutputType::ComExe => "ComExe",
    }
}

fn build_target_str(build_target: BuildTarget) -> &'static str {
    match build_target {
        BuildTarget::Bundle => "Bundle",
        BuildTarget::WrapperExe => "WrapperExe",
        BuildTarget::WrapperLibrary => "WrapperLibrary",
        BuildTarget::WrappedComServer => "WrappedComServer",
    }
}

fn has_any_properties(properties: &BasProjProperties) -> bool {
    properties.output_type.is_some()
        || properties.build_target.is_some()
        || properties.project_name.is_some()
        || properties.entry_point.is_some()
        || properties.runtime_flavor.is_some()
        || properties.default_runtime_profile.is_some()
        || properties.default_policy_preset.is_some()
        || properties.default_root_object.is_some()
        || properties.define_constants.is_some()
}

fn push_optional_property(xml: &mut String, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        xml.push_str(&format!("    <{name}>{}</{name}>\n", xml_escape(value)));
    }
}

fn serialize_module_item(xml: &mut String, module: &BasProjModule) {
    match module.kind {
        BasProjModuleKind::Module => {
            xml.push_str(&format!(
                "    <Module Include=\"{}\" />\n",
                xml_escape(&module.include)
            ));
        }
        BasProjModuleKind::ClassModule => {
            let has_metadata = module.vb_predeclared_id
                || module.vb_exposed
                || module.vb_global_namespace
                || module.vb_creatable
                || module.instancing.is_some()
                || module.prog_id.is_some()
                || module.description.is_some();
            if !has_metadata {
                xml.push_str(&format!(
                    "    <ClassModule Include=\"{}\" />\n",
                    xml_escape(&module.include)
                ));
                return;
            }

            xml.push_str(&format!(
                "    <ClassModule Include=\"{}\">\n",
                xml_escape(&module.include)
            ));
            if module.vb_predeclared_id {
                xml.push_str("      <VBPredeclaredId>True</VBPredeclaredId>\n");
            }
            if module.vb_exposed {
                xml.push_str("      <VBExposed>True</VBExposed>\n");
            }
            if module.vb_global_namespace {
                xml.push_str("      <VBGlobalNamespace>True</VBGlobalNamespace>\n");
            }
            if module.vb_creatable {
                xml.push_str("      <VBCreatable>True</VBCreatable>\n");
            }
            if let Some(instancing) = module.instancing {
                xml.push_str(&format!(
                    "      <Instancing>{}</Instancing>\n",
                    instancing_str(instancing)
                ));
            }
            if let Some(prog_id) = module.prog_id.as_deref() {
                xml.push_str(&format!("      <ProgId>{}</ProgId>\n", xml_escape(prog_id)));
            }
            if let Some(description) = module.description.as_deref() {
                xml.push_str(&format!(
                    "      <Description>{}</Description>\n",
                    xml_escape(description)
                ));
            }
            xml.push_str("    </ClassModule>\n");
        }
        BasProjModuleKind::DocumentModule => {
            if let Some(host_document_type) = module.host_document_type.as_deref() {
                xml.push_str(&format!(
                    "    <DocumentModule Include=\"{}\">\n",
                    xml_escape(&module.include)
                ));
                xml.push_str(&format!(
                    "      <HostDocumentType>{}</HostDocumentType>\n",
                    xml_escape(host_document_type)
                ));
                xml.push_str("    </DocumentModule>\n");
            } else {
                xml.push_str(&format!(
                    "    <DocumentModule Include=\"{}\" />\n",
                    xml_escape(&module.include)
                ));
            }
        }
    }
}

fn serialize_project_reference_item(xml: &mut String, reference: &BasProjProjectReference) {
    match reference.kind {
        BasProjProjectReferenceKind::Project => {
            xml.push_str(&format!(
                "    <ProjectReference Include=\"{}\" />\n",
                xml_escape(&reference.include)
            ));
        }
        BasProjProjectReferenceKind::HostInjected => {
            xml.push_str(&format!(
                "    <ProjectReference Include=\"{}\">\n",
                xml_escape(&reference.include)
            ));
            xml.push_str("      <Kind>HostInjected</Kind>\n");
            xml.push_str("    </ProjectReference>\n");
        }
    }
}

fn serialize_com_reference_item(xml: &mut String, reference: &BasProjComReference) {
    let has_metadata = reference.guid.is_some()
        || reference.version_major.is_some()
        || reference.version_minor.is_some()
        || reference.lcid.is_some()
        || reference.import_lib.is_some();
    if !has_metadata {
        xml.push_str(&format!(
            "    <COMReference Include=\"{}\" />\n",
            xml_escape(&reference.include)
        ));
        return;
    }

    xml.push_str(&format!(
        "    <COMReference Include=\"{}\">\n",
        xml_escape(&reference.include)
    ));
    if let Some(guid) = reference.guid.as_deref() {
        xml.push_str(&format!("      <Guid>{}</Guid>\n", xml_escape(guid)));
    }
    if let Some(version_major) = reference.version_major {
        xml.push_str(&format!(
            "      <VersionMajor>{version_major}</VersionMajor>\n"
        ));
    }
    if let Some(version_minor) = reference.version_minor {
        xml.push_str(&format!(
            "      <VersionMinor>{version_minor}</VersionMinor>\n"
        ));
    }
    if let Some(lcid) = reference.lcid {
        xml.push_str(&format!("      <Lcid>{lcid}</Lcid>\n"));
    }
    if let Some(import_lib) = reference.import_lib.as_deref() {
        xml.push_str(&format!(
            "      <ImportLib>{}</ImportLib>\n",
            xml_escape(import_lib)
        ));
    }
    xml.push_str("    </COMReference>\n");
}

fn serialize_native_reference_item(xml: &mut String, reference: &BasProjNativeReference) {
    if let Some(path) = reference.path.as_deref() {
        xml.push_str(&format!(
            "    <NativeReference Include=\"{}\">\n",
            xml_escape(&reference.include)
        ));
        xml.push_str(&format!("      <Path>{}</Path>\n", xml_escape(path)));
        xml.push_str("    </NativeReference>\n");
    } else {
        xml.push_str(&format!(
            "    <NativeReference Include=\"{}\" />\n",
            xml_escape(&reference.include)
        ));
    }
}

fn serialize_native_export_item(xml: &mut String, export: &BasProjNativeExport) {
    let has_metadata = export.module.is_some()
        || export.procedure.is_some()
        || export.calling_convention.is_some()
        || export.ordinal.is_some()
        || export.category.is_some()
        || export.description.is_some()
        || export.argument_descriptions.is_some();
    if !has_metadata {
        xml.push_str(&format!(
            "    <NativeExport Include=\"{}\" />\n",
            xml_escape(&export.include)
        ));
        return;
    }

    xml.push_str(&format!(
        "    <NativeExport Include=\"{}\">\n",
        xml_escape(&export.include)
    ));
    if let Some(module) = export.module.as_deref() {
        xml.push_str(&format!("      <Module>{}</Module>\n", xml_escape(module)));
    }
    if let Some(procedure) = export.procedure.as_deref() {
        xml.push_str(&format!(
            "      <Procedure>{}</Procedure>\n",
            xml_escape(procedure)
        ));
    }
    if let Some(calling_convention) = export.calling_convention {
        xml.push_str(&format!(
            "      <CallingConvention>{}</CallingConvention>\n",
            calling_convention_str(calling_convention)
        ));
    }
    if let Some(ordinal) = export.ordinal {
        xml.push_str(&format!("      <Ordinal>{ordinal}</Ordinal>\n"));
    }
    if let Some(category) = export.category.as_deref() {
        xml.push_str(&format!(
            "      <Category>{}</Category>\n",
            xml_escape(category)
        ));
    }
    if let Some(description) = export.description.as_deref() {
        xml.push_str(&format!(
            "      <Description>{}</Description>\n",
            xml_escape(description)
        ));
    }
    if let Some(argument_descriptions) = export.argument_descriptions.as_deref() {
        xml.push_str(&format!(
            "      <ArgumentDescriptions>{}</ArgumentDescriptions>\n",
            xml_escape(argument_descriptions)
        ));
    }
    xml.push_str("    </NativeExport>\n");
}

fn runtime_flavor_str(rf: RuntimeFlavor) -> &'static str {
    match rf {
        RuntimeFlavor::Lite => "Lite",
        RuntimeFlavor::Jit => "Jit",
    }
}

fn calling_convention_str(cc: CallingConvention) -> &'static str {
    match cc {
        CallingConvention::Stdcall => "Stdcall",
        CallingConvention::Cdecl => "Cdecl",
    }
}

fn instancing_str(inst: Instancing) -> &'static str {
    match inst {
        Instancing::Private => "Private",
        Instancing::PublicNotCreatable => "PublicNotCreatable",
        Instancing::MultiUse => "MultiUse",
        Instancing::GlobalMultiUse => "GlobalMultiUse",
        Instancing::SingleUse => "SingleUse",
        Instancing::GlobalSingleUse => "GlobalSingleUse",
    }
}

fn module_filename(module: &oxvba_compiler::ModuleUnit) -> String {
    let ext = match module.module_kind {
        ModuleKind::Procedural => "bas",
        ModuleKind::Class | ModuleKind::Document => "cls",
        _ => "bas",
    };
    format!("{}.{}", module.module_name, ext)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
