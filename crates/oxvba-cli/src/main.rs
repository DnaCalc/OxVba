//! `oxvba` — command-line driver for the clean execution stack.
//!
//! Two subcommands run VBA through `oxvba_bind` → `oxvba_bundle::linearize` →
//! `oxvba_vm2`:
//!   * `run <source.bas>` — execute a single source module.
//!   * `run-project [path] [--entry M.P]` — load a `.basproj`/`.vbp` (and its
//!     transitive project-reference graph) into a closure and execute it.
//!
//! Both accept the shared runner-bootstrap flags (HAL profile / host policy /
//! capability overrides). The legacy `compile`/`build`/COM-management/native-export
//! subcommands were removed with the legacy compiler and host execution paths.

use std::path::{Path, PathBuf};
use std::{env, fs};

use oxvba_hal::model::{
    HalRuntimeClass, UiVirtualizationMode, UnsupportedFeatureMode, WasmRuntimeClass,
};
use oxvba_host::{
    Engine, HostConfig, ResolvedRunnerBootstrap, RunnerBootstrapFallbacks, RunnerBootstrapOptions,
    resolve_runner_bootstrap, resolve_runner_bootstrap_with_fallbacks,
};
use oxvba_runtime::{VarType, Variant};

fn main() {
    let cli_args: Vec<String> = env::args().skip(1).collect();
    match cli_args.first().map(String::as_str) {
        Some("run-project") => run_project(cli_args),
        Some("run") | None => run_execute(cli_args),
        Some("help") | Some("--help") | Some("-h") => print_usage(),
        Some(other) => {
            eprintln!("oxvba: unknown subcommand `{other}`");
            print_usage();
            std::process::exit(2);
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage:\n  \
         oxvba run <source.bas> [--dump-values] [--jit] [bootstrap options]\n  \
         oxvba run-project [path] [--entry <Module.Procedure>] [--dump-values] [--jit] [bootstrap options]\n\n\
         bootstrap options:\n  \
         --profile <id>  --policy <preset>  --config <path>  --runtime-class <class>\n  \
         --allow-interaction|--allow-process-spawn|--allow-filesystem-mutation <bool>\n  \
         --allow-dynamic-link|--allow-com-activation|--deterministic-mode <bool>\n  \
         --ui-virtualization <mode>  --unsupported-mode <mode>  --wasm-runtime-class <class>\n  \
         --dump-bootstrap"
    );
}

// ---------------------------------------------------------------------------
// run subcommand: execute a single VBA source module on the clean path
// ---------------------------------------------------------------------------

fn run_execute(cli_args: Vec<String>) {
    let args = parse_run_args_from(cli_args);
    let config = HostConfig {
        enable_jit: args.as_ref().map(|a| a.enable_jit).unwrap_or(false),
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
    let dump_values = args.as_ref().map(|a| a.dump_values).unwrap_or(false);

    match engine.execute_source_with_variant_snapshot_clean(&source) {
        Ok(values) => {
            if dump_values {
                print_values(&values);
            }
        }
        Err(err) => {
            eprintln!("oxvba: execution failed: {err}");
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// run-project subcommand: load a `.basproj`/`.vbp` closure and execute it
// ---------------------------------------------------------------------------

fn run_project(args: Vec<String>) {
    let parsed = parse_run_project_args_from(args).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba run-project [path] [--entry <Module.Procedure>] [bootstrap options]"
        );
        std::process::exit(2);
    });

    // The clean path runs a `.basproj`/`.vbp` and its reference graph as a closure.
    let Some(project_file) = resolve_project_file(parsed.input_path.as_ref()) else {
        let location = parsed
            .input_path
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string());
        eprintln!("oxvba run-project: no .basproj or .vbp project file found at {location}");
        std::process::exit(1);
    };

    // The single-project view supplies the runner-bootstrap fallbacks (default
    // profile / policy declared in the project file).
    let loaded = load_project_for_bootstrap(&project_file).unwrap_or_else(|err| {
        eprintln!("oxvba run-project: {err}");
        std::process::exit(1);
    });

    let mut engine = Engine::new(HostConfig {
        enable_jit: parsed.enable_jit,
    });
    let resolved =
        resolve_project_runner_bootstrap(&loaded, &parsed.bootstrap, |key| env::var(key).ok())
            .unwrap_or_else(|err| {
                eprintln!("oxvba run-project: bootstrap failed: {err}");
                std::process::exit(2);
            });
    engine.set_runtime_profile(resolved.runtime_profile);
    engine.set_host_policy(resolved.policy.clone());
    if parsed.dump_bootstrap {
        println!("BOOTSTRAP:{}", resolved.fingerprint());
    }

    let closure = oxvba_project::load_project_closure_with_entry(
        &project_file,
        parsed.entry_point_override.as_deref(),
    )
    .unwrap_or_else(|err| {
        eprintln!("oxvba run-project: {err}");
        std::process::exit(1);
    });

    match engine.execute_project_closure_with_variant_snapshot(&closure) {
        Ok(values) => {
            if parsed.dump_values {
                print_values(&values);
            }
        }
        Err(err) => {
            eprintln!("oxvba run-project: {err}");
            std::process::exit(1);
        }
    }
}

/// Resolve a run-project input to a concrete `.basproj`/`.vbp` file. A directory
/// is searched for a unique project file; a directory with no project file (or a
/// non-project file argument) returns `None`.
fn resolve_project_file(input: Option<&PathBuf>) -> Option<PathBuf> {
    let input = input.cloned().unwrap_or_else(|| PathBuf::from("."));
    if input.is_dir() {
        if let Ok(Some(p)) = oxvba_project::discover_project_file_in_dir(&input, "basproj") {
            return Some(p);
        }
        if let Ok(Some(p)) = oxvba_project::discover_project_file_in_dir(&input, "vbp") {
            return Some(p);
        }
        return None;
    }
    match input.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("basproj") || ext.eq_ignore_ascii_case("vbp") => {
            Some(input)
        }
        _ => None,
    }
}

