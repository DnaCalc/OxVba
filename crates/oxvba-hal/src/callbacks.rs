//! Host callback trait for interactive operations.
//!
//! Enables external code to supply responses to MsgBox, InputBox, and other
//! interactive operations without native UI. This is a host-side mechanism —
//! the HAL trait surface does not change.

/// Trait implemented by host embeddings to handle interactive operations.
pub trait HostCallbacks: Send + Sync {
    /// Called when VBA executes `MsgBox`. Returns the button code.
    fn on_msg_box(&self, prompt: &str, style: i32) -> i32;
    /// Called when VBA executes `InputBox`. Returns the user input string.
    fn on_input_box(&self, prompt: &str, default: &str) -> String;
    /// Called when VBA sets `Application.StatusBar`.
    fn on_status_bar(&self, text: &str);
    /// Called when VBA executes `Debug.Print`.
    fn on_debug_print(&self, text: &str);
}

/// Default callbacks: returns `style.max(1)` for MsgBox, `default` for InputBox.
pub struct DefaultHostCallbacks;

impl HostCallbacks for DefaultHostCallbacks {
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
        style.max(1)
    }

    fn on_input_box(&self, _prompt: &str, default: &str) -> String {
        default.to_string()
    }

    fn on_status_bar(&self, _text: &str) {}

    fn on_debug_print(&self, text: &str) {
        eprintln!("[oxvba-hal] debug.print: {text}");
    }
}
