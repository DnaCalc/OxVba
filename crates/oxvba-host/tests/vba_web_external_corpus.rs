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
use oxvba_hal::model::HostPolicy;
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
    ProbeNestedJson = nested("child")("name") & "|" & CStr(nested("items")(2))
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
        text(5),
        "a=A%20%2B%20B&c%20%26%20d=Howdy%21",
        "omitted enum default still encodes spaces strictly: {snapshot:?}"
    );
    assert_eq!(
        text(6),
        "a=A%20%2B%20B&c%20%26%20d=Howdy%21",
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
        errors.contains("NestedJson=5:"),
        "nested JSON object/array storage remains the only expected residual error: {snapshot:?}"
    );
    assert!(
        !errors.contains("DictionaryJson=")
            && !errors.contains("CollectionJson=")
            && !errors.contains("DictionaryUrlEncoded=")
            && !errors.contains("HeaderSummary=")
            && !errors.contains("CookieSummary="),
        "resolved residuals should stay resolved: {snapshot:?}"
    );
}