fn load_project_for_bootstrap(
    file: &Path,
) -> Result<oxvba_project::LoadedProject, oxvba_project::BasProjError> {
    if file
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("vbp"))
    {
        oxvba_project::load_vbp(file)
    } else {
        oxvba_project::load_basproj(file)
    }
}

fn resolve_project_runner_bootstrap(
    loaded: &oxvba_project::LoadedProject,
    bootstrap: &RunnerBootstrapOptions,
    env_get: impl Fn(&str) -> Option<String>,
) -> Result<ResolvedRunnerBootstrap, String> {
    resolve_runner_bootstrap_with_fallbacks(
        bootstrap,
        &RunnerBootstrapFallbacks {
            profile: loaded.default_runtime_profile.clone(),
            policy_preset: loaded.default_policy_preset.clone(),
        },
        env_get,
    )
}

fn print_values(values: &[Variant]) {
    let payload = values
        .iter()
        .map(format_variant_value)
        .collect::<Vec<_>>()
        .join("|");
    println!("VALUES:{payload}");
}

// ---------------------------------------------------------------------------
// argument parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RunArgs {
    source: String,
    dump_values: bool,
    dump_bootstrap: bool,
    enable_jit: bool,
    bootstrap: RunnerBootstrapOptions,
}

#[derive(Debug, Clone)]
struct RunProjectArgs {
    input_path: Option<PathBuf>,
    enable_jit: bool,
    dump_values: bool,
    dump_bootstrap: bool,
    bootstrap: RunnerBootstrapOptions,
    entry_point_override: Option<String>,
}

fn parse_run_args_from(args: Vec<String>) -> Option<RunArgs> {
    let mut iter = args.into_iter();
    if iter.next()? != "run" {
        return None;
    }
    let collected: Vec<String> = iter.collect();
    let mut path: Option<String> = None;
    let mut dump_values = false;
    let mut dump_bootstrap = false;
    let mut enable_jit = false;
    let mut bootstrap = RunnerBootstrapOptions::default();

    let mut i = 0;
    while i < collected.len() {
        let arg = collected[i].as_str();
        match arg {
            "--dump-values" => dump_values = true,
            "--dump-bootstrap" => dump_bootstrap = true,
            "--jit" => enable_jit = true,
            _ => match consume_bootstrap_flag(&mut bootstrap, arg, collected.get(i + 1)) {
                Some(Ok(())) => i += 1,
                Some(Err(())) => return None,
                None if !arg.starts_with('-') && path.is_none() => path = Some(arg.to_string()),
                None => return None,
            },
        }
        i += 1;
    }

    let source = fs::read_to_string(path?).ok()?;
    Some(RunArgs {
        source,
        dump_values,
        dump_bootstrap,
        enable_jit,
        bootstrap,
    })
}

