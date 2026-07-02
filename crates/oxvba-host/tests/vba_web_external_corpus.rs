//! Ignored VBA-Web external-corpus checks.
//!
//! These tests use the local checkout under `.external/vba-corpus/vba-web` and
//! synthesize temporary `.basproj` files. They deliberately do not include the
//! local `ExcelApplicationShim.bas` fixture, so failures here catch regressions
//! in host-injected `Application` metadata and project-closure execution.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use oxvba_com::{
    OptionalParamDefault, PortableComProjection, SourceTypeKind, TypeLibMemberInvokeKind,
    TypeLibMetadataBlob, TypeLibParamType, TypeLibResolveRequest, TypeLibResolvedIdentity,
    TypeLibWireType,
    platform::portable::{PortableDispatch, PortableObjectFactory},
};
use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig, HostProfileProvider};
use oxvba_project::load_project_closure;
use oxvba_runtime::Variant;
use oxvba_symbol::{CatalogTypeLibResolver, TypeLibResolver};

static EXTRACTED_SPEC_RUN_LOCK: Mutex<()> = Mutex::new(());

const VBA_WEB_ROOT: &str = ".external/vba-corpus/vba-web";
const VBA_WEB_EXTRACTED_SPECS_ROOT: &str =
    ".external/vba-corpus/vba-web/fixtures/extracted-spec-workbooks/VBA-Web_-_Specs";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostShape {
    InlineHostFile,
    ReferencedHostProject,
    HostInjectedProfile,
}

impl HostShape {
    fn suffix(self) -> &'static str {
        match self {
            HostShape::InlineHostFile => "InlineHost",
            HostShape::ReferencedHostProject => "ReferencedHost",
            HostShape::HostInjectedProfile => "HostInjected",
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn vba_web_root() -> PathBuf {
    repo_root().join(VBA_WEB_ROOT)
}

fn vba_web_extracted_specs_root() -> PathBuf {
    repo_root().join(VBA_WEB_EXTRACTED_SPECS_ROOT)
}

fn require_vba_web_root() -> PathBuf {
    let root = vba_web_root();
    assert!(
        root.join("src/WebHelpers.bas").is_file(),
        "VBA-Web external corpus checkout is required at {}",
        root.display()
    );
    root
}

fn require_vba_web_extracted_specs_root() -> PathBuf {
    let root = vba_web_extracted_specs_root();
    assert!(
        root.join("SpecSuite.cls").is_file() && root.join("Specs_WebRequest.bas").is_file(),
        "extracted VBA-Web spec workbook modules are required at {}; extract the local spec workbook before running this ignored test",
        root.display()
    );
    root
}

fn q(path: &Path) -> String {
    path.display().to_string()
}

fn application_module() -> &'static str {
    r#"
Attribute VB_Name = "Application"
Option Explicit

Public Function Run(ByVal Macro As Variant, Optional ByVal Arg1 As Variant) As Variant
    Run = 42
End Function

Public Sub OnTime(ByVal EarliestTime As Variant, ByVal Procedure As String, Optional ByVal LatestTime As Variant, Optional ByVal Schedule As Variant)
End Sub
"#
}

fn host_probe_module() -> &'static str {
    r#"
Attribute VB_Name = "HostProbe"
Option Explicit

Public Sub AssertHostRoot()
    Dim value As Variant
    value = Application.Run("MacroName", 1)
    If value <> 42 Then Err.Raise 52901, "VbaWebHostProbe", "Application.Run returned " & CStr(value)
    Application.OnTime 0, "MacroName"
End Sub
"#
}

fn scripting_com_reference_xml() -> &'static str {
    r#"    <COMReference Include="Scripting">
      <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
      <ImportLib>scrrun.dll</ImportLib>
    </COMReference>
"#
}

fn core_basproj(root: &Path, shape: HostShape, extra_core_module: Option<&str>) -> String {
    let project_name = format!("VbaWebCore{}", shape.suffix());
    let host_reference = match shape {
        HostShape::InlineHostFile => r#"    <Module Include="Application.bas" />
"#
        .to_string(),
        HostShape::ReferencedHostProject => {
            r#"    <ProjectReference Include="FakeExcelHost.basproj" />
"#
            .to_string()
        }
        HostShape::HostInjectedProfile => r#"    <ProjectReference Include="Excel.Application">
      <Kind>HostInjected</Kind>
    </ProjectReference>
"#
        .to_string(),
    };
    let extra_core_module = extra_core_module
        .map(|module| format!("    <Module Include=\"{module}\" />\n"))
        .unwrap_or_default();
    format!(
        r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <ProjectName>{}</ProjectName>
  </PropertyGroup>
  <ItemGroup>
{}    <Module Include="HostProbe.bas" />
{}{}    <Module Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
    <ClassModule Include="{}" />
  </ItemGroup>
</Project>
"#,
        project_name,
        host_reference,
        extra_core_module,
        scripting_com_reference_xml(),
        q(&root.join("src/WebHelpers.bas")),
        q(&root.join("src/IWebAuthenticator.cls")),
        q(&root.join("src/WebAsyncWrapper.cls")),
        q(&root.join("src/WebClient.cls")),
        q(&root.join("src/WebRequest.cls")),
        q(&root.join("src/WebResponse.cls")),
        q(&root.join("authenticators/DigestAuthenticator.cls")),
        q(&root.join("authenticators/EmptyAuthenticator.cls")),
        q(&root.join("authenticators/FacebookAuthenticator.cls")),
        q(&root.join("authenticators/GoogleAuthenticator.cls")),
        q(&root.join("authenticators/HttpBasicAuthenticator.cls")),
        q(&root.join("authenticators/OAuth1Authenticator.cls")),
        q(&root.join("authenticators/OAuth2Authenticator.cls")),
        q(&root.join("authenticators/OPSAuthenticator.cls")),
        q(&root.join("authenticators/TodoistAuthenticator.cls")),
        q(&root.join("authenticators/TwitterAuthenticator.cls")),
        q(&root.join("authenticators/WindowsAuthenticator.cls")),
    )
}

fn write_fake_host_project(temp: &Path) {
    std::fs::write(temp.join("Application.bas"), application_module()).expect("write Application");
    std::fs::write(
        temp.join("FakeExcelHost.basproj"),
        r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <ProjectName>FakeExcelHost</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Application.bas" />
  </ItemGroup>
</Project>
"#,
    )
    .expect("write fake host basproj");
}

