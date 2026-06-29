//! `oxvba` — command-line driver for the clean execution stack.
//!
//! The run subcommands execute VBA through `oxvba_bind` →
//! `oxvba_oxir::elaborate` → `oxvba_vm3`:
//!   * `run <source.bas>` — execute a single source module.
//!   * `run-project [path] [--entry M.P]` — load a `.basproj`/`.vbp` (and its
//!     transitive project-reference graph) into a closure and execute it.
//!   * `build <project> --target WrappedComServer --out-dir <dir>` — emit the
//!     clean package and wrapper artifacts for an in-process COM DLL target.
//!
//! The run commands accept the shared runner-bootstrap flags (HAL profile / host
//! policy / capability overrides). The legacy compiler and host execution paths
//! remain removed; `build` is the clean wrapper-artifact lane.

use std::path::{Path, PathBuf};
use std::{env, fs};

use oxvba_diagnostics::{
    Diagnostic as OxDiagnostic, DiagnosticPhase as OxDiagnosticPhase, DiagnosticReport,
};
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
        Some("build") => run_build(cli_args),
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
         oxvba run-project [path] [--entry <Module.Procedure>] [--dump-values] [--jit] [bootstrap options]\n  \
         oxvba build <project.basproj|project.vbp> --target WrappedComServer --out-dir <dir>\n\n\
         diagnostics:\n  \
         --diagnostic-format <human|json>\n\n\
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
    let args = if cli_args.is_empty() {
        None
    } else {
        Some(parse_run_args_from(cli_args).unwrap_or_else(|| {
            eprintln!("usage: oxvba run <source.bas> [--diagnostic-format <human|json>] [bootstrap options]");
            std::process::exit(2);
        }))
    };
    let diagnostic_format = args
        .as_ref()
        .map(|a| a.diagnostic_format)
        .unwrap_or_default();
    let config = HostConfig {
        enable_jit: args.as_ref().map(|a| a.enable_jit).unwrap_or(false),
    };
    let mut engine = Engine::new(config);
    if let Some(run_args) = args.as_ref() {
        let resolved = resolve_runner_bootstrap(&run_args.bootstrap, |key| env::var(key).ok())
            .unwrap_or_else(|err| {
                let diagnostic = OxDiagnostic::error(
                    "HOST-E-BOOTSTRAP",
                    OxDiagnosticPhase::Host,
                    format!("bootstrap failed: {err}"),
                );
                emit_diagnostic("oxvba", &diagnostic, diagnostic_format);
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

    match engine.execute_source_with_variant_snapshot_vm3(&source) {
        oxvba_host::Vm3Snapshot::Ran(values) => {
            if dump_values {
                print_values(&values);
            }
        }
        oxvba_host::Vm3Snapshot::Unsupported(what) => {
            let diagnostic = OxDiagnostic::error(
                "VM3-E-UNSUPPORTED",
                OxDiagnosticPhase::Host,
                format!("vm3 does not support this program: {what}"),
            );
            emit_diagnostic("oxvba: unsupported", &diagnostic, diagnostic_format);
            std::process::exit(1);
        }
        oxvba_host::Vm3Snapshot::Failed(msg) => {
            let diagnostic = OxDiagnostic::error("VM3-E-RUNTIME", OxDiagnosticPhase::Host, msg);
            emit_diagnostic("oxvba: execution failed", &diagnostic, diagnostic_format);
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
            "usage: oxvba run-project [path] [--entry <Module.Procedure>] [--diagnostic-format <human|json>] [bootstrap options]"
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
        let diagnostic = OxDiagnostic::error(
            "PROJ-E-PROJECT-FILE-NOT-FOUND",
            OxDiagnosticPhase::ProjectLoad,
            format!("no .basproj or .vbp project file found at {location}"),
        )
        .with_help("Pass an explicit .basproj or .vbp path, or run from a directory containing exactly one project file.");
        emit_diagnostic("oxvba run-project", &diagnostic, parsed.diagnostic_format);
        std::process::exit(1);
    };

    // The single-project view supplies the runner-bootstrap fallbacks (default
    // profile / policy declared in the project file).
    let loaded = load_project_for_bootstrap(&project_file).unwrap_or_else(|err| {
        emit_diagnostic(
            "oxvba run-project",
            &err.to_diagnostic(),
            parsed.diagnostic_format,
        );
        std::process::exit(1);
    });

    let mut engine = Engine::new(HostConfig {
        enable_jit: parsed.enable_jit,
    });
    let resolved =
        resolve_project_runner_bootstrap(&loaded, &parsed.bootstrap, |key| env::var(key).ok())
            .unwrap_or_else(|err| {
                let diagnostic = OxDiagnostic::error(
                    "HOST-E-BOOTSTRAP",
                    OxDiagnosticPhase::Host,
                    format!("bootstrap failed: {err}"),
                );
                emit_diagnostic("oxvba run-project", &diagnostic, parsed.diagnostic_format);
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
        emit_diagnostic(
            "oxvba run-project",
            &err.to_diagnostic(),
            parsed.diagnostic_format,
        );
        std::process::exit(1);
    });

    match engine.execute_project_closure_with_variant_snapshot_vm3(&closure) {
        oxvba_host::Vm3Snapshot::Ran(values) => {
            if parsed.dump_values {
                print_values(&values);
            }
        }
        oxvba_host::Vm3Snapshot::Unsupported(what) => {
            let diagnostic = OxDiagnostic::error(
                "VM3-E-UNSUPPORTED",
                OxDiagnosticPhase::Host,
                format!("vm3 does not support this program: {what}"),
            );
            emit_diagnostic("oxvba run-project", &diagnostic, parsed.diagnostic_format);
            std::process::exit(1);
        }
        oxvba_host::Vm3Snapshot::Failed(msg) => {
            let diagnostic = OxDiagnostic::error("VM3-E-RUNTIME", OxDiagnosticPhase::Host, msg);
            emit_diagnostic("oxvba run-project", &diagnostic, parsed.diagnostic_format);
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// build subcommand: emit clean wrapper artifacts
// ---------------------------------------------------------------------------

fn run_build(args: Vec<String>) {
    let parsed = parse_build_args_from(args).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba build <project.basproj|project.vbp> --target WrappedComServer --out-dir <dir> [--diagnostic-format <human|json>]"
        );
        std::process::exit(2);
    });

    match oxvba_build::build_wrapped_com_server(&oxvba_build::WrappedComServerBuildOptions {
        project_path: parsed.project_path,
        out_dir: parsed.out_dir,
        compile_dll: true,
        comhost_dll_path: None,
    }) {
        Ok(output) => {
            println!("OXI:{}", output.oxi_path.display());
            println!("COM_DESCRIPTOR:{}", output.descriptor_path.display());
            println!("IDL:{}", output.idl_path.display());
            if let Some(path) = output.comhost_source_path.as_ref() {
                println!("COMHOST_SOURCE:{}", path.display());
            }
            println!("DLL_TARGET:{}", output.dll_target_path.display());
            println!("TLB_TARGET:{}", output.tlb_target_path.display());
        }
        Err(err) => {
            let diagnostic = OxDiagnostic::error(
                "BUILD-E-WRAPPED-COM-SERVER",
                OxDiagnosticPhase::Host,
                format!("WrappedComServer build failed: {err}"),
            );
            emit_diagnostic("oxvba build", &diagnostic, parsed.diagnostic_format);
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

fn emit_diagnostic(prefix: &str, diagnostic: &OxDiagnostic, format: DiagnosticFormat) {
    match format {
        DiagnosticFormat::Human => eprintln!("{prefix}: {}", diagnostic.render_human()),
        DiagnosticFormat::Json => {
            match DiagnosticReport::single(diagnostic.clone()).to_json_pretty() {
                Ok(json) => eprintln!("{json}"),
                Err(err) => eprintln!(
                    "{prefix}: failed to serialize diagnostic JSON: {err}; {}",
                    diagnostic.render_human()
                ),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// argument parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum DiagnosticFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug, Clone)]
struct RunArgs {
    source: String,
    dump_values: bool,
    dump_bootstrap: bool,
    enable_jit: bool,
    diagnostic_format: DiagnosticFormat,
    bootstrap: RunnerBootstrapOptions,
}

#[derive(Debug, Clone)]
struct RunProjectArgs {
    input_path: Option<PathBuf>,
    enable_jit: bool,
    dump_values: bool,
    dump_bootstrap: bool,
    diagnostic_format: DiagnosticFormat,
    bootstrap: RunnerBootstrapOptions,
    entry_point_override: Option<String>,
}

#[derive(Debug, Clone)]
struct BuildArgs {
    project_path: PathBuf,
    out_dir: PathBuf,
    diagnostic_format: DiagnosticFormat,
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
    let mut diagnostic_format = DiagnosticFormat::Human;
    let mut bootstrap = RunnerBootstrapOptions::default();

    let mut i = 0;
    while i < collected.len() {
        let arg = collected[i].as_str();
        match arg {
            "--dump-values" => dump_values = true,
            "--dump-bootstrap" => dump_bootstrap = true,
            "--jit" => enable_jit = true,
            "--diagnostic-format" => {
                i += 1;
                diagnostic_format = parse_diagnostic_format(collected.get(i)?)?;
            }
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
        diagnostic_format,
        bootstrap,
    })
}

fn parse_build_args_from(args: Vec<String>) -> Option<BuildArgs> {
    let mut iter = args.into_iter();
    if iter.next()? != "build" {
        return None;
    }
    let collected: Vec<String> = iter.collect();
    let mut project_path: Option<PathBuf> = None;
    let mut target: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut diagnostic_format = DiagnosticFormat::Human;

    let mut i = 0;
    while i < collected.len() {
        let arg = collected[i].as_str();
        match arg {
            "--target" => {
                i += 1;
                target = Some(collected.get(i)?.clone());
            }
            "--out-dir" => {
                i += 1;
                out_dir = Some(PathBuf::from(collected.get(i)?));
            }
            "--diagnostic-format" => {
                i += 1;
                diagnostic_format = parse_diagnostic_format(collected.get(i)?)?;
            }
            _ if !arg.starts_with('-') && project_path.is_none() => {
                project_path = Some(PathBuf::from(arg));
            }
            _ => return None,
        }
        i += 1;
    }

    if !target
        .as_deref()
        .is_some_and(|target| target.eq_ignore_ascii_case("WrappedComServer"))
    {
        return None;
    }

    Some(BuildArgs {
        project_path: project_path?,
        out_dir: out_dir?,
        diagnostic_format,
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
    let mut diagnostic_format = DiagnosticFormat::Human;
    let mut bootstrap = RunnerBootstrapOptions::default();
    let mut entry_point_override: Option<String> = None;

    let mut i = 0;
    while i < collected.len() {
        let arg = collected[i].as_str();
        match arg {
            "--jit" => enable_jit = true,
            "--dump-values" => dump_values = true,
            "--dump-bootstrap" => dump_bootstrap = true,
            "--diagnostic-format" => {
                i += 1;
                diagnostic_format = parse_diagnostic_format(collected.get(i)?)?;
            }
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
        diagnostic_format,
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
        VarType::Record => match value.as_com_record() {
            Some(record) => format!("record:{record:?}"),
            None => "record:<invalid>".to_string(),
        },
        VarType::ProcRef => match value.as_proc_ref() {
            Some(proc) => format!("proc:{proc}"),
            None => "proc:<invalid>".to_string(),
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

fn parse_diagnostic_format(value: &str) -> Option<DiagnosticFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "human" => Some(DiagnosticFormat::Human),
        "json" => Some(DiagnosticFormat::Json),
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
            "--diagnostic-format".to_string(),
            "json".to_string(),
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
        assert_eq!(parsed.diagnostic_format, DiagnosticFormat::Json);
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
            "--diagnostic-format".to_string(),
            "json".to_string(),
            "--profile".to_string(),
            "windows-stdio".to_string(),
            "--jit".to_string(),
        ];
        let parsed = parse_run_project_args_from(args).expect("args should parse");
        assert_eq!(parsed.entry_point_override.as_deref(), Some("Startup.Boot"));
        assert_eq!(parsed.bootstrap.profile.as_deref(), Some("windows-stdio"));
        assert!(parsed.enable_jit);
        assert_eq!(parsed.diagnostic_format, DiagnosticFormat::Json);
        assert_eq!(parsed.input_path, Some(PathBuf::from(".")));
    }

    #[test]
    fn parse_build_args_for_wrapped_com_server() {
        let args = vec![
            "build".to_string(),
            "demo.basproj".to_string(),
            "--target".to_string(),
            "WrappedComServer".to_string(),
            "--out-dir".to_string(),
            "target/oxvba-build/demo".to_string(),
            "--diagnostic-format".to_string(),
            "json".to_string(),
        ];
        let parsed = parse_build_args_from(args).expect("args should parse");
        assert_eq!(parsed.project_path, PathBuf::from("demo.basproj"));
        assert_eq!(parsed.out_dir, PathBuf::from("target/oxvba-build/demo"));
        assert_eq!(parsed.diagnostic_format, DiagnosticFormat::Json);
    }

    #[test]
    fn reject_build_args_without_wrapped_com_server_target() {
        let args = vec![
            "build".to_string(),
            "demo.basproj".to_string(),
            "--target".to_string(),
            "Bundle".to_string(),
            "--out-dir".to_string(),
            "target/oxvba-build/demo".to_string(),
        ];
        assert!(parse_build_args_from(args).is_none());
    }

    #[test]
    fn reject_unknown_diagnostic_format() {
        let args = vec![
            "run".to_string(),
            "Cargo.toml".to_string(),
            "--diagnostic-format".to_string(),
            "sarif".to_string(),
        ];
        assert!(parse_run_args_from(args).is_none());
    }

    #[test]
    fn format_scalar_variants() {
        assert_eq!(format_variant_value(&Variant::from_i32(42)), "i32:42");
        assert_eq!(format_variant_value(&Variant::from_bool(true)), "bool:true");
        assert_eq!(format_variant_value(&Variant::empty()), "empty");
    }
}