fn parse_run_project_args_from(args: Vec<String>) -> Option<RunProjectArgs> {
    let mut iter = args.into_iter();
    if iter.next()? != "run-project" {
        return None;
    }
    let collected: Vec<String> = iter.collect();
    let mut input_path: Option<PathBuf> = None;
    let mut enable_jit = false;
    let mut dump_values = false;
    let mut dump_bootstrap = false;
    let mut bootstrap = RunnerBootstrapOptions::default();
    let mut entry_point_override: Option<String> = None;

    let mut i = 0;
    while i < collected.len() {
        let arg = collected[i].as_str();
        match arg {
            "--jit" => enable_jit = true,
            "--dump-values" => dump_values = true,
            "--dump-bootstrap" => dump_bootstrap = true,
            "--entry" => {
                i += 1;
                entry_point_override = Some(collected.get(i)?.clone());
            }
            _ => match consume_bootstrap_flag(&mut bootstrap, arg, collected.get(i + 1)) {
                Some(Ok(())) => i += 1,
                Some(Err(())) => return None,
                None if !arg.starts_with('-') && input_path.is_none() => {
                    input_path = Some(PathBuf::from(arg));
                }
                None => return None,
            },
        }
        i += 1;
    }

    Some(RunProjectArgs {
        input_path,
        enable_jit,
        dump_values,
        dump_bootstrap,
        bootstrap,
        entry_point_override,
    })
}

/// Interpret `flag` as a runner-bootstrap option (each consumes exactly one value
/// from `value`). Returns `None` if `flag` is not a bootstrap flag, `Some(Ok(()))`
/// if applied, `Some(Err(()))` if the value is missing or invalid.
fn consume_bootstrap_flag(
    bootstrap: &mut RunnerBootstrapOptions,
    flag: &str,
    value: Option<&String>,
) -> Option<Result<(), ()>> {
    let is_bootstrap_flag = matches!(
        flag,
        "--config"
            | "--profile"
            | "--policy"
            | "--runtime-class"
            | "--allow-interaction"
            | "--allow-process-spawn"
            | "--allow-filesystem-mutation"
            | "--allow-dynamic-link"
            | "--allow-com-activation"
            | "--deterministic-mode"
            | "--ui-virtualization"
            | "--unsupported-mode"
            | "--wasm-runtime-class"
    );
    if !is_bootstrap_flag {
        return None;
    }
    let Some(v) = value else {
        return Some(Err(()));
    };
    let overrides = &mut bootstrap.overrides;
    let outcome = match flag {
        "--config" => {
            bootstrap.config_path = Some(PathBuf::from(v.as_str()));
            Ok(())
        }
        "--profile" => {
            bootstrap.profile = Some(v.clone());
            Ok(())
        }
        "--policy" => {
            bootstrap.policy_preset = Some(v.clone());
            Ok(())
        }
        "--runtime-class" => set_or_err(&mut overrides.runtime_class, parse_runtime_class(v)),
        "--allow-interaction" => set_or_err(&mut overrides.allow_interaction, parse_bool(v)),
        "--allow-process-spawn" => set_or_err(&mut overrides.allow_process_spawn, parse_bool(v)),
        "--allow-filesystem-mutation" => {
            set_or_err(&mut overrides.allow_filesystem_mutation, parse_bool(v))
        }
        "--allow-dynamic-link" => set_or_err(&mut overrides.allow_dynamic_link, parse_bool(v)),
        "--allow-com-activation" => set_or_err(&mut overrides.allow_com_activation, parse_bool(v)),
        "--deterministic-mode" => set_or_err(&mut overrides.deterministic_mode, parse_bool(v)),
        "--ui-virtualization" => {
            set_or_err(&mut overrides.ui_virtualization, parse_ui_virtualization(v))
        }
        "--unsupported-mode" => set_or_err(
            &mut overrides.unsupported_feature_mode,
            parse_unsupported_mode(v),
        ),
        "--wasm-runtime-class" => set_or_err(
            &mut overrides.wasm_runtime_class,
            parse_wasm_runtime_class(v),
        ),
        _ => unreachable!("flag set was checked above"),
    };
    Some(outcome)
}

