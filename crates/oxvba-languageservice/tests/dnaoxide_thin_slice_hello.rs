use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use oxvba_host::{
    DebugBreakpointBindingStatus, DebugWatchEvaluationStatus, DirectHostCapabilityStatus,
    DirectHostCommandStatus, EmbeddedBuildRequest, EmbeddedBuildRunHost, EmbeddedBuildStatus,
    EmbeddedExecutionSourcePolicy, EmbeddedRunRequest, EmbeddedRunStatus, EmbeddedWorkspaceInput,
    Engine, HostConfig, ImmediateEvaluationRequest, ImmediateVariantEvaluationOutput,
};
use oxvba_languageservice::HostWorkspaceSession;
use oxvba_project::{ComProjectSelectionStatus, ComSelectionService};

#[test]
fn dnaoxide_thin_slice_hello_overlay_build_run_immediate_debug_watch_breakpoint() {
    let project = TempProject::new("dnaoxide_thin_slice_hello");
    let project_file = project.write_project(
        "ThinSliceHello",
        &[module_item("Module1.bas")],
        &[module_source(BASELINE_SOURCE)],
    );

    let mut workspace = HostWorkspaceSession::load_workspace_path(&project_file)
        .expect("DnaOxIde thin slice project should load through direct host session");
    let document = workspace
        .documents()
        .into_iter()
        .next()
        .expect("Module1 document should be present");

    workspace
        .set_document_text(&document.id, OVERLAY_SOURCE)
        .expect("overlay edit should apply to host workspace session");
    let roster = workspace
        .workspace_roster(EmbeddedExecutionSourcePolicy::WorkspaceOverlay)
        .expect("workspace roster");
    let module = roster
        .modules
        .iter()
        .find(|module| module.logical_module_name == "Module1")
        .expect("Module1 roster entry");
    assert!(module.has_workspace_overlay);
    assert!(module.document_version > 1);

    let engine = Engine::new(HostConfig::default());
    let build_run_host = EmbeddedBuildRunHost::new(&engine);
    let workspace_input = EmbeddedWorkspaceInput::workspace_overlay(project_file.clone());
    let snapshot = workspace
        .prepare_embedded_workspace_snapshot(&workspace_input)
        .expect("overlay snapshot should be available for build/run");

    let build_request =
        EmbeddedBuildRequest::with_request_id(snapshot.clone(), "build:dnaoxide:thin-slice-hello");
    let build_result = build_run_host.build_workspace(&build_request);
    assert_eq!(
        build_result.request_id.as_str(),
        "build:dnaoxide:thin-slice-hello"
    );
    assert_eq!(build_result.status, EmbeddedBuildStatus::Succeeded);
    assert!(build_result.diagnostics.is_empty());

    let run_request = EmbeddedRunRequest::with_request_id(
        snapshot.clone(),
        "run:dnaoxide:thin-slice-hello:immediate",
    );
    let run_session = build_run_host
        .run_project(&run_request)
        .expect("runtime session should be ready");
    assert_eq!(
        run_session.run_result().status,
        EmbeddedRunStatus::SessionReady
    );
    assert!(
        run_session
            .runtime_session_id()
            .as_str()
            .contains("run:dnaoxide")
    );

    let mut immediate = run_session.into_immediate_session();
    immediate.set_default_target_module(Some("Module1"));
    let immediate_result = immediate
        .evaluate_variant(&ImmediateEvaluationRequest::query("HelloValue()"))
        .expect("Immediate query should evaluate over the overlay-backed runtime");
    let ImmediateVariantEvaluationOutput::Value(value) = immediate_result.output else {
        panic!("expected Immediate value result");
    };
    assert_eq!(value.variant_value.as_i32(), Some(42));
    assert_eq!(
        immediate.runtime_session_id().expect("runtime id").as_str(),
        "runtime:run:dnaoxide:thin-slice-hello:immediate"
    );

    let debug_run_request =
        EmbeddedRunRequest::with_request_id(snapshot, "run:dnaoxide:thin-slice-hello:debug");
    let debug_run_session = build_run_host
        .run_project(&debug_run_request)
        .expect("debug runtime session should be ready");
    let mut debug = debug_run_session.into_debug_session();
    let watch = debug.add_watch("y");
    assert!(watch.watch_id.as_str().contains(":watch:1"));
    assert!(matches!(
        &debug.evaluate_watches()[0].status,
        DebugWatchEvaluationStatus::Unavailable(issue)
            if issue.stable_code == "DH-NOT-PAUSED"
    ));

    let (breakpoint_module, breakpoint_line) = debug
        .runtime()
        .compiled()
        .procedure_runtime_metadata
        .values()
        .find(|metadata| metadata.procedure_name.eq_ignore_ascii_case("Main"))
        .map(|metadata| {
            (
                metadata.module_name.clone(),
                metadata.statement_line_numbers[0],
            )
        })
        .expect("Main runtime metadata should expose a statement source line");
    let breakpoint = debug.set_source_breakpoint(breakpoint_module, breakpoint_line);
    assert_eq!(
        breakpoint.binding_status,
        DebugBreakpointBindingStatus::Bound
    );
    assert!(breakpoint.unresolved_reason.is_none());
    assert!(breakpoint.breakpoint_id.as_str().contains(":breakpoint:1"));
    let disabled_breakpoint = debug
        .set_breakpoint_enabled(&breakpoint.breakpoint_id, false)
        .expect("disable fixture breakpoint before stepping");
    assert!(!disabled_breakpoint.enabled);

    let paused_at_entry = debug.start_variants().expect("debug start should pause");
    let oxvba_host::HostDebugVariantRunResult::Paused(entry_pause) = paused_at_entry else {
        panic!("expected entry pause");
    };
    assert_eq!(format!("{:?}", entry_pause.stop.reason), "Entry");
    assert!(
        entry_pause.frames[0]
            .frame_id
            .as_str()
            .contains(":frame:1:")
    );

    let stepped = debug
        .step_into_variants()
        .expect("debug step into should pause");
    let oxvba_host::HostDebugVariantRunResult::Paused(callee_pause) = stepped else {
        panic!("expected callee pause");
    };
    let current_frame = callee_pause.frames.last().expect("current debug frame");
    assert!(current_frame.procedure_name.eq_ignore_ascii_case("Foo"));
    assert!(current_frame.frame_id.as_str().contains(":frame:"));

    let watch_values = debug.evaluate_watches();
    assert!(matches!(
        &watch_values[0].status,
        DebugWatchEvaluationStatus::Value(value)
            if value.name.eq_ignore_ascii_case("y") && value.variant_value.as_i32() == Some(4)
    ));
}

