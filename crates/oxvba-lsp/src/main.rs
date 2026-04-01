use oxvba_lsp::{OxvbaLspCore, server_capabilities, server_info};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{InitializeParams, InitializeResult, InitializedParams, MessageType};
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    _core: OxvbaLspCore,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            _core: OxvbaLspCore::new(),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
