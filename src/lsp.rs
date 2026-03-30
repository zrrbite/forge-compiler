//! Forge Language Server — provides IDE features via the Language Server Protocol.
//!
//! Supports:
//! - Diagnostics (lex and parse errors reported as you type)
//! - Go to definition (functions, structs, variables)
//! - Hover (show function signatures and struct definitions)

use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use forge::ast::*;
use forge::lexer::Lexer;
use forge::lexer::token::Span;
use forge::parser::Parser;

/// A symbol definition: name, kind, location, and hover info.
#[derive(Clone)]
struct SymbolDef {
    name: String,
    #[allow(dead_code)]
    kind: &'static str,
    span: Span,
    hover: String,
}

struct ForgeLanguageServer {
    client: Client,
    documents: Mutex<std::collections::HashMap<Url, String>>,
}

impl ForgeLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn diagnose(&self, source: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
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

    /// Collect symbol definitions from the AST.
    fn collect_symbols(&self, source: &str) -> Vec<SymbolDef> {
        let (tokens, lex_errors) = Lexer::new(source).tokenize();
        if !lex_errors.is_empty() {
            return Vec::new();
        }
        let (program, parse_errors) = Parser::new(tokens).parse();
        if !parse_errors.is_empty() {
            return Vec::new();
        }

        let mut symbols = Vec::new();
        for item in &program.items {
            match &item.kind {
                ItemKind::Function(func) => {
                    let params: Vec<String> = func
                        .params
                        .iter()
                        .map(|p| {
                            let mut s = if p.mutable {
                                "mut ".to_string()
                            } else {
                                String::new()
                            };
                            s.push_str(&p.name);
                            s.push_str(": ");
                            s.push_str(&format_type(&p.ty));
                            s
                        })
                        .collect();
                    let ret = func
                        .return_type
                        .as_ref()
                        .map(|t| format!(" -> {}", format_type(t)))
                        .unwrap_or_default();
                    let hover = format!("fn {}({}){}", func.name, params.join(", "), ret);
                    symbols.push(SymbolDef {
                        name: func.name.clone(),
                        kind: "function",
                        span: item.span,
                        hover,
                    });
                }
                ItemKind::Struct(s) => {
                    let fields: Vec<String> = s
                        .fields
                        .iter()
                        .map(|f| format!("{}: {}", f.name, format_type(&f.ty)))
                        .collect();
                    let hover = format!("struct {} {{\n  {}\n}}", s.name, fields.join(",\n  "));
                    symbols.push(SymbolDef {
                        name: s.name.clone(),
                        kind: "struct",
                        span: item.span,
                        hover,
                    });
                }
                ItemKind::Impl(imp) => {
                    for method in &imp.methods {
                        let params: Vec<String> = method
                            .params
                            .iter()
                            .map(|p| {
                                let mut s = if p.mutable {
                                    "mut ".to_string()
                                } else {
                                    String::new()
                                };
                                s.push_str(&p.name);
                                s.push_str(": ");
                                s.push_str(&format_type(&p.ty));
                                s
                            })
                            .collect();
                        let ret = method
                            .return_type
                            .as_ref()
                            .map(|t| format!(" -> {}", format_type(t)))
                            .unwrap_or_default();
                        let hover = format!(
                            "fn {}.{}({}){}",
                            imp.target,
                            method.name,
                            params.join(", "),
                            ret
                        );
                        // Use the item span since Function doesn't have its own span
                        symbols.push(SymbolDef {
                            name: method.name.clone(),
                            kind: "method",
                            span: item.span,
                            hover,
                        });
                    }
                }
                ItemKind::Trait(t) => {
                    symbols.push(SymbolDef {
                        name: t.name.clone(),
                        kind: "trait",
                        span: item.span,
                        hover: format!("trait {}", t.name),
                    });
                }
                ItemKind::Enum(e) => {
                    symbols.push(SymbolDef {
                        name: e.name.clone(),
                        kind: "enum",
                        span: item.span,
                        hover: format!("enum {}", e.name),
                    });
                }
                _ => {}
            }

            // Collect let bindings from function bodies
            if let ItemKind::Function(func) = &item.kind {
                collect_let_symbols(&func.body.stmts, source, &mut symbols);
            }
        }
        symbols
    }

    /// Find the word at the given position in source.
    fn word_at_position(&self, source: &str, pos: Position) -> Option<String> {
        let offset = line_col_to_byte_offset(source, pos.line, pos.character)?;
        let bytes = source.as_bytes();
        if offset >= bytes.len() {
            return None;
        }
        // Find word boundaries
        let mut start = offset;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        let mut end = offset;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if start == end {
            return None;
        }
        Some(source[start..end].to_string())
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
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "forge-lsp".into(),
                version: Some("0.2.0".into()),
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
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().unwrap();
        let source = match docs.get(&uri) {
            Some(s) => s.clone(),
            None => return Ok(None),
        };
        drop(docs);

        let word = match self.word_at_position(&source, pos) {
            Some(w) => w,
            None => return Ok(None),
        };

        let symbols = self.collect_symbols(&source);
        for sym in &symbols {
            if sym.name == word {
                let (line, col) = byte_offset_to_line_col(&source, sym.span.start);
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range: Range {
                        start: Position::new(line, col),
                        end: Position::new(line, col + word.len() as u32),
                    },
                })));
            }
        }
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().unwrap();
        let source = match docs.get(&uri) {
            Some(s) => s.clone(),
            None => return Ok(None),
        };
        drop(docs);

        let word = match self.word_at_position(&source, pos) {
            Some(w) => w,
            None => return Ok(None),
        };

        let symbols = self.collect_symbols(&source);
        for sym in &symbols {
            if sym.name == word {
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```forge\n{}\n```", sym.hover),
                    }),
                    range: None,
                }));
            }
        }

        // Built-in functions
        let builtin_hover = match word.as_str() {
            "print" => Some("fn print(value) — print to stdout"),
            "input" => Some("fn input(prompt?) — read line from stdin"),
            "to_str" => Some("fn to_str(value) -> str — convert to string"),
            "to_int" => Some("fn to_int(s: str) -> i64 — parse integer"),
            "to_float" => Some("fn to_float(s: str) -> f64 — parse float"),
            "args" => Some("fn args() -> [str] — command-line arguments"),
            "exec" => Some("fn exec(cmd: str, args: [str]) -> ExecResult — run command"),
            "env_get" => Some("fn env_get(key: str) -> str — get env variable"),
            "env_set" => Some("fn env_set(key: str, value: str) — set env variable"),
            "stdin_lines" => Some("fn stdin_lines() -> [str] — read all stdin lines"),
            "HashMap" => Some("fn HashMap() -> Map — create empty hash map"),
            _ => None,
        };

        if let Some(info) = builtin_hover {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```forge\n{info}\n```"),
                }),
                range: None,
            }));
        }

        Ok(None)
    }
}