fn write_synthetic_project(
    shape: HostShape,
    harness_source: Option<&str>,
    core_probe_source: Option<&str>,
) -> PathBuf {
    let root = require_vba_web_root();
    let temp = std::env::temp_dir().join(format!(
        "oxvba-vbaweb-{}-{}-{}",
        shape.suffix().to_ascii_lowercase(),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("create temp project");
    if shape != HostShape::HostInjectedProfile {
        std::fs::write(temp.join("Application.bas"), application_module())
            .expect("write Application module");
    }
    if shape == HostShape::ReferencedHostProject {
        write_fake_host_project(&temp);
    }
    std::fs::write(temp.join("HostProbe.bas"), host_probe_module()).expect("write HostProbe");
    if let Some(core_probe_source) = core_probe_source {
        std::fs::write(temp.join("VbaWebCoreProbe.bas"), core_probe_source)
            .expect("write core probe module");
    }
    let core_project = format!("VbaWebCore{}.basproj", shape.suffix());
    let core_probe_module = core_probe_source.map(|_| "VbaWebCoreProbe.bas");
    std::fs::write(
        temp.join(&core_project),
        core_basproj(&root, shape, core_probe_module),
    )
    .expect("write core basproj");
    if let Some(harness_source) = harness_source {
        std::fs::write(temp.join("HarnessMain.bas"), harness_source).expect("write harness module");
        let harness_project = format!("VbaWebHarness{}.basproj", shape.suffix());
        std::fs::write(
            temp.join(&harness_project),
            format!(
                r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>VbaWebHarness{}</ProjectName>
    <EntryPoint>HarnessMain.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="{}" />
{}    <Module Include="HarnessMain.bas" />
  </ItemGroup>
</Project>
"#,
                shape.suffix(),
                core_project,
                scripting_com_reference_xml()
            ),
        )
        .expect("write harness basproj");
        temp.join(harness_project)
    } else {
        temp.join(core_project)
    }
}

fn app_member(
    name: &str,
    token: i32,
    parameter_names: Vec<&str>,
    parameter_optional: Vec<bool>,
    parameter_types: Vec<TypeLibParamType>,
    return_type: Option<TypeLibParamType>,
) -> oxvba_com::TypeLibMemberMetadata {
    let parameter_optional_defaults = parameter_optional
        .iter()
        .map(|optional| {
            if *optional {
                OptionalParamDefault::OptionalVariant
            } else {
                OptionalParamDefault::Required
            }
        })
        .collect::<Vec<_>>();
    oxvba_com::TypeLibMemberMetadata {
        name: name.to_string(),
        token,
        vtable_slot: None,
        requires_argument: !parameter_names.is_empty(),
        invoke_kind: TypeLibMemberInvokeKind::Method,
        parameter_names: parameter_names.into_iter().map(str::to_string).collect(),
        parameter_optional,
        parameter_optional_defaults,
        is_default_member: false,
        parameter_wire_types: parameter_types
            .iter()
            .cloned()
            .map(TypeLibWireType::Automation)
            .collect(),
        parameter_iids: vec![None; parameter_types.len()],
        parameter_types,
        return_wire_type: return_type.map(TypeLibWireType::Automation),
        return_type,
        callconv_is_stdcall: false,
        is_dual: true,
        interface_iid: None,
        source_typekind: Some(SourceTypeKind::Dispatch),
        vtable_slot_bound: None,
    }
}

fn application_blob() -> TypeLibMetadataBlob {
    TypeLibMetadataBlob {
        identity: TypeLibResolvedIdentity {
            reference_name: "Excel".into(),
            requested_coclass: Some("Application".into()),
            importlib: "excel".into(),
            libid: None,
            major_version: 1,
            minor_version: 0,
            lcid: None,
            cache_key: "vba-web-external-application".into(),
        },
        activation_prog_id: Some("Excel.Application".into()),
        member_name_to_token: vec![("Run".into(), 10), ("OnTime".into(), 11)],
        members: vec![
            app_member(
                "Run",
                10,
                vec!["Macro", "Arg1"],
                vec![false, true],
                vec![TypeLibParamType::Variant, TypeLibParamType::Variant],
                Some(TypeLibParamType::Variant),
            ),
            app_member(
                "OnTime",
                11,
                vec!["EarliestTime", "Procedure", "LatestTime", "Schedule"],
                vec![false, false, true, true],
                vec![
                    TypeLibParamType::Variant,
                    TypeLibParamType::String,
                    TypeLibParamType::Variant,
                    TypeLibParamType::Variant,
                ],
                None,
            ),
        ],
        events: Vec::new(),
        coclass_names: vec!["Application".into()],
    }
}

struct VbaWebResolver;

impl TypeLibResolver for VbaWebResolver {
    fn resolve(&self, request: &TypeLibResolveRequest) -> Option<TypeLibMetadataBlob> {
        if request
            .reference_name
            .eq_ignore_ascii_case("Excel.Application")
            || request.reference_name.eq_ignore_ascii_case("Excel")
        {
            return Some(application_blob());
        }
        CatalogTypeLibResolver.resolve(request)
    }
}

struct RecordingApplicationFactory {
    calls: Arc<Mutex<Vec<String>>>,
}

impl PortableObjectFactory for RecordingApplicationFactory {
    fn create(&self) -> Box<dyn PortableDispatch> {
        Box::new(RecordingApplication {
            calls: self.calls.clone(),
        })
    }
}

struct RecordingApplication {
    calls: Arc<Mutex<Vec<String>>>,
}

impl PortableDispatch for RecordingApplication {
    fn invoke(&self, member: &str, args: &[Variant]) -> Result<Variant, String> {
        self.calls
            .lock()
            .map_err(|_| "call log poisoned".to_string())?
            .push(format!("{member}:{}", args.len()));
        match member {
            "Run" => Ok(Variant::from_i32(42)),
            "OnTime" => Ok(Variant::empty()),
            other => Err(format!("unexpected Application invoke `{other}`")),
        }
    }

    fn get(&self, member: &str) -> Result<Variant, String> {
        Err(format!("unexpected Application get `{member}`"))
    }

    fn put(&self, member: &str, _value: Variant) -> Result<(), String> {
        Err(format!("unexpected Application put `{member}`"))
    }

    fn member_names(&self) -> Vec<String> {
        vec!["Run".into(), "OnTime".into()]
    }
}

fn engine(shape: HostShape, calls: Arc<Mutex<Vec<String>>>) -> Engine {
    if shape != HostShape::HostInjectedProfile {
        return Engine::new(HostConfig { enable_jit: false });
    }
    let projection = Arc::new(PortableComProjection::new());
    projection.register_object(
        "Excel.Application",
        Arc::new(RecordingApplicationFactory { calls }),
    );
    let profile = HostProfileProvider::new()
        .with_typelib_resolver(Arc::new(VbaWebResolver))
        .with_portable_com_projection(projection)
        .with_host_policy(HostPolicy::interactive_dev());
    Engine::new(HostConfig { enable_jit: false }).with_host_profile_provider(profile)
}

fn run_project(path: &Path, shape: HostShape, calls: Arc<Mutex<Vec<String>>>) -> Vec<Variant> {
    let closure = load_project_closure(path).expect("load project closure");
    engine(shape, calls)
        .execute_project_closure_with_variant_snapshot(&closure)
        .expect("execute project closure")
}

fn run_harness_for_shape(shape: HostShape, harness: &str) -> Arc<Mutex<Vec<String>>> {
    let project = write_synthetic_project(shape, Some(harness), None);
    let calls = Arc::new(Mutex::new(Vec::new()));
    run_project(&project, shape, calls.clone());
    calls
}

fn run_harness_with_core_probe_for_shape(
    shape: HostShape,
    core_probe: &str,
    harness: &str,
) -> Arc<Mutex<Vec<String>>> {
    let project = write_synthetic_project(shape, Some(harness), Some(core_probe));
    let calls = Arc::new(Mutex::new(Vec::new()));
    run_project(&project, shape, calls.clone());
    calls
}

fn sanitized_spec_module_copy_with_override(
    source: &Path,
    temp: &Path,
    override_text: Option<&str>,
) -> String {
    let text = override_text
        .map(str::to_string)
        .unwrap_or_else(|| std::fs::read_to_string(source).expect("read extracted spec module"));
    let mut sanitized = String::new();
    let mut relocated_default_attrs = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let compact = trimmed.to_ascii_lowercase().replace([' ', '\t'], "");
        let is_default_member_attr =
            compact.starts_with("attribute") && compact.contains(".vb_usermemid=0");
        if trimmed.starts_with("Attribute ") && trimmed.contains('.') {
            if is_default_member_attr {
                relocated_default_attrs.push(trimmed.to_string());
            }
            continue;
        }
        sanitized.push_str(line);
        sanitized.push('\n');
    }
    for attr in relocated_default_attrs {
        sanitized.push_str(&attr);
        sanitized.push('\n');
    }
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .expect("module file name");
    std::fs::write(temp.join(file_name), sanitized).expect("write sanitized spec module");
    file_name.to_string()
}

fn write_extracted_spec_runner_project(harness_source: &str) -> PathBuf {
    write_extracted_spec_runner_project_with_overrides(harness_source, &HashMap::new())
}

fn write_extracted_spec_runner_project_with_overrides(
    harness_source: &str,
    overrides: &HashMap<&'static str, String>,
) -> PathBuf {
    let root = require_vba_web_extracted_specs_root();
    let temp = std::env::temp_dir().join(format!(
        "oxvba-vbaweb-spec-runner-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("create temp spec project");
    std::fs::write(temp.join("HarnessMain.bas"), harness_source).expect("write spec harness");
    std::fs::write(
        temp.join("ThisWorkbook.bas"),
        r#"
Attribute VB_Name = "ThisWorkbook"
Option Explicit

Public Property Get Path() As String
    Path = ""
End Property
"#,
    )
    .expect("write ThisWorkbook shim");
    std::fs::write(
        temp.join("SpecRunner.bas"),
        r#"
Attribute VB_Name = "SpecRunner"
Option Explicit
"#,
    )
    .expect("write SpecRunner shim");
    std::fs::write(
        temp.join("Specs.bas"),
        r#"
Attribute VB_Name = "Specs"
Option Explicit

Public Property Get HttpbinBaseUrl() As String
    HttpbinBaseUrl = "http://httpbin.org/"
End Property

Public Sub RunSpecs()
    Dim Reporter As New WorkbookReporter
    Reporter.Start NumSuites:=1
    Reporter.Output Specs_WebRequest.Specs
    Reporter.Done
End Sub
"#,
    )
    .expect("write Specs compatibility module");
    std::fs::write(
        temp.join("ImmediateReporter.cls"),
        r#"
Attribute VB_Name = "ImmediateReporter"
Option Explicit

Private WithEvents pSpecs As SpecSuite

Public Sub ListenTo(Specs As SpecSuite)
    Debug.Print "===" & IIf(Specs.Description <> "", " " & Specs.Description & " ===", "")
    Set pSpecs = Specs
End Sub

Public Sub Done()
    Debug.Print "= spec run complete ="
End Sub

Private Sub pSpecs_Result(Spec As SpecDefinition)
    Debug.Print Spec.Description
End Sub
"#,
    )
    .expect("write ImmediateReporter shim");
    std::fs::write(
        temp.join("WorkbookReporter.cls"),
        r#"
Attribute VB_Name = "WorkbookReporter"
Option Explicit

Private pCount As Long
Private pTotal As Long
Private pSuites As Collection

Public Sub ConnectTo(Target As Variant)
    Debug.Print "= workbook reporter connected ="
End Sub

Public Sub Start(Optional NumSuites As Long = 0)
    pCount = 0
    pTotal = NumSuites
    Set pSuites = New Collection
    Debug.Print "= spec run start " & CStr(NumSuites) & " ="
End Sub

Public Sub Output(Suite As SpecSuite)
    If pSuites Is Nothing Then
        Set pSuites = New Collection
    End If
    pCount = pCount + 1
    pSuites.Add Suite
    Debug.Print Suite.Description & " " & CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count)
    If pTotal > 0 Then
        Debug.Print "progress " & CStr(pCount) & "/" & CStr(pTotal)
    End If
End Sub

Public Sub Done()
    Dim Failed As Boolean
    Dim Suite As SpecSuite
    If Not pSuites Is Nothing Then
        For Each Suite In pSuites
            If Suite.FailedSpecs.Count > 0 Then
                Failed = True
                Exit For
            End If
        Next Suite
    End If
    Debug.Print "= spec run " & IIf(Failed, "FAIL", "PASS") & " ="
End Sub
"#,
    )
    .expect("write WorkbookReporter shim");

    let module_names = ["Specs_WebRequest.bas", "WebHelpers.bas"];
    let class_names = [
        "Dictionary.cls",
        "IWebAuthenticator.cls",
        "SpecConverter.cls",
        "SpecDefinition.cls",
        "SpecExpectation.cls",
        "SpecSuite.cls",
        "WebClient.cls",
        "WebRequest.cls",
        "WebResponse.cls",
    ];
    let mut items = String::new();
    for name in module_names {
        let copied = sanitized_spec_module_copy_with_override(
            &root.join(name),
            &temp,
            overrides.get(name).map(String::as_str),
        );
        items.push_str(&format!(
            "    <Module Include=\"{}\" />\n",
            q(&temp.join(copied))
        ));
    }
    for name in class_names {
        let copied = sanitized_spec_module_copy_with_override(
            &root.join(name),
            &temp,
            overrides.get(name).map(String::as_str),
        );
        items.push_str(&format!(
            "    <ClassModule Include=\"{}\" />\n",
            q(&temp.join(copied))
        ));
    }
    items.push_str("    <Module Include=\"ThisWorkbook.bas\" />\n");
    items.push_str("    <Module Include=\"SpecRunner.bas\" />\n");
    items.push_str("    <Module Include=\"Specs.bas\" />\n");
    items.push_str("    <Module Include=\"HarnessMain.bas\" />\n");
    items.push_str("    <ClassModule Include=\"ImmediateReporter.cls\" />\n");
    items.push_str("    <ClassModule Include=\"WorkbookReporter.cls\" />\n");

    let project = temp.join("VbaWebExtractedSpecs.basproj");
    std::fs::write(
        &project,
        format!(
            r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>VbaWebExtractedSpecs</ProjectName>
    <EntryPoint>HarnessMain.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="Excel.Application">
      <Kind>HostInjected</Kind>
    </ProjectReference>
{}{}
  </ItemGroup>
</Project>
"#,
            scripting_com_reference_xml(),
            items
        ),
    )
    .expect("write extracted spec basproj");
    project
}

fn run_extracted_spec_harness(harness: &str) -> Vec<Variant> {
    let _guard = EXTRACTED_SPEC_RUN_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let project = write_extracted_spec_runner_project(harness);
    run_project(
        &project,
        HostShape::HostInjectedProfile,
        Arc::new(Mutex::new(Vec::new())),
    )
}

fn run_extracted_spec_harness_with_overrides(
    harness: &str,
    overrides: &HashMap<&'static str, String>,
) -> Vec<Variant> {
    let _guard = EXTRACTED_SPEC_RUN_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let project = write_extracted_spec_runner_project_with_overrides(harness, overrides);
    run_project(
        &project,
        HostShape::HostInjectedProfile,
        Arc::new(Mutex::new(Vec::new())),
    )
}

fn limited_webrequest_specs_source(limit: usize) -> String {
    let root = require_vba_web_extracted_specs_root();
    let source = std::fs::read_to_string(root.join("Specs_WebRequest.bas"))
        .expect("read extracted Specs_WebRequest module");
    let mut out = String::new();
    let mut seen = 0usize;
    let mut inserted = false;
    for line in source.lines() {
        if !inserted && line.trim_start().starts_with("With Specs.It(") {
            if seen >= limit {
                out.push_str("    Exit Function\n");
                inserted = true;
            }
            seen += 1;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[test]
#[ignore = "external corpus; requires .external/vba-corpus/vba-web checkout"]
fn vba_web_raw_upstream_sources_build_without_application_shim() {
    let project = write_synthetic_project(HostShape::HostInjectedProfile, None, None);
    let _values = run_project(
        &project,
        HostShape::HostInjectedProfile,
        Arc::new(Mutex::new(Vec::new())),
    );
}

#[test]
#[ignore = "external corpus; requires .external/vba-corpus/vba-web checkout"]
fn vba_web_raw_upstream_sources_run_pure_helper_harness_without_application_shim() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public Sub Main()
    Dim encoded As String
    encoded = WebHelpers.UrlEncode("a b+c")
    If encoded <> "a%20b%2Bc" Then Err.Raise 52001, "VbaWebHarness", encoded

    Dim decoded As String
    decoded = WebHelpers.UrlDecode("a%20b%2Bc")
    If decoded <> "a b+c" Then Err.Raise 52002, "VbaWebHarness", decoded

    Dim joined As String
    joined = WebHelpers.JoinUrl("https://example.test/api/", "/v1")
    If joined <> "https://example.test/api/v1" Then Err.Raise 52003, "VbaWebHarness", joined

    Dim methodName As String
    methodName = WebHelpers.MethodToName(WebMethod.HttpPost)
    If methodName <> "POST" Then Err.Raise 52004, "VbaWebHarness", methodName

    Dim obfuscated As String
    obfuscated = WebHelpers.Obfuscate("abcdef", "#")
    If obfuscated <> "######" Then Err.Raise 52005, "VbaWebHarness", obfuscated
End Sub
"########;
    run_harness_for_shape(HostShape::HostInjectedProfile, harness);
}

#[test]
#[ignore = "external corpus; requires .external/vba-corpus/vba-web checkout"]
fn vba_web_host_root_shapes_execute_equivalent_harnesses() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public Sub Main()
    HostProbe.AssertHostRoot

    Dim encoded As String
    encoded = WebHelpers.UrlEncode("a b+c")
    If encoded <> "a%20b%2Bc" Then Err.Raise 52201, "VbaWebHarness", encoded

    Dim decoded As String
    decoded = WebHelpers.UrlDecode("a%20b%2Bc")
    If decoded <> "a b+c" Then Err.Raise 52202, "VbaWebHarness", decoded

    Dim joined As String
    joined = WebHelpers.JoinUrl("https://example.test/api/", "/v1")
    If joined <> "https://example.test/api/v1" Then Err.Raise 52203, "VbaWebHarness", joined
End Sub
"########;
    run_harness_for_shape(HostShape::InlineHostFile, harness);
    run_harness_for_shape(HostShape::ReferencedHostProject, harness);
    let calls = run_harness_for_shape(HostShape::HostInjectedProfile, harness);
    assert_eq!(
        calls.lock().expect("call log").as_slice(),
        ["Run:2".to_string(), "OnTime:2".to_string()]
    );
}

#[test]
#[ignore = "external corpus; requires .external/vba-corpus/vba-web checkout and normal Scripting.Dictionary COM registration"]
fn vba_web_broad_library_harness_executes_with_normal_com_dependencies() {
    let core_probe = r########"
Attribute VB_Name = "VbaWebCoreProbe"
Option Explicit

Private AssertCounter As Long
Private ProbeStage As Long

Private Sub AssertEqual(ByVal label As String, ByVal actual As Variant, ByVal expected As Variant)
    AssertCounter = AssertCounter + 1
    If actual <> expected Then Err.Raise 52300 + AssertCounter, "VbaWebBroadHarness", label & ": " & CStr(actual) & " <> " & CStr(expected)
End Sub

Public Sub RunBroadProbe()
    On Error GoTo ProbeFailed
    ProbeStage = 1
    HostProbe.AssertHostRoot

    AssertEqual "Obfuscate default", WebHelpers.Obfuscate("secret"), "******"
    AssertEqual "Obfuscate custom", WebHelpers.Obfuscate("abc", "_"), "___"
    AssertEqual "MethodToName GET", WebHelpers.MethodToName(WebMethod.HttpGet), "GET"
    AssertEqual "MethodToName PATCH", WebHelpers.MethodToName(WebMethod.HttpPatch), "PATCH"
    AssertEqual "Json media type", WebHelpers.FormatToMediaType(WebFormat.Json), "application/json"
    AssertEqual "Plain media type", WebHelpers.FormatToMediaType(WebFormat.PlainText), "text/plain"
    AssertEqual "JoinUrl left slash", WebHelpers.JoinUrl("a/", "b"), "a/b"
    AssertEqual "JoinUrl right slash", WebHelpers.JoinUrl("a", "/b"), "a/b"
    AssertEqual "UrlEncode strict", WebHelpers.UrlEncode("A + B"), "A%20%2B%20B"
    AssertEqual "UrlEncode form", WebHelpers.UrlEncode("A + B", EncodingMode:=UrlEncodingMode.FormUrlEncoding), "A+%2B+B"
    AssertEqual "UrlDecode form", WebHelpers.UrlDecode("A+%2B+B", EncodingMode:=UrlEncodingMode.FormUrlEncoding), "A + B"

    ProbeStage = 2
    Dim parsed As Dictionary
    Set parsed = WebHelpers.ParseUrlEncoded("a=1&b=3.14&c=Howdy%21&d+%26+e=A+%2B+B")
    AssertEqual "ParseUrlEncoded count", parsed.Count, 4
    If IsEmpty(parsed("c")) Then Err.Raise 52331, "VbaWebBroadHarness", "ParseUrlEncoded c is Empty"
    If IsNull(parsed("c")) Then Err.Raise 52332, "VbaWebBroadHarness", "ParseUrlEncoded c is Null"
    AssertEqual "ParseUrlEncoded c", parsed("c"), "Howdy!"
    AssertEqual "ParseUrlEncoded encoded key", parsed("d & e"), "A + B"

    ProbeStage = 3
    Dim obj As New Dictionary
    obj.Add "a", 1
    obj.Add "b", "Howdy!"
    obj.Add "c & d", "A + B"

    ProbeStage = 4
    Dim json As Object
    Set json = WebHelpers.ParseJson("{""a"":1,""b"":3.14,""c"":""Howdy!"",""d"":true}")
    AssertEqual "ParseJson number", json("a"), 1
    AssertEqual "ParseJson string", json("c"), "Howdy!"
    Dim nested As Object
    Set nested = WebHelpers.ParseJson("{""child"":{""name"":""Ada""},""items"":[1,2]}")
    AssertEqual "ParseJson nested object", nested("child")("name"), "Ada"
    AssertEqual "ParseJson nested array", nested("items")(2), 2

    ProbeStage = 5
    Dim keyValue As Dictionary
    Set keyValue = WebHelpers.CreateKeyValue("abc", 123)
    AssertEqual "CreateKeyValue key", keyValue("Key"), "abc"
    AssertEqual "CreateKeyValue value", keyValue("Value"), 123

    Dim keyValues As New Collection
    keyValues.Add WebHelpers.CreateKeyValue("a", 123)
    keyValues.Add WebHelpers.CreateKeyValue("b", 456)
    AssertEqual "FindInKeyValues", WebHelpers.FindInKeyValues(keyValues, "b"), 456
    WebHelpers.AddOrReplaceInKeyValues keyValues, "b", "def"
    WebHelpers.AddOrReplaceInKeyValues keyValues, "c", "ghi"
    AssertEqual "AddOrReplace count", keyValues.Count, 3
    AssertEqual "AddOrReplace retained order", keyValues(2)("Value"), "def"

    Dim cloned As Dictionary
    Set cloned = WebHelpers.CloneDictionary(obj)
    AssertEqual "CloneDictionary count", cloned.Count, obj.Count
    AssertEqual "CloneDictionary value", cloned("b"), "Howdy!"

    Dim coll As New Collection
    coll.Add "abc"
    coll.Add 123
    Dim clonedColl As Collection
    Set clonedColl = WebHelpers.CloneCollection(coll)
    AssertEqual "CloneCollection count", clonedColl.Count, 2
    AssertEqual "CloneCollection value", clonedColl(1), "abc"

    ProbeStage = 6
    Dim request As New WebRequest
    request.Resource = "orders/{id}"
    request.Method = WebMethod.HttpPost
    request.AddUrlSegment "id", "A + B"
    AssertEqual "Request encoded segment value", WebHelpers.UrlEncode(request.UrlSegments("id")), "A%20%2B%20B"
    AssertEqual "Request segment formatted resource", request.FormattedResource, "orders/A%20%2B%20B"
    request.AddQuerystringParam "page", 2
    AssertEqual "Request one query formatted resource", request.FormattedResource, "orders/A%20%2B%20B?page=2"
    request.AddQuerystringParam "active", True
    request.AddHeader "X-Test", "yes"
    request.AddCookie "session", "abc 123"
    request.AddBodyParameter "message", "Howdy!"
    request.AddBodyParameter "count", 3
    AssertEqual "Request resource field", request.Resource, "orders/{id}"
    AssertEqual "Request segment count", request.UrlSegments.Count, 1
    AssertEqual "Request formatted resource", request.FormattedResource, "orders/A%20%2B%20B?page=2&active=true"
    AssertEqual "Request content type", request.ContentType, "application/json"
    AssertEqual "Request accept", request.Accept, "application/json"
    AssertEqual "Request header count", request.Headers.Count, 1
    AssertEqual "Request cookie count", request.Cookies.Count, 1
    AssertEqual "Request body JSON", request.Body, "{""message"":""Howdy!"",""count"":3}"

    Dim requestClone As WebRequest
    Set requestClone = request.Clone
    AssertEqual "Request clone method", requestClone.Method, WebMethod.HttpPost
    AssertEqual "Request clone resource", requestClone.Resource, "orders/{id}"
    AssertEqual "Request clone body", requestClone.Body, request.Body

    Dim options As New Dictionary
    Dim segments As New Dictionary
    segments.Add "id", "bob@example.test"
    options.Add "UrlSegments", segments
    Dim requestFromOptions As New WebRequest
    requestFromOptions.CreateFromOptions options
    AssertEqual "CreateFromOptions segment count", requestFromOptions.UrlSegments.Count, 1
    AssertEqual "CreateFromOptions segment value", requestFromOptions.UrlSegments("id"), "bob@example.test"

    ProbeStage = 7
    Dim response As New WebResponse

    Dim updated As New WebResponse
    updated.StatusCode = WebStatusCode.Created
    updated.StatusDescription = "Created"
    updated.Content = "Ok"
    response.Update updated
    AssertEqual "Response update status", response.StatusCode, WebStatusCode.Created
    AssertEqual "Response update content", response.Content, "Ok"

    Dim client As New WebClient
    client.BaseUrl = "https://example.test/api"
    AssertEqual "Client GetFullUrl", client.GetFullUrl(request), "https://example.test/api/orders/A%20%2B%20B?page=2&active=true"
    client.SetProxy "proxy:8080", "user", "pass", "skip"
    AssertEqual "Client proxy server", client.ProxyServer, "proxy:8080"
    AssertEqual "Client proxy bypass", client.ProxyBypassList, "skip"

    ProbeStage = 8
    Dim basic As New HttpBasicAuthenticator
    basic.Setup "user", "pass"
    AssertEqual "Basic username", basic.Username, "user"
    AssertEqual "Basic password", basic.Password, "pass"

    Dim digest As New DigestAuthenticator
    digest.Setup "Mufasa", "Circle Of Life"
    AssertEqual "Digest initially unauthenticated", digest.IsAuthenticated, False
    Exit Sub

ProbeFailed:
    Dim errNumber As Long
    Dim errDescription As String
    errNumber = Err.Number
    errDescription = Err.Description
    On Error GoTo 0
    If errNumber >= 52300 Then Err.Raise errNumber, "VbaWebBroadHarness", errDescription
    Err.Raise 52400 + ProbeStage, "VbaWebBroadHarness", "stage " & CStr(ProbeStage) & ": " & errDescription
End Sub
"########;
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public Sub Main()
    VbaWebCoreProbe.RunBroadProbe
End Sub
"########;
    run_harness_with_core_probe_for_shape(HostShape::HostInjectedProfile, core_probe, harness);
}

#[test]
#[ignore = "external corpus; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_spec_framework_runner_records_returned_suite() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public SuiteSummary As String
Public FailureSummary As String
Public ErrorSummary As String
Public PhaseSummary As String

Private Sub AppendFailure(ByVal SuiteName As String, ByVal Spec As SpecDefinition)
    Dim Expectation As SpecExpectation
    For Each Expectation In Spec.FailedExpectations
        FailureSummary = FailureSummary & SuiteName & "::" & Spec.Description & " => " & Expectation.FailureMessage & vbLf
    Next Expectation
End Sub

Private Sub RecordSuite(ByVal SuiteName As String, ByVal Suite As SpecSuite)
    Dim Spec As SpecDefinition
    PhaseSummary = PhaseSummary & SuiteName & ":counting;"
    SuiteSummary = SuiteSummary & SuiteName & ":" & CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count) & ":" & CStr(Suite.PendingSpecs.Count) & vbLf
    PhaseSummary = PhaseSummary & SuiteName & ":failures;"
    For Each Spec In Suite.FailedSpecs
        AppendFailure SuiteName, Spec
    Next Spec
    PhaseSummary = PhaseSummary & SuiteName & ":done;"
End Sub

Public Sub Main()
    On Error GoTo SpecRunnerFailed
    Dim Reporter As New WorkbookReporter
    PhaseSummary = "start;"
    RecordSuite "Framework", BuildSuite()
    Reporter.Start NumSuites:=1
    Reporter.Output BuildSuite()
    Reporter.Done
    PhaseSummary = PhaseSummary & "reporter-done;"
    PhaseSummary = PhaseSummary & "main-done;"
    Exit Sub

SpecRunnerFailed:
    ErrorSummary = PhaseSummary & CStr(Err.Number) & ":" & Err.Description
End Sub

Private Function BuildSuite() As SpecSuite
    Set BuildSuite = New SpecSuite
    With BuildSuite.It("records a failing expectation")
        .Expect("actual").ToEqual "expected"
    End With
End Function
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        !combined.contains("Object required"),
        "VBA-Web WebRequest spec runner should execute without runtime error: {snapshot:?}"
    );
    assert!(
        combined.contains("Framework:1:0:1:0"),
        "VBA-Web extracted spec runner should record suite counts: {snapshot:?}"
    );
    assert!(
        combined.contains("Framework::records a failing expectation"),
        "VBA-Web extracted spec runner should record expectation failures: {snapshot:?}"
    );
    assert!(
        combined.contains("reporter-done;"),
        "VBA-Web extracted workbook reporter console shim should run: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_webrequest_collection_body_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Request As WebRequest
    Dim Body As Object
    Set Request = New WebRequest
    Mark "request"
    Request.Body = Array("A", "B", "C")
    Mark "array-let"
    ProbeSummary = ProbeSummary & Request.Body & ";"
    Mark "array-body"
    Set Body = New Collection
    Mark "new-collection"
    Body.Add "A"
    Mark "add-a"
    Body.Add "B"
    Mark "add-b"
    Body.Add "C"
    Mark "add-c"
    Set Request.Body = Body
    Mark "set-body"
    ProbeSummary = ProbeSummary & Request.Body & ";"
    Mark "collection-body"
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        !combined.contains("Object required"),
        "VBA-Web extracted WebRequest collection body probe failed: {snapshot:?}"
    );
    assert!(
        combined.contains("[\"A\",\"B\",\"C\"];array-body;"),
        "VBA-Web extracted WebRequest array body probe did not format Array(...) as JSON: {snapshot:?}"
    );
    assert!(
        combined.contains("[\"A\",\"B\",\"C\"];collection-body;"),
        "VBA-Web extracted WebRequest collection body probe did not complete: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_webrequest_expectation_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Suite As New SpecSuite
    Dim Spec As SpecDefinition
    Dim Expectation As SpecExpectation
    Dim Request As WebRequest
    Dim Body As Object
    Set Spec = Suite.It("probe")
    Mark "it"
    Set Request = New WebRequest
    Request.Body = Array("A", "B", "C")
    Mark "array-let"
    ProbeSummary = ProbeSummary & "array=" & Request.Body & ";"
    Spec.Expect(Request.Body).ToEqual "[""A"",""B"",""C""]"
    Mark "array-expect"
    Set Body = New Collection
    Body.Add "A"
    Body.Add "B"
    Body.Add "C"
    Set Request.Body = Body
    Mark "set-body"
    ProbeSummary = ProbeSummary & "collection=" & Request.Body & ";"
    Spec.Expect(Request.Body).ToEqual "[""A"",""B"",""C""]"
    Mark "collection-expect"
    Suite.SpecDone Spec
    Mark "done"
    For Each Expectation In Spec.FailedExpectations
        ProbeSummary = ProbeSummary & "failure=" & Expectation.FailureMessage & ";"
    Next Expectation
    ProbeSummary = ProbeSummary & CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count)
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("done;1:1:0"),
        "VBA-Web extracted WebRequest expectation probe should complete with array and collection body expectations passing: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; runs extracted VBA-Web WebRequest spec suite"]
