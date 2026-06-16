//! Tests for `.basproj` XML parsing and loading.

use oxvba_project::*;

// ---------------------------------------------------------------------------
// Use Case A: Embedded Host Module
// ---------------------------------------------------------------------------

const USE_CASE_A_XML: &str = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>HostModule</OutputType>
    <ProjectName>VBAProject</ProjectName>
    <DefaultRootObject>Application</DefaultRootObject>
    <DefineConstants>VBA7=1;WIN64=1</DefineConstants>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Module1.bas" />
    <ClassModule Include="Calculator.cls" />
    <DocumentModule Include="Sheet1.cls">
      <HostDocumentType>Worksheet</HostDocumentType>
    </DocumentModule>
    <DocumentModule Include="ThisWorkbook.cls">
      <HostDocumentType>Workbook</HostDocumentType>
    </DocumentModule>
  </ItemGroup>
  <ItemGroup>
    <COMReference Include="Excel">
      <Guid>{00020813-0000-0000-C000-000000000046}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>9</VersionMinor>
      <Lcid>0</Lcid>
      <ImportLib>excel.exe</ImportLib>
    </COMReference>
  </ItemGroup>
</Project>"#;

#[test]
fn parse_use_case_a_host_module() {
    let basproj = parse_basproj_xml(USE_CASE_A_XML).unwrap();

    assert_eq!(basproj.sdk, "OxVba.Sdk/0.1.0");
    assert_eq!(basproj.properties.output_type, Some(OutputType::HostModule));
    assert!(basproj.properties.build_target.is_none());
    assert_eq!(
        basproj.properties.project_name.as_deref(),
        Some("VBAProject")
    );
    assert_eq!(
        basproj.properties.default_root_object.as_deref(),
        Some("Application")
    );
    assert_eq!(
        basproj.properties.define_constants.as_deref(),
        Some("VBA7=1;WIN64=1")
    );

    // Modules
    assert_eq!(basproj.modules.len(), 4);
    assert_eq!(basproj.modules[0].kind, BasProjModuleKind::Module);
    assert_eq!(basproj.modules[0].include, "Module1.bas");
    assert_eq!(basproj.modules[1].kind, BasProjModuleKind::ClassModule);
    assert_eq!(basproj.modules[1].include, "Calculator.cls");
    assert_eq!(basproj.modules[2].kind, BasProjModuleKind::DocumentModule);
    assert_eq!(basproj.modules[2].include, "Sheet1.cls");
    assert_eq!(
        basproj.modules[2].host_document_type.as_deref(),
        Some("Worksheet")
    );
    assert_eq!(basproj.modules[3].kind, BasProjModuleKind::DocumentModule);
    assert_eq!(
        basproj.modules[3].host_document_type.as_deref(),
        Some("Workbook")
    );

    // COM references
    assert_eq!(basproj.com_references.len(), 1);
    assert_eq!(basproj.com_references[0].include, "Excel");
    assert_eq!(
        basproj.com_references[0].guid.as_deref(),
        Some("{00020813-0000-0000-C000-000000000046}")
    );
    assert_eq!(basproj.com_references[0].version_major, Some(1));
    assert_eq!(basproj.com_references[0].version_minor, Some(9));
    assert_eq!(basproj.com_references[0].lcid, Some(0));
    assert_eq!(
        basproj.com_references[0].import_lib.as_deref(),
        Some("excel.exe")
    );
}

// ---------------------------------------------------------------------------
// Use Case B: Library with Native Exports
// ---------------------------------------------------------------------------