fn set_or_err<T>(slot: &mut Option<T>, parsed: Option<T>) -> Result<(), ()> {
    match parsed {
        Some(value) => {
            *slot = Some(value);
            Ok(())
        }
        None => Err(()),
    }
}

// ---------------------------------------------------------------------------
// value formatting + scalar parsers
// ---------------------------------------------------------------------------

fn format_variant_value(value: &Variant) -> String {
    match value.vtype() {
        VarType::Empty => "empty".to_string(),
        VarType::Null => "null".to_string(),
        VarType::Error => format!("error:{}", value.as_error_code().unwrap_or(0)),
        VarType::Integer => format!("i16:{}", value.as_i16().unwrap_or(0)),
        VarType::Long => format!("i32:{}", value.as_i32().unwrap_or(0)),
        VarType::SignedByte => format!("i8:{}", value.as_i8().unwrap_or(0)),
        VarType::Byte => format!("u8:{}", value.as_u8().unwrap_or(0)),
        VarType::UnsignedInteger => format!("u16:{}", value.as_u16().unwrap_or(0)),
        VarType::UnsignedLong => format!("u32:{}", value.as_u32().unwrap_or(0)),
        VarType::UnsignedInt => format!("uint:{}", value.as_u32().unwrap_or(0)),
        VarType::LongLong => format!("i64:{}", value.as_i64().unwrap_or(0)),
        VarType::UnsignedLongLong => format!("u64:{}", value.as_u64().unwrap_or(0)),
        VarType::Single | VarType::Double | VarType::Date => {
            format!("f64:{}", value.as_f64().unwrap_or(0.0))
        }
        VarType::Decimal => match value.as_decimal96() {
            Some(value) => format!("decimal:{value}"),
            None => "decimal:<invalid>".to_string(),
        },
        VarType::Currency => format!(
            "currency:{}",
            oxvba_runtime::CurrencyValue::from_scaled_i64(
                value.as_currency_scaled_i64().unwrap_or(0)
            )
        ),
        VarType::Boolean => format!("bool:{}", value.as_bool().unwrap_or(false)),
        VarType::String => format!(
            "string:{:?}",
            value
                .as_bstr()
                .map(|text| text.as_str())
                .unwrap_or_default()
        ),
        VarType::ArrayVariant => match value.as_safearray() {
            Some(array) => format!("array:{array:?}"),
            None => "array:<invalid>".to_string(),
        },
        VarType::Object => match value.as_object_ref() {
            Some(handle) => format!("object:{handle}"),
            None => "object:<null>".to_string(),
        },
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
    use super::*;

    #[test]
    fn parse_run_args_with_flags() {
        let args = vec![
            "run".to_string(),
            "Cargo.toml".to_string(),
            "--dump-values".to_string(),
            "--jit".to_string(),
        ];
        let parsed = parse_run_args_from(args).expect("args should parse");
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

    #[test]
    fn reject_bootstrap_flag_missing_value() {
        let args = vec![
            "run".to_string(),
            "Cargo.toml".to_string(),
            "--profile".to_string(),
        ];
        assert!(parse_run_args_from(args).is_none());
    }

    #[test]
    fn parse_run_project_args_with_entry_override() {
        let args = vec![
            "run-project".to_string(),
            ".".to_string(),
            "--entry".to_string(),
            "Startup.Boot".to_string(),
            "--profile".to_string(),
            "windows-stdio".to_string(),
            "--jit".to_string(),
        ];
        let parsed = parse_run_project_args_from(args).expect("args should parse");
        assert_eq!(parsed.entry_point_override.as_deref(), Some("Startup.Boot"));
        assert_eq!(parsed.bootstrap.profile.as_deref(), Some("windows-stdio"));
        assert!(parsed.enable_jit);
        assert_eq!(parsed.input_path, Some(PathBuf::from(".")));
    }

    #[test]
    fn format_scalar_variants() {
        assert_eq!(format_variant_value(&Variant::from_i32(42)), "i32:42");
        assert_eq!(format_variant_value(&Variant::from_bool(true)), "bool:true");
        assert_eq!(format_variant_value(&Variant::empty()), "empty");
    }
}
