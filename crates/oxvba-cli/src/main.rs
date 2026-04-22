use oxvba_com::{TypeLibResolveRequest, resolve_known_typelib_identity};
use oxvba_compiler::{ProjectReference, ReferenceKind, ReferencedProjectManifest};
use oxvba_hal::model::{
    HalRuntimeClass, UiVirtualizationMode, UnsupportedFeatureMode, WasmRuntimeClass,
};
use oxvba_host::{
    Engine, HostConfig, ImmediateEvaluationOutput, ImmediateEvaluationRequest, ImmediateSession,
    RunnerBootstrapFallbacks, RunnerBootstrapOptions, TypeLibraryCatalogEntry,
    resolve_runner_bootstrap, resolve_runner_bootstrap_with_fallbacks,
};
use oxvba_runtime::{RuntimeValue, value_tags::EMPTY_TAG};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    let cli_args: Vec<String> = env::args().skip(1).collect();
    let subcommand = cli_args.first().map(|s| s.as_str());

    match subcommand {
        Some("compile") => run_compile(cli_args),
        Some("build") => run_build(cli_args),
        Some("com-ref") => run_com_ref(cli_args),
        Some("repl") | Some("immediate") => run_immediate(cli_args),
        Some("run-project") => run_project(cli_args),
        Some("explain") | Some("host-check") => run_explain(cli_args),
        Some("init") => run_init(cli_args),
        Some("import-vbp") => run_import_vbp(cli_args),
        _ => run_execute(cli_args),
    }
}

// ---------------------------------------------------------------------------
// build subcommand: project target -> compile -> .oxb
// ---------------------------------------------------------------------------

fn run_build(args: Vec<String>) {
    let parsed = parse_build_args(args).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba build [path] [-o <output.oxb>] [--project-ref <path>] [--com-ref <lib-or-lib=importlib>] [--native-ref <path>]"
        );
        std::process::exit(2);
    });

    let input = parsed.input_path.unwrap_or_else(|| PathBuf::from("."));

    let mut loaded = load_run_project_target(Some(input.clone())).unwrap_or_else(|err| {
        eprintln!("oxvba build: {err}");
        std::process::exit(1);
    });
    apply_cli_reference_overrides(&mut loaded, &parsed.references).unwrap_or_else(|err| {
        eprintln!("oxvba build: {err}");
        std::process::exit(2);
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

    let out = parsed
        .output_path
        .unwrap_or_else(|| default_build_output_path(&input, &loaded));

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
    let parsed = parse_run_project_args_from(args).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba run-project [path] [--entry <Module.Procedure>] [runtime/bootstrap options]"
        );
        std::process::exit(2);
    });

    let mut loaded = load_run_project_target(parsed.input_path).unwrap_or_else(|err| {
        eprintln!("oxvba run-project: {err}");
        std::process::exit(1);
    });
    if let Some(entry_point) = parsed.entry_point_override.as_deref() {
        oxvba_project::override_loaded_project_entry_point(&mut loaded, entry_point)
            .unwrap_or_else(|err| {
                eprintln!("oxvba run-project: {err}");
                std::process::exit(1);
            });
    }
    apply_cli_reference_overrides(&mut loaded, &parsed.references).unwrap_or_else(|err| {
        eprintln!("oxvba run-project: {err}");
        std::process::exit(2);
    });

    let config = HostConfig {
        enable_jit: parsed.enable_jit,
        root_object_name: Some(loaded.default_root_object.clone()),
    };
    let mut engine = Engine::new(config);

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

    let result = engine.execute_project_with_snapshot_phased(&loaded.manifest);

    match result {
        Ok(values) => {
            if parsed.dump_slots {
                let payload = values
                    .iter()
                    .map(|v| v.project_compat_slot_i32().unwrap_or(EMPTY_TAG).to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                println!("SLOTS:{payload}");
            }
            if parsed.dump_values {
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

fn run_immediate(args: Vec<String>) {
    let parsed = parse_immediate_args_from(args).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba repl|immediate [path] [--module <ModuleName>] [--project-ref <path>] [--com-ref <lib-or-lib=importlib>] [--native-ref <path>] [runtime/bootstrap options]"
        );
        std::process::exit(2);
    });

    let mut loaded = load_run_project_target(parsed.input_path).unwrap_or_else(|err| {
        eprintln!("oxvba repl: {err}");
        std::process::exit(1);
    });
    apply_cli_reference_overrides(&mut loaded, &parsed.references).unwrap_or_else(|err| {
        eprintln!("oxvba repl: {err}");
        std::process::exit(2);
    });

    let config = HostConfig {
        enable_jit: false,
        root_object_name: Some(loaded.default_root_object.clone()),
    };
    let mut engine = Engine::new(config);
    let resolved =
        resolve_project_runner_bootstrap(&loaded, &parsed.bootstrap, |key| env::var(key).ok())
            .unwrap_or_else(|err| {
                eprintln!("oxvba repl: bootstrap failed: {err}");
                std::process::exit(2);
            });
    engine.set_runtime_profile(resolved.runtime_profile);
    engine.set_host_policy(resolved.policy.clone());
    if parsed.dump_bootstrap {
        println!("BOOTSTRAP:{}", resolved.fingerprint());
    }

    let mut session = engine
        .prepare_immediate_session(&loaded.manifest)
        .unwrap_or_else(|err| {
            eprintln!("oxvba repl: {err}");
            std::process::exit(1);
        });
    session.set_default_target_module(parsed.default_module);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    let mut errors = stderr.lock();
    if let Err(err) = run_immediate_shell(&mut session, &mut input, &mut output, &mut errors) {
        eprintln!("oxvba repl: {err}");
        std::process::exit(1);
    }
}

#[derive(Debug, Clone)]
struct RunProjectArgs {
    input_path: Option<PathBuf>,
    enable_jit: bool,
    dump_values: bool,
    dump_slots: bool,
    dump_bootstrap: bool,
    bootstrap: RunnerBootstrapOptions,
    entry_point_override: Option<String>,
    references: CliReferenceArgs,
}

#[derive(Debug, Clone)]
struct ImmediateArgs {
    input_path: Option<PathBuf>,
    dump_bootstrap: bool,
    bootstrap: RunnerBootstrapOptions,
    default_module: Option<String>,
    references: CliReferenceArgs,
}

fn parse_run_project_args_from(args: Vec<String>) -> Option<RunProjectArgs> {
    let mut iter = args.into_iter();
    let cmd = iter.next()?;
    if cmd != "run-project" {
        return None;
    }

    let mut input_path: Option<PathBuf> = None;
    let mut enable_jit = false;
    let mut dump_values = false;
    let mut dump_slots = false;
    let mut dump_bootstrap = false;
    let mut bootstrap = RunnerBootstrapOptions::default();
    let mut entry_point_override: Option<String> = None;
    let mut references = CliReferenceArgs::default();

    let collected: Vec<String> = iter.collect();
    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "--jit" => enable_jit = true,
            "--dump-values" => dump_values = true,
            "--dump-slots" => dump_slots = true,
            "--dump-bootstrap" => dump_bootstrap = true,
            "--entry" => {
                i += 1;
                entry_point_override = Some(collected.get(i)?.clone());
            }
            "--config" => {
                i += 1;
                bootstrap.config_path = Some(PathBuf::from(collected.get(i)?.as_str()));
            }
            "--profile" => {
                i += 1;
                bootstrap.profile = Some(collected.get(i)?.clone());
            }
            "--policy" => {
                i += 1;
                bootstrap.policy_preset = Some(collected.get(i)?.clone());
            }
            "--runtime-class" => {
                i += 1;
                bootstrap.overrides.runtime_class = Some(parse_runtime_class(collected.get(i)?)?);
            }
            "--allow-interaction" => {
                i += 1;
                bootstrap.overrides.allow_interaction = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-process-spawn" => {
                i += 1;
                bootstrap.overrides.allow_process_spawn = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-filesystem-mutation" => {
                i += 1;
                bootstrap.overrides.allow_filesystem_mutation =
                    Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-dynamic-link" => {
                i += 1;
                bootstrap.overrides.allow_dynamic_link = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-com-activation" => {
                i += 1;
                bootstrap.overrides.allow_com_activation = Some(parse_bool(collected.get(i)?)?);
            }
            "--deterministic-mode" => {
                i += 1;
                bootstrap.overrides.deterministic_mode = Some(parse_bool(collected.get(i)?)?);
            }
            "--ui-virtualization" => {
                i += 1;
                bootstrap.overrides.ui_virtualization =
                    Some(parse_ui_virtualization(collected.get(i)?)?);
            }
            "--unsupported-mode" => {
                i += 1;
                bootstrap.overrides.unsupported_feature_mode =
                    Some(parse_unsupported_mode(collected.get(i)?)?);
            }
            "--wasm-runtime-class" => {
                i += 1;
                bootstrap.overrides.wasm_runtime_class =
                    Some(parse_wasm_runtime_class(collected.get(i)?)?);
            }
            "--project-ref" => {
                i += 1;
                references
                    .project_refs
                    .push(PathBuf::from(collected.get(i)?.as_str()));
            }
            "--com-ref" => {
                i += 1;
                references
                    .com_refs
                    .push(parse_cli_com_reference(collected.get(i)?)?);
            }
            "--native-ref" => {
                i += 1;
                references
                    .native_refs
                    .push(oxvba_project::BasProjNativeReference {
                        include: collected.get(i)?.clone(),
                        path: Some(collected.get(i)?.clone()),
                    });
            }
            arg if !arg.starts_with('-') && input_path.is_none() => {
                input_path = Some(PathBuf::from(arg));
            }
            _ => return None,
        }
        i += 1;
    }

    Some(RunProjectArgs {
        input_path,
        enable_jit,
        dump_values,
        dump_slots,
        dump_bootstrap,
        bootstrap,
        entry_point_override,
        references,
    })
}

