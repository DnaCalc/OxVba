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
    #[error("failed to compile WrappedComServer type library with MIDL: {message}")]
    MidlFailed { message: String },
    #[error("failed to emit WrappedComServer type library with OleAut32: {message}")]
    OleAutTypeLibFailed { message: String },
    #[error("failed to copy prebuilt WrappedComServer COM host: {message}")]
    ComHostCopyFailed { message: String },
    #[error("required tool `{tool}` was not found; searched {searched}")]
    ToolNotFound {
        tool: &'static str,
        searched: String,
    },
    #[error("WrappedComServer DLL compilation is only supported on Windows hosts")]
    UnsupportedPlatform,
}

pub fn copy_prebuilt_comhost_dll(
    explicit_host_path: Option<&Path>,
    dll_target_path: &Path,
) -> Result<PathBuf, ShimCompileError> {
    let host_path = if let Some(path) = explicit_host_path {
        path.to_path_buf()
    } else {
        find_prebuilt_comhost_dll().ok_or_else(|| ShimCompileError::ToolNotFound {
            tool: "oxvba_comhost.dll",
            searched:
                "explicit build option, OXVBA_COMHOST_DLL, executable directory, and workspace target release/debug directories"
                    .to_string(),
        })?
    };
    if !host_path.exists() {
        return Err(ShimCompileError::ToolNotFound {
            tool: "oxvba_comhost.dll",
            searched: host_path.display().to_string(),
        });
    }
    if let Some(parent) = dll_target_path.parent() {
        create_dir_all(parent)?;
    }
    fs::copy(&host_path, dll_target_path).map_err(|source| {
        ShimCompileError::ComHostCopyFailed {
            message: format!(
                "{} -> {}: {source}",
                host_path.display(),
                dll_target_path.display()
            ),
        }
    })?;
    Ok(host_path)
}

pub fn find_prebuilt_comhost_dll() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("OXVBA_COMHOST_DLL").map(PathBuf::from)
        && path.exists()
    {
        return Some(path);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join(dll_file_name("oxvba_comhost"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(Path::to_path_buf)?;
    for profile in ["release", "debug"] {
        let candidate = workspace_root
            .join("target")
            .join(profile)
            .join(dll_file_name("oxvba_comhost"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

pub fn compile_typelib(idl_path: &Path, tlb_target_path: &Path) -> Result<(), ShimCompileError> {
    if !cfg!(target_os = "windows") {
        return Err(ShimCompileError::UnsupportedPlatform);
    }

    let midl = find_midl().ok_or_else(|| ShimCompileError::ToolNotFound {
        tool: "midl.exe",
        searched: "PATH and Windows Kits 10 bin directories".to_string(),
    })?;
    let include_root =
        windows_sdk_include_root(&midl).ok_or_else(|| ShimCompileError::ToolNotFound {
            tool: "Windows SDK IDL includes",
            searched: "Windows Kits 10 Include directories matching midl.exe".to_string(),
        })?;
    let cl_dir = find_cl_dir();
    let out_dir = tlb_target_path.parent().unwrap_or_else(|| Path::new("."));
    create_dir_all(out_dir)?;
    let tlb_name = tlb_target_path
        .file_name()
        .ok_or_else(|| ShimCompileError::MidlFailed {
            message: format!("invalid TLB output path `{}`", tlb_target_path.display()),
        })?;

    let mut command = Command::new(&midl);
    command
        .arg("/nologo")
        .arg("/env")
        .arg("x64")
        .arg("/client")
        .arg("none")
        .arg("/server")
        .arg("none")
        .arg("/I")
        .arg(include_root.join("um"))
        .arg("/I")
        .arg(include_root.join("shared"))
        .arg("/out")
        .arg(out_dir)
        .arg("/tlb")
        .arg(tlb_name)
        .arg(idl_path);
    if let Some(cl_dir) = cl_dir {
        let old_path = std::env::var_os("PATH").unwrap_or_default();
        let mut new_path = cl_dir.into_os_string();
        new_path.push(";");
        new_path.push(old_path);
        command.env("PATH", new_path);
    }

    let output = command.output().map_err(|source| ShimCompileError::Io {
        path: midl.display().to_string(),
        source,
    })?;
    if !output.status.success() {
        return Err(ShimCompileError::MidlFailed {
            message: format!(
                "status={:?}\nstdout:\n{}\nstderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    if !tlb_target_path.exists() {
        return Err(ShimCompileError::MidlFailed {
            message: format!(
                "MIDL succeeded but did not create `{}`",
                tlb_target_path.display()
            ),
        });
    }
    Ok(())
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

fn find_midl() -> Option<PathBuf> {
    find_on_path("midl.exe").or_else(|| {
        let kits_root = program_files_x86()
            .join("Windows Kits")
            .join("10")
            .join("bin");
        versioned_tool_candidates(&kits_root, "x64", "midl.exe")
            .into_iter()
            .next()
    })
}

fn windows_sdk_include_root(midl: &Path) -> Option<PathBuf> {
    let arch_dir = midl.parent()?;
    let version_dir = arch_dir.parent()?;
    let version = version_dir.file_name()?;
    let bin_dir = version_dir.parent()?;
    let sdk_root = bin_dir.parent()?;
    let include = sdk_root.join("Include").join(version);
    if include.join("um").join("oaidl.idl").exists()
        || include.join("um").join("OAIdl.Idl").exists()
    {
        Some(include)
    } else {
        latest_windows_sdk_include()
    }
}

fn latest_windows_sdk_include() -> Option<PathBuf> {
    let include_root = program_files_x86()
        .join("Windows Kits")
        .join("10")
        .join("Include");
    let mut candidates: Vec<PathBuf> = fs::read_dir(include_root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("um").join("oaidl.idl").exists())
        .collect();
    candidates.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    candidates.into_iter().next()
}

fn find_cl_dir() -> Option<PathBuf> {
    find_on_path("cl.exe")
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| {
            let mut candidates = Vec::new();
            for root in [
                program_files().join("Microsoft Visual Studio"),
                program_files_x86().join("Microsoft Visual Studio"),
            ] {
                collect_matching_files(&root, "cl.exe", 12, &mut candidates);
            }
            candidates.retain(|path| {
                path.components()
                    .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
                    .collect::<Vec<_>>()
                    .windows(3)
                    .any(|window| {
                        window[0] == "bin" && window[1] == "hostx64" && window[2] == "x64"
                    })
            });
            candidates.sort_by(|a, b| b.cmp(a));
            candidates
                .into_iter()
                .next()
                .and_then(|path| path.parent().map(Path::to_path_buf))
        })
}

fn find_on_path(file_name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(file_name))
        .find(|path| path.exists())
}

fn versioned_tool_candidates(root: &Path, arch: &str, file_name: &str) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = fs::read_dir(root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path().join(arch).join(file_name))
        .filter(|path| path.exists())
        .collect();
    candidates.sort_by(|a, b| b.cmp(a));
    candidates
}

fn collect_matching_files(
    root: &Path,
    file_name: &str,
    max_depth: usize,
    matches: &mut Vec<PathBuf>,
) {
    if max_depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_matching_files(&path, file_name, max_depth - 1, matches);
        } else if path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
        {
            matches.push(path);
        }
    }
}

fn program_files() -> PathBuf {
    std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
}

fn program_files_x86() -> PathBuf {
    std::env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"))
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
