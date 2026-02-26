use oxvba_host::{Engine, HostConfig};
use std::{env, fs};

fn main() {
    let config = HostConfig {
        enable_jit: false,
        root_object_name: Some("Application".to_string()),
    };

    let engine = Engine::new(config);
    let source = load_source_from_args().unwrap_or_else(|| "Sub Main()\nEnd Sub".to_string());

    if let Err(err) = engine.execute_source(&source) {
        eprintln!("oxvba: execution failed: {err}");
        std::process::exit(1);
    }
}

fn load_source_from_args() -> Option<String> {
    let mut args = env::args().skip(1);
    let cmd = args.next()?;
    if cmd != "run" {
        return None;
    }

    let path = args.next()?;
    fs::read_to_string(path).ok()
}
