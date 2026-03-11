use oxvba_hal::model::{
    HalRuntimeClass, UiVirtualizationMode, UnsupportedFeatureMode, WasmRuntimeClass,
};
use oxvba_host::{Engine, HostConfig, RunnerBootstrapOptions, resolve_runner_bootstrap};
use oxvba_runtime::{RuntimeValue, bstr::BStr, value_tags::EMPTY_TAG};
use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let args = parse_run_args();
    let config = HostConfig {
        enable_jit: args.as_ref().map(|a| a.enable_jit).unwrap_or(false),
        root_object_name: Some("Application".to_string()),
    };
    let mut engine = Engine::new(config);
    if let Some(run_args) = args.as_ref() {
        let resolved = resolve_runner_bootstrap(&run_args.bootstrap, |key| env::var(key).ok())
            .unwrap_or_else(|err| {
                eprintln!("oxvba: bootstrap failed: {err}");
                std::process::exit(2);
            });
        engine.set_runtime_profile(resolved.runtime_profile);
        engine.set_host_policy(resolved.policy.clone());
        if run_args.dump_bootstrap {
            println!("BOOTSTRAP:{}", resolved.fingerprint());
        }
    }
    let source = args
        .as_ref()
        .map(|a| a.source.clone())
        .unwrap_or_else(|| "Sub Main()\nEnd Sub".to_string());

    let dump_slots = args.as_ref().map(|a| a.dump_slots).unwrap_or(false);
    let dump_values = args.as_ref().map(|a| a.dump_values).unwrap_or(false);

    let execution = if dump_slots || dump_values {
        engine
            .execute_source_with_value_snapshot(&source)
            .map(|values| ExecutionResult {
                slots: values
                    .iter()
                    .map(|value| value.to_legacy_i32().unwrap_or(EMPTY_TAG))
                    .collect(),
                values,
            })
    } else {
        engine
            .execute_source_with_snapshot(&source)
            .map(|values| ExecutionResult {
                slots: values
                    .iter()
                    .map(|value| value.to_legacy_i32().unwrap_or(EMPTY_TAG))
                    .collect(),
                values,
            })
    };

    match execution {
        Ok(result) => {
            if dump_slots {
                let payload = result
                    .slots
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                println!("SLOTS:{payload}");
            }
            if dump_values {
                let payload = result
                    .values
                    .iter()
                    .map(format_runtime_value)
                    .collect::<Vec<_>>()
                    .join("|");
                println!("VALUES:{payload}");
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
    dump_values: bool,
    dump_bootstrap: bool,
    enable_jit: bool,
    bootstrap: RunnerBootstrapOptions,
}

struct ExecutionResult {
    slots: Vec<i32>,
    values: Vec<RuntimeValue>,
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
    let mut dump_values = false;
    let mut dump_bootstrap = false;
    let mut enable_jit = false;
    let mut bootstrap = RunnerBootstrapOptions::default();

    let args = args.collect::<Vec<_>>();
    let mut index = 0usize;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--dump-slots" => dump_slots = true,
            "--dump-values" => dump_values = true,
            "--dump-bootstrap" => dump_bootstrap = true,
            "--jit" => enable_jit = true,
            "--config" => {
                index += 1;
                bootstrap.config_path = Some(PathBuf::from(args.get(index)?.as_str()));
            }
            "--profile" => {
                index += 1;
                bootstrap.profile = Some(args.get(index)?.clone());
            }
            "--policy" => {
                index += 1;
                bootstrap.policy_preset = Some(args.get(index)?.clone());
            }
            "--runtime-class" => {
                index += 1;
                bootstrap.overrides.runtime_class = Some(parse_runtime_class(args.get(index)?)?);
            }
            "--allow-interaction" => {
                index += 1;
                bootstrap.overrides.allow_interaction = Some(parse_bool(args.get(index)?)?);
            }
            "--allow-process-spawn" => {
                index += 1;
                bootstrap.overrides.allow_process_spawn = Some(parse_bool(args.get(index)?)?);
            }
            "--allow-filesystem-mutation" => {
                index += 1;
                bootstrap.overrides.allow_filesystem_mutation = Some(parse_bool(args.get(index)?)?);
            }
            "--allow-dynamic-link" => {
                index += 1;
                bootstrap.overrides.allow_dynamic_link = Some(parse_bool(args.get(index)?)?);
            }
            "--allow-com-activation" => {
                index += 1;
                bootstrap.overrides.allow_com_activation = Some(parse_bool(args.get(index)?)?);
            }
            "--deterministic-mode" => {
                index += 1;
                bootstrap.overrides.deterministic_mode = Some(parse_bool(args.get(index)?)?);
            }
            "--ui-virtualization" => {
                index += 1;
                bootstrap.overrides.ui_virtualization =
                    Some(parse_ui_virtualization(args.get(index)?)?);
            }
            "--unsupported-mode" => {
                index += 1;
                bootstrap.overrides.unsupported_feature_mode =
                    Some(parse_unsupported_mode(args.get(index)?)?);
            }
            "--wasm-runtime-class" => {
                index += 1;
                bootstrap.overrides.wasm_runtime_class =
                    Some(parse_wasm_runtime_class(args.get(index)?)?);
            }
            _ if !arg.starts_with("--") && path.is_none() => path = Some(arg.clone()),
            _ => return None,
        }
        index += 1;
    }

    let source = fs::read_to_string(path?).ok()?;
    Some(RunArgs {
        source,
        dump_slots,
        dump_values,
        dump_bootstrap,
        enable_jit,
        bootstrap,
    })
}