const USE_CASE_B_XML: &str = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <ProjectName>FinanceAddIn</ProjectName>
    <RuntimeFlavor>Jit</RuntimeFlavor>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="AddInSetup.bas" />
    <Module Include="PricingFunctions.bas" />
    <ClassModule Include="PricingEngine.cls">
      <VBExposed>True</VBExposed>
      <VBPredeclaredId>True</VBPredeclaredId>
    </ClassModule>
  </ItemGroup>
  <ItemGroup>
    <ProjectReference Include="..\CoreMath\CoreMath.basproj" />
    <COMReference Include="Scripting">
      <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
      <ImportLib>scrrun.dll</ImportLib>
    </COMReference>
  </ItemGroup>
  <ItemGroup>
    <NativeExport Include="CalcBlackScholes">
      <Module>PricingFunctions</Module>
      <Procedure>BlackScholes</Procedure>
      <CallingConvention>Stdcall</CallingConvention>
    </NativeExport>
    <NativeExport Include="xlAutoOpen">
      <Module>AddInSetup</Module>
      <Procedure>AutoOpen</Procedure>
      <CallingConvention>Stdcall</CallingConvention>
    </NativeExport>
  </ItemGroup>
</Project>"#;

#[test]
fn parse_use_case_b_library_exports() {
    let basproj = parse_basproj_xml(USE_CASE_B_XML).unwrap();

    assert_eq!(basproj.properties.output_type, Some(OutputType::Library));
    assert_eq!(
        basproj.properties.project_name.as_deref(),
        Some("FinanceAddIn")
    );
    assert_eq!(basproj.properties.runtime_flavor, Some(RuntimeFlavor::Jit));

    // Modules
    assert_eq!(basproj.modules.len(), 3);
    assert_eq!(basproj.modules[2].kind, BasProjModuleKind::ClassModule);
    assert!(basproj.modules[2].vb_exposed);
    assert!(basproj.modules[2].vb_predeclared_id);
    assert!(!basproj.modules[2].vb_global_namespace);
    assert!(!basproj.modules[2].vb_creatable);

    // Project references
    assert_eq!(basproj.project_references.len(), 1);
    assert_eq!(
        basproj.project_references[0].include,
        r"..\CoreMath\CoreMath.basproj"
    );

    // COM references
    assert_eq!(basproj.com_references.len(), 1);
    assert_eq!(basproj.com_references[0].include, "Scripting");

    // Native exports
    assert_eq!(basproj.native_exports.len(), 2);
    assert_eq!(basproj.native_exports[0].include, "CalcBlackScholes");
    assert_eq!(
        basproj.native_exports[0].module.as_deref(),
        Some("PricingFunctions")
    );
    assert_eq!(
        basproj.native_exports[0].procedure.as_deref(),
        Some("BlackScholes")
    );
    assert_eq!(
        basproj.native_exports[0].calling_convention,
        Some(CallingConvention::Stdcall)
    );
    assert_eq!(basproj.native_exports[1].include, "xlAutoOpen");
    assert_eq!(
        basproj.native_exports[1].module.as_deref(),
        Some("AddInSetup")
    );
}

// ---------------------------------------------------------------------------
// Use Case C: Standalone Executable
// ---------------------------------------------------------------------------

const USE_CASE_C_XML: &str = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ReportGenerator</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Main.bas" />
    <Module Include="FileProcessor.bas" />
    <ClassModule Include="Report.cls" />
  </ItemGroup>
  <ItemGroup>
    <COMReference Include="Scripting">
      <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
      <ImportLib>scrrun.dll</ImportLib>
    </COMReference>
  </ItemGroup>
</Project>"#;

#[test]
fn parse_use_case_c_exe() {
    let basproj = parse_basproj_xml(USE_CASE_C_XML).unwrap();

    assert_eq!(basproj.properties.output_type, Some(OutputType::Exe));
    assert_eq!(
        basproj.properties.project_name.as_deref(),
        Some("ReportGenerator")
    );
    assert_eq!(basproj.properties.entry_point.as_deref(), Some("Main.Main"));

    assert_eq!(basproj.modules.len(), 3);
    assert_eq!(basproj.com_references.len(), 1);
    assert!(basproj.native_exports.is_empty());
}

// ---------------------------------------------------------------------------
// Minimal convention-driven project
// ---------------------------------------------------------------------------

const MINIMAL_XML: &str = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
  </PropertyGroup>
</Project>"#;

