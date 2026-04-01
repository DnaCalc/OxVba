mod syntax;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bruto_ide::language::{BuildResult, Language};
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

    let host_binary = current_host_binary()?;
    fs::copy(&host_binary, &artifacts.exe_path)
        .map_err(|err| format!("failed to stage Bruto host binary: {err}"))?;

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

fn current_host_binary() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe()
        .map_err(|err| format!("failed to resolve current executable: {err}"))?;
    if is_bruto_host_binary(&current_exe) {
        return Ok(current_exe);
    }

    let mut candidates = Vec::new();
    if let Some(parent) = current_exe.parent() {
        candidates.push(parent.join(format!("oxvba-bruto{}", exe_suffix())));
        if parent.file_name().and_then(|name| name.to_str()) == Some("deps") {
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.join(format!("oxvba-bruto{}", exe_suffix())));
            }
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("failed to locate the oxvba-bruto host binary".to_string())
}

fn is_bruto_host_binary(path: &std::path::Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("oxvba-bruto") | Some("oxvba-bruto.exe")
    )
}

#[cfg(test)]
mod tests {
    use super::OxvbaBrutoLanguage;
    use bruto_ide::language::Language;
    use std::fs;
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
        ensure_debug_host_binary();
        let language = OxvbaBrutoLanguage;
        let result = language
            .build("Sub Main()\n    Print \"42\"\nEnd Sub\n")
            .expect("Bruto OxVba build should succeed");
        let root = std::path::Path::new(&result.source_path)
            .parent()
            .expect("Bruto source path should have a parent");
        assert!(root.join("Program.oxb").exists());
        assert!(std::path::Path::new(&result.exe_path).exists());

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
    fn build_stages_current_host_binary() {
        ensure_debug_host_binary();
        let language = OxvbaBrutoLanguage;
        let first = language
            .build("Sub Main()\n    Print \"one\"\nEnd Sub\n")
            .expect("first Bruto build should succeed");
        let second = language
            .build("Sub Main()\n    Print \"two\"\nEnd Sub\n")
            .expect("second Bruto build should succeed");

        let first_exe = fs::read(&first.exe_path).expect("first Bruto exe should exist");
        let second_exe = fs::read(&second.exe_path).expect("second Bruto exe should exist");
        assert_eq!(first_exe, second_exe);

        cleanup_artifacts(&first.source_path);
        cleanup_artifacts(&second.source_path);
    }

    #[test]
    fn build_reports_compile_errors() {
        ensure_debug_host_binary();
        let language = OxvbaBrutoLanguage;
        let err = match language.build("") {
            Ok(_) => panic!("invalid source should not build"),
            Err(err) => err,
        };
        assert!(err.contains("failed") || err.contains("error"));
    }

    fn ensure_debug_host_binary() {
        let status = Command::new("cargo")
            .args(["build", "-p", "oxvba-bruto"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("should be able to build oxvba-bruto for tests");
        assert!(status.success(), "oxvba-bruto debug build should succeed");
    }

    fn cleanup_artifacts(source_path: &str) {
        let Some(root) = std::path::Path::new(source_path).parent() else {
            return;
        };
        let _ = fs::remove_dir_all(root);
    }
}
