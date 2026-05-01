use std::path::{Path, PathBuf};

use oxvba_host::{
    Engine, HostConfig, ImmediateEvaluationOutput, ImmediateSession, PhaseDiagnostic,
    ProjectRuntimeSession,
};
use oxvba_languageservice::{DocumentId, HostWorkspaceSession};
use oxvba_project::{LoadedProject, load_workspace_target};
use oxvba_web_host::{
    WebDiagnostic, WebHostCommand, WebHostEvent, WebOutputStream, WebRunState,
    project_immediate_result, project_workspace_loaded,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellAssetKind {
    Html,
    JavaScript,
    Css,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellAsset {
    pub path: &'static str,
    pub kind: ShellAssetKind,
    pub contents: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellManifest {
    pub app_name: &'static str,
    pub bridge_contract_version: &'static str,
    pub entry_asset_path: &'static str,
    pub assets: Vec<ShellAssetSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellAssetSummary {
    pub path: &'static str,
    pub kind: ShellAssetKind,
}

pub struct WebShellSession {
    engine: Engine,
    workspace_path: Option<PathBuf>,
    workspace_session: Option<HostWorkspaceSession>,
    loaded_project: Option<LoadedProject>,
    runtime_session: Option<ProjectRuntimeSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebShellError {
    NoWorkspaceLoaded,
    InvalidDocumentId(String),
    Host(String),
}

impl std::fmt::Display for WebShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebShellError::NoWorkspaceLoaded => write!(f, "no workspace is loaded"),
            WebShellError::InvalidDocumentId(document_id) => {
                write!(f, "invalid document id `{document_id}`")
            }
            WebShellError::Host(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for WebShellError {}

pub fn embedded_assets() -> Vec<ShellAsset> {
    vec![
        ShellAsset {
            path: "index.html",
            kind: ShellAssetKind::Html,
            contents: include_str!("../assets/index.html"),
        },
        ShellAsset {
            path: "app.js",
            kind: ShellAssetKind::JavaScript,
            contents: include_str!("../assets/app.js"),
        },
        ShellAsset {
            path: "styles.css",
            kind: ShellAssetKind::Css,
            contents: include_str!("../assets/styles.css"),
        },
    ]
}

pub fn shell_manifest() -> ShellManifest {
    let assets = embedded_assets()
        .into_iter()
        .map(|asset| ShellAssetSummary {
            path: asset.path,
            kind: asset.kind,
        })
        .collect();
    ShellManifest {
        app_name: "oxvba-web-shell",
        bridge_contract_version: "v1",
        entry_asset_path: "index.html",
        assets,
    }
}

impl Default for WebShellSession {
    fn default() -> Self {
        Self::new()
    }
}

impl WebShellSession {
    pub fn new() -> Self {
        Self {
            engine: Engine::new(HostConfig {
                enable_jit: false,
                root_object_name: None,
            }),
            workspace_path: None,
            workspace_session: None,
            loaded_project: None,
            runtime_session: None,
        }
    }

    pub fn handle_command(
        &mut self,
        command: WebHostCommand,
    ) -> Result<Vec<WebHostEvent>, WebShellError> {
        match command {
            WebHostCommand::LoadWorkspace { path } => self.load_workspace(Path::new(&path)),
            WebHostCommand::ReloadWorkspace => self.reload_workspace(),
            WebHostCommand::ListDocuments => self.list_documents(),
            WebHostCommand::SetDocumentText {
                document_id,
                source,
            } => self.set_document_text(&document_id, &source),
            WebHostCommand::CloseDocument { document_id } => self.close_document(&document_id),
            WebHostCommand::RunProject => self.run_project(),
            WebHostCommand::ResetRuntime => self.reset_runtime(),
            WebHostCommand::ImmediateEvaluate(request) => self.immediate_evaluate(request),
            other => Ok(vec![WebHostEvent::Error {
                operation: format!("{other:?}"),
                message: "command not yet wired in shell baseline".to_string(),
            }]),
        }
    }

    fn load_workspace(&mut self, path: &Path) -> Result<Vec<WebHostEvent>, WebShellError> {
        let workspace_session = HostWorkspaceSession::load_workspace_path(path)
            .map_err(|err| WebShellError::Host(err.to_string()))?;
        let loaded_project =
            load_workspace_target(path).map_err(|err| WebShellError::Host(err.to_string()))?;
        self.workspace_path = Some(path.to_path_buf());
        self.runtime_session = None;
        self.loaded_project = Some(loaded_project);
        self.workspace_session = Some(workspace_session);
        Ok(self.workspace_loaded_events())
    }

    fn reload_workspace(&mut self) -> Result<Vec<WebHostEvent>, WebShellError> {
        let path = self
            .workspace_path
            .clone()
            .ok_or(WebShellError::NoWorkspaceLoaded)?;
        self.load_workspace(&path)
    }

    fn list_documents(&self) -> Result<Vec<WebHostEvent>, WebShellError> {
        let workspace_session = self
            .workspace_session
            .as_ref()
            .ok_or(WebShellError::NoWorkspaceLoaded)?;
        let workspace_path = self
            .workspace_path
            .as_ref()
            .ok_or(WebShellError::NoWorkspaceLoaded)?;
        Ok(vec![project_workspace_loaded(
            workspace_path.display().to_string(),
            &workspace_session.documents(),
        )])
    }

    fn set_document_text(
        &mut self,
        document_id: &str,
        source: &str,
    ) -> Result<Vec<WebHostEvent>, WebShellError> {
        let document_id = parse_document_id(document_id)?;
        let workspace_session = self
            .workspace_session
            .as_mut()
            .ok_or(WebShellError::NoWorkspaceLoaded)?;
        workspace_session
            .set_document_text(&document_id, source)
            .map_err(|err| WebShellError::Host(err.to_string()))?;
        self.document_diagnostics_events(&document_id.0)
    }

    fn close_document(&mut self, document_id: &str) -> Result<Vec<WebHostEvent>, WebShellError> {
        let document_id = parse_document_id(document_id)?;
        let workspace_session = self
            .workspace_session
            .as_mut()
            .ok_or(WebShellError::NoWorkspaceLoaded)?;
        workspace_session
            .close_document(&document_id)
            .map_err(|err| WebShellError::Host(err.to_string()))?;
        self.document_diagnostics_events(&document_id.0)
    }

    fn run_project(&mut self) -> Result<Vec<WebHostEvent>, WebShellError> {
        let manifest = self
            .loaded_project
            .as_ref()
            .ok_or(WebShellError::NoWorkspaceLoaded)?
            .manifest
            .clone();
        let runtime = self
            .engine
            .start_project_runtime_session(&manifest)
            .map_err(map_phase_diagnostic)?;
        let snapshot_len = runtime.snapshot().len();
        self.runtime_session = Some(runtime);
        Ok(vec![
            WebHostEvent::RunStateChanged(WebRunState::Running),
            WebHostEvent::OutputLine {
                stream: WebOutputStream::Stdout,
                text: format!("project run completed with {snapshot_len} user slots"),
            },
            WebHostEvent::RunStateChanged(WebRunState::Completed),
        ])
    }

    fn reset_runtime(&mut self) -> Result<Vec<WebHostEvent>, WebShellError> {
        let manifest = self
            .loaded_project
            .as_ref()
            .ok_or(WebShellError::NoWorkspaceLoaded)?
            .manifest
            .clone();
        let runtime = self
            .engine
            .compile_and_prepare_session(&manifest)
            .map_err(map_phase_diagnostic)?;
        self.runtime_session = Some(runtime);
        Ok(vec![
            WebHostEvent::OutputLine {
                stream: WebOutputStream::Stdout,
                text: "runtime reset to a fresh prepared session".to_string(),
            },
            WebHostEvent::RunStateChanged(WebRunState::Idle),
        ])
    }

    fn immediate_evaluate(
        &mut self,
        request: oxvba_web_host::WebImmediateRequest,
    ) -> Result<Vec<WebHostEvent>, WebShellError> {
        let manifest = self
            .loaded_project
            .as_ref()
            .ok_or(WebShellError::NoWorkspaceLoaded)?
            .manifest
            .clone();
        let runtime = match self.runtime_session.take() {
            Some(runtime) => runtime,
            None => self
                .engine
                .compile_and_prepare_session(&manifest)
                .map_err(map_phase_diagnostic)?,
        };
        let request = oxvba_host::ImmediateEvaluationRequest::from(request);
        let target_module = request.target_module.clone();
        let mut session = ImmediateSession::new(&self.engine, manifest, runtime);
        if let Some(module_name) = target_module {
            session.set_default_target_module(Some(module_name));
        }
        let result = session
            .evaluate(&request)
            .map_err(|err| WebShellError::Host(err.to_string()))?;
        let runtime = session.into_runtime();
        self.runtime_session = Some(runtime);

        let immediate_result = project_immediate_result(&result.output, &result.diagnostics);
        let mut events = vec![WebHostEvent::ImmediateResult(immediate_result)];
        match &result.output {
            ImmediateEvaluationOutput::PrintedLine(line) => {
                events.push(WebHostEvent::OutputLine {
                    stream: WebOutputStream::Stdout,
                    text: line.clone(),
                });
            }
            ImmediateEvaluationOutput::Value(value) => {
                events.push(WebHostEvent::OutputLine {
                    stream: WebOutputStream::Stdout,
                    text: value.display_text.clone(),
                });
            }
            ImmediateEvaluationOutput::Empty | ImmediateEvaluationOutput::Reset => {}
        }
        Ok(events)
    }

    fn workspace_loaded_events(&self) -> Vec<WebHostEvent> {
        let workspace_session = self.workspace_session.as_ref().expect("workspace session");
        let workspace_path = self.workspace_path.as_ref().expect("workspace path");
        let mut events = vec![project_workspace_loaded(
            workspace_path.display().to_string(),
            &workspace_session.documents(),
        )];
        for document in workspace_session.documents() {
            if let Ok(mut diagnostic_events) = self.document_diagnostics_events(&document.id.0) {
                events.append(&mut diagnostic_events);
            }
        }
        events.push(WebHostEvent::RunStateChanged(WebRunState::Idle));
        events
    }

    fn document_diagnostics_events(
        &self,
        document_id: &str,
    ) -> Result<Vec<WebHostEvent>, WebShellError> {
        let document_id = parse_document_id(document_id)?;
        let workspace_session = self
            .workspace_session
            .as_ref()
            .ok_or(WebShellError::NoWorkspaceLoaded)?;
        let diagnostics = workspace_session
            .diagnostics(&document_id)
            .map_err(|err| WebShellError::Host(err.to_string()))?
            .into_iter()
            .map(WebDiagnostic::from)
            .collect();
        Ok(vec![WebHostEvent::DiagnosticsUpdated {
            document_id: document_id.0,
            diagnostics,
        }])
    }
}

fn parse_document_id(document_id: &str) -> Result<DocumentId, WebShellError> {
    let trimmed = document_id.trim();
    if trimmed.is_empty() {
        Err(WebShellError::InvalidDocumentId(document_id.to_string()))
    } else {
        Ok(DocumentId(trimmed.to_string()))
    }
}

fn map_phase_diagnostic(err: PhaseDiagnostic) -> WebShellError {
    WebShellError::Host(err.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use oxvba_web_host::{
        WebHostCommand, WebHostEvent, WebImmediateDisplayStyle, WebImmediateInputKind,
        WebImmediateOutput, WebRunState,
    };

    use super::{ShellAssetKind, WebShellSession, embedded_assets, shell_manifest};

    #[test]
    fn embedded_assets_include_expected_frontend_files() {
        let assets = embedded_assets();
        assert_eq!(assets.len(), 3);
        assert_eq!(assets[0].path, "index.html");
        assert_eq!(assets[1].path, "app.js");
        assert_eq!(assets[2].path, "styles.css");
        assert!(assets.iter().all(|asset| !asset.contents.trim().is_empty()));
    }

    #[test]
    fn shell_manifest_matches_asset_inventory() {
        let manifest = shell_manifest();
        assert_eq!(manifest.app_name, "oxvba-web-shell");
        assert_eq!(manifest.bridge_contract_version, "v1");
        assert_eq!(manifest.entry_asset_path, "index.html");
        assert_eq!(manifest.assets.len(), 3);
        assert_eq!(manifest.assets[0].kind, ShellAssetKind::Html);
    }

    #[test]
    fn load_workspace_emits_workspace_and_diagnostics() {
        let temp_root = unique_temp_dir("oxvba_web_shell_workspace_load");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");
        fs::write(
            temp_root.join("Main.bas"),
            "Attribute VB_Name = \"Main\"\nPublic Sub Main()\n    Print \"hello\"\nEnd Sub\n",
        )
        .expect("module");

        let mut shell = WebShellSession::new();
        let events = shell
            .handle_command(WebHostCommand::LoadWorkspace {
                path: temp_root.display().to_string(),
            })
            .expect("load workspace");

        assert!(matches!(
            events.first(),
            Some(WebHostEvent::WorkspaceLoaded(_))
        ));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, WebHostEvent::DiagnosticsUpdated { .. }))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, WebHostEvent::RunStateChanged(WebRunState::Idle)))
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn run_and_reset_commands_drive_shell_session() {
        let temp_root = unique_temp_dir("oxvba_web_shell_run_reset");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");
        fs::write(
            temp_root.join("Main.bas"),
            "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nEnd Sub\n",
        )
        .expect("module");

        let mut shell = WebShellSession::new();
        shell
            .handle_command(WebHostCommand::LoadWorkspace {
                path: temp_root.display().to_string(),
            })
            .expect("load");

        let run_events = shell
            .handle_command(WebHostCommand::RunProject)
            .expect("run project");
        assert!(
            run_events
                .iter()
                .any(|event| matches!(event, WebHostEvent::RunStateChanged(WebRunState::Running)))
        );
        assert!(
            run_events.iter().any(|event| matches!(
                event,
                WebHostEvent::RunStateChanged(WebRunState::Completed)
            ))
        );

        let reset_events = shell
            .handle_command(WebHostCommand::ResetRuntime)
            .expect("reset runtime");
        assert!(
            reset_events
                .iter()
                .any(|event| matches!(event, WebHostEvent::RunStateChanged(WebRunState::Idle)))
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn immediate_command_flows_through_shell_runtime() {
        let temp_root = unique_temp_dir("oxvba_web_shell_immediate");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");
        fs::write(
            temp_root.join("Main.bas"),
            "Attribute VB_Name = \"Main\"\nDim counter As Long\nPublic Function IncrementCounter() As Long\n    counter = counter + 1\n    IncrementCounter = counter\nEnd Function\n",
        )
        .expect("module");

        let mut shell = WebShellSession::new();
        shell
            .handle_command(WebHostCommand::LoadWorkspace {
                path: temp_root.display().to_string(),
            })
            .expect("load");

        let events = shell
            .handle_command(WebHostCommand::ImmediateEvaluate(
                oxvba_web_host::WebImmediateRequest {
                    source_text: "IncrementCounter()".to_string(),
                    kind: WebImmediateInputKind::Auto,
                    display_style: WebImmediateDisplayStyle::ImmediateWindow,
                    target_module: Some("Main".to_string()),
                },
            ))
            .expect("immediate");

        assert!(events.iter().any(|event| matches!(
            event,
            WebHostEvent::ImmediateResult(result)
                if matches!(result.output, WebImmediateOutput::Value { .. })
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            WebHostEvent::OutputLine { text, .. } if text == "1"
        )));

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
