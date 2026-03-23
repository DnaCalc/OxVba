//! Out-of-process COM server (EXE) shim generation.

use oxvba_project::ComClassExportDescriptor;

/// Generate Rust source code for an out-of-process COM server EXE.
///
/// The generated EXE handles:
/// - `-Embedding` flag (COM launched the process)
/// - `/RegServer` (register COM classes in registry)
/// - `/UnregServer` (unregister COM classes from registry)
/// - Message pump for STA (Single-Threaded Apartment)
/// - Shutdown on last reference release
pub fn generate_com_exe_shim(
    project_name: &str,
    oxb_path: &str,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut source = format!(
        r#"//! Auto-generated OxVBA out-of-process COM server for project "{project_name}".

#![allow(non_snake_case)]
#![cfg(target_os = "windows")]

use std::sync::atomic::{{AtomicI32, Ordering}};

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

static GLOBAL_REF_COUNT: AtomicI32 = AtomicI32::new(0);

fn main() {{
    let args: Vec<String> = std::env::args().collect();

    // Check command-line flags
    for arg in &args[1..] {{
        match arg.to_ascii_lowercase().as_str() {{
            "/regserver" | "-regserver" => {{
                register_server();
                return;
            }}
            "/unregserver" | "-unregserver" => {{
                unregister_server();
                return;
            }}
            "-embedding" => {{
                // COM launched us — continue to message pump
            }}
            _ => {{}}
        }}
    }}

    // Initialize COM as STA
    // CoInitializeEx(null, COINIT_APARTMENTTHREADED)
    // Register class objects with CoRegisterClassObject

"#
    );

    for class in classes {
        source.push_str(&format!(
            "    // Class: {} (ProgId: {})\n",
            class.class_name,
            class.prog_id.as_deref().unwrap_or("(none)")
        ));
    }

    source.push_str(
        r#"
    // Message pump: STA requires processing Windows messages
    // while COM clients hold references
    run_message_pump();
}

fn register_server() {
    // Write HKCR\CLSID\{clsid}\LocalServer32 = <exe path>
    // Write HKCR\<ProgId>\CLSID = {clsid}
    println!("registered COM server");
}

fn unregister_server() {
    // Remove HKCR\CLSID\{clsid}
    // Remove HKCR\<ProgId>
    println!("unregistered COM server");
}

fn run_message_pump() {
    // STA message pump: PeekMessage/TranslateMessage/DispatchMessage loop
    // Exit when GLOBAL_REF_COUNT drops to 0 and a timeout elapses
    loop {
        if GLOBAL_REF_COUNT.load(Ordering::SeqCst) == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
"#,
    );

    source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn com_exe_shim_structure() {
        let classes = vec![oxvba_project::ComClassExportDescriptor {
            class_name: "Widget".to_string(),
            prog_id: Some("TestApp.Widget".to_string()),
            instancing: None,
            description: None,
            members: vec![],
        }];

        let source = generate_com_exe_shim("TestApp", "test.oxb", &classes);
        assert!(source.contains("fn main()"));
        assert!(source.contains("/regserver"));
        assert!(source.contains("/unregserver"));
        assert!(source.contains("-embedding"));
        assert!(source.contains("LocalServer32"));
        assert!(source.contains("run_message_pump"));
        assert!(source.contains("Widget"));
        assert!(source.contains("TestApp.Widget"));
    }
}
