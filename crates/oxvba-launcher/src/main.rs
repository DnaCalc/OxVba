//! oxvba-run: standalone launcher for compiled .oxb bundles.
//!
//! Usage:
//!   oxvba-run <bundle.oxb> [--jit] [runtime/bootstrap options]

use std::{env, fs, path::PathBuf, process};

use oxvba_compiler::OxBundle;
use oxvba_hal::{
    adapters::builder::HostBuilder,
    model::{HalRuntimeClass, UiVirtualizationMode, UnsupportedFeatureMode, WasmRuntimeClass},
};
use oxvba_host::{RunnerBootstrapOptions, resolve_runner_bootstrap};
use oxvba_jit::JitEngine;
use oxvba_runtime::RuntimeValue;

#[derive(Debug, Clone)]
struct LauncherArgs {
    bundle_path: PathBuf,
    use_jit: bool,
    dump_bootstrap: bool,
    bootstrap: RunnerBootstrapOptions,
}

fn main() {
    let args = parse_args(env::args().skip(1).collect()).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba-run <bundle.oxb> [--jit] [--dump-bootstrap] [runtime/bootstrap options]"
        );
        process::exit(2);
    });

    let bundle_data = fs::read(&args.bundle_path).unwrap_or_else(|err| {
        eprintln!("oxvba-run: cannot read {}: {err}", args.bundle_path.display());
        process::exit(1);
    });

    let bundle = OxBundle::deserialize_from_bytes(&bundle_data).unwrap_or_else(|err| {
        eprintln!("oxvba-run: invalid bundle: {err}");
        process::exit(1);
    });

    let resolved = resolve_runner_bootstrap(&args.bootstrap, |key| env::var(key).ok())
        .unwrap_or_else(|err| {
            eprintln!("oxvba-run: bootstrap failed: {err}");
            process::exit(2);
        });
    if args.dump_bootstrap {
        println!("BOOTSTRAP:{}", resolved.fingerprint());
    }

    let host_services = HostBuilder::new()
        .profile(resolved.runtime_profile.hal_profile())
        .policy(resolved.policy.clone())
        .build();

    let result: Result<Vec<RuntimeValue>, String> = if args.use_jit {
        let jit = JitEngine;
        jit.execute_and_snapshot_with_host(&bundle.bytecode, host_services)
            .map_err(|e| e.to_string())
    } else {
        oxvba_vm::execute_and_snapshot_with_host(&bundle.bytecode, host_services)
    };

    match result {
        Ok(values) => {
            if !values.is_empty() {
                for (i, val) in values.iter().enumerate() {
                    eprintln!("  slot[{i}] = {val:?}");
                }
            }
        }
        Err(err) => {
            eprintln!("oxvba-run: execution failed: {err}");
            process::exit(1);
        }
    }
}

fn parse_args(args: Vec<String>) -> Option<LauncherArgs> {
    let mut bundle_path: Option<PathBuf> = None;
    let mut use_jit = false;
    let mut dump_bootstrap = false;
    let mut bootstrap = RunnerBootstrapOptions::default();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--jit" => use_jit = true,
            "--dump-bootstrap" => dump_bootstrap = true,
            "--config" => {
                i += 1;
                bootstrap.config_path = Some(PathBuf::from(args.get(i)?.as_str()));
            }
            "--profile" => {
                i += 1;
                bootstrap.profile = Some(args.get(i)?.clone());
            }
            "--policy" => {
                i += 1;
                bootstrap.policy_preset = Some(args.get(i)?.clone());
            }
            "--runtime-class" => {
                i += 1;
                bootstrap.overrides.runtime_class = Some(parse_runtime_class(args.get(i)?)?);
            }
            "--allow-interaction" => {
                i += 1;
                bootstrap.overrides.allow_interaction = Some(parse_bool(args.get(i)?)?);
            }
            "--allow-process-spawn" => {
                i += 1;
                bootstrap.overrides.allow_process_spawn = Some(parse_bool(args.get(i)?)?);
            }
            "--allow-filesystem-mutation" => {
                i += 1;
                bootstrap.overrides.allow_filesystem_mutation = Some(parse_bool(args.get(i)?)?);
            }
            "--allow-dynamic-link" => {
                i += 1;
                bootstrap.overrides.allow_dynamic_link = Some(parse_bool(args.get(i)?)?);
            }
            "--allow-com-activation" => {
                i += 1;
                bootstrap.overrides.allow_com_activation = Some(parse_bool(args.get(i)?)?);
            }
            "--deterministic-mode" => {
                i += 1;
                bootstrap.overrides.deterministic_mode = Some(parse_bool(args.get(i)?)?);
            }
            "--ui-virtualization" => {
                i += 1;
                bootstrap.overrides.ui_virtualization = Some(parse_ui_virtualization(args.get(i)?)?);
            }
            "--unsupported-mode" => {
                i += 1;
                bootstrap.overrides.unsupported_feature_mode =
                    Some(parse_unsupported_mode(args.get(i)?)?);
            }
            "--wasm-runtime-class" => {
                i += 1;
                bootstrap.overrides.wasm_runtime_class =
                    Some(parse_wasm_runtime_class(args.get(i)?)?);
            }
            arg if !arg.starts_with('-') && bundle_path.is_none() => {
                bundle_path = Some(PathBuf::from(arg));
            }
            _ => return None,
        }
        i += 1;
    }

    Some(LauncherArgs {
        bundle_path: bundle_path?,
        use_jit,
        dump_bootstrap,
        bootstrap,
    })
}

fn parse_runtime_class(raw: &str) -> Option<HalRuntimeClass> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "host-native" => Some(HalRuntimeClass::HostNative),
        "windows-gui" => Some(HalRuntimeClass::WindowsGui),
        "windows-stdio" => Some(HalRuntimeClass::WindowsStdio),
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

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_ui_virtualization(raw: &str) -> Option<UiVirtualizationMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "disabled" => Some(UiVirtualizationMode::Disabled),
        "scripted-responses" => Some(UiVirtualizationMode::ScriptedResponses),
        "fail-on-prompt" => Some(UiVirtualizationMode::FailOnPrompt),
        _ => None,
    }
}

fn parse_unsupported_mode(raw: &str) -> Option<UnsupportedFeatureMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "compile-time" => Some(UnsupportedFeatureMode::CompileTime),
        "runtime" => Some(UnsupportedFeatureMode::Runtime),
        _ => None,
    }
}

fn parse_wasm_runtime_class(raw: &str) -> Option<WasmRuntimeClass> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "wasi" | "wasi-local" => Some(WasmRuntimeClass::Wasi),
        "browser-sandbox" => Some(WasmRuntimeClass::BrowserSandbox),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    #[test]
    fn parse_launcher_args_supports_bootstrap_flags() {
        let parsed = parse_args(vec![
            "demo.oxb".to_string(),
            "--jit".to_string(),
            "--profile".to_string(),
            "windows-stdio".to_string(),
            "--policy".to_string(),
            "interactive-dev".to_string(),
            "--allow-dynamic-link".to_string(),
            "false".to_string(),
            "--dump-bootstrap".to_string(),
        ])
        .expect("launcher args should parse");

        assert!(parsed.use_jit);
        assert!(parsed.dump_bootstrap);
        assert_eq!(parsed.bootstrap.profile.as_deref(), Some("windows-stdio"));
        assert_eq!(parsed.bootstrap.policy_preset.as_deref(), Some("interactive-dev"));
        assert_eq!(parsed.bootstrap.overrides.allow_dynamic_link, Some(false));
    }
}