#[test]
fn dnaoxide_thin_slice_hello_com_broken_reference_and_runtime_availability_are_typed() {
    let project = TempProject::new("dnaoxide_thin_slice_com_reference");
    let project_file = project.write_project(
        "ThinSliceCom",
        &[
            module_item("Module1.bas"),
            "    <COMReference Include=\"Missing.DnaOxIde.Component\">\n      <Guid>{11111111-2222-3333-4444-555555555555}</Guid>\n      <VersionMajor>1</VersionMajor>\n      <VersionMinor>0</VersionMinor>\n      <Lcid>0</Lcid>\n      <ImportLib>missing-dnaoxide.dll</ImportLib>\n    </COMReference>\n".to_string(),
        ],
        &[module_source("Sub Main()\nEnd Sub\n")],
    );

    let service = ComSelectionService;
    let surface = service
        .inspect_workspace_project_state(&project_file, &[])
        .expect("COM project surface");
    assert_eq!(surface.active_references.len(), 1);
    assert_eq!(
        surface.active_references[0].include,
        "Missing.DnaOxIde.Component"
    );
    assert!(matches!(
        surface.selections[0].status,
        ComProjectSelectionStatus::Missing
    ));

    let profile = service.capability_profile();
    assert_eq!(
        profile.runtime_invocation.kind,
        oxvba_host::DirectHostCapabilityKind::ComRuntimeInvocation
    );
    assert!(profile.runtime_availability.requires_windows);
    assert!(profile.runtime_availability.required_apartment.is_some());
    assert!(profile.runtime_availability.bitness_requirement.is_some());

    if cfg!(target_os = "windows") {
        assert!(matches!(
            profile.runtime_availability.command_status,
            DirectHostCommandStatus::Available
        ));
        assert!(matches!(
            profile.runtime_invocation.status,
            DirectHostCapabilityStatus::Available
        ));
    } else {
        assert!(matches!(
            &profile.runtime_availability.command_status,
            DirectHostCommandStatus::Disabled { reason }
                if reason.stable_code == "DH-COM-INVOCATION-UNAVAILABLE"
        ));
        assert!(matches!(
            &profile.runtime_invocation.status,
            DirectHostCapabilityStatus::Unavailable { reason }
                if reason.stable_code == "DH-COM-INVOCATION-UNAVAILABLE"
        ));
    }
}

const BASELINE_SOURCE: &str = r#"Sub Main()
    Call Foo(4)
End Sub

Sub Foo(ByVal y As Integer)
    Dim z As Integer
    z = y + 1
End Sub

Public Function HelloValue() As Integer
    HelloValue = 41
End Function
"#;

const OVERLAY_SOURCE: &str = r#"Sub Main()
    Call Foo(4)
End Sub

Sub Foo(ByVal y As Integer)
    Dim z As Integer
    z = y + 1
End Sub

Public Function HelloValue() As Integer
    HelloValue = 42
End Function
"#;

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(prefix: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("oxvba_{prefix}_{}_{}", std::process::id(), nonce));
        fs::create_dir_all(&root).expect("create temp project root");
        Self { root }
    }

    fn write_project(
        &self,
        project_name: &str,
        item_lines: &[String],
        modules: &[(&str, &str)],
    ) -> PathBuf {
        for (name, source) in modules {
            fs::write(self.root.join(name), source).expect("write module source");
        }
        let mut xml = format!(
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>{project_name}</ProjectName>\n    <EntryPoint>Module1.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n"
        );
        for item in item_lines {
            xml.push_str(item);
        }
        xml.push_str("  </ItemGroup>\n</Project>\n");
        let project_file = self.root.join(format!("{project_name}.basproj"));
        fs::write(&project_file, xml).expect("write basproj");
        project_file
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn module_item(path: &str) -> String {
    format!("    <Module Include=\"{path}\" />\n")
}

fn module_source(source: &str) -> (&str, &str) {
    ("Module1.bas", source)
}

#[allow(dead_code)]
fn assert_path_exists(path: &Path) {
    assert!(path.exists(), "expected path to exist: {}", path.display());
}
