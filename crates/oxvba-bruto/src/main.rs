use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use oxvba_compiler::OxBundle;
use oxvba_hal::{HostPolicy, callbacks::HostCallbacks};
use oxvba_host::{Engine, HostConfig};

fn main() -> turbo_vision::core::error::Result<()> {
    match startup_mode(std::env::args().skip(1)) {
        StartupMode::Run => {}
        StartupMode::ShowHelp => {
            print_help();
            return Ok(());
        }
        StartupMode::InvalidArgument(arg) => {
            eprintln!("oxvba-bruto: unrecognized argument: {arg}");
            eprintln!("Run `oxvba-bruto --help` for usage.");
            std::process::exit(2);
        }
    }

    if let Some((bundle_path, capture_path)) = bundle_mode_paths() {
        run_bundle_mode(&bundle_path, &capture_path);
        return Ok(());
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("oxvba-bruto requires an interactive terminal.");
        eprintln!("It starts a full-screen TUI rather than a line-oriented CLI.");
        eprintln!("Run `oxvba-bruto --help` for startup and key hints.");
        std::process::exit(2);
    }

    bruto_ide::ide::run(Box::new(oxvba_bruto_lang::OxvbaBrutoLanguage))
}

enum StartupMode {
    Run,
    ShowHelp,
    InvalidArgument(String),
}

fn startup_mode(args: impl Iterator<Item = String>) -> StartupMode {
    let mut saw_help = false;
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => saw_help = true,
            "--version" | "-V" => {
                println!("oxvba-bruto {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => return StartupMode::InvalidArgument(arg),
        }
    }

    if saw_help {
        StartupMode::ShowHelp
    } else {
        StartupMode::Run
    }
}

fn print_help() {
    println!("oxvba-bruto {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Interactive Bruto-IDE host for OxVba.");
    println!();
    println!("This is a full-screen TUI application, not a line-oriented CLI.");
    println!("Start it from an interactive terminal and use Bruto's keyboard shortcuts.");
    println!();
    println!("Common keys:");
    println!("  F9       Build");
    println!("  Ctrl-F9  Run");
    println!("  F5       Debug / continue");
    println!("  Alt-X    Exit");
}

fn bundle_mode_paths() -> Option<(PathBuf, PathBuf)> {
    let exe_path = std::env::current_exe().ok()?;
    let file_name = exe_path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if file_name == "oxvba-bruto.exe" || file_name == "oxvba-bruto" {
        return None;
    }

    let root = exe_path.parent()?;
    let bundle_path = root.join("Program.oxb");
    if !bundle_path.exists() {
        return None;
    }

    Some((bundle_path, root.join("console.txt")))
}

fn run_bundle_mode(bundle_path: &Path, capture_path: &Path) {
    std::fs::write(capture_path, "").expect("failed to reset Bruto console capture");
    let bundle_bytes = std::fs::read(bundle_path).expect("failed to read Bruto bundle");
    let bundle = OxBundle::deserialize_from_bytes(&bundle_bytes)
        .expect("failed to deserialize Bruto bundle");
    let callbacks = Arc::new(CaptureCallbacks {
        capture_path: capture_path.to_path_buf(),
        write_lock: Mutex::new(()),
    });
    let mut engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: Some("Application".to_string()),
    })
    .with_host_callbacks(callbacks);
    engine.set_host_policy(HostPolicy::interactive_dev());

    if let Err(err) = engine.execute_bundle_with_snapshot(&bundle) {
        eprintln!("OxVba Bruto: execution failed: {err}");
        std::process::exit(1);
    }
}

struct CaptureCallbacks {
    capture_path: PathBuf,
    write_lock: Mutex<()>,
}

impl CaptureCallbacks {
    fn append_line(&self, text: &str) {
        let _guard = self.write_lock.lock().expect("capture callback lock poisoned");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.capture_path)
            .expect("failed to open Bruto console capture");
        use std::io::Write;
        writeln!(file, "{text}").expect("failed to append Bruto console capture");
    }
}

impl HostCallbacks for CaptureCallbacks {
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
        style.max(1)
    }

    fn on_input_box(&self, _prompt: &str, default: &str) -> String {
        default.to_string()
    }

    fn on_status_bar(&self, _text: &str) {}

    fn on_console_print(&self, text: &str) -> bool {
        self.append_line(text);
        true
    }

    fn on_debug_print(&self, _text: &str) {}
}

#[cfg(test)]
mod tests {
    use super::{StartupMode, startup_mode};

    #[test]
    fn help_flag_is_recognized() {
        let mode = startup_mode(vec!["--help".to_string()].into_iter());
        assert!(matches!(mode, StartupMode::ShowHelp));
    }

    #[test]
    fn invalid_flag_is_rejected() {
        let mode = startup_mode(vec!["--bogus".to_string()].into_iter());
        assert!(matches!(mode, StartupMode::InvalidArgument(arg) if arg == "--bogus"));
    }

    #[test]
    fn no_args_runs() {
        let mode = startup_mode(Vec::<String>::new().into_iter());
        assert!(matches!(mode, StartupMode::Run));
    }
}
