//! Forge Language Server — provides IDE features via the Language Server Protocol.
//!
//! Currently supports:
//! - Diagnostics (lex and parse errors reported as you type)

use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use forge::lexer::Lexer;
use forge::parser::Parser;

struct ForgeLanguageServer {
    client: Client,
    /// Store document contents for incremental updates.
    documents: Mutex<std::collections::HashMap<Url, String>>,
}

impl ForgeLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Run lexer + parser on source and return diagnostics.
    fn diagnose(&self, source: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Lex
        let (tokens, lex_errors) = Lexer::new(source).tokenize();
        for err in &lex_errors {
            let (line, col) = byte_offset_to_line_col(source, err.span.start);
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position::new(line, col),
                    end: Position::new(line, col + 1),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("forge".into()),
                message: err.message.clone(),
                ..Default::default()
            });
        }

        if lex_errors.is_empty() {
            // Parse
            let (_, parse_errors) = Parser::new(tokens).parse();
            for err in &parse_errors {
                let (line, col) = byte_offset_to_line_col(source, err.span.start);
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position::new(line, col),
                        end: Position::new(line, col + 1),
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("forge".into()),
                    message: err.message.clone(),
                    ..Default::default()
                });
            }
        }

        diagnostics
    }

    async fn publish_diagnostics(&self, uri: Url, source: &str) {
        let diagnostics = self.diagnose(source);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for ForgeLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "forge-lsp".into(),
                version: Some("0.1.0".into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Forge language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        self.documents
            .lock()
            .unwrap()
            .insert(uri.clone(), text.clone());
        self.publish_diagnostics(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().last() {
            self.documents
                .lock()
                .unwrap()
                .insert(uri.clone(), change.text.clone());
            self.publish_diagnostics(uri, &change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.lock().unwrap().remove(&uri);
        // Clear diagnostics
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }
}

/// Convert a byte offset in source to (line, column), both 0-indexed.
fn byte_offset_to_line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(ForgeLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
