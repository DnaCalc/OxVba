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

#[test]
#[ignore = "external corpus; requires .external/vba-corpus/riff checkout"]
fn riff_closed_state_public_api_surface_executes() {
    let harness = r#"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public Result As String
Private Phase As String

Private Sub AssertLong(ByVal actual As Long, ByVal expected As Long, ByVal message As String)
    If actual <> expected Then
        Err.Raise 520, "RiffHarness", message & ": " & CStr(actual)
    End If
End Sub

Private Sub AssertSingle(ByVal actual As Single, ByVal expected As Single, ByVal message As String)
    If Abs(actual - expected) > 0.0001! Then
        Err.Raise 521, "RiffHarness", message & ": " & CStr(actual)
    End If
End Sub

Private Sub AssertBool(ByVal actual As Boolean, ByVal expected As Boolean, ByVal message As String)
    If actual <> expected Then
        Err.Raise 522, "RiffHarness", message & ": " & CStr(actual)
    End If
End Sub

Public Sub Main()
    On Error GoTo Failed
    Dim pL As Single
    Dim pR As Single

    Phase = "initial"
    AssertBool RiffIsInitialized, False, "Riff should start closed"
    RiffClose
    AssertBool RiffIsInitialized, False, "RiffClose should be idempotent while closed"

    Phase = "buffer operations"
    AssertLong RiffLoad("missing.wav"), -1, "closed RiffLoad"
    AssertLong RiffLoadFromMemory(Array(1, 2, 3)), -1, "closed RiffLoadFromMemory"
    RiffUnload -1
    AssertSingle RiffBufferDurationSec(-1), 0!, "invalid buffer duration"
    AssertBool RiffExportBufferWav(-1, "closed.wav"), False, "closed export"
    AssertBool RiffRenderOscillatorWav(0, 440!, 0.01!, "closed-osc.wav"), False, "closed oscillator export"
    Result = Result & "buffers;"

    Phase = "voice operations"
    AssertLong RiffPlay(-1), -1, "closed play"
    AssertLong RiffPlayOscillator(1, 440!), -1, "closed play oscillator"
    RiffPause -1
    RiffResume -1
    RiffStop -1
    RiffStopAll
    RiffFadeIn -1, 0.2!
    RiffFadeOut -1, 0.2!
    RiffSetLoopRegionSec -1, 0!, 1!
    RiffVoiceGetPeak -1, pL, pR
    AssertSingle pL, 0!, "closed voice peak left"
    AssertSingle pR, 0!, "closed voice peak right"
    Result = Result & "voice;"

    Phase = "voice properties"
    AssertBool RiffVoiceIsPlaying(-1), False, "invalid voice playing"
    AssertBool RiffVoiceIsPaused(-1), False, "invalid voice paused"
    AssertLong RiffVoiceBus(-1), 0, "invalid voice bus"
    RiffVoiceBus(-1) = 3
    AssertBool RiffVoiceLoop(-1), False, "invalid voice loop"
    RiffVoiceLoop(-1) = True
    AssertSingle RiffVoicePositionSec(-1), 0!, "invalid voice position"
    RiffVoicePositionSec(-1) = 1!
    AssertSingle RiffVoiceVolume(-1), 0!, "invalid voice volume"
    RiffVoiceVolume(-1) = 0.5!
    AssertSingle RiffVoicePitch(-1), 0!, "invalid voice pitch"
    RiffVoicePitch(-1) = 2!
    AssertSingle RiffVoicePan(-1), 0!, "invalid voice pan"
    RiffVoicePan(-1) = -0.5!
    AssertSingle RiffVoiceBitDepth(-1), 0!, "invalid voice bit depth"
    RiffVoiceBitDepth(-1) = 8!
    Result = Result & "props;ok"
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
        combined.contains("buffers;voice;props;ok"),
        "Riff closed-state public API harness should execute broad surface: {snapshot:?}"
    );
}

#[test]
#[ignore = "external corpus; requires .external/vba-corpus/riff checkout"]
fn riff_open_is_blocked_by_deterministic_native_policy() {
    let harness = r#"
Attribute VB_Name = "HarnessMain"
Option Explicit

Public Result As String

Public Sub Main()
    If RiffOpen() Then
        Result = "opened"
        RiffClose
    Else
        Result = "returned-false"
    End If
End Sub
"#;
    let project = temp_project(harness);
    let closure = oxvba_project::load_project_closure_with_entry(&project, None)
        .expect("load Riff project closure");
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::deterministic_runtime());
    let err = engine
        .execute_project_closure_with_variant_snapshot(&closure)
        .expect_err("deterministic policy should block RiffOpen native Declare path");
    let message = format!("{err:?}");
    assert!(
        message.contains("Declare")
            || message.contains("native")
            || message.contains("policy")
            || message.contains("filesystem")
            || message.contains("mutation"),
        "RiffOpen should fail at an explicit deterministic native-policy boundary, got {message}"
    );
}