fn parse_immediate_args_from(args: Vec<String>) -> Option<ImmediateArgs> {
    let mut iter = args.into_iter();
    let cmd = iter.next()?;
    if cmd != "repl" && cmd != "immediate" {
        return None;
    }

    let collected: Vec<String> = iter.collect();
    let mut input_path: Option<PathBuf> = None;
    let mut dump_bootstrap = false;
    let mut bootstrap = RunnerBootstrapOptions::default();
    let mut default_module: Option<String> = None;
    let mut references = CliReferenceArgs::default();

    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "--dump-bootstrap" => dump_bootstrap = true,
            "--module" => {
                i += 1;
                default_module = collected.get(i).cloned();
            }
            "--config" => {
                i += 1;
                bootstrap.config_path = Some(PathBuf::from(collected.get(i)?.as_str()));
            }
            "--profile" => {
                i += 1;
                bootstrap.profile = Some(collected.get(i)?.clone());
            }
            "--policy" => {
                i += 1;
                bootstrap.policy_preset = Some(collected.get(i)?.clone());
            }
            "--runtime-class" => {
                i += 1;
                bootstrap.overrides.runtime_class = Some(parse_runtime_class(collected.get(i)?)?);
            }
            "--allow-interaction" => {
                i += 1;
                bootstrap.overrides.allow_interaction = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-process-spawn" => {
                i += 1;
                bootstrap.overrides.allow_process_spawn = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-filesystem-mutation" => {
                i += 1;
                bootstrap.overrides.allow_filesystem_mutation =
                    Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-dynamic-link" => {
                i += 1;
                bootstrap.overrides.allow_dynamic_link = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-com-activation" => {
                i += 1;
                bootstrap.overrides.allow_com_activation = Some(parse_bool(collected.get(i)?)?);
            }
            "--deterministic-mode" => {
                i += 1;
                bootstrap.overrides.deterministic_mode = Some(parse_bool(collected.get(i)?)?);
            }
            "--ui-virtualization" => {
                i += 1;
                bootstrap.overrides.ui_virtualization =
                    Some(parse_ui_virtualization(collected.get(i)?)?);
            }
            "--unsupported-mode" => {
                i += 1;
                bootstrap.overrides.unsupported_feature_mode =
                    Some(parse_unsupported_mode(collected.get(i)?)?);
            }
            "--wasm-runtime-class" => {
                i += 1;
                bootstrap.overrides.wasm_runtime_class =
                    Some(parse_wasm_runtime_class(collected.get(i)?)?);
            }
            "--project-ref" => {
                i += 1;
                references
                    .project_refs
                    .push(PathBuf::from(collected.get(i)?));
            }
            "--com-ref" => {
                i += 1;
                references
                    .com_refs
                    .push(parse_cli_com_reference(collected.get(i)?)?);
            }
            "--native-ref" => {
                i += 1;
                references
                    .native_refs
                    .push(oxvba_project::BasProjNativeReference {
                        include: collected.get(i)?.clone(),
                        path: Some(collected.get(i)?.clone()),
                    });
            }
            value if !value.starts_with('-') && input_path.is_none() => {
                input_path = Some(PathBuf::from(value));
            }
            _ => return None,
        }
        i += 1;
    }

    Some(ImmediateArgs {
        input_path,
        dump_bootstrap,
        bootstrap,
        default_module,
        references,
    })
}

fn run_immediate_shell<R: BufRead, W: Write, E: Write>(
    session: &mut ImmediateSession<'_>,
    input: &mut R,
    out: &mut W,
    err: &mut E,
) -> io::Result<()> {
    writeln!(
        out,
        "OxVba Immediate Window (bounded v1). Use .help for commands, .quit to exit."
    )?;
    loop {
        write!(out, "immediate> ")?;
        out.flush()?;

        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            writeln!(out)?;
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.eq_ignore_ascii_case(".quit") || trimmed.eq_ignore_ascii_case(".exit") {
            break;
        }

        if trimmed.eq_ignore_ascii_case(".help") {
            writeln!(out, ".help                show this help")?;
            writeln!(out, ".quit | .exit        leave the shell")?;
            writeln!(out, ".module              show the current default module")?;
            writeln!(
                out,
                ".module <name>       set the default module for unqualified calls"
            )?;
            writeln!(out, "reset                reset the live runtime session")?;
            writeln!(out, "? Proc(1)            invoke and print a return value")?;
            writeln!(out, "Call Proc(1)         invoke as a statement")?;
            continue;
        }

        if trimmed.eq_ignore_ascii_case(".module") {
            match session.default_target_module() {
                Some(module) => writeln!(out, "module: {module}")?,
                None => writeln!(out, "module: <none>")?,
            }
            continue;
        }

        if let Some(module_name) = trimmed.strip_prefix(".module ") {
            let module_name = module_name.trim();
            if module_name.is_empty() {
                writeln!(err, "immediate: module name cannot be empty")?;
            } else {
                session.set_default_target_module(Some(module_name.to_string()));
                writeln!(out, "module: {module_name}")?;
            }
            continue;
        }

        match session.evaluate(&ImmediateEvaluationRequest::new(trimmed)) {
            Ok(result) => {
                for diagnostic in result.diagnostics {
                    writeln!(err, "immediate: {diagnostic}")?;
                }
                match result.output {
                    ImmediateEvaluationOutput::Empty => {}
                    ImmediateEvaluationOutput::Value(value) => {
                        writeln!(out, "{}", value.display_text)?;
                    }
                    ImmediateEvaluationOutput::PrintedLine(line) => {
                        writeln!(out, "{line}")?;
                    }
                    ImmediateEvaluationOutput::Reset => {
                        writeln!(out, "reset")?;
                    }
                }
            }
            Err(err_value) => writeln!(err, "immediate: {err_value}")?,
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Default)]
struct CliReferenceArgs {
    project_refs: Vec<PathBuf>,
    com_refs: Vec<CliComReference>,
    native_refs: Vec<oxvba_project::BasProjNativeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliComReference {
    library_name: String,
    importlib: Option<String>,
}

#[derive(Debug, Clone)]
struct BuildArgs {
    input_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    references: CliReferenceArgs,
}

#[derive(Debug, Clone)]
struct ComRefQueryArgs {
    reference_name: Option<String>,
    prog_id: Option<String>,
    carrier_path: Option<PathBuf>,
    coclass: Option<String>,
    include_override: Option<String>,
    reference_include: Option<String>,
}

#[derive(Debug, Clone)]
enum ComRefCommand {
    List,
    Add,
    Repair,
}

#[derive(Debug, Clone)]
struct ComRefArgs {
    command: ComRefCommand,
    target_path: Option<PathBuf>,
    query: ComRefQueryArgs,
}

#[derive(Debug, Clone)]
struct ExplainArgs {
    input_path: Option<PathBuf>,
    bootstrap: RunnerBootstrapOptions,
    entry_point_override: Option<String>,
    references: CliReferenceArgs,
}

// ---------------------------------------------------------------------------
// init subcommand
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitKind {
    Application,
    Library,
    Addin,
    HostModule,
    ComServer,
    ComExe,
}

#[derive(Debug, Clone)]
struct InitArgs {
    target_dir: PathBuf,
    kind: InitKind,
    from_convention: bool,
}

fn parse_init_args_from(args: Vec<String>) -> Option<InitArgs> {
    let mut iter = args.into_iter();
    let cmd = iter.next()?;
    if cmd != "init" {
        return None;
    }

    let collected: Vec<String> = iter.collect();
    let mut target_dir: Option<PathBuf> = None;
    let mut kind = InitKind::Application;
    let mut from_convention = false;

    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "--kind" => {
                i += 1;
                kind = parse_init_kind(collected.get(i)?)?;
            }
            "--from-convention" => from_convention = true,
            arg if !arg.starts_with('-') && target_dir.is_none() => {
                target_dir = Some(PathBuf::from(arg));
            }
            _ => return None,
        }
        i += 1;
    }

    Some(InitArgs {
        target_dir: target_dir
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        kind,
        from_convention,
    })
}

fn parse_init_kind(value: &str) -> Option<InitKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "application" | "app" | "exe" => Some(InitKind::Application),
        "library" | "lib" => Some(InitKind::Library),
        "addin" | "add-in" => Some(InitKind::Addin),
        "host-module" | "hostmodule" | "host" => Some(InitKind::HostModule),
        "com-server" | "comserver" => Some(InitKind::ComServer),
        "com-exe" | "comexe" => Some(InitKind::ComExe),
        _ => None,
    }
}

fn init_output_type(kind: InitKind) -> &'static str {
    match kind {
        InitKind::Application => "Exe",
        InitKind::Library => "Library",
        InitKind::Addin => "Addin",
        InitKind::HostModule => "HostModule",
        InitKind::ComServer => "ComServer",
        InitKind::ComExe => "ComExe",
    }
}

fn init_property_group(kind: InitKind, project_name: &str) -> String {
    let mut property_group = format!(
        "  <PropertyGroup>\n    <OutputType>{}</OutputType>\n    <ProjectName>{}</ProjectName>\n",
        init_output_type(kind),
        project_name
    );
    if matches!(kind, InitKind::Application) {
        property_group.push_str("    <EntryPoint>Module1.Main</EntryPoint>\n");
    }
    if matches!(kind, InitKind::HostModule) {
        property_group.push_str("    <DefaultRootObject>Application</DefaultRootObject>\n");
    }
    property_group.push_str("  </PropertyGroup>\n");
    property_group
}

fn init_module_content(kind: InitKind) -> &'static str {
    match kind {
        InitKind::Application => {
            "Attribute VB_Name = \"Module1\"\n\nPublic Sub Main()\n    ' Your code here\nEnd Sub\n"
        }
        InitKind::Library => {
            "Attribute VB_Name = \"Module1\"\n\nPublic Function ExampleValue() As Long\n    ExampleValue = 42\nEnd Function\n"
        }
        InitKind::Addin => {
            "Attribute VB_Name = \"Module1\"\n\nPublic Sub RegisterAddin()\n    ' Add-in initialization entrypoints are host-specific.\nEnd Sub\n"
        }
        InitKind::HostModule => {
            "Attribute VB_Name = \"Module1\"\n\nPublic Sub Warmup()\n    ' Host modules are loaded by a host root object, not started by Main.\nEnd Sub\n"
        }
        InitKind::ComServer | InitKind::ComExe => {
            "Attribute VB_Name = \"Class1\"\nOption Explicit\n\nPublic Function Ping() As Long\n    Ping = 42\nEnd Function\n"
        }
    }
}

