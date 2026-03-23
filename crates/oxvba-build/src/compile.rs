//! Compile shim driver: builds generated Rust source into a binary.

use std::path::Path;

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
    let temp_dir = std::env::temp_dir().join(format!("oxvba_build_{}", std::process::id()));
    let src_dir = temp_dir.join("src");

    std::fs::create_dir_all(&src_dir).map_err(BuildError::Io)?;

    // Write source
    let src_file = match output_type {
        ShimOutputType::Exe => src_dir.join("main.rs"),
        ShimOutputType::Dll => src_dir.join("lib.rs"),
    };
    std::fs::write(&src_file, source).map_err(BuildError::Io)?;

    // Write Cargo.toml
    let crate_type = match output_type {
        ShimOutputType::Exe => "[[bin]]\nname = \"shim\"\npath = \"src/main.rs\"",
        ShimOutputType::Dll => "[lib]\ncrate-type = [\"cdylib\"]\npath = \"src/lib.rs\"",
    };
    let cargo_toml = format!(
        r#"[package]
name = "oxvba-shim"
version = "0.1.0"
edition = "2024"

{crate_type}

[dependencies]
oxvba-compiler = {{ path = "{}" }}
oxvba-host = {{ path = "{}" }}
oxvba-runtime = {{ path = "{}" }}
"#,
        // These paths would need to be absolute or relative to the temp project
        "../../crates/oxvba-compiler",
        "../../crates/oxvba-host",
        "../../crates/oxvba-runtime",
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
        ShimOutputType::Exe => temp_dir.join("target/release/shim"),
        ShimOutputType::Dll => temp_dir.join("target/release/oxvba_shim.dll"),
    };

    if built_binary.exists() {
        std::fs::copy(&built_binary, output_path).map_err(BuildError::Io)?;
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