/// Collect let-binding symbols from a list of statements.
fn collect_let_symbols(stmts: &[Stmt], _source: &str, symbols: &mut Vec<SymbolDef>) {
    for stmt in stmts {
        if let StmtKind::Let {
            name, ty, mutable, ..
        } = &stmt.kind
        {
            let ty_str = ty
                .as_ref()
                .map(|t| format!(": {}", format_type(t)))
                .unwrap_or_default();
            let mut_str = if *mutable { "let mut" } else { "let" };
            let hover = format!("{mut_str} {name}{ty_str}");
            symbols.push(SymbolDef {
                name: name.clone(),
                kind: "variable",
                span: stmt.span,
                hover,
            });
        }
    }
}

/// Format a type expression for display.
fn format_type(ty: &TypeExpr) -> String {
    match &ty.kind {
        TypeExprKind::Named(name) => name.clone(),
        TypeExprKind::Array { element, .. } => format!("[{}]", format_type(element)),
        TypeExprKind::Reference { mutable, inner } => {
            if *mutable {
                format!("&mut {}", format_type(inner))
            } else {
                format!("&{}", format_type(inner))
            }
        }
        TypeExprKind::Generic { name, args } => {
            let args_str: Vec<String> = args.iter().map(format_type).collect();
            format!("{}<{}>", name, args_str.join(", "))
        }
        TypeExprKind::ImplTrait(name) => format!("impl {name}"),
        TypeExprKind::Function {
            params,
            return_type,
        } => {
            let ps: Vec<String> = params.iter().map(format_type).collect();
            format!("fn({}) -> {}", ps.join(", "), format_type(return_type))
        }
    }
}

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

fn line_col_to_byte_offset(source: &str, line: u32, col: u32) -> Option<usize> {
    let mut cur_line = 0u32;
    let mut cur_col = 0u32;
    for (i, ch) in source.char_indices() {
        if cur_line == line && cur_col == col {
            return Some(i);
        }
        if ch == '\n' {
            if cur_line == line {
                return Some(i);
            }
            cur_line += 1;
            cur_col = 0;
        } else {
            cur_col += 1;
        }
    }
    if cur_line == line {
        Some(source.len())
    } else {
        None
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(ForgeLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