fn init_primary_file_name(kind: InitKind) -> &'static str {
    match kind {
        InitKind::ComServer | InitKind::ComExe => "Class1.cls",
        _ => "Module1.bas",
    }
}

fn init_item_group(kind: InitKind, project_name: &str) -> String {
    match kind {
        InitKind::ComServer | InitKind::ComExe => format!(
            "  <ItemGroup>\n    <ClassModule Include=\"Class1.cls\">\n      <VBExposed>True</VBExposed>\n      <VBCreatable>True</VBCreatable>\n      <Instancing>MultiUse</Instancing>\n      <ProgId>{}.Class1</ProgId>\n    </ClassModule>\n  </ItemGroup>\n",
            project_name
        ),
        _ => "  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n".to_string(),
    }
}

fn run_init(args: Vec<String>) {
    let parsed = parse_init_args_from(args).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba init [path] [--kind <application|library|addin|host-module|com-server|com-exe>] [--from-convention]"
        );
        std::process::exit(2);
    });
    let target_dir = parsed.target_dir;

    if parsed.from_convention {
        run_init_from_convention(&target_dir);
        return;
    }

    let project_name = target_dir
        .file_name()
        .map(|name| oxvba_project::infer_project_name_from_path(Path::new(name)))
        .unwrap_or_else(|| "NewProject".to_string());

    fs::create_dir_all(&target_dir).unwrap_or_else(|err| {
        eprintln!("oxvba init: cannot create directory: {err}");
        std::process::exit(1);
    });

    let basproj_content = format!(
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n{}{}</Project>\n",
        init_property_group(parsed.kind, &project_name),
        init_item_group(parsed.kind, &project_name),
    );

    let module_content = init_module_content(parsed.kind);
    let primary_file_name = init_primary_file_name(parsed.kind);

    let basproj_path = target_dir.join(format!("{project_name}.basproj"));
    let module_path = target_dir.join(primary_file_name);

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

    println!(
        "created {} + {} ({})",
        basproj_path.display(),
        primary_file_name,
        init_output_type(parsed.kind)
    );
}

fn run_init_from_convention(target_dir: &Path) {
    if !target_dir.exists() || !target_dir.is_dir() {
        eprintln!(
            "oxvba init: convention source directory does not exist: {}",
            target_dir.display()
        );
        std::process::exit(1);
    }
    if discover_basproj_in_dir(target_dir)
        .unwrap_or(None)
        .is_some()
        || discover_vbp_in_dir(target_dir).unwrap_or(None).is_some()
    {
        eprintln!(
            "oxvba init: {} already contains a project file; use the existing project instead",
            target_dir.display()
        );
        std::process::exit(1);
    }

    let loaded = load_convention_project(target_dir).unwrap_or_else(|err| {
        eprintln!("oxvba init: {err}");
        std::process::exit(1);
    });
    let basproj_path = target_dir.join(format!("{}.basproj", loaded.manifest.project_name));
    if basproj_path.exists() {
        eprintln!("oxvba init: {} already exists", basproj_path.display());
        std::process::exit(1);
    }
    let xml = oxvba_project::generate_basproj_xml(
        &loaded.manifest,
        loaded.output_type,
        Some(loaded.build_target),
        loaded.entry_point.as_deref(),
        Some(loaded.runtime_flavor),
        loaded.default_runtime_profile.as_deref(),
        loaded.default_policy_preset.as_deref(),
        Some(loaded.default_root_object.as_str()),
        &loaded.type_library_catalog,
        &loaded.native_exports,
        &loaded.class_module_metadata,
    );
    fs::write(&basproj_path, xml).unwrap_or_else(|err| {
        eprintln!("oxvba init: {err}");
        std::process::exit(1);
    });
    println!(
        "captured convention project {} → {}",
        target_dir.display(),
        basproj_path.display()
    );
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

    let xml = oxvba_project::vbp::generate_basproj_from_vbp(&basproj).unwrap_or_else(|err| {
        eprintln!("oxvba import-vbp: {err}");
        std::process::exit(1);
    });

    let out = output_path.unwrap_or_else(|| input_path.with_extension("basproj"));

    fs::write(&out, &xml).unwrap_or_else(|err| {
        eprintln!("oxvba import-vbp: cannot write {}: {err}", out.display());
        std::process::exit(1);
    });

    println!("imported {} → {}", input_path.display(), out.display());
}

fn run_com_ref(args: Vec<String>) {
    let parsed = parse_com_ref_args(args).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba com-ref <list|add|repair> [path] [--name <library> | --progid <progid> | --file <carrier>] [--coclass <name>] [--include <logical-name>] [--reference <active-include>]"
        );
        std::process::exit(2);
    });

    let service = oxvba_project::ComSelectionService;
    let target_path = parsed
        .target_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let discovered = discover_com_ref_candidates(&service, &parsed).unwrap_or_else(|err| {
        eprintln!("oxvba com-ref: {err}");
        std::process::exit(1);
    });

    match parsed.command {
        ComRefCommand::List => {
            let surface = service
                .inspect_workspace_project_state(&target_path, &discovered)
                .unwrap_or_else(|err| {
                    eprintln!("oxvba com-ref: {err}");
                    std::process::exit(1);
                });
            print_com_ref_surface(&surface, &discovered);
        }
        ComRefCommand::Add => {
            let project_file = resolve_mutable_basproj_target(&target_path).unwrap_or_else(|err| {
                eprintln!("oxvba com-ref: {err}");
                std::process::exit(1);
            });
            let candidate = choose_single_com_candidate(&discovered).unwrap_or_else(|err| {
                eprintln!("oxvba com-ref: {err}");
                std::process::exit(2);
            });
            let plan =
                service.plan_add_candidate(&candidate, parsed.query.include_override.as_deref());
            oxvba_project::apply_host_project_edits_to_basproj_path(&project_file, &plan.edits)
                .unwrap_or_else(|err| {
                    eprintln!("oxvba com-ref: {err}");
                    std::process::exit(1);
                });
            println!(
                "added COM reference `{}` to {}",
                plan.include,
                project_file.display()
            );
        }
        ComRefCommand::Repair => {
            let project_file = resolve_mutable_basproj_target(&target_path).unwrap_or_else(|err| {
                eprintln!("oxvba com-ref: {err}");
                std::process::exit(1);
            });
            let candidate = choose_single_com_candidate(&discovered).unwrap_or_else(|err| {
                eprintln!("oxvba com-ref: {err}");
                std::process::exit(2);
            });
            let surface = service
                .inspect_workspace_project_state(&project_file, &discovered)
                .unwrap_or_else(|err| {
                    eprintln!("oxvba com-ref: {err}");
                    std::process::exit(1);
                });
            let reference_include = parsed
                .query
                .reference_include
                .as_deref()
                .expect("repair parser requires --reference");
            let selection = surface
                .selections
                .iter()
                .find(|selection| {
                    selection
                        .reference
                        .include
                        .eq_ignore_ascii_case(reference_include)
                })
                .unwrap_or_else(|| {
                    eprintln!(
                        "oxvba com-ref: active COM reference `{reference_include}` was not found in {}",
                        project_file.display()
                    );
                    std::process::exit(1);
                });
            let plan = service.plan_repair_selection(selection, &candidate);
            oxvba_project::apply_host_project_edits_to_basproj_path(&project_file, &plan.edits)
                .unwrap_or_else(|err| {
                    eprintln!("oxvba com-ref: {err}");
                    std::process::exit(1);
                });
            println!(
                "repaired COM reference `{}` in {}",
                plan.include,
                project_file.display()
            );
        }
    }
}

fn parse_com_ref_args(args: Vec<String>) -> Option<ComRefArgs> {
    let mut iter = args.into_iter();
    let cmd = iter.next()?;
    if cmd != "com-ref" {
        return None;
    }
    let subcommand = iter.next()?;
    let command = match subcommand.as_str() {
        "list" => ComRefCommand::List,
        "add" => ComRefCommand::Add,
        "repair" => ComRefCommand::Repair,
        _ => return None,
    };

    let collected: Vec<String> = iter.collect();
    let mut target_path: Option<PathBuf> = None;
    let mut query = ComRefQueryArgs {
        reference_name: None,
        prog_id: None,
        carrier_path: None,
        coclass: None,
        include_override: None,
        reference_include: None,
    };

    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "--name" => {
                i += 1;
                query.reference_name = Some(collected.get(i)?.clone());
            }
            "--progid" => {
                i += 1;
                query.prog_id = Some(collected.get(i)?.clone());
            }
            "--file" => {
                i += 1;
                query.carrier_path = Some(PathBuf::from(collected.get(i)?.as_str()));
            }
            "--coclass" => {
                i += 1;
                query.coclass = Some(collected.get(i)?.clone());
            }
            "--include" => {
                i += 1;
                query.include_override = Some(collected.get(i)?.clone());
            }
            "--reference" => {
                i += 1;
                query.reference_include = Some(collected.get(i)?.clone());
            }
            value if !value.starts_with('-') && target_path.is_none() => {
                target_path = Some(PathBuf::from(value));
            }
            _ => return None,
        }
        i += 1;
    }

    let selector_count = usize::from(query.reference_name.is_some())
        + usize::from(query.prog_id.is_some())
        + usize::from(query.carrier_path.is_some());
    match command {
        ComRefCommand::List => {
            if selector_count > 1 {
                return None;
            }
            if query.include_override.is_some() || query.reference_include.is_some() {
                return None;
            }
        }
        ComRefCommand::Add => {
            if selector_count != 1 || query.reference_include.is_some() {
                return None;
            }
        }
        ComRefCommand::Repair => {
            if selector_count != 1 || query.reference_include.is_none() {
                return None;
            }
        }
    }

    Some(ComRefArgs {
        command,
        target_path,
        query,
    })
}

