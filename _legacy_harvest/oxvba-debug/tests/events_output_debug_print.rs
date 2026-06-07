#[path = "support_handle/mod.rs"]
mod support_handle;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_debug::{DebugAttachConfig, DebugEvent, DebugOutputChannel, attach_debug_session};
use oxvba_hal::callbacks::HostCallbacks;
use oxvba_host::{Engine, HostConfig};

#[derive(Default)]
struct RecordingCallbacks {
    debug_prints: Mutex<Vec<String>>,
}

impl HostCallbacks for RecordingCallbacks {
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
        style.max(1)
    }
    fn on_input_box(&self, _prompt: &str, default: &str) -> String {
        default.to_string()
    }
    fn on_status_bar(&self, _text: &str) {}
    fn on_debug_print(&self, text: &str) {
        self.debug_prints.lock().unwrap().push(text.to_string());
    }
}

#[test]
fn debug_print_emits_output_host_without_suppressing_host_callback() {
    let callbacks = Arc::new(RecordingCallbacks::default());
    let engine =
        Arc::new(Engine::new(HostConfig::default()).with_host_callbacks(callbacks.clone()));
    let manifest = ProjectManifest {
        project_name: "DebugPrintEvent".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source(
                "Module1",
                ModuleKind::Procedural,
                "Sub Main()\nDebug.Print \"trace\"\nEnd Sub",
            )
            .expect("module"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };
    let attach =
        attach_debug_session(engine, manifest, DebugAttachConfig::default()).expect("attach");
    let receiver = attach.handle.subscribe();
    let _ = attach.handle.start().expect("start");
    let _entry = receiver.recv().expect("entry stopped");
    let _ = attach.handle.continue_execution().expect("continue");
    let _continued = receiver.recv().expect("continued");
    let output = receiver.recv().expect("output");
    assert!(matches!(
        output,
        DebugEvent::Output { channel: DebugOutputChannel::Host, text, .. } if text == "trace"
    ));
    assert_eq!(callbacks.debug_prints.lock().unwrap().as_slice(), ["trace"]);
    attach.handle.detach().expect("detach");
}