#[test]
fn parse_minimal_project() {
    let basproj = parse_basproj_xml(MINIMAL_XML).unwrap();

    assert_eq!(basproj.sdk, "OxVba.Sdk/0.1.0");
    assert_eq!(basproj.properties.output_type, Some(OutputType::Exe));
    assert!(basproj.properties.build_target.is_none());
    assert!(basproj.properties.project_name.is_none());
    assert!(basproj.properties.entry_point.is_none());
    assert!(basproj.modules.is_empty());
    assert!(basproj.com_references.is_empty());
    assert!(basproj.native_exports.is_empty());
}

// ---------------------------------------------------------------------------
// DefineConstants parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_define_constants() {
    let constants = oxvba_project::model::parse_define_constants("VBA7=1;WIN64=1;DEBUG");
    assert_eq!(constants.len(), 3);
    assert_eq!(constants["VBA7"], 1);
    assert_eq!(constants["WIN64"], 1);
    assert_eq!(constants["DEBUG"], 1);
}

#[test]
fn parse_define_constants_with_spaces() {
    let constants = oxvba_project::model::parse_define_constants(" KEY1 = 42 ; KEY2 = -1 ; FLAG ");
    assert_eq!(constants.len(), 3);
    assert_eq!(constants["KEY1"], 42);
    assert_eq!(constants["KEY2"], -1);
    assert_eq!(constants["FLAG"], 1);
}

#[test]
fn parse_define_constants_empty() {
    let constants = oxvba_project::model::parse_define_constants("");
    assert!(constants.is_empty());
}

// ---------------------------------------------------------------------------
// Import file (bare <Project> without Sdk)
// ---------------------------------------------------------------------------

const IMPORT_FILE_XML: &str = r#"<Project>
  <ItemGroup>
    <NativeExport Include="CalcBlackScholes">
      <Module>PricingFunctions</Module>
      <Procedure>BlackScholes</Procedure>
      <CallingConvention>Stdcall</CallingConvention>
    </NativeExport>
    <NativeExport Include="CalcImpliedVol">
      <Module>VolSurface</Module>
      <Procedure>ImpliedVol</Procedure>
      <CallingConvention>Cdecl</CallingConvention>
      <Ordinal>2</Ordinal>
    </NativeExport>
  </ItemGroup>
</Project>"#;

#[test]
fn parse_import_file() {
    let basproj = parse_basproj_xml(IMPORT_FILE_XML).unwrap();

    // No Sdk in import files
    assert_eq!(basproj.sdk, "");
    assert!(basproj.modules.is_empty());
    assert_eq!(basproj.native_exports.len(), 2);
    assert_eq!(basproj.native_exports[0].include, "CalcBlackScholes");
    assert_eq!(
        basproj.native_exports[0].calling_convention,
        Some(CallingConvention::Stdcall)
    );
    assert_eq!(basproj.native_exports[1].include, "CalcImpliedVol");
    assert_eq!(
        basproj.native_exports[1].calling_convention,
        Some(CallingConvention::Cdecl)
    );
    assert_eq!(basproj.native_exports[1].ordinal, Some(2));
}

// ---------------------------------------------------------------------------
// Merge import
// ---------------------------------------------------------------------------

#[test]
fn merge_import_combines_items() {
    let mut parent = parse_basproj_xml(USE_CASE_B_XML).unwrap();
    let import = parse_basproj_xml(IMPORT_FILE_XML).unwrap();

    let parent_export_count = parent.native_exports.len();
    let import_export_count = import.native_exports.len();

    oxvba_project::parse::merge_import(&mut parent, import);

    assert_eq!(
        parent.native_exports.len(),
        parent_export_count + import_export_count
    );
}

// ---------------------------------------------------------------------------
// load_basproj_from_str with filesystem (Use Case A)
// ---------------------------------------------------------------------------