fn discover_com_ref_candidates(
    service: &oxvba_project::ComSelectionService,
    parsed: &ComRefArgs,
) -> Result<Vec<oxvba_project::ComSelectionCandidate>, String> {
    if let Some(reference_name) = parsed.query.reference_name.as_deref() {
        let query = oxvba_project::RegisteredComSelectionQuery {
            reference_name: reference_name.to_string(),
            requested_coclass: parsed.query.coclass.clone(),
            import_lib: None,
            guid: None,
            version_major: None,
            version_minor: None,
            lcid: None,
        };
        return service
            .discover_registered_candidates(&query)
            .map_err(|err| err.to_string());
    }

    if let Some(prog_id) = parsed.query.prog_id.as_deref() {
        return service
            .discover_prog_id_candidates(prog_id)
            .map_err(|err| err.to_string());
    }

    if let Some(carrier_path) = parsed.query.carrier_path.as_ref() {
        let query = oxvba_project::FileBackedComSelectionQuery {
            carrier_path: carrier_path.clone(),
            reference_name: None,
            requested_coclass: parsed.query.coclass.clone(),
        };
        return service
            .discover_file_backed_candidates(&query)
            .map_err(|err| err.to_string());
    }

    Ok(Vec::new())
}

fn choose_single_com_candidate(
    candidates: &[oxvba_project::ComSelectionCandidate],
) -> Result<oxvba_project::ComSelectionCandidate, String> {
    match candidates {
        [] => Err("no COM selection candidates matched the query".to_string()),
        [single] => Ok(single.clone()),
        many => {
            let strongest = many
                .iter()
                .map(|candidate| candidate.confidence)
                .min()
                .expect("non-empty candidates");
            let best = many
                .iter()
                .filter(|candidate| candidate.confidence == strongest)
                .cloned()
                .collect::<Vec<_>>();
            if best.len() == 1 {
                Ok(best[0].clone())
            } else {
                Err(format!(
                    "COM selection query is ambiguous; refine it with --progid, --file, or --coclass ({} candidates matched)",
                    many.len()
                ))
            }
        }
    }
}

fn resolve_mutable_basproj_target(path: &Path) -> Result<PathBuf, String> {
    let surface = oxvba_project::inspect_workspace_target(path).map_err(|err| err.to_string())?;
    if surface.workspace_kind != oxvba_project::HostWorkspaceTargetKind::BasProj {
        return Err(format!(
            "COM reference edits require a real .basproj target; `{}` resolved to {:?}",
            path.display(),
            surface.workspace_kind
        ));
    }
    surface.project_file.ok_or_else(|| {
        format!(
            "COM reference edits require a concrete .basproj file for `{}`",
            path.display()
        )
    })
}

fn print_com_ref_surface(
    surface: &oxvba_project::HostComProjectSelectionSurface,
    discovered: &[oxvba_project::ComSelectionCandidate],
) {
    println!("project: {}", surface.project_name);
    println!("workspace-kind: {:?}", surface.workspace_kind);
    if let Some(project_file) = surface.project_file.as_ref() {
        println!("project-file: {}", project_file.display());
    }
    println!("active-com-references:");
    if surface.selections.is_empty() {
        println!("  - <none>");
    } else {
        for (index, selection) in surface.selections.iter().enumerate() {
            println!(
                "  {}. {} [{}]",
                index + 1,
                selection.reference.include,
                com_selection_status_name(&selection.status)
            );
            if let Some(guid) = selection.reference.guid.as_deref() {
                println!("     guid: {guid}");
            }
            if let (Some(major), Some(minor)) = (
                selection.reference.version_major,
                selection.reference.version_minor,
            ) {
                println!("     version: {major}.{minor}");
            }
            if let Some(import_lib) = selection.reference.import_lib.as_deref() {
                println!("     importlib: {import_lib}");
            }
        }
    }

    if !discovered.is_empty() {
        println!("discovered-candidates:");
        for (index, candidate) in discovered.iter().enumerate() {
            println!(
                "  {}. {} ({:?}, {:?})",
                index + 1,
                candidate.identity.library_name,
                candidate.source_kind,
                candidate.confidence
            );
            if let Some(description) = candidate.friendly_description.as_deref() {
                println!("     description: {description}");
            }
            if let Some(guid) = candidate.identity.guid.as_deref() {
                println!("     guid: {guid}");
            }
            if let (Some(major), Some(minor)) = (
                candidate.identity.version_major,
                candidate.identity.version_minor,
            ) {
                println!("     version: {major}.{minor}");
            }
            if let Some(import_lib) = candidate.identity.import_lib.as_deref() {
                println!("     importlib: {import_lib}");
            }
            if !candidate.prog_ids.is_empty() {
                println!("     progids: {}", candidate.prog_ids.join(", "));
            }
        }
    }
}

fn com_selection_status_name(status: &oxvba_project::ComProjectSelectionStatus) -> &'static str {
    match status {
        oxvba_project::ComProjectSelectionStatus::ResolvedUnique { .. } => "resolved",
        oxvba_project::ComProjectSelectionStatus::Ambiguous { .. } => "ambiguous",
        oxvba_project::ComProjectSelectionStatus::Missing => "missing",
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn discover_basproj_in_dir(dir: &Path) -> Result<Option<PathBuf>, oxvba_project::BasProjError> {
    discover_project_files_in_dir(dir, "basproj")
}

fn discover_vbp_in_dir(dir: &Path) -> Result<Option<PathBuf>, oxvba_project::BasProjError> {
    discover_project_files_in_dir(dir, "vbp")
}

fn discover_project_files_in_dir(
    dir: &Path,
    extension: &str,
) -> Result<Option<PathBuf>, oxvba_project::BasProjError> {
    let entries = fs::read_dir(dir).map_err(|source| oxvba_project::BasProjError::Io {
        path: dir.display().to_string(),
        source,
    })?;
    let mut matches = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(extension))
        .collect::<Vec<_>>();
    matches.sort();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.into_iter().next()),
        _ => Err(oxvba_project::BasProjError::ProjectDiscoveryAmbiguous {
            directory: dir.display().to_string(),
            kind: extension.to_string(),
            candidates: matches
                .into_iter()
                .map(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_string()
                })
                .collect(),
        }),
    }
}

fn load_run_project_target(
    input_path: Option<PathBuf>,
) -> Result<oxvba_project::LoadedProject, oxvba_project::BasProjError> {
    let input = input_path.unwrap_or_else(|| PathBuf::from("."));
    if input.is_dir() {
        if let Some(basproj) = discover_basproj_in_dir(&input)? {
            return oxvba_project::load_basproj(&basproj);
        }
        if let Some(vbp) = discover_vbp_in_dir(&input)? {
            return oxvba_project::load_vbp(&vbp);
        }
        return load_convention_project(&input);
    }
    if input.extension().and_then(|ext| ext.to_str()) == Some("vbp") {
        return oxvba_project::load_vbp(&input);
    }
    oxvba_project::load_basproj(&input)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiscoveredProjectLane {
    BasProjDir,
    VbpDir,
    ConventionDir,
    BasProjFile,
    VbpFile,
}

impl DiscoveredProjectLane {
    fn as_str(self) -> &'static str {
        match self {
            DiscoveredProjectLane::BasProjDir => "basproj-dir",
            DiscoveredProjectLane::VbpDir => "vbp-dir",
            DiscoveredProjectLane::ConventionDir => "convention-dir",
            DiscoveredProjectLane::BasProjFile => "basproj-file",
            DiscoveredProjectLane::VbpFile => "vbp-file",
        }
    }
}

fn discover_run_project_lane(
    input_path: Option<PathBuf>,
) -> Result<
    (oxvba_project::LoadedProject, DiscoveredProjectLane, PathBuf),
    oxvba_project::BasProjError,
> {
    let input = input_path.unwrap_or_else(|| PathBuf::from("."));
    if input.is_dir() {
        if let Some(basproj) = discover_basproj_in_dir(&input)? {
            return Ok((
                oxvba_project::load_basproj(&basproj)?,
                DiscoveredProjectLane::BasProjDir,
                input,
            ));
        }
        if let Some(vbp) = discover_vbp_in_dir(&input)? {
            return Ok((
                oxvba_project::load_vbp(&vbp)?,
                DiscoveredProjectLane::VbpDir,
                input,
            ));
        }
        return Ok((
            load_convention_project(&input)?,
            DiscoveredProjectLane::ConventionDir,
            input,
        ));
    }
    if input.extension().and_then(|ext| ext.to_str()) == Some("vbp") {
        return Ok((
            oxvba_project::load_vbp(&input)?,
            DiscoveredProjectLane::VbpFile,
            input,
        ));
    }
    Ok((
        oxvba_project::load_basproj(&input)?,
        DiscoveredProjectLane::BasProjFile,
        input,
    ))
}

fn resolve_project_runner_bootstrap(
    loaded: &oxvba_project::LoadedProject,
    bootstrap: &RunnerBootstrapOptions,
    env_get: impl Fn(&str) -> Option<String>,
) -> Result<oxvba_host::ResolvedRunnerBootstrap, String> {
    resolve_runner_bootstrap_with_fallbacks(
        bootstrap,
        &RunnerBootstrapFallbacks {
            profile: loaded.default_runtime_profile.clone(),
            policy_preset: loaded.default_policy_preset.clone(),
        },
        env_get,
    )
}

fn load_convention_project(
    project_dir: &Path,
) -> Result<oxvba_project::LoadedProject, oxvba_project::BasProjError> {
    let project_name = project_dir
        .file_name()
        .map(|name| oxvba_project::infer_project_name_from_path(Path::new(name)))
        .unwrap_or_else(|| "ConventionProject".to_string());
    let xml = format!(
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>{}</ProjectName>\n  </PropertyGroup>\n</Project>\n",
        xml_escape(&project_name)
    );
    oxvba_project::load_basproj_from_str(&xml, project_dir)
}

