use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum ShimCompileError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to compile WrappedComServer shim with cargo: {message}")]
    CargoFailed { message: String },
    #[error("WrappedComServer DLL compilation is only supported on Windows hosts")]
    UnsupportedPlatform,
}

pub fn compile_shim_dll(
    shim_source_path: &Path,
    dll_target_path: &Path,
) -> Result<(), ShimCompileError> {
    if !cfg!(target_os = "windows") {
        return Err(ShimCompileError::UnsupportedPlatform);
    }

    let build_dir = dll_target_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".oxvba-shim-build");
    let src_dir = build_dir.join("src");
    create_dir_all(&src_dir)?;

    let shim_source =
        fs::read_to_string(shim_source_path).map_err(|source| ShimCompileError::Io {
            path: shim_source_path.display().to_string(),
            source,
        })?;
    write_text(&src_dir.join("lib.rs"), &shim_source)?;
    write_text(&build_dir.join("Cargo.toml"), &shim_cargo_toml())?;

    let target_dir = build_dir.join("target");
    let output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(build_dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .current_dir(&build_dir)
        .output()
        .map_err(|source| ShimCompileError::Io {
            path: "cargo".to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(ShimCompileError::CargoFailed {
            message: format!(
                "status={:?}\nstdout:\n{}\nstderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    let built_dll = target_dir
        .join("release")
        .join(dll_file_name("oxvba_wrapped_com_server_shim"));
    fs::copy(&built_dll, dll_target_path).map_err(|source| ShimCompileError::Io {
        path: format!("{} -> {}", built_dll.display(), dll_target_path.display()),
        source,
    })?;
    Ok(())
}

fn create_dir_all(path: &Path) -> Result<(), ShimCompileError> {
    fs::create_dir_all(path).map_err(|source| ShimCompileError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn write_text(path: &Path, text: &str) -> Result<(), ShimCompileError> {
    fs::write(path, text).map_err(|source| ShimCompileError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn shim_cargo_toml() -> String {
    let oxvba_build = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = oxvba_build
        .parent()
        .expect("oxvba-build lives under workspace crates directory");
    let crate_path = |name: &str| path_for_toml(&crates_dir.join(name));
    format!(
        r#"[package]
name = "oxvba-wrapped-com-server-shim"
version = "0.0.0"
edition = "2024"

[workspace]

[lib]
name = "oxvba_wrapped_com_server_shim"
crate-type = ["cdylib"]
path = "src/lib.rs"

[dependencies]
oxvba-build = {{ path = "{}" }}
oxvba-bundle = {{ path = "{}" }}
oxvba-com = {{ path = "{}" }}
oxvba-host = {{ path = "{}" }}
oxvba-runtime = {{ path = "{}" }}
serde_json = "1"
windows-sys = {{ version = "0.59", features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_Com",
    "Win32_System_LibraryLoader",
    "Win32_System_Ole",
    "Win32_System_Registry",
    "Win32_System_SystemServices",
    "Win32_System_Variant",
] }}
"#,
        path_for_toml(&oxvba_build),
        crate_path("oxvba-bundle"),
        crate_path("oxvba-com"),
        crate_path("oxvba-host"),
        crate_path("oxvba-runtime")
    )
}

fn path_for_toml(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn dll_file_name(stem: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("{stem}.dll")
    }
    #[cfg(target_os = "linux")]
    {
        format!("lib{stem}.so")
    }
    #[cfg(target_os = "macos")]
    {
        format!("lib{stem}.dylib")
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        stem.to_string()
    }
}