#[test]
fn load_use_case_a_from_str() {
    // Create a temp directory with module files
    let tmp = std::env::temp_dir().join("oxvba_test_load_a");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Write module source files
    std::fs::write(
        tmp.join("Module1.bas"),
        "Attribute VB_Name = \"Module1\"\nPublic Sub Hello()\nEnd Sub\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("Calculator.cls"),
        "Attribute VB_Name = \"Calculator\"\nPublic Function Add(a As Long, b As Long) As Long\n  Add = a + b\nEnd Function\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("Sheet1.cls"),
        "Attribute VB_Name = \"Sheet1\"\nPrivate Sub Worksheet_Change()\nEnd Sub\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("ThisWorkbook.cls"),
        "Attribute VB_Name = \"ThisWorkbook\"\nPrivate Sub Workbook_Open()\nEnd Sub\n",
    )
    .unwrap();

    let loaded = load_basproj_from_str(USE_CASE_A_XML, &tmp).unwrap();

    assert_eq!(loaded.manifest.project_name, "VBAProject");
    assert_eq!(loaded.manifest.project_kind, ProjectKind::Host);
    assert_eq!(loaded.manifest.modules.len(), 4);
    assert_eq!(
        loaded.manifest.modules[0].module_kind,
        ModuleKind::Procedural
    );
    assert_eq!(loaded.manifest.modules[1].module_kind, ModuleKind::Class);
    assert_eq!(loaded.manifest.modules[2].module_kind, ModuleKind::Document);
    assert_eq!(loaded.manifest.modules[3].module_kind, ModuleKind::Document);

    // Conditional constants
    assert_eq!(loaded.manifest.conditional_constants["VBA7"], 1);
    assert_eq!(loaded.manifest.conditional_constants["WIN64"], 1);

    // References
    assert_eq!(loaded.manifest.references.len(), 1);
    assert_eq!(
        loaded.manifest.references[0].reference_kind,
        ReferenceKind::TypeLibrary
    );
    assert_eq!(
        loaded.manifest.references[0].referenced_project_name,
        "Excel"
    );

    // Metadata
    assert_eq!(loaded.output_type, OutputType::HostModule);
    assert_eq!(loaded.build_target, BuildTarget::Bundle);
    assert_eq!(loaded.default_root_object, "Application");

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// load_basproj_from_str with filesystem (Use Case C)
// ---------------------------------------------------------------------------

#[test]
fn load_use_case_c_from_str() {
    let tmp = std::env::temp_dir().join("oxvba_test_load_c");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    std::fs::write(
        tmp.join("Main.bas"),
        "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nEnd Sub\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("FileProcessor.bas"),
        "Attribute VB_Name = \"FileProcessor\"\nPublic Sub Process()\nEnd Sub\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("Report.cls"),
        "Attribute VB_Name = \"Report\"\nPublic Property Get Title() As String\nEnd Property\n",
    )
    .unwrap();

    let loaded = load_basproj_from_str(USE_CASE_C_XML, &tmp).unwrap();

    assert_eq!(loaded.manifest.project_name, "ReportGenerator");
    assert_eq!(loaded.manifest.project_kind, ProjectKind::Source);
    assert_eq!(loaded.manifest.modules.len(), 4);
    assert!(
        loaded.manifest.modules[0]
            .module_name
            .starts_with("__OxVbaStartupEntryShim")
    );
    assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));
    assert_eq!(loaded.output_type, OutputType::Exe);

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// NativeExportDescriptor validation
// ---------------------------------------------------------------------------