fn parse_build_args(args: Vec<String>) -> Option<BuildArgs> {
    let mut iter = args.into_iter();
    let cmd = iter.next()?;
    if cmd != "build" {
        return None;
    }

    let collected: Vec<String> = iter.collect();
    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut references = CliReferenceArgs::default();
    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                output_path = collected.get(i).map(PathBuf::from);
            }
            "--project-ref" => {
                i += 1;
                references
                    .project_refs
                    .push(PathBuf::from(collected.get(i)?.as_str()));
            }
            "--com-ref" => {
                i += 1;
                references
                    .com_refs
                    .push(parse_cli_com_reference(collected.get(i)?)?);
            }
            "--native-ref" => {
                i += 1;
                references
                    .native_refs
                    .push(oxvba_project::BasProjNativeReference {
                        include: collected.get(i)?.clone(),
                        path: Some(collected.get(i)?.clone()),
                    });
            }
            arg if !arg.starts_with('-') && input_path.is_none() => {
                input_path = Some(PathBuf::from(arg));
            }
            _ => return None,
        }
        i += 1;
    }

    Some(BuildArgs {
        input_path,
        output_path,
        references,
    })
}

fn parse_explain_args_from(args: Vec<String>) -> Option<ExplainArgs> {
    let mut iter = args.into_iter();
    let cmd = iter.next()?;
    if cmd != "explain" && cmd != "host-check" {
        return None;
    }
    let collected: Vec<String> = iter.collect();
    let mut input_path: Option<PathBuf> = None;
    let mut bootstrap = RunnerBootstrapOptions::default();
    let mut entry_point_override: Option<String> = None;
    let mut references = CliReferenceArgs::default();

    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "--entry" => {
                i += 1;
                entry_point_override = Some(collected.get(i)?.clone());
            }
            "--config" => {
                i += 1;
                bootstrap.config_path = Some(PathBuf::from(collected.get(i)?.as_str()));
            }
            "--profile" => {
                i += 1;
                bootstrap.profile = Some(collected.get(i)?.clone());
            }
            "--policy" => {
                i += 1;
                bootstrap.policy_preset = Some(collected.get(i)?.clone());
            }
            "--runtime-class" => {
                i += 1;
                bootstrap.overrides.runtime_class = Some(parse_runtime_class(collected.get(i)?)?);
            }
            "--allow-interaction" => {
                i += 1;
                bootstrap.overrides.allow_interaction = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-process-spawn" => {
                i += 1;
                bootstrap.overrides.allow_process_spawn = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-filesystem-mutation" => {
                i += 1;
                bootstrap.overrides.allow_filesystem_mutation =
                    Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-dynamic-link" => {
                i += 1;
                bootstrap.overrides.allow_dynamic_link = Some(parse_bool(collected.get(i)?)?);
            }
            "--allow-com-activation" => {
                i += 1;
                bootstrap.overrides.allow_com_activation = Some(parse_bool(collected.get(i)?)?);
            }
            "--deterministic-mode" => {
                i += 1;
                bootstrap.overrides.deterministic_mode = Some(parse_bool(collected.get(i)?)?);
            }
            "--ui-virtualization" => {
                i += 1;
                bootstrap.overrides.ui_virtualization =
                    Some(parse_ui_virtualization(collected.get(i)?)?);
            }
            "--unsupported-mode" => {
                i += 1;
                bootstrap.overrides.unsupported_feature_mode =
                    Some(parse_unsupported_mode(collected.get(i)?)?);
            }
            "--wasm-runtime-class" => {
                i += 1;
                bootstrap.overrides.wasm_runtime_class =
                    Some(parse_wasm_runtime_class(collected.get(i)?)?);
            }
            "--project-ref" => {
                i += 1;
                references
                    .project_refs
                    .push(PathBuf::from(collected.get(i)?.as_str()));
            }
            "--com-ref" => {
                i += 1;
                references
                    .com_refs
                    .push(parse_cli_com_reference(collected.get(i)?)?);
            }
            "--native-ref" => {
                i += 1;
                references
                    .native_refs
                    .push(oxvba_project::BasProjNativeReference {
                        include: collected.get(i)?.clone(),
                        path: Some(collected.get(i)?.clone()),
                    });
            }
            arg if !arg.starts_with('-') && input_path.is_none() => {
                input_path = Some(PathBuf::from(arg));
            }
            _ => return None,
        }
        i += 1;
    }

    Some(ExplainArgs {
        input_path,
        bootstrap,
        entry_point_override,
        references,
    })
}

fn parse_cli_com_reference(value: &str) -> Option<CliComReference> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((library_name, importlib)) = trimmed.split_once('=') {
        let library_name = library_name.trim();
        let importlib = importlib.trim();
        if library_name.is_empty() {
            return None;
        }
        return Some(CliComReference {
            library_name: library_name.to_string(),
            importlib: if importlib.is_empty() {
                None
            } else {
                Some(importlib.to_string())
            },
        });
    }
    Some(CliComReference {
        library_name: trimmed.to_string(),
        importlib: None,
    })
}

