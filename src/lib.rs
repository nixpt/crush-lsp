//! # crush-lsp
//!
//! Language Server Protocol implementation for Crush.
//!
//! Ported from `exosphere/crates/ai/services/lsp` ("AI Native LSP Server"), which
//! had real `tower-lsp` protocol wiring and a real hardcoded capability-completion
//! dictionary, but its "AI/ecosystem integration" layer (`JokerAssistantClient`,
//! `CSSClient`, `CESClient`, `CDSClient`) was entirely stubbed — every method
//! either returned `Ok(vec![])` or a hardcoded placeholder string like
//! `"AI-powered hover information"`. That layer is deleted here, not carried
//! over. Two things replace it, both genuinely real:
//!   - Diagnostics now come from `crush-frontend::check_source` — the actual
//!     Crush parser/compiler, not a stub security scanner.
//!   - Hover now calls `ollama::LlmClient` — a real HTTP client against the
//!     fleet's live `pipefish` service (127.0.0.1:11450), not a fake session.

mod ollama;

use ollama::LlmClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// A completion suggestion. `relevance` is a static heuristic used only to
/// order results — it is not a model confidence score.
#[derive(Debug, Clone)]
pub struct CompletionSuggestion {
    pub item: CompletionItem,
    pub relevance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextKind {
    Filesystem,
    Network,
    Crypto,
    Database,
    Capability,
    Execution,
    General,
}

#[derive(Debug, Clone)]
pub struct CodeContext {
    pub kind: ContextKind,
    pub before_cursor: String,
}

pub struct CrushLanguageServer {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, String>>>,
    llm: LlmClient,
}

impl CrushLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            llm: LlmClient::new(),
        }
    }

    fn analyze_context(&self, position: &Position, content: &str) -> CodeContext {
        let line = content.lines().nth(position.line as usize).unwrap_or("");
        let char_idx = (position.character as usize).min(line.len());
        let before_cursor = &line[..char_idx];

        let kind = if before_cursor.ends_with("fs.") {
            ContextKind::Filesystem
        } else if before_cursor.ends_with("net.") {
            ContextKind::Network
        } else if before_cursor.ends_with("crypto.") {
            ContextKind::Crypto
        } else if before_cursor.ends_with("db.") {
            ContextKind::Database
        } else if before_cursor.contains("capability") || before_cursor.contains("cap.") {
            ContextKind::Capability
        } else if before_cursor.ends_with("exec.") || before_cursor.ends_with("run.") {
            ContextKind::Execution
        } else {
            ContextKind::General
        };

        CodeContext {
            kind,
            before_cursor: before_cursor.to_string(),
        }
    }

    /// Static, hand-written completions for Crush's capability API. Real and
    /// fast (no network round-trip) — kept as-is from the original port.
    fn completions_for(&self, context: &CodeContext) -> Vec<CompletionSuggestion> {
        let raw: &[(&str, &str, &str, f64)] = match context.kind {
            ContextKind::Filesystem => &[
                ("read", "Read file contents", "fs.read(path) - Read file contents with capability validation", 0.9),
                ("write", "Write to file", "fs.write(path, data) - Write data to file with security checks", 0.9),
                ("list", "List directory", "fs.list(path) - List directory contents", 0.9),
                ("exists", "Check if path exists", "fs.exists(path) - Check if file/directory exists", 0.9),
                ("mkdir", "Create directory", "fs.mkdir(path) - Create directory with permissions", 0.9),
                ("remove", "Remove file/directory", "fs.remove(path) - Remove file or directory", 0.9),
                ("copy", "Copy file", "fs.copy(src, dst) - Copy file with integrity checks", 0.9),
                ("move", "Move/rename file", "fs.move(src, dst) - Move or rename file", 0.9),
            ],
            ContextKind::Network => &[
                ("connect", "Connect to host", "net.connect(host, port) - Establish network connection", 0.85),
                ("listen", "Listen for connections", "net.listen(port) - Listen for incoming connections", 0.85),
                ("send", "Send data", "net.send(socket, data) - Send data over connection", 0.85),
                ("receive", "Receive data", "net.receive(socket) - Receive data from connection", 0.85),
                ("close", "Close connection", "net.close(socket) - Close network connection", 0.85),
                ("resolve", "DNS resolution", "net.resolve(hostname) - Resolve hostname to IP", 0.85),
                ("ssl_connect", "SSL/TLS connection", "net.ssl_connect(host, port) - Establish encrypted connection", 0.85),
            ],
            ContextKind::Crypto => &[
                ("hash", "Hash data", "crypto.hash(data, algorithm) - Hash data with specified algorithm", 0.95),
                ("encrypt", "Encrypt data", "crypto.encrypt(data, key) - Encrypt data with key", 0.95),
                ("decrypt", "Decrypt data", "crypto.decrypt(data, key) - Decrypt data with key", 0.95),
                ("sign", "Sign data", "crypto.sign(data, key) - Create cryptographic signature", 0.95),
                ("verify", "Verify signature", "crypto.verify(data, signature, key) - Verify cryptographic signature", 0.95),
                ("random", "Generate random", "crypto.random(bytes) - Generate cryptographically secure random bytes", 0.95),
                ("keygen", "Generate key", "crypto.keygen(algorithm) - Generate cryptographic key", 0.95),
            ],
            ContextKind::Database => &[
                ("connect", "Connect to database", "db.connect(url) - Connect to database instance", 0.88),
                ("query", "Execute query", "db.query(sql, params) - Execute SQL query", 0.88),
                ("execute", "Execute statement", "db.execute(sql, params) - Execute SQL statement", 0.88),
                ("transaction", "Start transaction", "db.transaction() - Start database transaction", 0.88),
                ("commit", "Commit transaction", "db.commit() - Commit current transaction", 0.88),
                ("rollback", "Rollback transaction", "db.rollback() - Rollback current transaction", 0.88),
            ],
            ContextKind::Capability => &[
                ("fs.read", "File read capability", "fs.read - Allows reading files", 0.92),
                ("fs.write", "File write capability", "fs.write - Allows writing files", 0.92),
                ("net.connect", "Network connect capability", "net.connect - Allows network connections", 0.92),
                ("process.exec", "Process execution capability", "process.exec - Allows running processes", 0.92),
                ("crypto.hash", "Crypto hash capability", "crypto.hash - Allows cryptographic hashing", 0.92),
                ("database.query", "Database query capability", "database.query - Allows database queries", 0.92),
            ],
            ContextKind::Execution => &[
                ("run", "Execute command", "exec.run(cmd, args) - Execute command with arguments", 0.87),
                ("spawn", "Spawn process", "exec.spawn(cmd, args) - Spawn background process", 0.87),
                ("pipeline", "Create pipeline", "exec.pipeline(cmds) - Create command pipeline", 0.87),
                ("capture", "Capture output", "exec.capture(cmd, args) - Capture command output", 0.87),
                ("timeout", "Execute with timeout", "exec.timeout(cmd, args, duration) - Execute with time limit", 0.87),
            ],
            ContextKind::General => &[
                ("let", "Variable declaration", "let name = value; - Declare variable", 0.8),
                ("fn", "Function definition", "fn name(params) { body } - Define function", 0.8),
                ("if", "Conditional statement", "if condition { body } else { body } - Conditional execution", 0.8),
                ("for", "Loop statement", "for item in collection { body } - Iterate over collection", 0.8),
                ("match", "Pattern matching", "match value { pattern => body, ... } - Pattern matching", 0.8),
                ("struct", "Structure definition", "struct Name { fields } - Define data structure", 0.8),
                ("enum", "Enumeration definition", "enum Name { Variant1, Variant2 } - Define enumeration", 0.8),
            ],
        };

        let kind = if context.kind == ContextKind::Capability {
            CompletionItemKind::VALUE
        } else if context.kind == ContextKind::General {
            CompletionItemKind::KEYWORD
        } else {
            CompletionItemKind::FUNCTION
        };

        raw.iter()
            .map(|(label, detail, doc, relevance)| CompletionSuggestion {
                item: CompletionItem {
                    label: label.to_string(),
                    kind: Some(kind),
                    detail: Some(detail.to_string()),
                    documentation: Some(Documentation::String(doc.to_string())),
                    ..Default::default()
                },
                relevance: *relevance,
            })
            .collect()
    }

    /// Real diagnostics via crush-frontend's actual parser/compiler — replaces
    /// the ported `CSSClient` stub, which always returned zero findings.
    fn real_diagnostics(&self, content: &str) -> Vec<Diagnostic> {
        match crush_frontend::check_source(content) {
            Ok((_program, compiler_diags)) => compiler_diags
                .into_iter()
                .map(|d| {
                    let line = d.location.line.saturating_sub(1);
                    let col = d.location.col.saturating_sub(1);
                    Diagnostic {
                        range: Range {
                            start: Position { line, character: col },
                            end: Position { line, character: col + 1 },
                        },
                        severity: Some(match d.severity {
                            crush_frontend::diagnostics::DiagnosticSeverity::Error => {
                                DiagnosticSeverity::ERROR
                            }
                            crush_frontend::diagnostics::DiagnosticSeverity::Warning => {
                                DiagnosticSeverity::WARNING
                            }
                        }),
                        code: Some(NumberOrString::String(d.code.to_string())),
                        source: Some("crush-lsp".to_string()),
                        message: match d.hint {
                            Some(hint) => format!("{} ({hint})", d.message),
                            None => d.message,
                        },
                        ..Default::default()
                    }
                })
                .collect(),
            Err(e) => {
                // parse_source doesn't expose structured per-error spans through
                // this Result — surface the joined message at line 0 rather than
                // guess a location. Real, just less precise than a compiler
                // pass that succeeded.
                vec![Diagnostic {
                    range: Range {
                        start: Position { line: 0, character: 0 },
                        end: Position { line: 0, character: 1 },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("crush-lsp".to_string()),
                    message: e.to_string(),
                    ..Default::default()
                }]
            }
        }
    }
}