#[test]
fn load_use_case_b_exports() {
    let tmp = std::env::temp_dir().join("oxvba_test_load_b");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    std::fs::write(
        tmp.join("AddInSetup.bas"),
        "Attribute VB_Name = \"AddInSetup\"\nPublic Sub AutoOpen()\nEnd Sub\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("PricingFunctions.bas"),
        "Attribute VB_Name = \"PricingFunctions\"\nPublic Function BlackScholes() As Double\nEnd Function\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("PricingEngine.cls"),
        "Attribute VB_Name = \"PricingEngine\"\nPublic Sub Calculate()\nEnd Sub\n",
    )
    .unwrap();

    let loaded = load_basproj_from_str(USE_CASE_B_XML, &tmp).unwrap();

    assert_eq!(loaded.native_exports.len(), 2);
    assert_eq!(loaded.native_exports[0].exported_name, "CalcBlackScholes");
    assert_eq!(loaded.native_exports[0].module_name, "PricingFunctions");
    assert_eq!(loaded.native_exports[0].procedure_name, "BlackScholes");
    assert_eq!(
        loaded.native_exports[0].calling_convention,
        CallingConvention::Stdcall
    );
    assert_eq!(loaded.native_exports[1].exported_name, "xlAutoOpen");
    assert_eq!(loaded.native_exports[1].module_name, "AddInSetup");

    assert_eq!(loaded.manifest.project_kind, ProjectKind::Library);
    assert_eq!(loaded.runtime_flavor, RuntimeFlavor::Jit);

    // References: 1 project + 1 COM
    assert_eq!(loaded.manifest.references.len(), 2);
    assert_eq!(
        loaded.manifest.references[0].reference_kind,
        ReferenceKind::Project
    );
    assert_eq!(
        loaded.manifest.references[0].referenced_project_name,
        "CoreMath"
    );
    assert_eq!(
        loaded.manifest.references[1].reference_kind,
        ReferenceKind::TypeLibrary
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// Duplicate export name detection
// ---------------------------------------------------------------------------

#[test]
fn duplicate_export_name_is_error() {
    let xml = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
  </PropertyGroup>
  <ItemGroup>
    <NativeExport Include="SameName">
      <Module>Mod1</Module>
      <Procedure>Proc1</Procedure>
    </NativeExport>
    <NativeExport Include="SameName">
      <Module>Mod2</Module>
      <Procedure>Proc2</Procedure>
    </NativeExport>
  </ItemGroup>
</Project>"#;

    let tmp = std::env::temp_dir().join("oxvba_test_dup_export");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let result = load_basproj_from_str(xml, &tmp);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("duplicate native export name"), "got: {err}");

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// Missing export metadata detection
// ---------------------------------------------------------------------------

#[test]
fn missing_export_module_is_error() {
    let xml = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
  </PropertyGroup>
  <ItemGroup>
    <NativeExport Include="Foo">
      <Procedure>Bar</Procedure>
    </NativeExport>
  </ItemGroup>
</Project>"#;

    let tmp = std::env::temp_dir().join("oxvba_test_missing_module");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let result = load_basproj_from_str(xml, &tmp);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Module"), "got: {err}");

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// Auto-discovery mode
// ---------------------------------------------------------------------------

#[test]
fn auto_discover_modules() {
    let tmp = std::env::temp_dir().join("oxvba_test_auto_discover");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    std::fs::write(tmp.join("Alpha.bas"), "valueOut = 1\nSub A()\nEnd Sub\n").unwrap();
    std::fs::write(tmp.join("Beta.bas"), "Public Sub B()\nEnd Sub\n").unwrap();
    std::fs::write(
        tmp.join("Gamma.cls"),
        "Public Property Get X() As Long\nEnd Property\n",
    )
    .unwrap();
    // Non-VBA file should be ignored
    std::fs::write(tmp.join("readme.txt"), "not a module").unwrap();

    let loaded = load_basproj_from_str(MINIMAL_XML, &tmp).unwrap();

    assert_eq!(loaded.manifest.modules.len(), 4);
    assert!(
        loaded.manifest.modules[0]
            .module_name
            .starts_with("__OxVbaStartupEntryShim")
    );
    assert_eq!(
        loaded.entry_point.as_deref(),
        Some("Alpha.__OxVbaTopLevelMainline")
    );
    // Sorted alphabetically
    assert_eq!(loaded.manifest.modules[1].module_name, "Alpha");
    assert_eq!(
        loaded.manifest.modules[1].module_kind,
        ModuleKind::Procedural
    );
    assert_eq!(loaded.manifest.modules[2].module_name, "Beta");
    assert_eq!(loaded.manifest.modules[3].module_name, "Gamma");
    assert_eq!(loaded.manifest.modules[3].module_kind, ModuleKind::Class);

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// Round-trip: ProjectManifest → .basproj XML → ProjectManifest
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Round-trip with native exports
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Missing OutputType is an error
// ---------------------------------------------------------------------------

#[test]
fn missing_output_type_is_error() {
    let xml = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <ProjectName>NoOutput</ProjectName>
  </PropertyGroup>
</Project>"#;

    let tmp = std::env::temp_dir().join("oxvba_test_no_output");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let result = load_basproj_from_str(xml, &tmp);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("OutputType"), "got: {err}");

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn invalid_project_enum_properties_are_errors() {
    for (property, value, expected) in [
        ("OutputType", "DefinitelyNotExe", "invalid OutputType"),
        ("BuildTarget", "NativeImageEventually", "invalid BuildTarget"),
        ("RuntimeFlavor", "Turbo", "invalid RuntimeFlavor"),
    ] {
        let xml = format!(
            r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>BadEnum</ProjectName>
    <{property}>{value}</{property}>
  </PropertyGroup>
</Project>"#
        );

        let err = parse_basproj_xml(&xml).expect_err("invalid enum value should fail");
        let message = err.to_string();
        assert!(
            message.contains(expected) && message.contains(value),
            "got: {message}"
        );
    }
}

// ---------------------------------------------------------------------------
// Addin no longer requires EntryPoint
// ---------------------------------------------------------------------------

#[test]
fn addin_without_entry_point_loads_successfully() {
    let xml = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Addin</OutputType>
    <ProjectName>MyAddin</ProjectName>
  </PropertyGroup>
</Project>"#;

    let tmp = std::env::temp_dir().join("oxvba_test_addin_no_ep");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let loaded = load_basproj_from_str(xml, &tmp).expect("addin without entry point should load");
    assert_eq!(loaded.output_type, OutputType::Addin);
    assert!(loaded.entry_point.is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// Multiple PropertyGroups merge (last wins)
// ---------------------------------------------------------------------------

#[test]
fn multiple_property_groups_merge() {
    let xml = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>First</ProjectName>
  </PropertyGroup>
  <PropertyGroup>
    <ProjectName>Second</ProjectName>
  </PropertyGroup>
</Project>"#;

    let basproj = parse_basproj_xml(xml).unwrap();
    // Last write wins for ProjectName
    assert_eq!(basproj.properties.project_name.as_deref(), Some("Second"));
    // OutputType from first group persists
    assert_eq!(basproj.properties.output_type, Some(OutputType::Exe));
}

#[test]
fn parse_explicit_build_target_property() {
    let xml = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <BuildTarget>WrapperLibrary</BuildTarget>
    <ProjectName>NativeBridge</ProjectName>
  </PropertyGroup>
</Project>"#;

    let basproj = parse_basproj_xml(xml).unwrap();
    assert_eq!(basproj.properties.output_type, Some(OutputType::Library));
    assert_eq!(
        basproj.properties.build_target,
        Some(BuildTarget::WrapperLibrary)
    );
}

#[test]
fn parse_wrapped_com_server_build_target_and_compat_alias() {
    for spelling in ["WrappedComServer", "WrapperComServer"] {
        let xml = format!(
            r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>ComServer</OutputType>
    <BuildTarget>{spelling}</BuildTarget>
    <ProjectName>ComWidget</ProjectName>
  </PropertyGroup>
</Project>"#
        );

        let basproj = parse_basproj_xml(&xml).expect("parse");
        assert_eq!(basproj.properties.output_type, Some(OutputType::ComServer));
        assert_eq!(
            basproj.properties.build_target,
            Some(BuildTarget::WrappedComServer)
        );
    }
}

// ---------------------------------------------------------------------------
// COMReference with all fields
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// ComServer OutputType parsing
// ---------------------------------------------------------------------------

const COM_SERVER_XML: &str = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>ComServer</OutputType>
    <ProjectName>MyComLib</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Helpers.bas" />
    <ClassModule Include="Widget.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>MyComLib.Widget</ProgId>
      <Description>A sample COM widget class</Description>
    </ClassModule>
    <ClassModule Include="Factory.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>GlobalMultiUse</Instancing>
    </ClassModule>
  </ItemGroup>
</Project>"#;

#[test]
fn parse_com_server_output_type() {
    let basproj = parse_basproj_xml(COM_SERVER_XML).unwrap();

    assert_eq!(basproj.properties.output_type, Some(OutputType::ComServer));
    assert_eq!(basproj.properties.project_name.as_deref(), Some("MyComLib"));

    // Modules
    assert_eq!(basproj.modules.len(), 3);
    assert_eq!(basproj.modules[0].kind, BasProjModuleKind::Module);

    // Widget class with full metadata
    let widget = &basproj.modules[1];
    assert_eq!(widget.kind, BasProjModuleKind::ClassModule);
    assert!(widget.vb_exposed);
    assert!(widget.vb_creatable);
    assert_eq!(widget.instancing, Some(oxvba_project::Instancing::MultiUse));
    assert_eq!(widget.prog_id.as_deref(), Some("MyComLib.Widget"));
    assert_eq!(
        widget.description.as_deref(),
        Some("A sample COM widget class")
    );

    // Factory class with instancing only
    let factory = &basproj.modules[2];
    assert_eq!(
        factory.instancing,
        Some(oxvba_project::Instancing::GlobalMultiUse)
    );
    assert!(factory.prog_id.is_none());
    assert!(factory.description.is_none());
}

#[test]
fn load_com_server_project() {
    let tmp = std::env::temp_dir().join("oxvba_test_com_server");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    std::fs::write(tmp.join("Helpers.bas"), "Public Sub Helper()\nEnd Sub\n").unwrap();
    std::fs::write(tmp.join("Widget.cls"), "Public Sub DoWork()\nEnd Sub\n").unwrap();
    std::fs::write(
        tmp.join("Factory.cls"),
        "Public Function Create() As Object\nEnd Function\n",
    )
    .unwrap();

    let loaded = load_basproj_from_str(COM_SERVER_XML, &tmp).unwrap();

    assert_eq!(loaded.output_type, OutputType::ComServer);
    assert_eq!(loaded.manifest.project_kind, ProjectKind::Library);
    assert_eq!(loaded.manifest.modules.len(), 3);

    // Class module metadata round-trips through LoadedProject
    assert_eq!(loaded.class_module_metadata.len(), 2);
    let widget_meta = loaded.class_module_metadata.get("Widget").unwrap();
    assert_eq!(
        widget_meta.instancing,
        Some(oxvba_project::Instancing::MultiUse)
    );
    assert_eq!(widget_meta.prog_id.as_deref(), Some("MyComLib.Widget"));
    assert_eq!(
        widget_meta.description.as_deref(),
        Some("A sample COM widget class")
    );

    let factory_meta = loaded.class_module_metadata.get("Factory").unwrap();
    assert_eq!(
        factory_meta.instancing,
        Some(oxvba_project::Instancing::GlobalMultiUse)
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// ComServer + Instancing/ProgId round-trip
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// All Instancing variants parse correctly
// ---------------------------------------------------------------------------

#[test]
fn all_instancing_variants_parse() {
    for (name, expected) in [
        ("Private", oxvba_project::Instancing::Private),
        (
            "PublicNotCreatable",
            oxvba_project::Instancing::PublicNotCreatable,
        ),
        ("MultiUse", oxvba_project::Instancing::MultiUse),
        ("GlobalMultiUse", oxvba_project::Instancing::GlobalMultiUse),
        ("SingleUse", oxvba_project::Instancing::SingleUse),
        (
            "GlobalSingleUse",
            oxvba_project::Instancing::GlobalSingleUse,
        ),
    ] {
        let xml = format!(
            r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup><OutputType>ComServer</OutputType></PropertyGroup>
  <ItemGroup>
    <ClassModule Include="C.cls">
      <Instancing>{name}</Instancing>
    </ClassModule>
  </ItemGroup>
</Project>"#
        );
        let basproj = parse_basproj_xml(&xml).unwrap();
        assert_eq!(
            basproj.modules[0].instancing,
            Some(expected),
            "Failed for {name}"
        );
    }
}

// ---------------------------------------------------------------------------
// Project reference resolution: two-project chain
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Diamond dependency: A→B, A→C, B→C
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Cyclic project reference detection
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Missing project reference error
// ---------------------------------------------------------------------------

#[test]
fn missing_project_reference_resolves_gracefully() {
    let tmp = std::env::temp_dir().join("oxvba_test_missing_ref");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    std::fs::write(tmp.join("Mod.bas"), "Public Sub M()\nEnd Sub\n").unwrap();

    let xml = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <ProjectName>TestProj</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Mod.bas" />
  </ItemGroup>
  <ItemGroup>
    <ProjectReference Include="..\NonExistent\NonExistent.basproj" />
  </ItemGroup>
</Project>"#;

    // Reference resolution failure is gracefully handled (unwrap_or_default)
    let loaded = load_basproj_from_str(xml, &tmp).unwrap();
    assert_eq!(loaded.manifest.project_name, "TestProj");
    // Failed resolution defaults to empty
    assert!(loaded.manifest.reference_projects.is_empty());

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// Native reference validation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// VBP round-trip: parse VBP → generate basproj XML → re-parse XML
// ---------------------------------------------------------------------------

#[test]
fn vbp_round_trip_preserves_modules_and_references() {
    let vbp_content = r#"Type=Exe
Reference=*\G{00020430-0000-0000-C000-000000000046}#2.0#0#C:\Windows\System32\stdole2.tlb#OLE Automation
Module=Module1; Module1.bas
Module=Utils; Utils.bas
Class=MyClass; MyClass.cls
Name="TestProject"
"#;
    let vbp = oxvba_project::vbp::parse_vbp(vbp_content).unwrap();
    assert_eq!(vbp.modules.len(), 2);
    assert_eq!(vbp.classes.len(), 1);
    assert_eq!(vbp.references.len(), 1);

    let xml = oxvba_project::vbp::generate_basproj_from_vbp(&vbp).unwrap();
    let basproj = oxvba_project::parse_basproj_xml(&xml).unwrap();

    // 2 Module items + 1 ClassModule = 3 total in basproj.modules
    assert_eq!(basproj.modules.len(), 3);
    assert_eq!(basproj.modules[0].kind, BasProjModuleKind::Module);
    assert_eq!(basproj.modules[0].include, "Module1.bas");
    assert_eq!(basproj.modules[1].kind, BasProjModuleKind::Module);
    assert_eq!(basproj.modules[1].include, "Utils.bas");
    assert_eq!(basproj.modules[2].kind, BasProjModuleKind::ClassModule);
    assert_eq!(basproj.modules[2].include, "MyClass.cls");

    // The VBP reference should produce a COMReference
    assert_eq!(basproj.com_references.len(), 1);
    assert_eq!(basproj.com_references[0].include, "OLE Automation");
    assert_eq!(
        basproj.com_references[0].guid.as_deref(),
        Some("{00020430-0000-0000-C000-000000000046}")
    );
    assert_eq!(basproj.com_references[0].version_major, Some(2));
    assert_eq!(basproj.com_references[0].version_minor, Some(0));

    // Project-level properties
    assert_eq!(
        basproj.properties.project_name.as_deref(),
        Some("TestProject")
    );
    assert_eq!(basproj.properties.output_type, Some(OutputType::Exe));
}

#[test]
fn vbp_round_trip_preserves_project_reference_items() {
    let vbp_content = r#"Type=Exe
Reference=*\A{11111111-2222-3333-4444-555555555555}#1.0#0#..\LibScale\LibScale.vbp#LibScale
Module=Main; Main.bas
Name="MainApp"
"#;
    let vbp = oxvba_project::vbp::parse_vbp(vbp_content).unwrap();
    assert_eq!(vbp.references.len(), 1);

    let xml = oxvba_project::vbp::generate_basproj_from_vbp(&vbp).unwrap();
    let basproj = oxvba_project::parse_basproj_xml(&xml).unwrap();

    assert_eq!(basproj.project_references.len(), 1);
    assert_eq!(
        basproj.project_references[0].include,
        "../LibScale/LibScale.vbp"
    );
    assert!(basproj.com_references.is_empty());
    assert_eq!(basproj.properties.project_name.as_deref(), Some("MainApp"));
}
