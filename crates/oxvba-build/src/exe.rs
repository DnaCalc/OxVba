//! EXE shim source generation.

use oxvba_project::LoadedProject;

/// Generate Rust source code for a standalone executable shim.
///
/// The generated code embeds the `.oxb` bundle via `include_bytes!`, creates
/// an Engine, deserializes the bundle, and executes it.
pub fn generate_exe_shim(loaded: &LoadedProject, oxb_path: &str) -> String {
    let project_name = &loaded.manifest.project_name;
    let _entry_point = loaded.entry_point.as_deref().unwrap_or("Main.Main");

    format!(
        r#"//! Auto-generated OxVBA executable shim for project "{project_name}".
//! Do not edit — regenerate with `oxvba build`.

use oxvba_compiler::OxBundle;
use oxvba_host::{{Engine, HostConfig}};

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

fn main() {{
    let bundle = OxBundle::deserialize_from_bytes(BUNDLE_BYTES)
        .expect("failed to deserialize embedded bundle");

    let config = HostConfig {{
        enable_jit: false,
        root_object_name: Some("Application".to_string()),
    }};
    let engine = Engine::new(config);

    // Execute the bundle
    let result = engine.execute_bundle_with_snapshot(&bundle);

    match result {{
        Ok(_) => {{}},
        Err(err) => {{
            eprintln!("{project_name}: execution failed: {{err}}");
            std::process::exit(1);
        }}
    }}
}}
"#
    )
}

/// Template-based generation for testing.
pub fn generate_exe_shim_template(
    project_name: &str,
    oxb_path: &str,
    _entry_point: &str,
) -> String {
    format!(
        r#"//! Auto-generated OxVBA executable shim for project "{project_name}".

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

fn main() {{
    let bundle = OxBundle::deserialize_from_bytes(BUNDLE_BYTES)
        .expect("failed to deserialize embedded bundle");
    let engine = Engine::new(HostConfig::default());
    engine.execute_bundle_with_snapshot(&bundle).unwrap();
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_shim_contains_project_name() {
        // We can't easily create a LoadedProject in tests without filesystem,
        // so test the output format indirectly
        let source = generate_exe_shim_template("TestApp", "bundle.oxb", "Main.Main");
        assert!(source.contains("TestApp"));
        assert!(source.contains("bundle.oxb"));
        assert!(source.contains("include_bytes!"));
    }
}
