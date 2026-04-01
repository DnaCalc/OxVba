use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    BasProjError, LoadedProject, infer_project_name_from_path, load_basproj, load_basproj_from_str,
    load_vbp,
};

/// Load a workspace target using the canonical OxVba project-discovery policy.
///
/// Accepted inputs are:
/// - a `.basproj` file
/// - a `.vbp` file
/// - a directory containing exactly one `.basproj`
/// - a directory containing exactly one `.vbp`
/// - a convention directory with no project file
pub fn load_workspace_target(path: &Path) -> Result<LoadedProject, BasProjError> {
    if path.is_dir() {
        if let Some(basproj) = discover_project_file_in_dir(path, "basproj")? {
            return load_basproj(&basproj);
        }
        if let Some(vbp) = discover_project_file_in_dir(path, "vbp")? {
            return load_vbp(&vbp);
        }
        return load_convention_project(path);
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("vbp") => load_vbp(path),
        Some("basproj") | None => load_basproj(path),
        Some(other) => Err(BasProjError::UnsupportedPath {
            path: path.display().to_string(),
            extension: other.to_string(),
        }),
    }
}

/// Discover a unique project file with the requested extension in a directory.
pub fn discover_project_file_in_dir(
    dir: &Path,
    extension: &str,
) -> Result<Option<PathBuf>, BasProjError> {
    let entries = fs::read_dir(dir).map_err(|source| BasProjError::Io {
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
        _ => Err(BasProjError::ProjectDiscoveryAmbiguous {
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

/// Load a convention directory into an in-memory project.
pub fn load_convention_project(project_dir: &Path) -> Result<LoadedProject, BasProjError> {
    let project_name = infer_project_name_from_path(project_dir);
    let xml = format!(
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>{}</ProjectName>\n  </PropertyGroup>\n</Project>\n",
        xml_escape(&project_name)
    );
    load_basproj_from_str(&xml, project_dir)
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{discover_project_file_in_dir, load_workspace_target};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_workspace_target_prefers_basproj_in_directory() {
        let temp_root = unique_temp_dir("oxvba_project_workspace_target_basproj");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Module1.bas"), "Sub Main()\nEnd Sub\n").expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let loaded = load_workspace_target(&temp_root).expect("workspace target");
        assert_eq!(loaded.manifest.project_name, "App");

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn load_workspace_target_falls_back_to_convention_directory() {
        let temp_root = unique_temp_dir("oxvba_project_workspace_target_convention");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Module1.bas"), "Sub Main()\nEnd Sub\n").expect("module");

        let loaded = load_workspace_target(&temp_root).expect("convention workspace");
        assert!(
            loaded
                .manifest
                .modules
                .iter()
                .any(|module| module.module_name == "Module1")
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn discover_project_file_in_dir_rejects_ambiguous_matches() {
        let temp_root = unique_temp_dir("oxvba_project_workspace_target_ambiguous");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("One.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\" />",
        )
        .expect("first basproj");
        fs::write(
            temp_root.join("Two.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\" />",
        )
        .expect("second basproj");

        let err = discover_project_file_in_dir(&temp_root, "basproj")
            .expect_err("ambiguous basproj discovery should fail");
        assert!(err.to_string().contains("project discovery is ambiguous"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nonce}"))
    }
}
