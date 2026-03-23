//! Generate `.basproj` XML from a `ProjectManifest` and associated metadata.

use std::collections::BTreeMap;

use oxvba_compiler::{ModuleKind, ProjectManifest, ReferenceKind};
use oxvba_host::TypeLibraryCatalogEntry;

use crate::model::*;

/// Generate a `.basproj` XML string from a `ProjectManifest` and associated metadata.
///
/// This is the inverse of `load_basproj` — used for round-trip testing and
/// project initialization.
pub fn generate_basproj_xml(
    manifest: &ProjectManifest,
    output_type: OutputType,
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
                            xml.push_str(
                                "      <VBPredeclaredId>True</VBPredeclaredId>\n",
                            );
                        }
                        if attrs.vb_exposed {
                            xml.push_str("      <VBExposed>True</VBExposed>\n");
                        }
                        if attrs.vb_global_namespace {
                            xml.push_str(
                                "      <VBGlobalNamespace>True</VBGlobalNamespace>\n",
                            );
                        }
                        if attrs.vb_creatable {
                            xml.push_str(
                                "      <VBCreatable>True</VBCreatable>\n",
                            );
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
            if reference.reference_kind == ReferenceKind::Project {
                xml.push_str(&format!(
                    "    <ProjectReference Include=\"{}\" />\n",
                    xml_escape(&format!(
                        "..\\{}\\{}.basproj",
                        reference.referenced_project_name,
                        reference.referenced_project_name
                    ))
                ));
            }
        }
        for catalog_entry in type_library_catalog {
            xml.push_str(&format!(
                "    <COMReference Include=\"{}\">\n",
                xml_escape(&catalog_entry.library_name)
            ));
            if let Some(ref libid) = catalog_entry.libid {
                xml.push_str(&format!(
                    "      <Guid>{}</Guid>\n",
                    xml_escape(libid)
                ));
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
                xml.push_str(&format!(
                    "      <Category>{}</Category>\n",
                    xml_escape(cat)
                ));
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
