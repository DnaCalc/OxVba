use std::path::PathBuf;

use oxvba_lsp::{OxvbaLspCore, server_capabilities, server_info};
use oxvba_languageservice::{
    CodeActionKind as OxCodeActionKind, CodeActionPlan, CompletionItem as OxCompletionItem,
    CompletionKind as OxCompletionKind, DiagnosticSeverity as OxDiagnosticSeverity, DocumentId,
    DocumentSymbol as OxDocumentSymbol, HoverInfo, Location as OxLocation,
    SemanticClassification, SemanticTokenKind as OxSemanticTokenKind,
    SignatureHelp as OxSignatureHelp, SpannedDiagnostic, TextEdit as OxTextEdit, TextSpan,
    WorkspaceSymbol as OxWorkspaceSymbol,
};
use tower_lsp::jsonrpc::Error;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionOrCommand, CodeActionParams, CompletionItem, CompletionItemKind,
    CompletionParams, CompletionResponse, Diagnostic, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, InitializeParams,
    InitializeResult, InitializedParams, Location, MarkedString, MessageType, Position,
    PrepareRenameResponse, Range, ReferenceParams, RenameParams, SemanticToken,
    SemanticTokens, SemanticTokensParams, SemanticTokensResult, SignatureHelp, SignatureHelpParams,
    SymbolInformation, SymbolKind, TextDocumentIdentifier, TextEdit, Url, WorkspaceEdit,
    WorkspaceSymbolParams,
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

    async fn publish_diagnostics_for_uri(&self, uri: &tower_lsp::lsp_types::Url) {
        let diagnostics = self
            .core
            .synchronized_document_id(uri)
            .map(|document| self.core.document_diagnostics(&document))
            .unwrap_or_default();
        let lsp_diagnostics = diagnostics
            .into_iter()
            .filter_map(|diagnostic| {
                self.core
                    .synchronized_document_id(uri)
                    .and_then(|document| self.core.document_source(&document))
                    .map(|source| diagnostic_to_lsp(&source, diagnostic))
            })
            .collect::<Vec<_>>();
        self.client
            .publish_diagnostics(uri.clone(), lsp_diagnostics, None)
            .await;
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
            return;
        }
        self.publish_diagnostics_for_uri(&params.text_document.uri).await;
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
            return;
        }
        self.publish_diagnostics_for_uri(&params.text_document.uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        if let Err(err) = self.core.close_text_document(&params.text_document.uri) {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("oxvba-lsp didClose sync failed: {err}"),
                )
                .await;
            return;
        }
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let (document, _source, position) =
            resolve_request_position(&self.core, &params.text_document_position_params.text_document, params.text_document_position_params.position)
                .map_err(Error::invalid_params)?;
        Ok(self.core.hover(&document, position).map(hover_to_lsp))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let (document, source, position) =
            resolve_request_position(&self.core, &params.text_document_position_params.text_document, params.text_document_position_params.position)
                .map_err(Error::invalid_params)?;
        let response = self
            .core
            .go_to_definition(&document, position)
            .and_then(|location| location_to_lsp(&self.core, &source, location));
        Ok(response.map(GotoDefinitionResponse::Scalar))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let (document, source, position) =
            resolve_request_position(&self.core, &params.text_document_position.text_document, params.text_document_position.position)
                .map_err(Error::invalid_params)?;
        let locations = self
            .core
            .find_references(&document, position)
            .into_iter()
            .filter_map(|location| location_to_lsp(&self.core, &source, location))
            .collect::<Vec<_>>();
        Ok(Some(locations))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let Some(document) = self.core.synchronized_document_id(uri) else {
            return Ok(None);
        };
        let Some(source) = self.core.document_source(&document) else {
            return Ok(None);
        };
        let symbols = self
            .core
            .document_symbols(&document)
            .into_iter()
            .map(|symbol| document_symbol_to_lsp(&source, symbol))
            .collect::<Vec<_>>();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let symbols = self
            .core
            .workspace_symbols(&params.query)
            .into_iter()
            .filter_map(|symbol| workspace_symbol_to_lsp(&self.core, symbol))
            .collect::<Vec<_>>();
        Ok(Some(symbols))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let (document, _source, position) = resolve_request_position(
            &self.core,
            &params.text_document_position.text_document,
            params.text_document_position.position,
        )
        .map_err(Error::invalid_params)?;
        let items = self
            .core
            .completions(&document, position)
            .into_iter()
            .map(completion_item_to_lsp)
            .collect::<Vec<_>>();
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let (document, _source, position) = resolve_request_position(
            &self.core,
            &params.text_document_position_params.text_document,
            params.text_document_position_params.position,
        )
        .map_err(Error::invalid_params)?;
        Ok(self
            .core
            .signature_help(&document, position)
            .map(signature_help_to_lsp))
    }

    async fn prepare_rename(
        &self,
        params: tower_lsp::lsp_types::TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let (document, source, position) =
            resolve_request_position(&self.core, &params.text_document, params.position)
                .map_err(Error::invalid_params)?;
        let preparation = self
            .core
            .prepare_rename(&document, position)
            .ok_or_else(|| Error::invalid_params("symbol at position is not renameable"))?;
        Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(&source, preparation.placeholder),
            placeholder: preparation.current_name,
        }))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let (document, _source, position) = resolve_request_position(
            &self.core,
            &params.text_document_position.text_document,
            params.text_document_position.position,
        )
        .map_err(Error::invalid_params)?;
        let preparation = self
            .core
            .prepare_rename(&document, position)
            .ok_or_else(|| Error::invalid_params("symbol at position is not renameable"))?;

        if !preparation.reference_analysis.safe_to_apply {
            return Err(Error::invalid_request());
        }

        let mut changes = std::collections::HashMap::<Url, Vec<TextEdit>>::new();
        for location in std::iter::once(preparation.declaration)
            .chain(preparation.reference_analysis.references.into_iter())
        {
            let Some(uri) = self.core.document_uri(&location.document) else {
                continue;
            };
            let Some(source) = self.core.document_source(&location.document) else {
                continue;
            };
            changes.entry(uri).or_default().push(TextEdit {
                range: span_to_range(&source, location.span),
                new_text: params.new_name.clone(),
            });
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }))
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<Vec<CodeActionOrCommand>>> {
        let uri = &params.text_document.uri;
        let Some(document) = self.core.synchronized_document_id(uri) else {
            return Ok(None);
        };
        let Some(source) = self.core.document_source(&document) else {
            return Ok(None);
        };

        let actions = self
            .core
            .code_actions(&document)
            .into_iter()
            .map(|action| code_action_to_lsp(&self.core, &source, action))
            .collect::<Vec<_>>();
        Ok(Some(actions))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;
        let Some(document) = self.core.synchronized_document_id(uri) else {
            return Ok(None);
        };
        let Some(source) = self.core.document_source(&document) else {
            return Ok(None);
        };
        let tokens = semantic_tokens_to_lsp(
            &source,
            self.core.semantic_classifications(&document),
        );
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
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

fn resolve_request_position(
    core: &OxvbaLspCore,
    document: &TextDocumentIdentifier,
    position: Position,
) -> std::result::Result<(DocumentId, String, u32), String> {
    let Some(document_id) = core.synchronized_document_id(&document.uri) else {
        return Err(format!("document `{}` is not synchronized", document.uri));
    };
    let Some(source) = core.document_source(&document_id) else {
        return Err(format!("document `{}` is not loaded", document.uri));
    };
    let offset = lsp_position_to_offset(&source, position)?;
    Ok((document_id, source, offset))
}

fn lsp_position_to_offset(source: &str, position: Position) -> std::result::Result<u32, String> {
    let mut offset = 0usize;
    let mut lines = source.split('\n');
    for _ in 0..position.line {
        let Some(line) = lines.next() else {
            return Err("position line is outside document".to_string());
        };
        offset += line.len() + 1;
    }

    let line = lines.next().unwrap_or("");
    let mut utf16 = 0u32;
    let mut byte_in_line = 0usize;
    for ch in line.chars() {
        if utf16 >= position.character {
            break;
        }
        utf16 += ch.len_utf16() as u32;
        byte_in_line += ch.len_utf8();
    }
    if utf16 < position.character {
        return Err("position character is outside line".to_string());
    }
    Ok((offset + byte_in_line) as u32)
}

fn offset_to_lsp_position(source: &str, offset: u32) -> Position {
    let offset = offset as usize;
    let bounded = offset.min(source.len());
    let prefix = &source[..bounded];
    let line = prefix.as_bytes().iter().filter(|byte| **byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let character = source[line_start..bounded].encode_utf16().count() as u32;
    Position::new(line, character)
}

fn span_to_range(source: &str, span: TextSpan) -> Range {
    Range::new(
        offset_to_lsp_position(source, span.start),
        offset_to_lsp_position(source, span.end),
    )
}

fn diagnostic_to_lsp(source: &str, diagnostic: SpannedDiagnostic) -> Diagnostic {
    Diagnostic {
        range: span_to_range(source, diagnostic.span),
        severity: Some(match diagnostic.severity {
            OxDiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
            OxDiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
        }),
        message: diagnostic.message,
        ..Diagnostic::default()
    }
}

fn hover_to_lsp(hover: HoverInfo) -> Hover {
    let mut lines = vec![hover.label];
    if let Some(detail) = hover.detail {
        lines.push(detail);
    }
    Hover {
        contents: HoverContents::Scalar(MarkedString::String(lines.join("\n"))),
        range: None,
    }
}

fn location_to_lsp(core: &OxvbaLspCore, fallback_source: &str, location: OxLocation) -> Option<Location> {
    let uri = core.document_uri(&location.document)?;
    let source = core
        .document_source(&location.document)
        .unwrap_or_else(|| fallback_source.to_string());
    Some(Location::new(uri, span_to_range(&source, location.span)))
}

fn document_symbol_to_lsp(source: &str, symbol: OxDocumentSymbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name,
        detail: symbol.detail,
        kind: symbol_kind_to_lsp(symbol.kind),
        tags: None,
        deprecated: None,
        range: span_to_range(source, symbol.span),
        selection_range: span_to_range(source, symbol.span),
        children: None,
    }
}

fn workspace_symbol_to_lsp(core: &OxvbaLspCore, symbol: OxWorkspaceSymbol) -> Option<SymbolInformation> {
    let source = core.document_source(&symbol.document)?;
    let uri = core.document_uri(&symbol.document)?;
    Some(SymbolInformation {
        name: symbol.symbol.name,
        kind: symbol_kind_to_lsp(symbol.symbol.kind),
        tags: None,
        deprecated: None,
        location: Location::new(uri, span_to_range(&source, symbol.symbol.span)),
        container_name: symbol.symbol.container_name,
    })
}

fn symbol_kind_to_lsp(kind: oxvba_languageservice::SymbolKind) -> SymbolKind {
    match kind {
        oxvba_languageservice::SymbolKind::Variable => SymbolKind::VARIABLE,
        oxvba_languageservice::SymbolKind::Procedure => SymbolKind::FUNCTION,
        oxvba_languageservice::SymbolKind::Parameter => SymbolKind::VARIABLE,
        oxvba_languageservice::SymbolKind::Constant => SymbolKind::CONSTANT,
        oxvba_languageservice::SymbolKind::TypeDef => SymbolKind::STRUCT,
        oxvba_languageservice::SymbolKind::EnumDef => SymbolKind::ENUM,
        oxvba_languageservice::SymbolKind::EnumMember => SymbolKind::ENUM_MEMBER,
        oxvba_languageservice::SymbolKind::External => SymbolKind::FUNCTION,
        oxvba_languageservice::SymbolKind::Property => SymbolKind::PROPERTY,
        oxvba_languageservice::SymbolKind::Event => SymbolKind::EVENT,
    }
}

fn completion_item_to_lsp(item: OxCompletionItem) -> CompletionItem {
    CompletionItem {
        label: item.label,
        kind: Some(match item.kind {
            OxCompletionKind::Keyword => CompletionItemKind::KEYWORD,
            OxCompletionKind::Variable => CompletionItemKind::VARIABLE,
            OxCompletionKind::Procedure => CompletionItemKind::FUNCTION,
            OxCompletionKind::Parameter => CompletionItemKind::VARIABLE,
            OxCompletionKind::Constant => CompletionItemKind::CONSTANT,
            OxCompletionKind::Intrinsic => CompletionItemKind::FUNCTION,
            OxCompletionKind::Property => CompletionItemKind::PROPERTY,
        }),
        detail: item.detail,
        ..CompletionItem::default()
    }
}

fn signature_help_to_lsp(help: OxSignatureHelp) -> SignatureHelp {
    let label = format!(
        "{}({}) -> {}",
        help.name,
        help.parameters
            .iter()
            .map(|parameter| format!("{} As {}", parameter.name, parameter.type_name))
            .collect::<Vec<_>>()
            .join(", "),
        help.return_type
    );
    SignatureHelp {
        signatures: vec![tower_lsp::lsp_types::SignatureInformation {
            label,
            documentation: None,
            parameters: Some(
                help.parameters
                    .into_iter()
                    .map(|parameter| tower_lsp::lsp_types::ParameterInformation {
                        label: tower_lsp::lsp_types::ParameterLabel::Simple(parameter.name),
                        documentation: Some(
                            tower_lsp::lsp_types::Documentation::String(format!(
                                "As {}",
                                parameter.type_name
                            )),
                        ),
                    })
                    .collect(),
            ),
            active_parameter: Some(help.active_parameter as u32),
        }],
        active_signature: Some(0),
        active_parameter: Some(help.active_parameter as u32),
    }
}

fn code_action_to_lsp(
    core: &OxvbaLspCore,
    source: &str,
    action: CodeActionPlan,
) -> CodeActionOrCommand {
    let uri = core
        .document_uri(&action.document)
        .expect("code action document should resolve to a file uri");
    let edits = action
        .edits
        .into_iter()
        .map(|edit| text_edit_to_lsp(source, edit))
        .collect::<Vec<_>>();
    CodeActionOrCommand::CodeAction(CodeAction {
        title: action.title,
        kind: Some(match action.kind {
            OxCodeActionKind::QuickFix => tower_lsp::lsp_types::CodeActionKind::QUICKFIX,
        }),
        diagnostics: Some(vec![diagnostic_to_lsp(source, action.diagnostic)]),
        edit: Some(WorkspaceEdit {
            changes: Some(std::iter::once((uri, edits)).collect()),
            document_changes: None,
            change_annotations: None,
        }),
        ..CodeAction::default()
    })
}

fn text_edit_to_lsp(source: &str, edit: OxTextEdit) -> TextEdit {
    TextEdit {
        range: span_to_range(source, edit.span),
        new_text: edit.new_text,
    }
}

fn semantic_tokens_to_lsp(source: &str, mut classifications: Vec<SemanticClassification>) -> Vec<SemanticToken> {
    classifications.sort_by_key(|classification| classification.span.start);
    let mut tokens = Vec::new();
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;

    for classification in classifications {
        let position = offset_to_lsp_position(source, classification.span.start);
        let delta_line = position.line.saturating_sub(previous_line);
        let delta_start = if delta_line == 0 {
            position.character.saturating_sub(previous_start)
        } else {
            position.character
        };
        tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length: semantic_token_length(source, classification.span),
            token_type: semantic_token_kind_index(classification.kind),
            token_modifiers_bitset: 0,
        });
        previous_line = position.line;
        previous_start = position.character;
    }

    tokens
}

fn semantic_token_length(source: &str, span: TextSpan) -> u32 {
    let start = span.start as usize;
    let end = (span.end as usize).min(source.len());
    source[start..end].encode_utf16().count() as u32
}

fn semantic_token_kind_index(kind: OxSemanticTokenKind) -> u32 {
    match kind {
        OxSemanticTokenKind::Keyword => 0,
        OxSemanticTokenKind::Procedure => 1,
        OxSemanticTokenKind::Property => 2,
        OxSemanticTokenKind::Variable => 3,
        OxSemanticTokenKind::Parameter => 4,
        OxSemanticTokenKind::Constant => 5,
        OxSemanticTokenKind::TypeDef => 6,
        OxSemanticTokenKind::EnumDef => 7,
        OxSemanticTokenKind::Event => 8,
        OxSemanticTokenKind::External => 9,
        OxSemanticTokenKind::Intrinsic => 1,
    }
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
