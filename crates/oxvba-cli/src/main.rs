use oxvba_hal::model::{
    HalRuntimeClass, UiVirtualizationMode, UnsupportedFeatureMode, WasmRuntimeClass,
};
use oxvba_host::{Engine, HostConfig, RunnerBootstrapOptions, resolve_runner_bootstrap};
use oxvba_runtime::{RuntimeValue, bstr::BStr, value_tags::EMPTY_TAG};
use std::path::{Path, PathBuf};
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
// build subcommand: project target -> compile -> .oxb
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

    let input = input_path.unwrap_or_else(|| PathBuf::from("."));

    let mut loaded = load_run_project_target(Some(input.clone())).unwrap_or_else(|err| {
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

    let out = output_path.unwrap_or_else(|| default_build_output_path(&input, &loaded));

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
            "usage: oxvba run-project [path] [--entry <Module.Procedure>] [--profile <id>] [--policy <preset>] [--jit] [--dump-slots] [--dump-values]"
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

    let config = HostConfig {
        enable_jit: parsed.enable_jit,
        root_object_name: Some(loaded.default_root_object.clone()),
    };
    let mut engine = Engine::new(config);

    let resolved = resolve_runner_bootstrap(&parsed.bootstrap, |key| env::var(key).ok())
        .unwrap_or_else(|err| {
            eprintln!("oxvba run-project: bootstrap failed: {err}");
            std::process::exit(2);
        });
    engine.set_runtime_profile(resolved.runtime_profile);
    engine.set_host_policy(resolved.policy.clone());

    let result = engine.execute_project_with_snapshot_phased(&loaded.manifest);

    match result {
        Ok(values) => {
            if parsed.dump_slots {
                let payload = values
                    .iter()
                    .map(|v| v.to_legacy_i32().unwrap_or(EMPTY_TAG).to_string())
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

#[derive(Debug, Clone)]
struct RunProjectArgs {
    input_path: Option<PathBuf>,
    enable_jit: bool,
    dump_values: bool,
    dump_slots: bool,
    bootstrap: RunnerBootstrapOptions,
    entry_point_override: Option<String>,
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
    let mut bootstrap = RunnerBootstrapOptions::default();
    let mut entry_point_override: Option<String> = None;

    let collected: Vec<String> = iter.collect();
    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "--jit" => enable_jit = true,
            "--dump-values" => dump_values = true,
            "--dump-slots" => dump_slots = true,
            "--entry" => {
                i += 1;
                entry_point_override = Some(collected.get(i)?.clone());
            }
            "--profile" => {
                i += 1;
                bootstrap.profile = Some(collected.get(i)?.clone());
            }
            "--policy" => {
                i += 1;
                bootstrap.policy_preset = Some(collected.get(i)?.clone());
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
        bootstrap,
        entry_point_override,
    })
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

    let mut i = 0;
    while i < collected.len() {
        match collected[i].as_str() {
            "--kind" => {
                i += 1;
                kind = parse_init_kind(collected.get(i)?)?;
            }
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
            "usage: oxvba init [path] [--kind <application|library|addin|host-module|com-server|com-exe>]"
        );
        std::process::exit(2);
    });
    let target_dir = parsed.target_dir;

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

fn load_convention_project(
    project_dir: &Path,
) -> Result<oxvba_project::LoadedProject, oxvba_project::BasProjError> {
    let project_name = project_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("ConventionProject");
    let xml = format!(
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>{}</ProjectName>\n  </PropertyGroup>\n</Project>\n",
        xml_escape(project_name)
    );
    oxvba_project::load_basproj_from_str(&xml, project_dir)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
    use super::{
        default_build_output_path, load_run_project_target, parse_init_args_from,
        parse_run_args_from, parse_run_project_args_from, run_init,
    };
    use oxvba_hal::model::UnsupportedFeatureMode;
    use oxvba_host::{Engine, HostConfig};
    use std::path::{Path, PathBuf};

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
}