fn apply_cli_reference_overrides(
    loaded: &mut oxvba_project::LoadedProject,
    references: &CliReferenceArgs,
) -> Result<(), String> {
    for project_path in &references.project_refs {
        let referenced = load_run_project_target(Some(project_path.clone()))
            .map_err(|err| format!("project reference {}: {err}", project_path.display()))?;
        let manifest = ReferencedProjectManifest {
            project_name: referenced.manifest.project_name.clone(),
            modules: referenced.manifest.modules.clone(),
        };
        upsert_project_reference(
            &mut loaded.manifest.references,
            ProjectReference {
                referenced_project_name: manifest.project_name.clone(),
                reference_kind: ReferenceKind::Project,
            },
        )?;
        upsert_referenced_project(&mut loaded.manifest.reference_projects, manifest);
    }

    for com_ref in &references.com_refs {
        let entry = TypeLibraryCatalogEntry {
            library_name: com_ref.library_name.clone(),
            importlib: com_ref.importlib.clone().unwrap_or_default(),
            libid: None,
            major_version: 0,
            minor_version: 0,
            lcid: None,
        };
        upsert_project_reference(
            &mut loaded.manifest.references,
            ProjectReference {
                referenced_project_name: entry.library_name.clone(),
                reference_kind: ReferenceKind::TypeLibrary,
            },
        )?;
        upsert_typelib_catalog_entry(&mut loaded.type_library_catalog, entry.clone());
        upsert_referenced_project(
            &mut loaded.manifest.reference_projects,
            project_manifest_for_cli_typelib_entry(&entry),
        );
    }

    if !references.native_refs.is_empty() {
        oxvba_project::resolve::resolve_native_references(&references.native_refs, Path::new("."))
            .map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn upsert_project_reference(
    references: &mut Vec<ProjectReference>,
    new_reference: ProjectReference,
) -> Result<(), String> {
    if let Some(index) = references.iter().position(|existing| {
        existing
            .referenced_project_name
            .eq_ignore_ascii_case(&new_reference.referenced_project_name)
    }) {
        if references[index].reference_kind != new_reference.reference_kind {
            return Err(format!(
                "reference conflict for `{}`: existing {:?}, new {:?}",
                new_reference.referenced_project_name,
                references[index].reference_kind,
                new_reference.reference_kind
            ));
        }
        references[index] = new_reference;
        return Ok(());
    }
    references.push(new_reference);
    Ok(())
}

fn upsert_referenced_project(
    projects: &mut Vec<ReferencedProjectManifest>,
    project: ReferencedProjectManifest,
) {
    if let Some(index) = projects.iter().position(|existing| {
        existing
            .project_name
            .eq_ignore_ascii_case(&project.project_name)
    }) {
        projects[index] = project;
    } else {
        projects.push(project);
    }
}

fn upsert_typelib_catalog_entry(
    catalog: &mut Vec<TypeLibraryCatalogEntry>,
    entry: TypeLibraryCatalogEntry,
) {
    if let Some(index) = catalog.iter().position(|existing| {
        existing
            .library_name
            .eq_ignore_ascii_case(&entry.library_name)
    }) {
        catalog[index] = entry;
    } else {
        catalog.push(entry);
    }
}

fn project_manifest_for_cli_typelib_entry(
    catalog_entry: &TypeLibraryCatalogEntry,
) -> ReferencedProjectManifest {
    let request = TypeLibResolveRequest {
        reference_name: catalog_entry.library_name.clone(),
        requested_coclass: None,
        importlib_hint: non_empty_trimmed(&catalog_entry.importlib),
        libid_hint: catalog_entry.libid.clone(),
        major_version_hint: Some(catalog_entry.major_version),
        minor_version_hint: Some(catalog_entry.minor_version),
        lcid_hint: catalog_entry.lcid,
    };
    if let Some(identity) = resolve_known_typelib_identity(&request) {
        return oxvba_compiler::project::project_imported_typelib_reference(&identity).manifest;
    }
    build_cli_typelib_binding_diagnostic_project(&request)
}

fn build_cli_typelib_binding_diagnostic_project(
    request: &TypeLibResolveRequest,
) -> ReferencedProjectManifest {
    let message = match (
        request.libid_hint.as_deref(),
        request.importlib_hint.as_deref(),
    ) {
        (Some(libid), _) => format!(
            "type-library reference `{}` with LIBID `{}` could not be resolved",
            request.reference_name, libid
        ),
        (None, Some(importlib)) => format!(
            "type-library reference `{}` with importlib `{}` could not be resolved",
            request.reference_name, importlib
        ),
        (None, None) => format!(
            "type-library reference `{}` needs a stronger identity hint (for example `--com-ref {}=scrrun.dll`)",
            request.reference_name, request.reference_name
        ),
    };
    ReferencedProjectManifest {
        project_name: request.reference_name.clone(),
        modules: vec![oxvba_compiler::ModuleUnit {
            module_name: "__OxVbaTypeLibBindingDiagnostic".to_string(),
            module_kind: oxvba_compiler::ModuleKind::Procedural,
            attributes: oxvba_compiler::ModuleAttributes {
                vb_name: "__OxVbaTypeLibBindingDiagnostic".to_string(),
                ..oxvba_compiler::ModuleAttributes::default()
            },
            source: format!(
                "Attribute VB_Name = \"__OxVbaTypeLibBindingDiagnostic\"\nmessage={message}\n"
            ),
        }],
    }
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn run_explain(args: Vec<String>) {
    let parsed = parse_explain_args_from(args).unwrap_or_else(|| {
        eprintln!(
            "usage: oxvba explain [path] [--entry <Module.Procedure>] [runtime/bootstrap options] [--project-ref <path>] [--com-ref <lib-or-lib=importlib>] [--native-ref <path>]"
        );
        std::process::exit(2);
    });
    let (mut loaded, lane, resolved_input) = discover_run_project_lane(parsed.input_path.clone())
        .unwrap_or_else(|err| {
            eprintln!("oxvba explain: {err}");
            std::process::exit(1);
        });
    if let Some(entry_point) = parsed.entry_point_override.as_deref() {
        oxvba_project::override_loaded_project_entry_point(&mut loaded, entry_point)
            .unwrap_or_else(|err| {
                eprintln!("oxvba explain: {err}");
                std::process::exit(1);
            });
    }
    apply_cli_reference_overrides(&mut loaded, &parsed.references).unwrap_or_else(|err| {
        eprintln!("oxvba explain: {err}");
        std::process::exit(2);
    });
    let resolved =
        resolve_project_runner_bootstrap(&loaded, &parsed.bootstrap, |key| env::var(key).ok())
            .unwrap_or_else(|err| {
                eprintln!("oxvba explain: bootstrap failed: {err}");
                std::process::exit(2);
            });
    print_explain_report(
        &resolved_input,
        lane,
        &loaded,
        &resolved,
        &parsed.references,
    );
}

fn print_explain_report(
    resolved_input: &Path,
    lane: DiscoveredProjectLane,
    loaded: &oxvba_project::LoadedProject,
    resolved: &oxvba_host::ResolvedRunnerBootstrap,
    references: &CliReferenceArgs,
) {
    println!("lane: {}", lane.as_str());
    println!("input: {}", resolved_input.display());
    println!("project: {}", loaded.manifest.project_name);
    println!("output-type: {}", cli_output_type_name(loaded.output_type));
    println!(
        "entrypoint: {}",
        loaded.entry_point.as_deref().unwrap_or("<auto>")
    );
    println!(
        "runtime-flavor: {}",
        match loaded.runtime_flavor {
            oxvba_project::RuntimeFlavor::Lite => "Lite",
            oxvba_project::RuntimeFlavor::Jit => "Jit",
        }
    );
    println!("runtime-profile: {}", resolved.runtime_profile.as_str());
    println!("policy-preset: {}", resolved.policy_preset.as_str());
    println!("bootstrap: {}", resolved.fingerprint());
    println!("references:");
    if loaded.manifest.references.is_empty() {
        println!("  - <none>");
    } else {
        for (index, reference) in loaded.manifest.references.iter().enumerate() {
            println!(
                "  {}. {:?}: {}",
                index + 1,
                reference.reference_kind,
                reference.referenced_project_name
            );
        }
    }
    if !references.native_refs.is_empty() {
        println!("native-references:");
        for reference in &references.native_refs {
            println!("  - {}", reference.include);
        }
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn cli_output_type_name(output_type: oxvba_project::OutputType) -> &'static str {
    match output_type {
        oxvba_project::OutputType::HostModule => "HostModule",
        oxvba_project::OutputType::Library => "Library",
        oxvba_project::OutputType::Exe => "Exe",
        oxvba_project::OutputType::Addin => "Addin",
        oxvba_project::OutputType::ComServer => "ComServer",
        oxvba_project::OutputType::ComExe => "ComExe",
    }
}

fn default_build_output_path(input: &Path, loaded: &oxvba_project::LoadedProject) -> PathBuf {
    if input.is_dir() || input == Path::new(".") {
        return input.join(format!("{}.oxb", loaded.manifest.project_name));
    }
    input
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{}.oxb", loaded.manifest.project_name))
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
                    .map(|value| {
                        value
                            .project_compat_slot_i32()
                            .unwrap_or(EMPTY_TAG)
                            .to_string()
                    })
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
        RuntimeValue::String(value) => format!("string:{:?}", value.as_str()),
        RuntimeValue::ArrayIntent(array) => format!("array:{array:?}"),
        RuntimeValue::Object(handle) => format!("object:{handle}"),
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
    use super::{
        apply_cli_reference_overrides, default_build_output_path, load_run_project_target,
        parse_cli_com_reference, parse_com_ref_args, parse_immediate_args_from,
        parse_init_args_from, parse_run_args_from, parse_run_project_args_from,
        resolve_project_runner_bootstrap, run_immediate_shell, run_init,
    };
    use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
    use oxvba_hal::model::{HostPolicyPreset, UnsupportedFeatureMode};
    use oxvba_host::{Engine, HostConfig, RunnerBootstrapOptions, RuntimeProfileId};
    use std::collections::BTreeMap;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let temp_root = std::env::temp_dir().join(format!(
            "{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        temp_root
    }

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
    }

    #[test]
    fn parse_run_project_bootstrap_override_flags() {
        let args = vec![
            "run-project".to_string(),
            ".".to_string(),
            "--config".to_string(),
            "runner.toml".to_string(),
            "--runtime-class".to_string(),
            "linux-stdio".to_string(),
            "--allow-dynamic-link".to_string(),
            "false".to_string(),
            "--unsupported-mode".to_string(),
            "compile-time".to_string(),
            "--dump-bootstrap".to_string(),
        ];
        let parsed = parse_run_project_args_from(args).expect("args should parse");
        assert!(parsed.dump_bootstrap);
        assert_eq!(
            parsed.bootstrap.config_path.as_deref(),
            Some(Path::new("runner.toml"))
        );
        assert_eq!(
            parsed.bootstrap.overrides.runtime_class,
            Some(oxvba_hal::model::HalRuntimeClass::LinuxStdio)
        );
        assert_eq!(parsed.bootstrap.overrides.allow_dynamic_link, Some(false));
        assert_eq!(
            parsed.bootstrap.overrides.unsupported_feature_mode,
            Some(UnsupportedFeatureMode::CompileTime)
        );
    }

    #[test]
    fn parse_run_project_args_supports_reference_flags() {
        let args = vec![
            "run-project".to_string(),
            ".".to_string(),
            "--project-ref".to_string(),
            "..\\Shared\\Shared.basproj".to_string(),
            "--com-ref".to_string(),
            "Scripting=scrrun.dll".to_string(),
            "--native-ref".to_string(),
            ".\\native\\helper.dll".to_string(),
        ];
        let parsed = parse_run_project_args_from(args).expect("args should parse");
        assert_eq!(parsed.references.project_refs.len(), 1);
        assert_eq!(parsed.references.com_refs.len(), 1);
        assert_eq!(parsed.references.native_refs.len(), 1);
        assert_eq!(
            parsed.references.com_refs[0],
            super::CliComReference {
                library_name: "Scripting".to_string(),
                importlib: Some("scrrun.dll".to_string())
            }
        );
    }

    #[test]
    fn parse_immediate_args_supports_module_and_reference_flags() {
        let args = vec![
            "repl".to_string(),
            ".".to_string(),
            "--module".to_string(),
            "Main".to_string(),
            "--profile".to_string(),
            "windows-stdio".to_string(),
            "--com-ref".to_string(),
            "Scripting=scrrun.dll".to_string(),
            "--native-ref".to_string(),
            ".\\native\\helper.dll".to_string(),
        ];

        let parsed = parse_immediate_args_from(args).expect("args should parse");
        assert_eq!(parsed.default_module.as_deref(), Some("Main"));
        assert_eq!(parsed.bootstrap.profile.as_deref(), Some("windows-stdio"));
        assert_eq!(parsed.references.com_refs.len(), 1);
        assert_eq!(parsed.references.native_refs.len(), 1);
    }

    #[test]
    fn run_immediate_shell_supports_module_query_reset_and_quit() {
        let manifest = ProjectManifest {
            project_name: "ImmediateCli".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source(
                    "Module1",
                    ModuleKind::Procedural,
                    r#"
Dim counter As Integer

Public Function IncrementCounter() As Integer
    counter = counter + 1
    IncrementCounter = counter
End Function
"#,
                )
                .expect("module unit"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");

        let mut input = Cursor::new(
            ".module Module1\nIncrementCounter()\nIncrementCounter()\nreset\nIncrementCounter()\n.quit\n",
        );
        let mut output = Vec::new();
        let mut errors = Vec::new();

        run_immediate_shell(&mut session, &mut input, &mut output, &mut errors)
            .expect("shell should succeed");

        let output_text = String::from_utf8(output).expect("utf8 output");
        assert!(output_text.contains("module: Module1"));
        assert!(output_text.contains("immediate> 1"));
        assert!(output_text.contains("immediate> 2"));
        assert!(output_text.contains("immediate> reset"));
        assert!(errors.is_empty(), "unexpected stderr: {:?}", errors);
    }

    #[test]
    fn run_immediate_shell_transcript_over_convention_project_is_deterministic() {
        let temp_root = unique_temp_dir("oxvba_cli_immediate_convention");
        std::fs::write(
            temp_root.join("Main.bas"),
            r#"
Dim counter As Integer

Public Sub Main()
End Sub

Public Function IncrementCounter() As Integer
    counter = counter + 1
    IncrementCounter = counter
End Function
"#,
        )
        .expect("write module");

        let loaded = load_run_project_target(Some(temp_root.clone())).expect("load convention");
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_immediate_session(&loaded.manifest)
            .expect("immediate session");
        let mut input =
            Cursor::new(".module Main\nIncrementCounter()\nIncrementCounter()\n.quit\n");
        let mut output = Vec::new();
        let mut errors = Vec::new();

        run_immediate_shell(&mut session, &mut input, &mut output, &mut errors)
            .expect("shell should succeed");

        let output_text = String::from_utf8(output).expect("utf8 output");
        assert_eq!(
            output_text,
            concat!(
                "OxVba Immediate Window (bounded v1). Use .help for commands, .quit to exit.\n",
                "immediate> module: Main\n",
                "immediate> 1\n",
                "immediate> 2\n",
                "immediate> "
            )
        );
        assert!(errors.is_empty(), "unexpected stderr: {:?}", errors);

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_immediate_shell_transcript_over_basproj_reset_is_deterministic() {
        let temp_root = unique_temp_dir("oxvba_cli_immediate_basproj");
        std::fs::write(
            temp_root.join("Main.bas"),
            r#"
Dim counter As Integer

Public Sub Main()
End Sub

Public Function IncrementCounter() As Integer
    counter = counter + 1
    IncrementCounter = counter
End Function
"#,
        )
        .expect("write module");
        std::fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write basproj");

        let loaded =
            load_run_project_target(Some(temp_root.join("App.basproj"))).expect("load basproj");
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_immediate_session(&loaded.manifest)
            .expect("immediate session");
        let mut input =
            Cursor::new(".module Main\nIncrementCounter()\nreset\nIncrementCounter()\n.quit\n");
        let mut output = Vec::new();
        let mut errors = Vec::new();

        run_immediate_shell(&mut session, &mut input, &mut output, &mut errors)
            .expect("shell should succeed");

        let output_text = String::from_utf8(output).expect("utf8 output");
        assert_eq!(
            output_text,
            concat!(
                "OxVba Immediate Window (bounded v1). Use .help for commands, .quit to exit.\n",
                "immediate> module: Main\n",
                "immediate> 1\n",
                "immediate> reset\n",
                "immediate> 1\n",
                "immediate> "
            )
        );
        assert!(errors.is_empty(), "unexpected stderr: {:?}", errors);

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn parse_cli_com_reference_supports_name_only_and_importlib_forms() {
        assert_eq!(
            parse_cli_com_reference("Scripting"),
            Some(super::CliComReference {
                library_name: "Scripting".to_string(),
                importlib: None
            })
        );
        assert_eq!(
            parse_cli_com_reference("Scripting=scrrun.dll"),
            Some(super::CliComReference {
                library_name: "Scripting".to_string(),
                importlib: Some("scrrun.dll".to_string())
            })
        );
    }

    #[test]
    fn parse_com_ref_add_args_supports_prog_id_and_include_override() {
        let args = vec![
            "com-ref".to_string(),
            "add".to_string(),
            ".\\App.basproj".to_string(),
            "--progid".to_string(),
            "Scripting.FileSystemObject".to_string(),
            "--include".to_string(),
            "Scripting".to_string(),
        ];

        let parsed = parse_com_ref_args(args).expect("args should parse");
        assert!(matches!(parsed.command, super::ComRefCommand::Add));
        assert_eq!(parsed.target_path, Some(PathBuf::from(".\\App.basproj")));
        assert_eq!(
            parsed.query.prog_id.as_deref(),
            Some("Scripting.FileSystemObject")
        );
        assert_eq!(parsed.query.include_override.as_deref(), Some("Scripting"));
    }

    #[test]
    fn parse_com_ref_repair_args_require_reference_and_single_selector() {
        let args = vec![
            "com-ref".to_string(),
            "repair".to_string(),
            ".\\App.basproj".to_string(),
            "--reference".to_string(),
            "Scripting".to_string(),
            "--file".to_string(),
            ".\\refs\\scrrun.dll".to_string(),
        ];

        let parsed = parse_com_ref_args(args).expect("args should parse");
        assert!(matches!(parsed.command, super::ComRefCommand::Repair));
        assert_eq!(parsed.query.reference_include.as_deref(), Some("Scripting"));
        assert_eq!(
            parsed.query.carrier_path,
            Some(PathBuf::from(".\\refs\\scrrun.dll"))
        );

        let invalid = vec![
            "com-ref".to_string(),
            "repair".to_string(),
            ".\\App.basproj".to_string(),
            "--name".to_string(),
            "Scripting".to_string(),
        ];
        assert!(parse_com_ref_args(invalid).is_none());
    }

    #[test]
    fn apply_cli_reference_overrides_adds_project_and_typelib_references() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_reference_override_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        let app_dir = temp_root.join("App");
        let lib_dir = temp_root.join("SharedLib");
        std::fs::create_dir_all(&app_dir).expect("create app dir");
        std::fs::create_dir_all(&lib_dir).expect("create lib dir");
        std::fs::write(app_dir.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write app main");
        std::fs::write(
            app_dir.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write app basproj");
        std::fs::write(
            lib_dir.join("Shared.bas"),
            "Public Function SharedValue() As Long\n    SharedValue = 7\nEnd Function\n",
        )
        .expect("write shared source");
        std::fs::write(
            lib_dir.join("SharedLib.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Library</OutputType>\n    <ProjectName>SharedLib</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Shared.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write shared basproj");

        let mut loaded = load_run_project_target(Some(app_dir.clone())).expect("load app");
        apply_cli_reference_overrides(
            &mut loaded,
            &super::CliReferenceArgs {
                project_refs: vec![lib_dir.join("SharedLib.basproj")],
                com_refs: vec![super::CliComReference {
                    library_name: "Scripting".to_string(),
                    importlib: Some("scrrun.dll".to_string()),
                }],
                native_refs: Vec::new(),
            },
        )
        .expect("apply references");

        assert!(loaded.manifest.references.iter().any(|reference| {
            reference.reference_kind == oxvba_compiler::ReferenceKind::Project
                && reference.referenced_project_name == "SharedLib"
        }));
        assert!(loaded.manifest.references.iter().any(|reference| {
            reference.reference_kind == oxvba_compiler::ReferenceKind::TypeLibrary
                && reference.referenced_project_name == "Scripting"
        }));
        assert!(
            loaded
                .manifest
                .reference_projects
                .iter()
                .any(|project| project.project_name == "SharedLib")
        );
        assert!(
            loaded
                .type_library_catalog
                .iter()
                .any(|entry| entry.library_name == "Scripting")
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_bootstrap_inherits_project_defaults_when_not_overridden() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_project_bootstrap_defaults_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");
        std::fs::write(
            temp_root.join("ProjectA.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>ProjectA</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n    <DefaultRuntimeProfile>windows-headless</DefaultRuntimeProfile>\n    <DefaultPolicyPreset>strict-ci</DefaultPolicyPreset>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write basproj");

        let loaded = load_run_project_target(Some(PathBuf::from(&temp_root)))
            .expect("directory with basproj should load the project file");
        let resolved =
            resolve_project_runner_bootstrap(&loaded, &RunnerBootstrapOptions::default(), |_| None)
                .expect("project defaults should resolve");
        assert_eq!(resolved.runtime_profile, RuntimeProfileId::WindowsHeadless);
        assert_eq!(resolved.policy_preset, HostPolicyPreset::StrictCi);

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_bootstrap_env_overrides_project_defaults() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_project_bootstrap_env_override_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");
        std::fs::write(
            temp_root.join("ProjectA.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>ProjectA</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n    <DefaultRuntimeProfile>windows-headless</DefaultRuntimeProfile>\n    <DefaultPolicyPreset>strict-ci</DefaultPolicyPreset>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write basproj");

        let loaded = load_run_project_target(Some(PathBuf::from(&temp_root)))
            .expect("directory with basproj should load the project file");
        let resolved =
            resolve_project_runner_bootstrap(&loaded, &RunnerBootstrapOptions::default(), |key| {
                match key {
                    "OXVBA_POLICY_PRESET" => Some("interactive-dev".to_string()),
                    _ => None,
                }
            })
            .expect("env override should resolve");
        assert_eq!(resolved.runtime_profile, RuntimeProfileId::WindowsHeadless);
        assert_eq!(resolved.policy_preset, HostPolicyPreset::InteractiveDev);

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn parse_init_args_with_kind_override() {
        let args = vec![
            "init".to_string(),
            ".\\new-lib".to_string(),
            "--kind".to_string(),
            "com-server".to_string(),
        ];
        let parsed = parse_init_args_from(args).expect("args should parse");
        assert_eq!(parsed.target_dir, PathBuf::from(".\\new-lib"));
        assert_eq!(super::init_output_type(parsed.kind), "ComServer");
    }

    #[test]
    fn run_project_directory_without_basproj_uses_convention_mode() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_convention_mode_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");

        let loaded = load_run_project_target(Some(temp_root.clone()))
            .expect("directory convention mode should load");
        assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_directory_without_basproj_sanitizes_directory_name_to_project_identifier() {
        let parent = std::env::temp_dir().join(format!(
            "oxvba_cli_convention_parent_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        let temp_root = parent.join("math-tool");
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");

        let loaded = load_run_project_target(Some(temp_root.clone()))
            .expect("directory convention mode should load");
        assert_eq!(loaded.manifest.project_name, "math_tool");

        std::fs::remove_dir_all(&parent).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_directory_convention_mode_executes_unique_sub_main() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_convention_sub_main_exec_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(
            temp_root.join("Main.bas"),
            "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nEnd Sub\n",
        )
        .expect("write main module");
        let loaded = load_run_project_target(Some(temp_root.clone()))
            .expect("directory convention mode should load");
        assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));

        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine
            .execute_project_with_snapshot_phased(&loaded.manifest)
            .expect("convention-mode Sub Main project should execute");

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_directory_prefers_nested_basproj_when_present() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_directory_basproj_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");
        std::fs::write(
            temp_root.join("ProjectA.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>ProjectA</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write basproj");

        let loaded = load_run_project_target(Some(PathBuf::from(&temp_root)))
            .expect("directory with basproj should load the project file");
        assert_eq!(loaded.manifest.project_name, "ProjectA");
        assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_directory_without_basproj_prefers_vbp_when_present() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_directory_vbp_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");
        std::fs::write(
            temp_root.join("Project1.vbp"),
            "Type=Exe\nName=\"Project1\"\nStartup=\"Sub Main\"\nModule=Main; Main.bas\n",
        )
        .expect("write vbp");

        let loaded = load_run_project_target(Some(PathBuf::from(&temp_root)))
            .expect("directory with only vbp should load through vbp adapter");
        assert_eq!(loaded.manifest.project_name, "Project1");
        assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_directory_convention_mode_executes_unique_top_level_mainline() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_convention_mainline_exec_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(
            temp_root.join("ScriptModule.bas"),
            "Attribute VB_Name = \"ScriptModule\"\nvalueOut = 41\nCall Bump(valueOut)\nSub Bump(ByRef value)\nvalue = value + 1\nEnd Sub\n",
        )
        .expect("write script module");

        let loaded = load_run_project_target(Some(temp_root.clone()))
            .expect("directory convention mode should load");
        assert_eq!(
            loaded.entry_point.as_deref(),
            Some("ScriptModule.__OxVbaTopLevelMainline")
        );

        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine
            .execute_project_with_snapshot_phased(&loaded.manifest)
            .expect("convention-mode top-level mainline project should execute");

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_directory_with_multiple_basproj_files_is_ambiguous() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_directory_multi_basproj_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(
            temp_root.join("A.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\" />\n",
        )
        .expect("write first basproj");
        std::fs::write(
            temp_root.join("B.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\" />\n",
        )
        .expect("write second basproj");

        let err = load_run_project_target(Some(PathBuf::from(&temp_root)))
            .expect_err("multiple basproj files should fail deterministically");
        assert!(
            err.to_string().contains("project discovery is ambiguous"),
            "got: {err}"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_directory_with_multiple_vbp_files_is_ambiguous() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_directory_multi_vbp_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");
        std::fs::write(
            temp_root.join("A.vbp"),
            "Type=Exe\nName=\"A\"\nStartup=\"Sub Main\"\nModule=Main; Main.bas\n",
        )
        .expect("write first vbp");
        std::fs::write(
            temp_root.join("B.vbp"),
            "Type=Exe\nName=\"B\"\nStartup=\"Sub Main\"\nModule=Main; Main.bas\n",
        )
        .expect("write second vbp");

        let err = load_run_project_target(Some(PathBuf::from(&temp_root)))
            .expect_err("multiple vbp files should fail deterministically");
        assert!(
            err.to_string().contains("project discovery is ambiguous"),
            "got: {err}"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_vbp_file_uses_vbp_adapter() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_vbp_run_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");
        std::fs::write(
            temp_root.join("Project1.vbp"),
            "Type=Exe\nName=\"Project1\"\nStartup=\"Sub Main\"\nModule=Main; Main.bas\n",
        )
        .expect("write vbp");

        let loaded = load_run_project_target(Some(temp_root.join("Project1.vbp")))
            .expect("vbp adapter should load executable project");
        assert_eq!(loaded.manifest.project_name, "Project1");
        assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn run_project_entry_override_replaces_loaded_startup_path() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_entry_override_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(
            temp_root.join("Main.bas"),
            "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nError 1\nEnd Sub\n",
        )
        .expect("write main module");
        std::fs::write(
            temp_root.join("Startup.bas"),
            "Attribute VB_Name = \"Startup\"\nPublic Sub Boot()\nEnd Sub\n",
        )
        .expect("write startup module");
        std::fs::write(
            temp_root.join("ProjectA.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>ProjectA</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n    <Module Include=\"Startup.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write basproj");

        let mut loaded = load_run_project_target(Some(PathBuf::from(&temp_root)))
            .expect("directory with basproj should load the project file");
        oxvba_project::override_loaded_project_entry_point(&mut loaded, "Startup.Boot")
            .expect("entry override should succeed");
        assert_eq!(loaded.entry_point.as_deref(), Some("Startup.Boot"));

        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine
            .execute_project_with_snapshot_phased(&loaded.manifest)
            .expect("overridden startup procedure should execute");

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn default_build_output_path_uses_project_name_for_directories() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_build_output_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");
        let loaded = oxvba_project::load_basproj_from_str(
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>BuildTarget</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
            &temp_root,
        )
        .expect("temp project should load");
        assert_eq!(
            default_build_output_path(&temp_root, &loaded),
            temp_root.join("BuildTarget.oxb")
        );
        assert_eq!(
            default_build_output_path(Path::new("legacy.vbp"), &loaded),
            PathBuf::from("BuildTarget.oxb")
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn init_library_scaffold_uses_library_output_without_entrypoint() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_init_library_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        run_init(vec![
            "init".to_string(),
            temp_root.to_string_lossy().to_string(),
            "--kind".to_string(),
            "library".to_string(),
        ]);

        let project_name = temp_root
            .file_name()
            .and_then(|name| name.to_str())
            .expect("project name");
        let basproj = std::fs::read_to_string(temp_root.join(format!("{project_name}.basproj")))
            .expect("library basproj should exist");
        let module =
            std::fs::read_to_string(temp_root.join("Module1.bas")).expect("module should exist");
        assert!(basproj.contains("<OutputType>Library</OutputType>"));
        assert!(!basproj.contains("<EntryPoint>"));
        assert!(module.contains("Public Function ExampleValue() As Long"));

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn init_scaffold_sanitizes_project_name_from_hyphenated_directory() {
        let parent = std::env::temp_dir().join(format!(
            "oxvba_cli_init_hyphen_parent_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        let temp_root = parent.join("new-app");
        run_init(vec![
            "init".to_string(),
            temp_root.to_string_lossy().to_string(),
        ]);

        let basproj =
            std::fs::read_to_string(temp_root.join("new_app.basproj")).expect("basproj exists");
        assert!(basproj.contains("<ProjectName>new_app</ProjectName>"));

        std::fs::remove_dir_all(&parent).expect("cleanup temp dir");
    }

    #[test]
    fn init_host_module_scaffold_sets_default_root_object() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_init_host_module_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        run_init(vec![
            "init".to_string(),
            temp_root.to_string_lossy().to_string(),
            "--kind".to_string(),
            "host-module".to_string(),
        ]);

        let project_name = temp_root
            .file_name()
            .and_then(|name| name.to_str())
            .expect("project name");
        let basproj = std::fs::read_to_string(temp_root.join(format!("{project_name}.basproj")))
            .expect("host module basproj should exist");
        assert!(basproj.contains("<OutputType>HostModule</OutputType>"));
        assert!(basproj.contains("<DefaultRootObject>Application</DefaultRootObject>"));
        assert!(!basproj.contains("<EntryPoint>"));

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn init_com_server_scaffold_loads_and_exposes_creatable_class() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_init_com_server_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        run_init(vec![
            "init".to_string(),
            temp_root.to_string_lossy().to_string(),
            "--kind".to_string(),
            "com-server".to_string(),
        ]);

        let project_name = temp_root
            .file_name()
            .and_then(|name| name.to_str())
            .expect("project name");
        let basproj_path = temp_root.join(format!("{project_name}.basproj"));
        let basproj =
            std::fs::read_to_string(&basproj_path).expect("com server basproj should exist");
        let class_source =
            std::fs::read_to_string(temp_root.join("Class1.cls")).expect("class should exist");
        assert!(basproj.contains("<OutputType>ComServer</OutputType>"));
        assert!(basproj.contains("<ClassModule Include=\"Class1.cls\">"));
        assert!(basproj.contains("<VBExposed>True</VBExposed>"));
        assert!(basproj.contains("<VBCreatable>True</VBCreatable>"));
        assert!(basproj.contains("<ProgId>"));
        assert!(class_source.contains("Public Function Ping() As Long"));

        let loaded = oxvba_project::load_basproj(&basproj_path).expect("com server should load");
        let compiled =
            oxvba_compiler::compile_project(&loaded.manifest).expect("com server should compile");
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
                include: format!(
                    "{}.{}",
                    m.module_name,
                    if matches!(m.module_kind, oxvba_compiler::ModuleKind::Class) {
                        "cls"
                    } else {
                        "bas"
                    }
                ),
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
        let exports = oxvba_project::validate::validate_com_class_exports(
            &modules_for_validation,
            &compiled,
            &loaded.class_module_metadata,
            &loaded.manifest.project_name,
        )
        .expect("com class export validation should succeed");
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].class_name, "Class1");
        let expected_prog_id = format!("{project_name}.Class1");
        assert_eq!(
            exports[0].prog_id.as_deref(),
            Some(expected_prog_id.as_str())
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn init_from_convention_captures_existing_modules_into_basproj() {
        let temp_root = std::env::temp_dir().join(format!(
            "oxvba_cli_init_from_convention_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main");
        std::fs::write(
            temp_root.join("Helpers.bas"),
            "Public Function AddOne(value As Long) As Long\n    AddOne = value + 1\nEnd Function\n",
        )
        .expect("write helper");

        run_init(vec![
            "init".to_string(),
            temp_root.to_string_lossy().to_string(),
            "--from-convention".to_string(),
        ]);

        let project_name = temp_root
            .file_name()
            .map(|name| oxvba_project::infer_project_name_from_path(Path::new(name)))
            .unwrap_or_else(|| "ConventionProject".to_string());
        let basproj_path = temp_root.join(format!("{project_name}.basproj"));
        let xml = std::fs::read_to_string(&basproj_path).expect("read generated basproj");
        assert!(xml.contains("<Module Include=\"Helpers.bas\" />"));
        assert!(xml.contains("<Module Include=\"Main.bas\" />"));
        assert!(xml.contains("<EntryPoint>Main.Main</EntryPoint>"));

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
    }
}
