use std::path::PathBuf;

use oxvba_lsp::{OxvbaLspCore, server_capabilities, server_info};
use tower_lsp::jsonrpc::Error;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    core: OxvbaLspCore,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            core: OxvbaLspCore::new(),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(path) = workspace_path(&params).map_err(Error::invalid_params)? {
            self.core
                .load_workspace_path(&path)
                .map_err(Error::invalid_params)?;
        }
        Ok(InitializeResult {
            capabilities: server_capabilities(),
            server_info: Some(server_info()),
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "oxvba-lsp bootstrap shell initialized; semantic features remain transport-neutral in oxvba-languageservice",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if let Err(err) = self
            .core
            .open_text_document(&params.text_document.uri, &params.text_document.text)
        {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("oxvba-lsp didOpen sync failed: {err}"),
                )
                .await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        if let Err(err) = self
            .core
            .change_text_document(&params.text_document.uri, &change.text)
        {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("oxvba-lsp didChange sync failed: {err}"),
                )
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        if let Err(err) = self.core.close_text_document(&params.text_document.uri) {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("oxvba-lsp didClose sync failed: {err}"),
                )
                .await;
        }
    }
}

fn workspace_path(params: &InitializeParams) -> std::result::Result<Option<PathBuf>, String> {
    if let Some(folders) = &params.workspace_folders {
        match folders.as_slice() {
            [] => {}
            [folder] => {
                let path = folder.uri.to_file_path().map_err(|_| {
                    format!(
                        "oxvba-lsp only supports local file workspaces; `{}` is not a file uri",
                        folder.uri
                    )
                })?;
                return Ok(Some(path));
            }
            _ => {
                let names = folders
                    .iter()
                    .map(|folder| folder.name.clone())
                    .collect::<Vec<_>>();
                return Err(format!(
                    "oxvba-lsp currently supports exactly one workspace folder, but received {} ({})",
                    folders.len(),
                    names.join(", ")
                ));
            }
        }
    }

    if let Some(root_uri) = &params.root_uri {
        let path = root_uri.to_file_path().map_err(|_| {
            format!("oxvba-lsp only supports local file workspaces; `{root_uri}` is not a file uri")
        })?;
        return Ok(Some(path));
    }

    Ok(None)
}

fn render_workspace_report(core: &OxvbaLspCore, path: &std::path::Path) -> String {
    let mut lines = Vec::new();
    let documents = core.workspace_documents();
    lines.push(format!("workspace: {}", path.display()));
    lines.push(format!("documents: {}", documents.len()));
    for document in documents {
        let diagnostics = core.document_diagnostics(&document);
        lines.push(format!("{} diagnostics={}", document, diagnostics.len()));
        for diagnostic in diagnostics {
            lines.push(format!(
                "  {:?} {}..{} {}",
                diagnostic.severity, diagnostic.span.start, diagnostic.span.end, diagnostic.message
            ));
        }
    }
    lines.join("\n")
}

fn maybe_run_debug_harness() -> bool {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() <= 1 {
        return false;
    }

    match args[1].as_str() {
        "debug-workspace" => {
            let Some(target) = args.get(2) else {
                eprintln!("usage: oxvba-lsp debug-workspace <path>");
                std::process::exit(2);
            };
            let path = PathBuf::from(target);
            let core = OxvbaLspCore::new();
            if let Err(err) = core.load_workspace_path(&path) {
                eprintln!("oxvba-lsp: failed to load workspace: {err}");
                std::process::exit(1);
            }
            println!("{}", render_workspace_report(&core, &path));
            true
        }
        _ => false,
    }
}

#[tokio::main]
async fn main() {
    if maybe_run_debug_harness() {
        return;
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::{render_workspace_report, workspace_path};
    use oxvba_lsp::OxvbaLspCore;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower_lsp::lsp_types::{InitializeParams, Url, WorkspaceFolder};

    #[test]
    fn debug_workspace_report_lists_documents_and_diagnostics() {
        let temp_root = unique_temp_dir("oxvba_lsp_debug_workspace");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("Module1.bas"),
            "Option Explicit\nSub Main()\n    x = 1\nEnd Sub\n",
        )
        .expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let core = OxvbaLspCore::new();
        core.load_workspace_path(&temp_root)
            .expect("workspace load");

        let report = render_workspace_report(&core, &temp_root);
        assert!(report.contains("workspace:"));
        assert!(report.contains("documents: "));
        assert!(report.contains("Module1"));
        assert!(report.contains("use of undeclared variable"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn workspace_path_accepts_single_workspace_folder() {
        let path = PathBuf::from(r"C:\Temp\OxVba");
        let params = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: Url::from_file_path(&path).expect("workspace uri"),
                name: "OxVba".to_string(),
            }]),
            ..InitializeParams::default()
        };

        let selected = workspace_path(&params).expect("single workspace");
        assert_eq!(selected, Some(path));
    }

    #[test]
    fn workspace_path_rejects_multiple_workspace_folders() {
        let first = PathBuf::from(r"C:\Temp\OxVbaOne");
        let second = PathBuf::from(r"C:\Temp\OxVbaTwo");
        let params = InitializeParams {
            workspace_folders: Some(vec![
                WorkspaceFolder {
                    uri: Url::from_file_path(&first).expect("first workspace uri"),
                    name: "One".to_string(),
                },
                WorkspaceFolder {
                    uri: Url::from_file_path(&second).expect("second workspace uri"),
                    name: "Two".to_string(),
                },
            ]),
            ..InitializeParams::default()
        };

        let err = workspace_path(&params).expect_err("multiple workspaces should fail");
        assert!(err.contains("exactly one workspace folder"));
        assert!(err.contains("One"));
        assert!(err.contains("Two"));
    }

    #[test]
    fn workspace_path_falls_back_to_root_uri_when_workspace_folders_absent() {
        let path = PathBuf::from(r"C:\Temp\OxVbaRoot");
        let params = InitializeParams {
            root_uri: Some(Url::from_file_path(&path).expect("root uri")),
            ..InitializeParams::default()
        };

        let selected = workspace_path(&params).expect("root uri fallback");
        assert_eq!(selected, Some(path));
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nonce}"))
    }
}
