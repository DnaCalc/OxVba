//! Compile shim driver: builds generated Rust source into a binary.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SHIM_BUILD_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Error type for build operations.
#[derive(Debug)]
pub enum BuildError {
    Io(std::io::Error),
    CompileFailed(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Io(e) => write!(f, "I/O error: {e}"),
            BuildError::CompileFailed(msg) => write!(f, "compilation failed: {msg}"),
        }
    }
}

/// Target output type.
#[derive(Debug, Clone, Copy)]
pub enum ShimOutputType {
    Exe,
    Dll,
    Xll,
}

/// Compile a generated Rust shim source file into a binary.
///
/// This creates a temporary Cargo project, writes the source, and runs
/// `cargo build --release`. The resulting binary is copied to `output_path`.
pub fn compile_shim(
    source: &str,
    output_path: &Path,
    output_type: ShimOutputType,
) -> Result<(), BuildError> {
    let temp_dir = std::env::temp_dir().join(format!(
        "oxvba_build_{}_{}",
        std::process::id(),
        SHIM_BUILD_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let src_dir = temp_dir.join("src");

    std::fs::create_dir_all(&src_dir).map_err(BuildError::Io)?;

    // Write source
    let src_file = match output_type {
        ShimOutputType::Exe => src_dir.join("main.rs"),
        ShimOutputType::Dll | ShimOutputType::Xll => src_dir.join("lib.rs"),
    };
    std::fs::write(&src_file, source).map_err(BuildError::Io)?;

    // Write Cargo.toml
    let crate_type = match output_type {
        ShimOutputType::Exe => "[[bin]]\nname = \"shim\"\npath = \"src/main.rs\"",
        ShimOutputType::Dll | ShimOutputType::Xll => {
            "[lib]\ncrate-type = [\"cdylib\"]\npath = \"src/lib.rs\""
        }
    };
    let compiler_path = workspace_crate_path("oxvba-compiler")?;
    let hal_path = workspace_crate_path("oxvba-hal")?;
    let host_path = workspace_crate_path("oxvba-host")?;
    let runtime_path = workspace_crate_path("oxvba-runtime")?;
    let cargo_toml = format!(
        r#"[package]
name = "oxvba-shim"
version = "0.1.0"
edition = "2024"

{crate_type}

[profile.release]
debug = true

[dependencies]
oxvba-compiler = {{ path = "{}" }}
oxvba-hal = {{ path = "{}" }}
oxvba-host = {{ path = "{}" }}
oxvba-runtime = {{ path = "{}" }}
"#,
        cargo_path_literal(&compiler_path),
        cargo_path_literal(&hal_path),
        cargo_path_literal(&host_path),
        cargo_path_literal(&runtime_path),
    );
    std::fs::write(temp_dir.join("Cargo.toml"), cargo_toml).map_err(BuildError::Io)?;

    // Run cargo build
    let output = std::process::Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&temp_dir)
        .output()
        .map_err(BuildError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Clean up temp dir
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(BuildError::CompileFailed(stderr.to_string()));
    }

    // Copy result to output_path
    let built_binary = match output_type {
        ShimOutputType::Exe => temp_dir.join(format!("target/release/shim{}", exe_suffix())),
        ShimOutputType::Dll | ShimOutputType::Xll => temp_dir.join("target/release/oxvba_shim.dll"),
    };

    if built_binary.exists() {
        std::fs::copy(&built_binary, output_path).map_err(BuildError::Io)?;
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

fn workspace_crate_path(crate_name: &str) -> Result<PathBuf, BuildError> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_name);
    crate_root.canonicalize().map_err(BuildError::Io)
}

fn cargo_path_literal(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

fn exe_suffix() -> &'static str {
    if cfg!(windows) { ".exe" } else { "" }
}
