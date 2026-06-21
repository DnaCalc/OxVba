//! Ignored VBA-Web external-corpus checks.
//!
//! These tests use the local checkout under `.external/vba-corpus/vba-web` and
//! synthesize temporary `.basproj` files. They deliberately do not include the
//! local `ExcelApplicationShim.bas` fixture, so failures here catch regressions
//! in host-injected `Application` metadata and project-closure execution.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use oxvba_com::{
    OptionalParamDefault, PortableComProjection, SourceTypeKind, TypeLibMemberInvokeKind,
    TypeLibMetadataBlob, TypeLibParamType, TypeLibResolveRequest, TypeLibResolvedIdentity,
    TypeLibWireType,
    platform::portable::{PortableDispatch, PortableObjectFactory},
};
use oxvba_host::{Engine, HostConfig, HostProfileProvider};
use oxvba_project::load_project_closure;
use oxvba_runtime::Variant;
use oxvba_symbol::{CatalogTypeLibResolver, TypeLibResolver};

const VBA_WEB_ROOT: &str = ".external/vba-corpus/vba-web";

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

fn require_vba_web_root() -> PathBuf {
    let root = vba_web_root();
    assert!(
        root.join("src/WebHelpers.bas").is_file(),
        "VBA-Web external corpus checkout is required at {}",
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

fn core_basproj(root: &Path, shape: HostShape) -> String {
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
    format!(
        r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <ProjectName>{}</ProjectName>
  </PropertyGroup>
  <ItemGroup>
{}    <Module Include="HostProbe.bas" />
    <COMReference Include="Scripting">
      <Guid>{{420B2830-E718-11CF-893D-00A0C9054228}}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
      <ImportLib>scrrun.dll</ImportLib>
    </COMReference>
    <Module Include="{}" />
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

fn write_synthetic_project(shape: HostShape, harness_source: Option<&str>) -> PathBuf {
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
    let core_project = format!("VbaWebCore{}.basproj", shape.suffix());
    std::fs::write(temp.join(&core_project), core_basproj(&root, shape))
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
    <Module Include="HarnessMain.bas" />
  </ItemGroup>
</Project>
"#,
                shape.suffix(),
                core_project
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
        return_wire_type: return_type.clone().map(TypeLibWireType::Automation),
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
        .with_portable_com_projection(projection);
    Engine::new(HostConfig { enable_jit: false }).with_host_profile_provider(profile)
}

fn run_project(path: &Path, shape: HostShape, calls: Arc<Mutex<Vec<String>>>) -> Vec<Variant> {
    let closure = load_project_closure(path).expect("load project closure");
    engine(shape, calls)
        .execute_project_closure_with_variant_snapshot(&closure)
        .expect("execute project closure")
}

fn run_harness_for_shape(shape: HostShape, harness: &str) -> Arc<Mutex<Vec<String>>> {
    let project = write_synthetic_project(shape, Some(harness));
    let calls = Arc::new(Mutex::new(Vec::new()));
    run_project(&project, shape, calls.clone());
    calls
}

#[test]
#[ignore = "external corpus; requires .external/vba-corpus/vba-web checkout"]
fn vba_web_raw_upstream_sources_build_without_application_shim() {
    let project = write_synthetic_project(HostShape::HostInjectedProfile, None);
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