fn vba_web_extracted_webrequest_spec_suite_records_results() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public SuiteSummary As String
Public FailureSummary As String
Public ErrorSummary As String
Public PhaseSummary As String

Public Sub Main()
    On Error GoTo Failed
    Dim Suite As SpecSuite
    Dim Spec As SpecDefinition
    Dim Expectation As SpecExpectation
    PhaseSummary = "start;"
    Set Suite = Specs_WebRequest.Specs
    PhaseSummary = PhaseSummary & "suite-built;"
    SuiteSummary = CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count) & ":" & CStr(Suite.PendingSpecs.Count)
    For Each Spec In Suite.FailedSpecs
        FailureSummary = FailureSummary & Spec.Description & vbLf
        For Each Expectation In Spec.FailedExpectations
            FailureSummary = FailureSummary & "  " & Expectation.FailureMessage & vbLf
        Next Expectation
    Next Spec
    PhaseSummary = PhaseSummary & "done;"
    Exit Sub

Failed:
    ErrorSummary = PhaseSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("done;"),
        "VBA-Web extracted WebRequest spec suite should finish: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; set VBA_WEB_SPEC_LIMIT to run a prefix of Specs_WebRequest.Specs"]
fn vba_web_extracted_webrequest_spec_suite_prefix_records_results() {
    let limit = std::env::var("VBA_WEB_SPEC_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10);
    let mut overrides = HashMap::new();
    overrides.insert(
        "Specs_WebRequest.bas",
        limited_webrequest_specs_source(limit),
    );
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public SuiteSummary As String
Public FailureSummary As String
Public ErrorSummary As String
Public PhaseSummary As String

Public Sub Main()
    On Error GoTo Failed
    Dim Suite As SpecSuite
    Dim Spec As SpecDefinition
    Dim Expectation As SpecExpectation
    PhaseSummary = "start;"
    Set Suite = Specs_WebRequest.Specs
    PhaseSummary = PhaseSummary & "suite-built;"
    SuiteSummary = CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count) & ":" & CStr(Suite.PendingSpecs.Count)
    For Each Spec In Suite.FailedSpecs
        FailureSummary = FailureSummary & Spec.Description & vbLf
        For Each Expectation In Spec.FailedExpectations
            FailureSummary = FailureSummary & "  " & Expectation.FailureMessage & vbLf
        Next Expectation
    Next Spec
    PhaseSummary = PhaseSummary & "done;"
    Exit Sub

Failed:
    ErrorSummary = PhaseSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness_with_overrides(harness, &overrides);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("done;"),
        "VBA-Web extracted WebRequest spec suite prefix {limit} should finish: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_immediate_reporter_event_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Suite As New SpecSuite
    Dim Reporter As New ImmediateReporter
    Reporter.ListenTo Suite
    Mark "listen"
    With Suite.It("reported")
        .Expect("actual").ToEqual "expected"
    End With
    Mark "with"
    ProbeSummary = ProbeSummary & CStr(Suite.Specs.Count) & ":" & CStr(Suite.FailedSpecs.Count)
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("with;0:0"),
        "VBA-Web extracted ImmediateReporter event probe should complete before returned spec termination: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_many_returned_specs_terminate() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Function BuildSuite() As SpecSuite
    Set BuildSuite = New SpecSuite
    Dim i As Long
    For i = 1 To 60
        With BuildSuite.It("probe " & CStr(i))
            .Expect(i).ToEqual i
        End With
    Next i
End Function

Public Sub Main()
    On Error GoTo Failed
    Dim Suite As SpecSuite
    Set Suite = BuildSuite()
    ProbeSummary = CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count)
    Exit Sub

