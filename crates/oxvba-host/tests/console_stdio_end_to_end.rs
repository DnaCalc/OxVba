use std::sync::{Arc, Mutex};

use oxvba_hal::callbacks::HostCallbacks;
use oxvba_host::{Engine, HostConfig, RuntimeProfileId};
use oxvba_runtime::{Variant, bstr::BStr};

#[derive(Default)]
struct ConsoleCallbacks {
    console_lines: Mutex<Vec<String>>,
    debug_lines: Mutex<Vec<String>>,
    inputs: Mutex<Vec<String>>,
}

impl ConsoleCallbacks {
    fn with_inputs(inputs: &[&str]) -> Self {
        Self {
            inputs: Mutex::new(inputs.iter().rev().map(|s| s.to_string()).collect()),
            ..Self::default()
        }
    }

    fn console_output(&self) -> Vec<String> {
        self.console_lines
            .lock()
            .expect("console output lock")
            .clone()
    }

    fn debug_output(&self) -> Vec<String> {
        self.debug_lines.lock().expect("debug output lock").clone()
    }
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
        self.console_lines
            .lock()
            .expect("console output lock")
            .push(text.to_string());
        true
    }

    fn on_console_input_line(&self) -> Option<String> {
        self.inputs.lock().expect("inputs lock").pop()
    }

    fn on_debug_print(&self, text: &str) {
        self.debug_lines
            .lock()
            .expect("debug output lock")
            .push(text.to_string());
    }
}

fn engine_with_profile(
    runtime_profile: RuntimeProfileId,
    enable_jit: bool,
    callbacks: Arc<dyn HostCallbacks>,
) -> Engine {
    let mut engine = Engine::new(HostConfig { enable_jit });
    engine.set_runtime_profile(runtime_profile);
    engine.set_host_callbacks(Some(callbacks));
    engine
}

#[test]
fn console_print_and_input_execute_on_windows_stdio_profile() {
    let callbacks = Arc::new(ConsoleCallbacks::with_inputs(&[
        "42,hello there",
        "rest of line",
    ]));
    let source = "Sub Main()\n\
            Dim a\n\
            Dim b\n\
            Dim c\n\
            Print \"hello\"\n\
            Input a, b\n\
            Line Input c\n\
            Debug.Print \"trace\"\n\
            End Sub";
    let values = engine_with_profile(RuntimeProfileId::WindowsStdio, false, callbacks.clone())
        .execute_source_with_variant_snapshot(source)
        .expect("console stdio execution should succeed");
    assert_eq!(callbacks.console_output(), vec!["hello".to_string()]);
    assert_eq!(callbacks.debug_output(), vec!["trace".to_string()]);
    assert_eq!(
        values,
        vec![
            Variant::from_i32(42),
            Variant::from_string(BStr::from("hello there")),
            Variant::from_string(BStr::from("rest of line")),
        ],
        "windows stdio host should preserve console/debug behavior on the VM backend"
    );
}

#[test]
fn debug_print_executes_on_windows_stdio_profile_for_vm() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    let source = "Sub Main()\nDebug.Print \"trace\"\nEnd Sub";
    let values = engine_with_profile(RuntimeProfileId::WindowsStdio, false, callbacks.clone())
        .execute_source_with_variant_snapshot(source)
        .expect("debug-print execution should succeed");
    assert_eq!(values, Vec::<Variant>::new());
    assert_eq!(callbacks.debug_output(), vec!["trace".to_string()]);
    assert_eq!(callbacks.console_output(), Vec::<String>::new());
}

#[test]
fn debug_print_multiple_exprs_executes_on_windows_stdio_profile_for_vm() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    let source = "Sub Main()\nOn Error Resume Next\nError 9\nDebug.Print \"trace\", Err.LastDllError\nEnd Sub";
    engine_with_profile(RuntimeProfileId::WindowsStdio, false, callbacks.clone())
        .execute_source(source)
        .expect("multi-expr debug-print execution should succeed");
    assert_eq!(callbacks.debug_output(), vec!["trace\t0".to_string()]);
    assert_eq!(callbacks.console_output(), Vec::<String>::new());
}

#[test]
fn windows_stdio_profile_reports_jit_unavailable_without_vm_fallback() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    let source = "Sub Main()\nDebug.Print \"trace\"\nEnd Sub";
    let err = engine_with_profile(RuntimeProfileId::WindowsStdio, true, callbacks.clone())
        .execute_source_with_variant_snapshot(source)
        .expect_err("JIT request should not silently fall back to VM execution");
    assert!(
        err.contains("JIT execution"),
        "unexpected JIT unavailable diagnostic: {err}"
    );
    assert_eq!(callbacks.debug_output(), Vec::<String>::new());
    assert_eq!(callbacks.console_output(), Vec::<String>::new());
}

#[test]
fn console_print_and_input_execute_on_linux_stdio_profile() {
    let callbacks = Arc::new(ConsoleCallbacks::with_inputs(&["7,alpha", "omega"]));
    let source = "Sub Main()\n\
        Dim a\n\
        Dim b\n\
        Dim c\n\
        Print \"hello-linux\"\n\
        Input a, b\n\
        Line Input c\n\
        End Sub";
    let values = engine_with_profile(RuntimeProfileId::LinuxStdio, false, callbacks.clone())
        .execute_source_with_variant_snapshot(source)
        .expect("console stdio execution should succeed");
    assert_eq!(callbacks.console_output(), vec!["hello-linux".to_string()]);
    assert_eq!(callbacks.debug_output(), Vec::<String>::new());
    assert_eq!(
        values,
        vec![
            Variant::from_i32(7),
            Variant::from_string(BStr::from("alpha")),
            Variant::from_string(BStr::from("omega")),
        ]
    );
}