fn format_runtime_value(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Empty => "empty".to_string(),
        RuntimeValue::Null => "null".to_string(),
        RuntimeValue::ErrorCode(code) => format!("error:{code}"),
        RuntimeValue::I32(value) => format!("i32:{value}"),
        RuntimeValue::Bool(value) => format!("bool:{value}"),
        RuntimeValue::String(BStr(value)) => format!("string:{value:?}"),
        RuntimeValue::ArrayIntent(array) => format!("array:{array:?}"),
        RuntimeValue::ObjectHandle(handle) => format!("object:{handle}"),
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_runtime_class(value: &str) -> Option<HalRuntimeClass> {
    match value.trim().to_ascii_lowercase().as_str() {
        "host-native" => Some(HalRuntimeClass::HostNative),
        "windows-gui" => Some(HalRuntimeClass::WindowsGui),
        "windows-headless" => Some(HalRuntimeClass::WindowsHeadless),
        "linux-stdio" => Some(HalRuntimeClass::LinuxStdio),
        "linux-headless" => Some(HalRuntimeClass::LinuxHeadless),
        "macos-gui" => Some(HalRuntimeClass::MacOsGui),
        "macos-headless" => Some(HalRuntimeClass::MacOsHeadless),
        "wasi-local" => Some(HalRuntimeClass::WasmWasiLocal),
        "browser-sandbox" => Some(HalRuntimeClass::WasmBrowserSandbox),
        "null-floor" => Some(HalRuntimeClass::NullFloor),
        _ => None,
    }
}

fn parse_ui_virtualization(value: &str) -> Option<UiVirtualizationMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" => Some(UiVirtualizationMode::Disabled),
        "scripted-responses" => Some(UiVirtualizationMode::ScriptedResponses),
        "fail-on-prompt" => Some(UiVirtualizationMode::FailOnPrompt),
        _ => None,
    }
}

fn parse_unsupported_mode(value: &str) -> Option<UnsupportedFeatureMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "compile-time" => Some(UnsupportedFeatureMode::CompileTime),
        "runtime" => Some(UnsupportedFeatureMode::Runtime),
        _ => None,
    }
}

fn parse_wasm_runtime_class(value: &str) -> Option<WasmRuntimeClass> {
    match value.trim().to_ascii_lowercase().as_str() {
        "wasi" | "wasi-local" => Some(WasmRuntimeClass::Wasi),
        "browser-sandbox" => Some(WasmRuntimeClass::BrowserSandbox),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_run_args_from;
    use oxvba_hal::model::UnsupportedFeatureMode;

    #[test]
    fn parse_run_args_with_flags() {
        let path = "Cargo.toml".to_string();
        let args = vec![
            "run".to_string(),
            path,
            "--dump-slots".to_string(),
            "--dump-values".to_string(),
            "--jit".to_string(),
        ];
        let parsed = parse_run_args_from(args).expect("args should parse");
        assert!(parsed.dump_slots);
        assert!(parsed.dump_values);
        assert!(parsed.enable_jit);
    }

    #[test]
    fn parse_runner_bootstrap_flags() {
        let args = vec![
            "run".to_string(),
            "Cargo.toml".to_string(),
            "--profile".to_string(),
            "linux-stdio".to_string(),
            "--policy".to_string(),
            "strict-ci".to_string(),
            "--allow-dynamic-link".to_string(),
            "false".to_string(),
            "--unsupported-mode".to_string(),
            "compile-time".to_string(),
            "--dump-bootstrap".to_string(),
        ];
        let parsed = parse_run_args_from(args).expect("args should parse");
        assert!(parsed.dump_bootstrap);
        assert_eq!(parsed.bootstrap.profile.as_deref(), Some("linux-stdio"));
        assert_eq!(parsed.bootstrap.policy_preset.as_deref(), Some("strict-ci"));
        assert_eq!(parsed.bootstrap.overrides.allow_dynamic_link, Some(false));
        assert_eq!(
            parsed.bootstrap.overrides.unsupported_feature_mode,
            Some(UnsupportedFeatureMode::CompileTime)
        );
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
