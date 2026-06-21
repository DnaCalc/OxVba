//! Ignored Riff external-corpus checks.
//!
//! These tests use the local checkout under `.external/vba-corpus/riff` but
//! synthesize their own temporary `.basproj` fixtures. The external source is
//! not committed; only these black-box characterization harnesses are.

use std::path::{Path, PathBuf};

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::Variant;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn riff_module() -> PathBuf {
    workspace_root().join(".external/vba-corpus/riff/upstream/package/Riff.bas")
}

fn require_riff_module() -> PathBuf {
    let path = riff_module();
    assert!(
        path.exists(),
        "missing Riff external corpus module at {}",
        path.display()
    );
    path
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn temp_project(harness_source: &str) -> PathBuf {
    let temp = std::env::temp_dir().join(format!(
        "oxvba-riff-corpus-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("create temp Riff project");
    std::fs::write(temp.join("HarnessMain.bas"), harness_source).expect("write Riff harness");
    let riff = require_riff_module();
    let riff_include = xml_escape(&riff.display().to_string());
    std::fs::write(
        temp.join("RiffHarness.basproj"),
        format!(
            r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>RiffHarness</ProjectName>
    <EntryPoint>HarnessMain.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="{}" />
    <Module Include="HarnessMain.bas" />
  </ItemGroup>
</Project>
"#,
            riff_include
        ),
    )
    .expect("write Riff project");
    temp.join("RiffHarness.basproj")
}

fn run_harness(harness_source: &str) -> Vec<Variant> {
    let project = temp_project(harness_source);
    let closure = oxvba_project::load_project_closure_with_entry(&project, None)
        .expect("load Riff project closure");
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::deterministic_runtime());
    engine
        .execute_project_closure_with_variant_snapshot(&closure)
        .expect("execute Riff harness")
}

#[test]
#[ignore = "external corpus; requires .external/vba-corpus/riff checkout"]
fn riff_closed_state_property_harness_executes() {
    let harness = r#"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public Result As String
Private Phase As String

Private Sub AssertSingle(ByVal actual As Single, ByVal expected As Single, ByVal message As String)
    If Abs(actual - expected) > 0.0001! Then
        Err.Raise 514, "RiffHarness", message & ": " & CStr(actual)
    End If
End Sub

Public Sub Main()
    On Error GoTo Failed
    Phase = "initial"
    If RiffIsInitialized Then Err.Raise 513, "RiffHarness", "Riff should start closed"
    Phase = "master"
    RiffMasterVolume = 0.25!
    Result = Result & "master=" & CStr(RiffMasterVolume) & ";"
    AssertSingle RiffMasterVolume, 0!, "closed master volume remains default"
    Phase = "bus"
    RiffBusVolume(0) = 0.5!
    Result = Result & "bus0=" & CStr(RiffBusVolume(0)) & ";"
    AssertSingle RiffBusVolume(0), 0!, "closed bus volume remains default"
    Phase = "invalid voice"
    RiffVoiceVolume(-1) = 0.25!
    Result = Result & "invalidVoice=" & CStr(RiffVoiceVolume(-1)) & ";"
    AssertSingle RiffVoiceVolume(-1), 0!, "invalid voice volume read"
    Result = Result & "ok"
    Exit Sub

Failed:
    Result = "failed:" & Phase & ":" & CStr(Err.Number) & ":" & Err.Description
End Sub
"#;
    let snapshot = run_harness(harness);
    let combined = snapshot
        .iter()
        .filter_map(|value| value.as_bstr())
        .map(|text| text.as_str().to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        combined.contains("master=;bus0=;invalidVoice=;ok"),
        "Riff closed-state harness should complete without opening WASAPI: {snapshot:?}"
    );
}
