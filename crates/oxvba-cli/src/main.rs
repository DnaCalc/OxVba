use oxvba_hal::model::{
    HalRuntimeClass, UiVirtualizationMode, UnsupportedFeatureMode, WasmRuntimeClass,
};
use oxvba_host::{Engine, HostConfig, RunnerBootstrapOptions, resolve_runner_bootstrap};
use oxvba_runtime::{RuntimeValue, bstr::BStr, value_tags::EMPTY_TAG};
use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let cli_args: Vec<String> = env::args().skip(1).collect();
    let subcommand = cli_args.first().map(|s| s.as_str());

    match subcommand {
        Some("compile") => run_compile(cli_args),
        Some("build") => run_build(cli_args),
        Some("run-project") => run_project(cli_args),
        Some("init") => run_init(cli_args),
        Some("import-vbp") => run_import_vbp(cli_args),
        _ => run_execute(cli_args),
    }
}

// ---------------------------------------------------------------------------
// build subcommand: .basproj → compile → .oxb
// ---------------------------------------------------------------------------

fn run_build(args: Vec<String>) {
    let mut iter = args.into_iter();
    let _ = iter.next(); // "build"

    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let collected: Vec<String> = iter.collect();
    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                output_path = collected.get(i).map(PathBuf::from);
            }
            arg if !arg.starts_with('-') && input_path.is_none() => {
                input_path = Some(PathBuf::from(arg));
            }
            _ => {
                eprintln!("oxvba build: unknown argument: {}", collected[i]);
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let input = input_path.unwrap_or_else(|| {
        // Try to find a .basproj in current directory
        discover_basproj(".").unwrap_or_else(|| {
            eprintln!("usage: oxvba build [<project.basproj>] [-o <output.oxb>]");
            std::process::exit(2);
        })
    });

    let mut loaded = oxvba_project::load_basproj(&input).unwrap_or_else(|err| {
        eprintln!("oxvba build: {err}");
        std::process::exit(1);
    });

    let compiled = oxvba_compiler::compile_project(&loaded.manifest).unwrap_or_else(|err| {
        eprintln!("oxvba build: compile failed: {err}");
        std::process::exit(1);
    });

    // Post-compilation validation: enrich native export descriptors
    if !loaded.native_exports.is_empty() {
        oxvba_project::validate::validate_native_exports(&mut loaded.native_exports, &compiled)
            .unwrap_or_else(|err| {
                eprintln!("oxvba build: export validation failed: {err}");
                std::process::exit(1);
            });
    }

    // Validate COM class exports for ComServer/ComExe projects
    let com_class_exports = if matches!(
        loaded.output_type,
        oxvba_project::OutputType::ComServer | oxvba_project::OutputType::ComExe
    ) {
        // Build BasProjModule proxies from the manifest for validation
        let modules_for_validation: Vec<oxvba_project::BasProjModule> = loaded
            .manifest
            .modules
            .iter()
            .map(|m| oxvba_project::BasProjModule {
                kind: match m.module_kind {
                    oxvba_compiler::ModuleKind::Class => {
                        oxvba_project::BasProjModuleKind::ClassModule
                    }
                    oxvba_compiler::ModuleKind::Document => {
                        oxvba_project::BasProjModuleKind::DocumentModule
                    }
                    _ => oxvba_project::BasProjModuleKind::Module,
                },
                include: format!("{}.cls", m.module_name),
                vb_predeclared_id: m.attributes.vb_predeclared_id,
                vb_exposed: m.attributes.vb_exposed,
                vb_global_namespace: m.attributes.vb_global_namespace,
                vb_creatable: m.attributes.vb_creatable,
                host_document_type: None,
                instancing: None,
                prog_id: None,
                description: None,
            })
            .collect();
        oxvba_project::validate::validate_com_class_exports(
            &modules_for_validation,
            &compiled,
            &loaded.class_module_metadata,
            &loaded.manifest.project_name,
        )
        .unwrap_or_else(|err| {
            eprintln!("oxvba build: COM class validation failed: {err}");
            std::process::exit(1);
        })
    } else {
        Vec::new()
    };

    let mut bundle =
        oxvba_compiler::OxBundle::from_compiled_project(&compiled, &loaded.manifest.project_name);

    // Store COM class exports in the bundle's export inventory
    if !com_class_exports.is_empty()
        && let Some(ref mut inventory) = bundle.export_inventory
    {
        inventory.com_class_exports = com_class_exports
            .iter()
            .map(|c| oxvba_compiler::ComClassExportEntry {
                class_name: c.class_name.clone(),
                prog_id: c.prog_id.clone(),
                instancing: c.instancing.map(|i| format!("{i:?}")),
                clsid: None,
                description: c.description.clone(),
            })
            .collect();
    }
    let bytes = bundle.serialize_to_bytes().unwrap_or_else(|err| {
        eprintln!("oxvba build: bundle serialization failed: {err}");
        std::process::exit(1);
    });

    let out = output_path.unwrap_or_else(|| {
        input
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(format!("{}.oxb", loaded.manifest.project_name))
    });

    fs::write(&out, &bytes).unwrap_or_else(|err| {
        eprintln!("oxvba build: cannot write {}: {err}", out.display());
        std::process::exit(1);
    });

    println!(
        "built {} → {} ({} bytes)",
        input.display(),
        out.display(),
        bytes.len()
    );
}