#[async_trait::async_trait]
impl LanguageServer for CrushLanguageServer {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "crush-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("crush-lsp".to_string()),
                        inter_file_dependencies: true,
                        workspace_diagnostics: false,
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: Some(true),
                        },
                    },
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "crush-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        self.documents.write().await.insert(uri.clone(), content.clone());
        let diagnostics = self.real_diagnostics(&content);
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.content_changes[0].text.clone();
        self.documents.write().await.insert(uri.clone(), content.clone());
        let diagnostics = self.real_diagnostics(&content);
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let documents = self.documents.read().await;
        let Some(content) = documents.get(&uri) else {
            return Ok(None);
        };

        let context = self.analyze_context(&position, content);
        let mut suggestions = self.completions_for(&context);
        suggestions.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());

        let items: Vec<CompletionItem> = suggestions.into_iter().map(|s| s.item).collect();
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let documents = self.documents.read().await;
        let Some(content) = documents.get(&uri) else {
            return Ok(None);
        };
        let line = content
            .lines()
            .nth(position.line as usize)
            .unwrap_or("")
            .to_string();
        drop(documents);

        if line.trim().is_empty() {
            return Ok(None);
        }

        match self.llm.explain(&line, position.line + 1).await {
            Ok(explanation) => Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: explanation,
                }),
                range: None,
            })),
            Err(e) => {
                self.client
                    .log_message(MessageType::WARNING, format!("crush-lsp: hover LLM call failed: {e}"))
                    .await;
                Ok(None)
            }
        }
    }
}
