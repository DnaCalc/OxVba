#![cfg(target_os = "windows")]

use std::path::PathBuf;

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};

fn unique_temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("oxvba_{tag}_{}", std::process::id()))
}

fn vba_literal(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('"', "\"\"")
}

#[test]
fn shell_returns_task_id_without_waiting_for_process_exit() {
    let dir = unique_temp_dir("shell_async");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let marker = dir.join("marker.txt");
    let script = dir.join("delayed.cmd");
    std::fs::write(
        &script,
        format!(
            "@echo off\r\nping -n 3 127.0.0.1 > NUL\r\necho done > \"{}\"\r\n",
            marker.display()
        ),
    )
    .expect("write delayed cmd");
    let command = script.to_string_lossy().to_string();
    let source = format!(
        "Public probe As String\n\
         Sub Main()\n\
            Dim taskId As Variant\n\
            taskId = Shell(\"{}\", 0)\n\
            probe = CStr(VarType(taskId)) & \"|\" & CStr(CDbl(taskId) > 0)\n\
         End Sub\n",
        vba_literal(std::path::Path::new(&command)),
    );

    let mut engine = Engine::new(HostConfig::vm3());
    engine.set_host_policy(HostPolicy::interactive_dev());
    let started = std::time::Instant::now();
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let elapsed = started.elapsed();

    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(
        snap[0].as_bstr().map(|value| value.as_str()),
        Some("5|True".to_string()),
        "Shell should return a positive Variant/Double task id: {snap:?}"
    );
    assert!(
        elapsed < std::time::Duration::from_millis(1200),
        "Shell should return before the delayed child exits, elapsed={elapsed:?}"
    );
    assert!(
        !marker.exists(),
        "delayed marker already existed when Shell returned; command may have blocked"
    );

    for _ in 0..40 {
        if marker.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let finished = marker.exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(finished, "delayed shell command did not finish");
}
