use oxvba_host::{Engine, HostConfig};
use std::{env, fs};

fn main() {
    let args = parse_run_args();
    let config = HostConfig {
        enable_jit: args.as_ref().map(|a| a.enable_jit).unwrap_or(false),
        root_object_name: Some("Application".to_string()),
    };
    let engine = Engine::new(config);
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
    enable_jit: bool,
}

fn parse_run_args() -> Option<RunArgs> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    parse_run_args_from(args)
}

fn parse_run_args_from(args: Vec<String>) -> Option<RunArgs> {
    let mut args = args.into_iter();
    let cmd = args.next()?;
    if cmd != "run" {
        return None;
    }

    let mut path: Option<String> = None;
    let mut dump_slots = false;
    let mut enable_jit = false;

    for arg in args {
        match arg.as_str() {
            "--dump-slots" => dump_slots = true,
            "--jit" => enable_jit = true,
            _ if !arg.starts_with("--") && path.is_none() => path = Some(arg),
            _ => return None,
        }
    }

    let source = fs::read_to_string(path?).ok()?;
    Some(RunArgs {
        source,
        dump_slots,
        enable_jit,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_run_args_from;

    #[test]
    fn parse_run_args_with_flags() {
        let path = "Cargo.toml".to_string();
        let args = vec![
            "run".to_string(),
            path,
            "--dump-slots".to_string(),
            "--jit".to_string(),
        ];
        let parsed = parse_run_args_from(args).expect("args should parse");
        assert!(parsed.dump_slots);
        assert!(parsed.enable_jit);
    }

    #[test]
    fn reject_unknown_flags() {
        let args = vec![
            "run".to_string(),
            "Cargo.toml".to_string(),
            "--unknown".to_string(),
        ];
        assert!(parse_run_args_from(args).is_none());
    }
}