Failed:
    ErrorSummary = CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("60:60:0"),
        "VBA-Web extracted many returned specs should terminate and register: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_webrequest_add_body_parameter_error_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Request As WebRequest
    Set Request = New WebRequest
    Request.Body = "Howdy"
    Mark "body"
    On Error Resume Next
    Request.AddBodyParameter "Message", "Goodby"
    Mark "after-add"
    ProbeSummary = ProbeSummary & CStr(Err.Number)
    On Error GoTo Failed
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("body;after-add;"),
        "VBA-Web extracted AddBodyParameter error probe should resume after error: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_webrequest_body_parameter_format_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Request As WebRequest
    Set Request = New WebRequest
    Request.AddBodyParameter "A", 123
    Mark "add-a"
    Request.AddBodyParameter "B", "Howdy!"
    Mark "add-b"
    ProbeSummary = ProbeSummary & "json=" & Request.Body & ";"
    Request.Format = WebFormat.Json
    Mark "json-format"
    ProbeSummary = ProbeSummary & "json2=" & Request.Body & ";"
    Request.Format = WebFormat.FormUrlEncoded
    Mark "form-format"
    ProbeSummary = ProbeSummary & "form=" & Request.Body & ";"
    Mark "done"
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("done;"),
        "VBA-Web extracted WebRequest body parameter format probe should complete: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_webrequest_cookie_default_chain_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Request As WebRequest
    Set Request = New WebRequest
    Request.AddCookie "A[1]", "cookie"
    Request.AddCookie "B", "cookie 2"
    Mark "added"
    ProbeSummary = ProbeSummary & "count=" & CStr(Request.Cookies.Count) & ";"
    ProbeSummary = ProbeSummary & "k1=" & CStr(Request.Cookies(1)("Key")) & ";"
    ProbeSummary = ProbeSummary & "v2=" & CStr(Request.Cookies(2)("Value")) & ";"
    Mark "done"
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("k1=A%5B1%5D;v2=cookie%202;done;"),
        "VBA-Web extracted WebRequest cookie default chain probe should complete: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_webrequest_cookie_expectation_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Suite As New SpecSuite
    Dim Spec As SpecDefinition
    Set Spec = Suite.It("cookie")
    Dim Request As WebRequest
    Set Request = New WebRequest
    Request.AddCookie "A[1]", "cookie"
    Request.AddCookie "B", "cookie 2"
    Mark "added"
    Spec.Expect(Request.Cookies.Count).ToEqual 2
    Mark "count"
    Spec.Expect(Request.Cookies(1)("Key")).ToEqual "A%5B1%5D"
    Mark "key"
    Spec.Expect(Request.Cookies(2)("Value")).ToEqual "cookie%202"
    Mark "value"
    Suite.SpecDone Spec
    ProbeSummary = ProbeSummary & CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count)
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("value;1:1:0"),
        "VBA-Web extracted WebRequest cookie expectation probe should complete: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_webrequest_dictionary_body_probe() {
    let step_limit = std::env::var("VBA_WEB_DICT_STEP")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(99);
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String
Public StepLimit As Long

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Request As WebRequest
    Dim Body As Object
    Set Request = New WebRequest
    Mark "request"
    If StepLimit <= 1 Then Exit Sub
    Set Body = New Dictionary
    Mark "new-dict"
    If StepLimit <= 2 Then Exit Sub
    Body.Add "A", 123
    Mark "add-a"
    If StepLimit <= 3 Then Exit Sub
    Body.Add "B", "456"
    Mark "add-b"
    If StepLimit <= 4 Then Exit Sub
    Body.Add "C", 789
    Mark "add-c"
    If StepLimit <= 5 Then Exit Sub
    Set Request.Body = Body
    Mark "set-body"
    If StepLimit <= 6 Then Exit Sub
    ProbeSummary = ProbeSummary & Request.Body & ";"
    Mark "body"
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let harness = harness
        .replace(
            "Public StepLimit As Long",
            &format!(
                "Public StepLimit As Long\nPrivate Const HarnessStepLimit As Long = {step_limit}"
            ),
        )
        .replace(
            "On Error GoTo Failed\n    Dim Request As WebRequest",
            "On Error GoTo Failed\n    StepLimit = HarnessStepLimit\n    Dim Request As WebRequest",
        );
    let snapshot = run_extracted_spec_harness(&harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    if step_limit >= 7 {
        assert!(
            combined.contains("{\"A\":123,\"B\":\"456\",\"C\":789};body;"),
            "VBA-Web extracted WebRequest dictionary body probe should format Dictionary as JSON: {snapshot:?}"
        );
    } else {
        assert!(
            !combined.contains("Error"),
            "VBA-Web extracted WebRequest dictionary body prefix {step_limit} should not fail: {snapshot:?}"
        );
    }
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_dictionary_keys_and_default_member_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Body As Object
    Dim Key As Variant
    Set Body = New Dictionary
    Body.Add "A", 123
    Body.Add "B", "456"
    Body.Add "C", 789
    Mark "added"
    For Each Key In Body.Keys
        ProbeSummary = ProbeSummary & CStr(Key) & "=" & CStr(Body(Key)) & ";"
    Next Key
    Mark "done"
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("A=123;B=456;C=789;done;"),
        "VBA-Web extracted Dictionary keys/default-member probe should enumerate and index values: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_with_temporary_spec_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim Suite As New SpecSuite
    Dim Request As WebRequest
    Dim Body As Object
    With Suite.It("probe")
        Mark "it"
        Set Request = New WebRequest
        Request.Body = Array("A", "B", "C")
        Mark "array-let"
        .Expect(Request.Body).ToEqual "[""A"",""B"",""C""]"
        Mark "array-expect"
        Set Body = New Collection
        Body.Add "A"
        Body.Add "B"
        Body.Add "C"
        Set Request.Body = Body
        Mark "set-body"
        .Expect(Request.Body).ToEqual "[""A"",""B"",""C""]"
        Mark "collection-expect"
    End With
    Mark "after-with"
    ProbeSummary = ProbeSummary & CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count)
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("after-with;0:0:0"),
        "VBA-Web extracted With temporary spec probe did not reach End With cleanly: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus investigation; requires extracted VBA-Web spec workbook modules and normal Scripting.Dictionary COM registration"]
