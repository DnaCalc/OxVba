//! `Debug.Print` and bare `Print` route through the HAL host callbacks on the
//! clean stack (binder → vm2 → oxvba-lib → HAL diagnostics/console callback).
//! Multiple `Print`/`Debug.Print` arguments are tab-joined (the comma print-zone
//! rendering; `;`/`Tab()`/`Spc()` fidelity is deferred — see POST_CLEANUP.md).

use std::sync::{Arc, Mutex};

use oxvba_hal::callbacks::HostCallbacks;
use oxvba_host::{Engine, HostConfig, RuntimeProfileId};

#[derive(Default)]
struct ConsoleCallbacks {
    console_lines: Mutex<Vec<String>>,
    debug_lines: Mutex<Vec<String>>,
}

impl ConsoleCallbacks {
    fn console_output(&self) -> Vec<String> {
        self.console_lines.lock().expect("console lock").clone()
    }
    fn debug_output(&self) -> Vec<String> {
        self.debug_lines.lock().expect("debug lock").clone()
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
            .expect("console lock")
            .push(text.to_string());
        true
    }
    fn on_debug_print(&self, text: &str) {
        self.debug_lines
            .lock()
            .expect("debug lock")
            .push(text.to_string());
    }
}

fn run(source: &str, jit_requested: bool, callbacks: Arc<dyn HostCallbacks>) -> Result<(), String> {
    let mut engine = Engine::new(if jit_requested {
        HostConfig::jit()
    } else {
        HostConfig::vm3()
    });
    engine.set_runtime_profile(RuntimeProfileId::WindowsStdio);
    engine.set_host_callbacks(Some(callbacks));
    engine
        .execute_source_with_variant_snapshot_clean(source)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn debug_print_routes_to_host_debug_callback() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    run(
        "Sub Main()\nDebug.Print \"trace\"\nEnd Sub",
        false,
        callbacks.clone(),
    )
    .expect("Debug.Print should execute");
    assert_eq!(callbacks.debug_output(), vec!["trace".to_string()]);
    assert!(callbacks.console_output().is_empty());
}

#[test]
fn debug_print_joins_multiple_args_with_tab() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    run(
        "Sub Main()\nDebug.Print \"a\", 1, True\nEnd Sub",
        false,
        callbacks.clone(),
    )
    .expect("multi-arg Debug.Print should execute");
    assert_eq!(callbacks.debug_output(), vec!["a\t1\tTrue".to_string()]);
}

#[test]
fn bare_print_routes_to_host_console_callback() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    run(
        "Sub Main()\nPrint \"hello\"\nEnd Sub",
        false,
        callbacks.clone(),
    )
    .expect("bare Print should execute");
    assert_eq!(callbacks.console_output(), vec!["hello".to_string()]);
    assert!(callbacks.debug_output().is_empty());
}

#[test]
fn debug_assert_evaluates_without_breaking_in_headless_runtime() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    // A failing assertion does not break (no debugger) and prints nothing.
    run(
        "Sub Main()\nDebug.Assert 1 = 2\nEnd Sub",
        false,
        callbacks.clone(),
    )
    .expect("Debug.Assert should be a no-op break in a headless run");
    assert!(callbacks.debug_output().is_empty());
    assert!(callbacks.console_output().is_empty());
}

#[test]
fn debug_assert_condition_is_evaluated_in_headless_runtime() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    let mut engine = Engine::new(HostConfig::vm3());
    engine.set_runtime_profile(RuntimeProfileId::WindowsStdio);
    engine.set_host_callbacks(Some(callbacks.clone()));
    let snap = engine
        .execute_source_with_variant_snapshot_clean(
            "Public touched As Long\n\
             Sub Main()\n\
                Debug.Assert MarkTouched()\n\
             End Sub\n\
             Function MarkTouched() As Boolean\n\
                touched = 7\n\
                MarkTouched = False\n\
             End Function\n",
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        snap[0].as_i32(),
        Some(7),
        "Debug.Assert side effect: {snap:?}"
    );
    assert!(callbacks.debug_output().is_empty());
    assert!(callbacks.console_output().is_empty());
}

#[test]
fn jit_request_is_rejected_rather_than_falling_back() {
    let callbacks = Arc::new(ConsoleCallbacks::default());
    let err = run("Sub Main()\nDebug.Print \"x\"\nEnd Sub", true, callbacks)
        .expect_err("JIT execution is not implemented; it must not silently fall back");
    assert!(
        err.contains("CallNative") || err.contains("JIT"),
        "unexpected diagnostic: {err}"
    );
}
