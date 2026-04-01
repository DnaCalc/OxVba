use std::path::PathBuf;

use oxvba_lsp::{OxvbaLspCore, server_capabilities, server_info};
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
        for path in workspace_paths(&params) {
            let _ = self.core.load_workspace_path(&path);
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

fn workspace_paths(params: &InitializeParams) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(folders) = &params.workspace_folders {
        for folder in folders {
            if let Ok(path) = folder.uri.to_file_path() {
                paths.push(path);
            }
        }
    }
    if paths.is_empty()
        && let Some(root_uri) = &params.root_uri
        && let Ok(path) = root_uri.to_file_path()
    {
        paths.push(path);
    }
    paths
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