fn vba_web_extracted_returned_suite_termination_probe() {
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ProbeSummary As String
Public ErrorSummary As String

Private Sub Mark(ByVal Name As String)
    ProbeSummary = ProbeSummary & Name & ";"
End Sub

Private Function BuildSuite() As SpecSuite
    Set BuildSuite = New SpecSuite
    With BuildSuite.It("probe")
        Mark "it"
        .Expect("actual").ToEqual "expected"
        Mark "expect"
    End With
    Mark "before-return"
End Function

Public Sub Main()
    On Error GoTo Failed
    Dim Suite As SpecSuite
    Set Suite = BuildSuite()
    Mark "after-call"
    ProbeSummary = ProbeSummary & CStr(Suite.Specs.Count) & ":" & CStr(Suite.PassedSpecs.Count) & ":" & CStr(Suite.FailedSpecs.Count)
    Exit Sub

Failed:
    ErrorSummary = ProbeSummary & CStr(Err.Number) & ":" & Err.Description
End Sub
"########;
    let snapshot = run_extracted_spec_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("after-call;1:0:1"),
        "VBA-Web extracted returned-suite termination probe did not complete cleanly: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus characterization; requires .external/vba-corpus/vba-web checkout and normal Scripting.Dictionary COM registration"]
fn vba_web_residuals_are_isolated_and_characterized() {
    let core_probe = r########"
Attribute VB_Name = "VbaWebResidualProbe"
Option Explicit

Public Function ProbeScalarJson() As String
    ProbeScalarJson = WebHelpers.ConvertToJson("Howdy!") & "|" & WebHelpers.ConvertToJson(True) & "|" & WebHelpers.ConvertToJson(3)
End Function

Public Function ProbeDictionaryJson() As String
    Dim dict As New Dictionary
    dict.Add "message", "Howdy!"
    dict.Add "count", 3
    ProbeDictionaryJson = WebHelpers.ConvertToJson(dict)
End Function

Public Function ProbeDictionaryIntrospection() As String
    Dim dict As New Dictionary
    ProbeDictionaryIntrospection = CStr(VBA.VarType(dict)) & "|" & VBA.TypeName(dict) & "|" & ProbeVariantIntrospection(dict)
End Function

Private Function ProbeVariantIntrospection(ByVal Value As Variant) As String
    ProbeVariantIntrospection = CStr(VBA.VarType(Value)) & "|" & VBA.TypeName(Value)
End Function

Public Function ProbeCollectionJson() As String
    Dim coll As New Collection
    coll.Add "abc"
    coll.Add 123
    ProbeCollectionJson = WebHelpers.ConvertToJson(coll)
End Function

Public Function ProbeNestedJson() As String
    Dim nested As Dictionary
    Set nested = WebHelpers.ParseJson("{""child"":{""name"":""Ada""},""items"":[1,2]}")
    Dim child As Object
    Set child = nested("child")
    Dim items As Object
    Set items = nested("items")
    ProbeNestedJson = child("name") & "|" & CStr(items(2))
End Function

Public Function ProbeDictionaryUrlEncoded() As String
    Dim form As New Dictionary
    form.Add "a", "A + B"
    form.Add "c & d", "Howdy!"
    ProbeDictionaryUrlEncoded = WebHelpers.ConvertToUrlEncoded(form)
End Function

Public Function ProbeCollectionUrlEncoded() As String
    Dim kv As New Collection
    kv.Add WebHelpers.CreateKeyValue("a", "A + B")
    kv.Add WebHelpers.CreateKeyValue("c & d", "Howdy!")
    ProbeCollectionUrlEncoded = WebHelpers.ConvertToUrlEncoded(kv)
End Function

Public Function ProbeHeaders() As String
    Dim response As New WebResponse
    Dim headers As Collection
    Set headers = response.ExtractHeaders("Content-Type: application/json" & vbCrLf & "Set-Cookie: sid=abc%20123; Path=/" & vbCrLf)
    ProbeHeaders = CStr(headers.Count) & "|" & headers(1)("Key") & "|" & headers(1)("Value")
End Function

Public Function ProbeCookies() As String
    Dim response As New WebResponse
    Dim headers As Collection
    Set headers = response.ExtractHeaders("Content-Type: application/json" & vbCrLf & "Set-Cookie: sid=abc%20123; Path=/" & vbCrLf)
    Dim cookies As Collection
    Set cookies = response.ExtractCookies(headers)
    ProbeCookies = CStr(cookies.Count) & "|" & cookies(1)("Key") & "|" & cookies(1)("Value")
End Function
"########;
    let harness = r########"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public ScalarJson As String
Public DictionaryJson As String
Public DictionaryIntrospection As String
Public CollectionJson As String
Public NestedJson As String
Public DictionaryUrlEncoded As String
Public CollectionUrlEncoded As String
Public HeaderSummary As String
Public CookieSummary As String
Public ErrorSummary As String

Public Sub Main()
    On Error Resume Next
    ScalarJson = VbaWebResidualProbe.ProbeScalarJson
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "ScalarJson=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    DictionaryJson = VbaWebResidualProbe.ProbeDictionaryJson
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "DictionaryJson=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    DictionaryIntrospection = VbaWebResidualProbe.ProbeDictionaryIntrospection
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "DictionaryIntrospection=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    CollectionJson = VbaWebResidualProbe.ProbeCollectionJson
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "CollectionJson=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    NestedJson = VbaWebResidualProbe.ProbeNestedJson
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "NestedJson=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    DictionaryUrlEncoded = VbaWebResidualProbe.ProbeDictionaryUrlEncoded
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "DictionaryUrlEncoded=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    CollectionUrlEncoded = VbaWebResidualProbe.ProbeCollectionUrlEncoded
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "CollectionUrlEncoded=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    HeaderSummary = VbaWebResidualProbe.ProbeHeaders
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "HeaderSummary=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    CookieSummary = VbaWebResidualProbe.ProbeCookies
    If Err.Number <> 0 Then ErrorSummary = ErrorSummary & "CookieSummary=" & CStr(Err.Number) & ":" & Err.Description & ";": Err.Clear
    On Error GoTo 0
End Sub
"########;
    let project = write_synthetic_project(
        HostShape::HostInjectedProfile,
        Some(harness),
        Some(core_probe),
    );
    let snapshot = run_project(
        &project,
        HostShape::HostInjectedProfile,
        Arc::new(Mutex::new(Vec::new())),
    );
    let text = |index: usize| {
        snapshot
            .get(index)
            .and_then(|value| value.as_bstr())
            .map(|text| text.as_str().to_string())
            .unwrap_or_default()
    };
    assert_eq!(text(0), "\"Howdy!\"|true|3", "scalar JSON: {snapshot:?}");
    assert_eq!(
        text(1),
        "{\"message\":\"Howdy!\",\"count\":3}",
        "Dictionary JSON now depends on vbObject=9: {snapshot:?}"
    );
    assert_eq!(
        text(2),
        "9|Dictionary|9|Dictionary",
        "VarType/TypeName for Dictionary object and ByVal Variant object: {snapshot:?}"
    );
    assert_eq!(
        text(3),
        "[\"abc\",123]",
        "Collection JSON now depends on vbObject=9: {snapshot:?}"
    );
    assert_eq!(
        text(4),
        "Ada|2",
        "nested JSON should preserve internal runtime objects through native COM containers: {snapshot:?}"
    );
    assert_eq!(
        text(5),
        "a=A+%2B+B&c+%26+d=Howdy%21",
        "omitted enum default should use form URL encoding: {snapshot:?}"
    );
    assert_eq!(
        text(6),
        "a=A+%2B+B&c+%26+d=Howdy%21",
        "ByRef writeback to Variant-held Dictionary default member should no longer fault: {snapshot:?}"
    );
    assert_eq!(
        text(7),
        "2|Content-Type|application/json",
        "header extraction: {snapshot:?}"
    );
    assert_eq!(text(8), "1|sid|abc 123", "cookie extraction: {snapshot:?}");
    let errors = text(9);
    assert!(
        errors.is_empty(),
        "all characterized VBA-Web residual probes should now pass: {snapshot:?}"
    );
    assert!(
        !errors.contains("NestedJson=")
            && !errors.contains("DictionaryJson=")
            && !errors.contains("CollectionJson=")
            && !errors.contains("DictionaryUrlEncoded=")
            && !errors.contains("HeaderSummary=")
            && !errors.contains("CookieSummary="),
        "resolved residuals should stay resolved: {snapshot:?}"
    );
}
