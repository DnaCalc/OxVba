use std::path::{Path, PathBuf};

use oxvba_build::idl::generate_idl;
#[cfg(target_os = "windows")]
use oxvba_build::typelib_gen::generate_typelib;
use oxvba_compiler::{ModuleKind, compile_project};
use oxvba_project::{
    BasProjModule, BasProjModuleKind, ComClassExportDescriptor, load_basproj_from_str,
    validate::validate_com_class_exports,
};

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "{prefix}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn build_export_descriptors(widget_source: &str) -> Vec<ComClassExportDescriptor> {
    let temp_root = TempDirGuard::new("oxvba_com_attribute_export");
    std::fs::write(temp_root.path().join("Widget.cls"), widget_source).expect("write class module");

    let loaded = load_basproj_from_str(
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>ComServer</OutputType>
    <ProjectName>AttrExport</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <ClassModule Include=\"Widget.cls\">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <ProgId>AttrExport.Widget</ProgId>
    </ClassModule>
  </ItemGroup>
</Project>
",
        temp_root.path(),
    )
    .expect("project should load");
    let compiled = compile_project(&loaded.manifest).expect("project should compile");
    let modules_for_validation: Vec<BasProjModule> = loaded
        .manifest
        .modules
        .iter()
        .map(|module| BasProjModule {
            kind: match module.module_kind {
                ModuleKind::Class => BasProjModuleKind::ClassModule,
                ModuleKind::Document => BasProjModuleKind::DocumentModule,
                _ => BasProjModuleKind::Module,
            },
            include: format!(
                "{}.{}",
                module.module_name,
                if matches!(module.module_kind, ModuleKind::Class) {
                    "cls"
                } else {
                    "bas"
                }
            ),
            vb_predeclared_id: module.attributes.vb_predeclared_id,
            vb_exposed: module.attributes.vb_exposed,
            vb_global_namespace: module.attributes.vb_global_namespace,
            vb_creatable: module.attributes.vb_creatable,
            host_document_type: None,
            instancing: None,
            prog_id: None,
            description: None,
        })
        .collect();

    validate_com_class_exports(
        &modules_for_validation,
        &compiled,
        &loaded.class_module_metadata,
        &loaded.manifest.project_name,
    )
    .expect("COM export validation should succeed")
}

fn widget_member_attribute_source() -> &'static str {
    concat!(
        "Attribute VB_Name = \"Widget\"\n",
        "Option Explicit\n",
        "Private stored As Long\n",
        "Private Sub Class_Initialize()\n",
        "stored = 41\n",
        "End Sub\n",
        "Public Property Get Value() As Long\n",
        "Value = stored + 1\n",
        "End Property\n",
        "Attribute Value.VB_UserMemId = 0\n",
        "Public Property Get NewEnum() As Long\n",
        "NewEnum = stored\n",
        "End Property\n",
        "Attribute NewEnum.VB_UserMemId = -4\n",
        "Attribute NewEnum.VB_MemberFlags = \"40\"\n"
    )
}

#[test]
fn source_member_attributes_flow_into_com_export_validation_and_idl() {
    let exports = build_export_descriptors(widget_member_attribute_source());
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0].class_name, "Widget");
    assert_eq!(exports[0].prog_id.as_deref(), Some("AttrExport.Widget"));

    let value = exports[0]
        .members
        .iter()
        .find(|member| member.member_name.eq_ignore_ascii_case("Value"))
        .expect("Value member should be exported");
    assert_eq!(value.dispatch_id, Some(0));
    assert!(value.is_default_member);

    let new_enum = exports[0]
        .members
        .iter()
        .find(|member| member.member_name.eq_ignore_ascii_case("NewEnum"))
        .expect("NewEnum member should be exported");
    assert_eq!(new_enum.dispatch_id, Some(-4));
    assert_eq!(new_enum.member_flags, Some(0x40));

    let idl = generate_idl("AttrExport", &exports).to_ascii_lowercase();
    assert!(idl.contains("[id(0), propget, defaultbind] hresult value("));
    assert!(idl.contains("[id(-4), propget, restricted, hidden] hresult newenum("));
}

#[cfg(target_os = "windows")]
#[test]
fn source_member_attributes_flow_into_generated_typelib() {
    use oxvba_com::windows_typelib_loader::{
        enumerate_typelib_members, load_typelib_from_path, release_typelib,
    };

    let exports = build_export_descriptors(widget_member_attribute_source());
    let temp_root = TempDirGuard::new("oxvba_com_attribute_typelib");
    let tlb_path = temp_root.path().join("AttrExport.tlb");
    let tlb_path = tlb_path.to_string_lossy().to_string();

    generate_typelib("AttrExport", &tlb_path, &exports).expect("typelib generation should work");
    let typelib = load_typelib_from_path(&tlb_path).expect("typelib should load");
    let members = enumerate_typelib_members(typelib).expect("typelib members should enumerate");
    unsafe { release_typelib(typelib) };

    let value = members
        .iter()
        .find(|member| member.name.eq_ignore_ascii_case("Value"))
        .expect("Value should be present in typelib");
    assert_eq!(value.token, 0);
    assert!(value.is_default_member);

    let new_enum = members
        .iter()
        .find(|member| member.name.eq_ignore_ascii_case("NewEnum"))
        .expect("NewEnum should be present in typelib");
    assert_eq!(new_enum.token, -4);
}
