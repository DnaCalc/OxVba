use std::sync::{Arc, Mutex};

use oxvba_hal::callbacks::HostCallbacks;
use oxvba_host::{Engine, HostConfig, RuntimeProfileId, compat::RuntimeValueCompatEngineExt};

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ))
}

#[derive(Default)]
struct ConsoleCallbacks {
    console_lines: Mutex<Vec<String>>,
}

impl ConsoleCallbacks {
    fn console_output(&self) -> Vec<String> {
        self.console_lines
            .lock()
            .expect("console output lock")
            .clone()
    }
}

impl HostCallbacks for ConsoleCallbacks {
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
        style.max(1)
    }

    fn on_input_box(&self, _prompt: &str, default: &str) -> String {
        default.to_string()
    }

    fn on_status_bar(&self, _text: &str) {}

    fn on_console_print(&self, text: &str) -> bool {
        self.console_lines
            .lock()
            .expect("console output lock")
            .push(text.to_string());
        true
    }

    fn on_debug_print(&self, _text: &str) {}
}

fn engine_with_console(enable_jit: bool, callbacks: Arc<dyn HostCallbacks>) -> Engine {
    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_runtime_profile(RuntimeProfileId::WindowsStdio);
    engine.set_host_callbacks(Some(callbacks));
    engine
}

