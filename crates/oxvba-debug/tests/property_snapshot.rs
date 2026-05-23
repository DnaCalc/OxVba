#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugEvent, DebugEventReceiver, DebugRunResultView};
use serde::Serialize;

const BASELINE: &str = include_str!("snapshots/canonical_event_log.json");

#[derive(Debug, Serialize)]
struct SnapshotEntry {
    seq: u64,
    kind: &'static str,
    detail: String,
}

#[test]
fn canonical_sequence_event_and_view_log_matches_snapshot() {
    let attach = support_handle::attach(support_handle::multi_module_manifest());
    let receiver = attach.events;
    let handle = attach.handle;

    let mut entries = drain_events(&receiver);
    let start = handle.start().expect("start");
    entries.push(result_entry("start", &start));
    entries.extend(drain_events(&receiver));

    let breakpoint = handle
        .set_source_breakpoint("Module1", 2, true)
        .expect("breakpoint");
    entries.push(SnapshotEntry {
        seq: 0,
        kind: "result.breakpoint",
        detail: format!(
            "{}:{}:{}",
            breakpoint.module, breakpoint.file_line, breakpoint.enabled
        ),
    });
    entries.extend(drain_events(&receiver));

    let continued = handle.continue_execution().expect("continue");
    entries.push(result_entry("continue", &continued));
    entries.extend(drain_events(&receiver));
    handle.detach().expect("detach");

    let actual = serde_json::to_string_pretty(&entries).expect("snapshot json") + "\n";
    assert_eq!(
        actual, BASELINE,
        "canonical debug event snapshot changed; inspect this diff as a regression sentinel and update tests/snapshots/canonical_event_log.json only for intentional protocol changes"
    );
}

fn drain_events(receiver: &DebugEventReceiver) -> Vec<SnapshotEntry> {
    let mut entries = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        entries.push(event_entry(&event));
    }
    entries
}

fn event_entry(event: &DebugEvent) -> SnapshotEntry {
    match event {
        DebugEvent::ModuleLoaded { seq, module, .. } => SnapshotEntry {
            seq: *seq,
            kind: "event.moduleLoaded",
            detail: module.name.clone(),
        },
        DebugEvent::ThreadStarted { seq, thread_id, .. } => SnapshotEntry {
            seq: *seq,
            kind: "event.threadStarted",
            detail: thread_id.to_string(),
        },
        DebugEvent::Stopped {
            seq,
            reason,
            location,
            ..
        } => SnapshotEntry {
            seq: *seq,
            kind: "event.stopped",
            detail: location
                .as_ref()
                .map(|location| format!("{:?}@{}:{}", reason, location.module, location.file_line))
                .unwrap_or_else(|| format!("{:?}@<none>", reason)),
        },
        DebugEvent::Continued { seq, .. } => SnapshotEntry {
            seq: *seq,
            kind: "event.continued",
            detail: "all".to_string(),
        },
        DebugEvent::Exited { seq, exit_code, .. } => SnapshotEntry {
            seq: *seq,
            kind: "event.exited",
            detail: format!("{exit_code:?}"),
        },
        DebugEvent::BreakpointChanged {
            seq,
            change,
            breakpoint,
            ..
        } => SnapshotEntry {
            seq: *seq,
            kind: "event.breakpointChanged",
            detail: format!(
                "{:?}:{}:{}",
                change, breakpoint.module, breakpoint.file_line
            ),
        },
        DebugEvent::Output {
            seq, channel, text, ..
        } => SnapshotEntry {
            seq: *seq,
            kind: "event.output",
            detail: format!("{:?}:{text}", channel),
        },
    }
}

fn result_entry(command: &'static str, result: &DebugRunResultView) -> SnapshotEntry {
    match result {
        DebugRunResultView::Paused(pause) => SnapshotEntry {
            seq: 0,
            kind: "result.paused",
            detail: pause
                .current_location
                .as_ref()
                .map(|location| {
                    format!(
                        "{command}:{:?}@{}:{}",
                        pause.reason, location.module, location.file_line
                    )
                })
                .unwrap_or_else(|| format!("{command}:{:?}@<none>", pause.reason)),
        },
        DebugRunResultView::Exited(exit) => SnapshotEntry {
            seq: 0,
            kind: "result.exited",
            detail: format!("{command}:{:?}", exit.exit_code),
        },
    }
}
