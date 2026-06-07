use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_debug::{DebugAttachConfig, DebugEvent, DebugOutputChannel, attach_debug_session};
use oxvba_hal::callbacks::HostCallbacks;
use oxvba_host::{Engine, HostConfig, RuntimeProfileId};

#[derive(Default)]
struct ConsoleCallbacks {
    console_lines: Mutex<Vec<String>>,
}

impl HostCallbacks for ConsoleCallbacks {
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
        style.max(1)
    }
    fn on_input_box(&self, _prompt: &str, default: &str) -> String {
        default.to_string()
    }
    fn on_status_bar(&self, _text: &str) {}
    fn on_console_print(&self, text: &str) -> bool {
        self.console_lines.lock().unwrap().push(text.to_string());
        true
    }
    fn on_debug_print(&self, _text: &str) {}
}

#[test]
fn stdio_output_emits_typed_output_channels() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    let mut engine = Engine::new(HostConfig::default());
    engine.set_runtime_profile(RuntimeProfileId::WindowsStdio);
    engine.set_host_callbacks(Some(callbacks.clone()));
    let manifest = ProjectManifest {
        project_name: "StdioOutputEvent".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source(
                "Module1",
                ModuleKind::Procedural,
                "Sub Main()\nPrint \"hello\"\nEnd Sub",
            )
            .expect("module"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };
    let attach = attach_debug_session(Arc::new(engine), manifest, DebugAttachConfig::default())
        .expect("attach");
    let receiver = attach.handle.subscribe();
    let _ = attach.handle.start().expect("start");
    let _entry = receiver.recv().expect("entry stopped");
    let _ = attach.handle.continue_execution().expect("continue");
    let _continued = receiver.recv().expect("continued");
    let output = receiver.recv().expect("output");
    assert!(matches!(
        output,
        DebugEvent::Output { channel: DebugOutputChannel::Stdout, text, .. } if text == "hello"
    ));
    assert_eq!(
        callbacks.console_lines.lock().unwrap().as_slice(),
        ["hello"]
    );
    attach.handle.detach().expect("detach");
}
