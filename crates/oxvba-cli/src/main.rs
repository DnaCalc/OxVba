use oxvba_host::{Engine, HostConfig};
use std::{env, fs};

fn main() {
    let config = HostConfig {
        enable_jit: false,
        root_object_name: Some("Application".to_string()),
    };

    let engine = Engine::new(config);
    let args = parse_run_args();
    let source = args
        .as_ref()
        .map(|a| a.source.clone())
        .unwrap_or_else(|| "Sub Main()\nEnd Sub".to_string());

    match engine.execute_source_with_snapshot(&source) {
        Ok(slots) => {
            if args.as_ref().map(|a| a.dump_slots).unwrap_or(false) {
                let payload = slots
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                println!("SLOTS:{payload}");
            }
        }
        Err(err) => {
            eprintln!("oxvba: execution failed: {err}");
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone)]
struct RunArgs {
    source: String,
    dump_slots: bool,
}

fn parse_run_args() -> Option<RunArgs> {
    let mut args = env::args().skip(1);
    let cmd = args.next()?;
    if cmd != "run" {
        return None;
    }

    let path = args.next()?;
    let source = fs::read_to_string(path).ok()?;
    let dump_slots = args.any(|a| a == "--dump-slots");
    Some(RunArgs { source, dump_slots })
}