// ---------------------------------------------------------------------------
// run-project subcommand
// ---------------------------------------------------------------------------

fn run_project(args: Vec<String>) {
    let mut iter = args.into_iter();
    let _ = iter.next(); // "run-project"

    let mut input_path: Option<PathBuf> = None;
    let mut enable_jit = false;
    let mut dump_values = false;
    let mut dump_slots = false;
    let mut bootstrap = RunnerBootstrapOptions::default();

    let collected: Vec<String> = iter.collect();
    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "--jit" => enable_jit = true,
            "--dump-values" => dump_values = true,
            "--dump-slots" => dump_slots = true,
            "--profile" => {
                i += 1;
                bootstrap.profile = collected.get(i).cloned();
            }
            "--policy" => {
                i += 1;
                bootstrap.policy_preset = collected.get(i).cloned();
            }
            arg if !arg.starts_with('-') && input_path.is_none() => {
                input_path = Some(PathBuf::from(arg));
            }
            _ => {
                eprintln!("oxvba run-project: unknown argument: {}", collected[i]);
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let input = input_path.unwrap_or_else(|| {
        discover_basproj(".").unwrap_or_else(|| {
            eprintln!("usage: oxvba run-project [<project.basproj>] [--jit] [--dump-values] [--dump-slots]");
            std::process::exit(2);
        })
    });

    let loaded = oxvba_project::load_basproj(&input).unwrap_or_else(|err| {
        eprintln!("oxvba run-project: {err}");
        std::process::exit(1);
    });

    let config = HostConfig {
        enable_jit,
        root_object_name: Some(loaded.default_root_object.clone()),
    };
    let mut engine = Engine::new(config);

    let resolved =
        resolve_runner_bootstrap(&bootstrap, |key| env::var(key).ok()).unwrap_or_else(|err| {
            eprintln!("oxvba run-project: bootstrap failed: {err}");
            std::process::exit(2);
        });
    engine.set_runtime_profile(resolved.runtime_profile);
    engine.set_host_policy(resolved.policy.clone());

    let result = engine.execute_project_with_snapshot_phased(&loaded.manifest);

    match result {
        Ok(values) => {
            if dump_slots {
                let payload = values
                    .iter()
                    .map(|v| v.to_legacy_i32().unwrap_or(EMPTY_TAG).to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                println!("SLOTS:{payload}");
            }
            if dump_values {
                let payload = values
                    .iter()
                    .map(format_runtime_value)
                    .collect::<Vec<_>>()
                    .join("|");
                println!("VALUES:{payload}");
            }
        }
        Err(err) => {
            eprintln!("oxvba run-project: {err}");
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// init subcommand
// ---------------------------------------------------------------------------

fn run_init(args: Vec<String>) {
    let mut iter = args.into_iter();
    let _ = iter.next(); // "init"

    let target_dir = iter
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let project_name = target_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("NewProject")
        .to_string();

    fs::create_dir_all(&target_dir).unwrap_or_else(|err| {
        eprintln!("oxvba init: cannot create directory: {err}");
        std::process::exit(1);
    });

    let basproj_content = format!(
        r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>{project_name}</ProjectName>
    <EntryPoint>Module1.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Module1.bas" />
  </ItemGroup>
</Project>
"#
    );

    let module_content =
        "Attribute VB_Name = \"Module1\"\n\nPublic Sub Main()\n    ' Your code here\nEnd Sub\n";

    let basproj_path = target_dir.join(format!("{project_name}.basproj"));
    let module_path = target_dir.join("Module1.bas");

    if basproj_path.exists() {
        eprintln!("oxvba init: {} already exists", basproj_path.display());
        std::process::exit(1);
    }

    fs::write(&basproj_path, basproj_content).unwrap_or_else(|err| {
        eprintln!("oxvba init: {err}");
        std::process::exit(1);
    });
    fs::write(&module_path, module_content).unwrap_or_else(|err| {
        eprintln!("oxvba init: {err}");
        std::process::exit(1);
    });

    println!("created {} + Module1.bas", basproj_path.display());
}

// ---------------------------------------------------------------------------
// import-vbp subcommand
// ---------------------------------------------------------------------------

fn run_import_vbp(args: Vec<String>) {
    let mut iter = args.into_iter();
    let _ = iter.next(); // "import-vbp"

    let input_path = iter.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: oxvba import-vbp <input.vbp> [-o <output.basproj>]");
        std::process::exit(2);
    });

    let mut output_path: Option<PathBuf> = None;
    let collected: Vec<String> = iter.collect();
    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                output_path = collected.get(i).map(PathBuf::from);
            }
            _ => {}
        }
        i += 1;
    }

    let content = fs::read_to_string(&input_path).unwrap_or_else(|err| {
        eprintln!(
            "oxvba import-vbp: cannot read {}: {err}",
            input_path.display()
        );
        std::process::exit(1);
    });

    let basproj = oxvba_project::vbp::parse_vbp(&content).unwrap_or_else(|err| {
        eprintln!("oxvba import-vbp: parse failed: {err}");
        std::process::exit(1);
    });

    let xml = oxvba_project::vbp::generate_basproj_from_vbp(&basproj);

    let out = output_path.unwrap_or_else(|| input_path.with_extension("basproj"));

    fs::write(&out, &xml).unwrap_or_else(|err| {
        eprintln!("oxvba import-vbp: cannot write {}: {err}", out.display());
        std::process::exit(1);
    });

    println!("imported {} → {}", input_path.display(), out.display());
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn discover_basproj(dir: &str) -> Option<PathBuf> {
    let dir = std::path::Path::new(dir);
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("basproj") {
            return Some(path);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Existing compile subcommand
// ---------------------------------------------------------------------------

fn run_compile(args: Vec<String>) {
    let compile_args = parse_compile_args(args).unwrap_or_else(|| {
        eprintln!("usage: oxvba compile <input.vb> [-o <output.oxb>]");
        std::process::exit(2);
    });

    let source = fs::read_to_string(&compile_args.input_path).unwrap_or_else(|err| {
        eprintln!(
            "oxvba: cannot read {}: {err}",
            compile_args.input_path.display()
        );
        std::process::exit(1);
    });

    let (bytecode, metadata) = oxvba_compiler::compile_with_runtime_metadata(&source)
        .unwrap_or_else(|err| {
            eprintln!("oxvba: compile failed: {err}");
            std::process::exit(1);
        });

    let bundle = oxvba_compiler::OxBundle::new(bytecode, metadata);
    let bytes = bundle.serialize_to_bytes().unwrap_or_else(|err| {
        eprintln!("oxvba: bundle serialization failed: {err}");
        std::process::exit(1);
    });

    let output_path = compile_args
        .output_path
        .unwrap_or_else(|| compile_args.input_path.with_extension("oxb"));

    fs::write(&output_path, &bytes).unwrap_or_else(|err| {
        eprintln!("oxvba: cannot write {}: {err}", output_path.display());
        std::process::exit(1);
    });

    println!(
        "compiled {} → {} ({} bytes)",
        compile_args.input_path.display(),
        output_path.display(),
        bytes.len()
    );
}

fn run_execute(cli_args: Vec<String>) {
    let args = parse_run_args_from(cli_args);
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
            .map(|values| ExecutionResult { values })
    } else {
        engine
            .execute_source_with_snapshot(&source)
            .map(|values| ExecutionResult { values })
    };

    match execution {
        Ok(result) => {
            if dump_slots {
                let payload = result
                    .values
                    .iter()
                    .map(|value| value.to_legacy_i32().unwrap_or(EMPTY_TAG).to_string())
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
    values: Vec<RuntimeValue>,
}

#[derive(Debug)]
struct CompileArgs {
    input_path: PathBuf,
    output_path: Option<PathBuf>,
}

fn parse_compile_args(args: Vec<String>) -> Option<CompileArgs> {
    let mut iter = args.into_iter();
    let _cmd = iter.next()?; // "compile"

    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;

    let collected: Vec<String> = iter.collect();
    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                output_path = Some(PathBuf::from(collected.get(i)?));
            }
            arg if !arg.starts_with('-') && input_path.is_none() => {
                input_path = Some(PathBuf::from(arg));
            }
            _ => return None,
        }
        i += 1;
    }

    Some(CompileArgs {
        input_path: input_path?,
        output_path,
    })
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
        RuntimeValue::I64(value) => format!("i64:{value}"),
        RuntimeValue::F64(value) => format!("f64:{}", value.as_f64()),
        RuntimeValue::Decimal(value) => format!("decimal:{}", value),
        RuntimeValue::Currency(value) => format!("currency:{}", value),
        RuntimeValue::Bool(value) => format!("bool:{value}"),
        RuntimeValue::String(BStr(value)) => format!("string:{value:?}"),
        RuntimeValue::ArrayIntent(array) => format!("array:{array:?}"),
        RuntimeValue::ObjectHandle(handle) => format!("object:{handle}"),
        RuntimeValue::BindingHandle(handle) => format!("binding:{handle}"),
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
