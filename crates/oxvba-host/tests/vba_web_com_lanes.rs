//! VBA-Web-shaped runtime/host/COM progression lanes.
//!
//! These tests are intentionally smaller than the COM matrix. They prove the corpus
//! harness can graduate from pure helpers into policy-owned COM object-model use
//! without requiring live COM in default CI.

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};

fn run_source_with_policy(
    source: &str,
    policy: HostPolicy,
) -> Result<Vec<oxvba_runtime::Variant>, String> {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(policy);
    engine
        .execute_source_with_variant_snapshot_clean(source)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

fn first_i32(values: &[oxvba_runtime::Variant]) -> Option<i32> {
    values.iter().find_map(|value| value.as_i32())
}

#[test]
fn vba_web_dictionary_createobject_is_policy_gated() {
    let mut policy = HostPolicy::deterministic_runtime();
    policy.allow_com_activation = false;
    let err = run_source_with_policy(
        "Sub Main()\n\
         Dim d As Object\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         End Sub\n",
        policy,
    )
    .expect_err("COM activation must be denied by host policy");
    assert!(
        err.contains("PolicyDenied") || err.contains("policy") || err.contains("denied"),
        "unexpected diagnostic: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "live COM; requires Scripting.Dictionary registration"]
fn vba_web_dictionary_late_bound_smoke_executes_when_available() {
    let source = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d As Object\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         d.Add \"status\", 200\n\
         d.Add \"body\", \"ok\"\n\
         verdict = d.Count * 1000\n\
         If d.Exists(\"status\") Then verdict = verdict + d(\"status\")\n\
         If d(\"body\") = \"ok\" Then verdict = verdict + 1\n\
         End Sub\n";
    let values = run_source_with_policy(source, HostPolicy::interactive_dev())
        .expect("live Scripting.Dictionary smoke should run when registered");
    assert_eq!(first_i32(&values), Some(2201), "snapshot: {values:?}");
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "live COM; requires WinHttpRequest registration; no network I/O"]
fn vba_web_winhttp_activation_and_setup_smoke_executes_when_available() {
    let source = "Public verdict As Long\n\
         Sub Main()\n\
         Dim req As Object\n\
         Set req = CreateObject(\"WinHttp.WinHttpRequest.5.1\")\n\
         req.SetTimeouts 1, 1, 1, 1\n\
         req.Open \"GET\", \"https://example.test/\", False\n\
         verdict = 1\n\
         End Sub\n";
    let values = run_source_with_policy(source, HostPolicy::interactive_dev())
        .expect("live WinHttpRequest activation/setup smoke should run when registered");
    assert_eq!(first_i32(&values), Some(1), "snapshot: {values:?}");
}
