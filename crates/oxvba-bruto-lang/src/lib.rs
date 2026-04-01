mod syntax;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use bruto_ide::language::{BuildResult, Language};
use oxvba_build::compile::{ShimOutputType, compile_shim};
use oxvba_compiler::{
    ModuleKind, OxBundle, ProjectKind, ProjectManifest, compile_project, module_unit_from_source,
};
use syntax::OxvbaHighlighter;
use turbo_vision::views::syntax::SyntaxHighlighter;

static BRUTO_BUILD_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct OxvbaBrutoLanguage;

impl Language for OxvbaBrutoLanguage {
    fn name(&self) -> &str {
        "OxVba"
    }

    fn file_extension(&self) -> &str {
        "bas"
    }

    fn sample_program(&self) -> &str {
        SAMPLE_PROGRAM
    }

    fn create_highlighter(&self) -> Box<dyn SyntaxHighlighter> {
        Box::new(OxvbaHighlighter::new())
    }

    fn build(&self, source: &str) -> Result<BuildResult, String> {
        build_bruto_program(source)
    }
}

const SAMPLE_PROGRAM: &str = "Sub Main()\n    Print \"Hello from OxVba\"\nEnd Sub\n";

struct BrutoArtifacts {
    source_path: PathBuf,
    bundle_path: PathBuf,
    exe_path: PathBuf,
    capture_path: PathBuf,
}

fn build_bruto_program(source: &str) -> Result<BuildResult, String> {
    let artifacts = allocate_artifacts()?;
    fs::write(&artifacts.source_path, source)
        .map_err(|err| format!("failed to write Bruto source file: {err}"))?;
    fs::write(&artifacts.capture_path, "")
        .map_err(|err| format!("failed to initialize Bruto console capture: {err}"))?;

    let module = module_unit_from_source("Main", ModuleKind::Procedural, source)
        .map_err(|err| format!("source preparation failed: {err}"))?;
    let manifest = ProjectManifest {
        project_name: "BrutoScratch".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };

    let compiled = compile_project(&manifest).map_err(|err| format!("build failed: {err}"))?;
    let bundle = OxBundle::from_compiled_project(&compiled, &manifest.project_name);
    let bundle_bytes = bundle
        .serialize_to_bytes()
        .map_err(|err| format!("bundle serialization failed: {err}"))?;
    fs::write(&artifacts.bundle_path, bundle_bytes)
        .map_err(|err| format!("failed to write Bruto bundle: {err}"))?;

    let shim_source = generate_bruto_exe_shim(&artifacts.bundle_path, &artifacts.capture_path);
    compile_shim(&shim_source, &artifacts.exe_path, ShimOutputType::Exe)
        .map_err(|err| format!("native shim build failed: {err}"))?;

    Ok(BuildResult {
        exe_path: artifacts.exe_path.display().to_string(),
        source_path: artifacts.source_path.display().to_string(),
        console_capture_path: artifacts.capture_path.display().to_string(),
    })
}

fn allocate_artifacts() -> Result<BrutoArtifacts, String> {
    let root_dir = std::env::temp_dir().join("oxvba-bruto").join(format!(
        "build_{}_{}",
        std::process::id(),
        BRUTO_BUILD_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root_dir)
        .map_err(|err| format!("failed to create Bruto build directory: {err}"))?;
    Ok(BrutoArtifacts {
        source_path: root_dir.join("Program.bas"),
        bundle_path: root_dir.join("Program.oxb"),
        exe_path: root_dir.join(format!("Program{}", exe_suffix())),
        capture_path: root_dir.join("console.txt"),
    })
}

fn exe_suffix() -> &'static str {
    if cfg!(windows) { ".exe" } else { "" }
}

fn rust_string_literal(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn generate_bruto_exe_shim(bundle_path: &Path, capture_path: &Path) -> String {
    let bundle_literal = rust_string_literal(bundle_path);
    let capture_literal = rust_string_literal(capture_path);
    format!(
        r#"//! Auto-generated OxVba Bruto executable shim.
//! Do not edit.

use oxvba_compiler::OxBundle;
use oxvba_hal::{{HostPolicy, callbacks::HostCallbacks}};
use oxvba_host::{{Engine, HostConfig}};
use std::sync::{{Arc, Mutex}};

const BUNDLE_BYTES: &[u8] = include_bytes!("{bundle_literal}");
const CONSOLE_CAPTURE_PATH: &str = "{capture_literal}";

struct CaptureCallbacks {{
    write_lock: Mutex<()>,
}}

impl CaptureCallbacks {{
    fn append_line(&self, text: &str) {{
        let _guard = self.write_lock.lock().expect("capture callback lock poisoned");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(CONSOLE_CAPTURE_PATH)
            .expect("failed to open Bruto console capture");
        use std::io::Write;
        writeln!(file, "{{text}}").expect("failed to append Bruto console capture");
    }}
}}

impl HostCallbacks for CaptureCallbacks {{
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {{
        style.max(1)
    }}

    fn on_input_box(&self, _prompt: &str, default: &str) -> String {{
        default.to_string()
    }}

    fn on_status_bar(&self, _text: &str) {{}}

    fn on_console_print(&self, text: &str) -> bool {{
        self.append_line(text);
        true
    }}

    fn on_debug_print(&self, _text: &str) {{}}
}}

fn main() {{
    std::fs::write(CONSOLE_CAPTURE_PATH, "").expect("failed to reset Bruto console capture");

    let bundle = OxBundle::deserialize_from_bytes(BUNDLE_BYTES)
        .expect("failed to deserialize embedded bundle");
    let callbacks = Arc::new(CaptureCallbacks {{
        write_lock: Mutex::new(()),
    }});
    let mut engine = Engine::new(HostConfig {{
        enable_jit: false,
        root_object_name: Some("Application".to_string()),
    }})
    .with_host_callbacks(callbacks);
    engine.set_host_policy(HostPolicy::interactive_dev());

    match engine.execute_bundle_with_snapshot(&bundle) {{
        Ok(_) => {{}}
        Err(err) => {{
            eprintln!("OxVba Bruto: execution failed: {{err}}");
            std::process::exit(1);
        }}
    }}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::OxvbaBrutoLanguage;
    use bruto_ide::language::Language;
    use std::fs;
    use std::path::Path;
    use std::process::{Command, Stdio};

    #[test]
    fn bruto_language_surface_is_stable() {
        let language = OxvbaBrutoLanguage;
        assert_eq!(language.name(), "OxVba");
        assert_eq!(language.file_extension(), "bas");
        assert!(language.sample_program().contains("Sub Main()"));
    }

    #[test]
    fn build_and_run_round_trip_captures_console_output() {
        let language = OxvbaBrutoLanguage;
        let result = language
            .build("Sub Main()\n    Print \"42\"\nEnd Sub\n")
            .expect("Bruto OxVba build should succeed");

        let status = Command::new(&result.exe_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("Bruto OxVba shim should run");
        assert!(status.success());

        let captured = fs::read_to_string(&result.console_capture_path)
            .expect("Bruto OxVba console capture should exist");
        assert_eq!(captured.trim(), "42");

        cleanup_artifacts(&result.source_path);
    }

    #[test]
    fn build_reports_compile_errors() {
        let language = OxvbaBrutoLanguage;
        let err = match language.build("") {
            Ok(_) => panic!("invalid source should not build"),
            Err(err) => err,
        };
        assert!(err.contains("failed") || err.contains("error"));
    }

    fn cleanup_artifacts(source_path: &str) {
        let Some(root) = Path::new(source_path).parent() else {
            return;
        };
        let _ = fs::remove_dir_all(root);
    }
}