#[test]
fn basproj_exe_executes_unique_top_level_mainline() {
    let temp_root = unique_temp_dir("oxvba_host_top_level_mainline");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    let script_path = temp_root.join("ScriptModule.bas");
    std::fs::write(
        &script_path,
        "valueOut = 41\nCall Bump(valueOut)\nPrint CStr(valueOut)\nSub Bump(ByRef value)\nvalue = value + 1\nEnd Sub\n",
    )
    .expect("write script module");

    let basproj_path = temp_root.join("ProjectA.basproj");
    std::fs::write(
        &basproj_path,
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded = oxvba_project::load_basproj(&basproj_path).expect("basproj should load");
    assert_eq!(
        loaded.entry_point.as_deref(),
        Some("ScriptModule.__OxVbaTopLevelMainline")
    );
    for enable_jit in [false, true] {
        let callbacks = Arc::new(ConsoleCallbacks::default());
        engine_with_console(enable_jit, callbacks.clone())
            .execute_project_with_snapshot_phased(&loaded.manifest)
            .expect("project execution should succeed");
        assert_eq!(
            callbacks.console_output(),
            vec!["42".to_string()],
            "top-level basproj mainline should preserve console-observable result for enable_jit={enable_jit}"
        );
    }

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn basproj_exe_honors_explicit_entry_point_over_sub_main_fallback() {
    let temp_root = unique_temp_dir("oxvba_host_basproj_explicit_entry");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    std::fs::write(
        temp_root.join("MainModule.bas"),
        "Public Sub Main()\nError 1\nEnd Sub\n",
    )
    .expect("write main module");
    std::fs::write(
        temp_root.join("StartupModule.bas"),
        "Public Sub Boot()\nEnd Sub\n",
    )
    .expect("write startup module");

    let basproj_path = temp_root.join("ProjectA.basproj");
    std::fs::write(
        &basproj_path,
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
    <EntryPoint>StartupModule.Boot</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"MainModule.bas\" />
    <Module Include=\"StartupModule.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded = oxvba_project::load_basproj(&basproj_path).expect("basproj should load");
    assert_eq!(loaded.entry_point.as_deref(), Some("StartupModule.Boot"));
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("explicit entrypoint should execute instead of Sub Main fallback");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn basproj_exe_top_level_mainline_shares_option_private_module_state_with_helper_procedures() {
    let temp_root = unique_temp_dir("oxvba_host_top_level_option_private");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    let script_path = temp_root.join("ScriptModule.bas");
    std::fs::write(
        &script_path,
        "Option Private Module\nPrivate counter As Long\ncounter = 41\nPrint \"pre=\" & CStr(counter)\nCall Bump\nPrint \"post=\" & CStr(counter)\nPublic Sub Bump()\ncounter = counter + 1\nvalueOut = counter\nPrint \"bump=\" & CStr(counter)\nEnd Sub\n",
    )
    .expect("write script module");

    let basproj_path = temp_root.join("ProjectA.basproj");
    std::fs::write(
        &basproj_path,
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded = oxvba_project::load_basproj(&basproj_path).expect("basproj should load");
    for enable_jit in [false, true] {
        let callbacks = Arc::new(ConsoleCallbacks::default());
        engine_with_console(enable_jit, callbacks.clone())
            .execute_project_with_snapshot_phased(&loaded.manifest)
            .expect("project execution should succeed with module-scope state preserved");
        assert_eq!(
            callbacks.console_output(),
            vec![
                "pre=41".to_string(),
                "bump=42".to_string(),
                "post=42".to_string()
            ],
            "Option Private/module-state top-level basproj should share module-scope state across helper procedures for enable_jit={enable_jit}"
        );
    }

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn basproj_exe_top_level_mainline_preserves_mixed_module_scope_declarations() {
    let temp_root = unique_temp_dir("oxvba_host_top_level_mixed_module_scope");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    let script_path = temp_root.join("ScriptModule.bas");
    std::fs::write(
        &script_path,
        concat!(
            "Option Explicit\n",
            "Option Private Module\n",
            "Rem module comment\n",
            "#Const ENABLE = True\n",
            "DefLng A-Z\n",
            "Public valueOut As Long\n",
            "Public sharedCount As Long\n",
            "Private counter As Long\n",
            "Global totalCount As Long\n",
            "Static stickyCount As Long\n",
            "Private Type CounterState\n",
            "    Value As Long\n",
            "End Type\n",
            "Public Enum CounterMode\n",
            "    CounterModeDefault = 1\n",
            "End Enum\n",
            "counter = 41\n",
            "valueOut = counter\n",
            "Call Bump(valueOut)\n",
            "Print CStr(valueOut)\n",
            "Public Sub Bump(ByRef value)\n",
            "    value = value + 1\n",
            "End Sub\n",
        ),
    )
    .expect("write script module");

    let basproj_path = temp_root.join("ProjectA.basproj");
    std::fs::write(
        &basproj_path,
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded = oxvba_project::load_basproj(&basproj_path).expect("basproj should load");
    for enable_jit in [false, true] {
        let callbacks = Arc::new(ConsoleCallbacks::default());
        engine_with_console(enable_jit, callbacks.clone())
            .execute_project_with_snapshot_phased(&loaded.manifest)
            .expect("project execution should succeed with mixed module declarations preserved");
        assert_eq!(
            callbacks.console_output(),
            vec!["42".to_string()],
            "mixed module-scope declarations should survive top-level basproj lowering for enable_jit={enable_jit}"
        );
    }

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn vbp_exe_honors_explicit_startup_procedure() {
    let temp_root = unique_temp_dir("oxvba_host_vbp_explicit_startup");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    std::fs::write(
        temp_root.join("Main.bas"),
        "Public Sub Main()\nError 1\nEnd Sub\n",
    )
    .expect("write main module");
    std::fs::write(
        temp_root.join("Startup.bas"),
        "Public Sub Boot()\nEnd Sub\n",
    )
    .expect("write startup module");
    let vbp_path = temp_root.join("Project1.vbp");
    std::fs::write(
        &vbp_path,
        "Type=Exe\nName=\"Project1\"\nStartup=\"Startup.Boot\"\nModule=Main; Main.bas\nModule=Startup; Startup.bas\n",
    )
    .expect("write vbp");

    let loaded = oxvba_project::vbp::load_vbp(&vbp_path).expect("vbp should load");
    assert_eq!(loaded.entry_point.as_deref(), Some("Startup.Boot"));
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("explicit Startup=Module.Procedure should execute");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn vbp_exe_sub_main_fallback_executes_supported_project() {
    let temp_root = unique_temp_dir("oxvba_host_vbp_sub_main");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
        .expect("write main module");
    let vbp_path = temp_root.join("Project1.vbp");
    std::fs::write(
        &vbp_path,
        "Type=Exe\nName=\"Project1\"\nStartup=\"Sub Main\"\nModule=Main; Main.bas\n",
    )
    .expect("write vbp");

    let loaded = oxvba_project::vbp::load_vbp(&vbp_path).expect("vbp should load");
    assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let snapshot = engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("Sub Main fallback project should execute");
    assert!(
        snapshot.is_empty(),
        "startup shim should leave no user slots in empty startup path: {snapshot:?}"
    );

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn vbp_exe_unique_top_level_mainline_executes_supported_project() {
    let temp_root = unique_temp_dir("oxvba_host_vbp_top_level_mainline");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    std::fs::write(
        temp_root.join("ScriptModule.bas"),
        "valueOut = 41\nCall Bump(valueOut)\nPrint CStr(valueOut)\nSub Bump(ByRef value)\nvalue = value + 1\nEnd Sub\n",
    )
    .expect("write script module");
    let vbp_path = temp_root.join("Project1.vbp");
    std::fs::write(
        &vbp_path,
        "Type=Exe\nName=\"Project1\"\nModule=ScriptModule; ScriptModule.bas\n",
    )
    .expect("write vbp");

    let loaded = oxvba_project::vbp::load_vbp(&vbp_path).expect("vbp should load");
    assert_eq!(
        loaded.entry_point.as_deref(),
        Some("ScriptModule.__OxVbaTopLevelMainline")
    );
    for enable_jit in [false, true] {
        let callbacks = Arc::new(ConsoleCallbacks::default());
        engine_with_console(enable_jit, callbacks.clone())
            .execute_project_with_snapshot_phased(&loaded.manifest)
            .expect("top-level mainline vbp project should execute");
        assert_eq!(
            callbacks.console_output(),
            vec!["42".to_string()],
            "top-level VBP mainline should preserve console-observable result for enable_jit={enable_jit}"
        );
    }

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn vbp_exe_resolves_project_reference_to_referenced_vbp_project() {
    let temp_root = unique_temp_dir("oxvba_host_vbp_project_reference");
    let lib_root = temp_root.join("LibScale");
    let app_root = temp_root.join("MainApp");
    std::fs::create_dir_all(&lib_root).expect("create temp library root");
    std::fs::create_dir_all(&app_root).expect("create temp app root");

    std::fs::write(
        lib_root.join("M01.bas"),
        "Option Explicit\nPublic Function Value() As Long\nValue = 42\nEnd Function\n",
    )
    .expect("write library module");
    std::fs::write(
        lib_root.join("LibScale.vbp"),
        "Type=OleDll\nName=\"LibScale\"\nModule=M01; M01.bas\n",
    )
    .expect("write library vbp");

    std::fs::write(app_root.join("Main.bas"), "Call LibScale.M01.Value()\n")
        .expect("write main module");
    std::fs::write(
        app_root.join("MainApp.vbp"),
        "Type=Exe\nName=\"MainApp\"\nReference=*\\A{11111111-2222-3333-4444-555555555555}#1.0#0#..\\LibScale\\LibScale.vbp#LibScale\nModule=Main; Main.bas\n",
    )
    .expect("write app vbp");

    let loaded =
        oxvba_project::vbp::load_vbp(&app_root.join("MainApp.vbp")).expect("vbp should load");
    assert_eq!(
        loaded.entry_point.as_deref(),
        Some("Main.__OxVbaTopLevelMainline")
    );
    assert_eq!(loaded.manifest.reference_projects.len(), 1);
    assert_eq!(
        loaded.manifest.reference_projects[0].project_name,
        "LibScale"
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("vbp project reference graph should execute");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}
