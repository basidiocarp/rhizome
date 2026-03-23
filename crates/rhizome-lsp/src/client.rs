use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::oneshot;

use crate::convert::{path_to_lsp_uri, uri_to_file_path};
use rhizome_core::LanguageServerConfig;

pub struct LspClient {
    stdin: Arc<tokio::sync::Mutex<BufWriter<ChildStdin>>>,
    pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
    diagnostics_cache: Arc<Mutex<HashMap<String, Vec<lsp_types::Diagnostic>>>>,
    next_id: AtomicI64,
    initialized: bool,
    reader_handle: tokio::task::JoinHandle<()>,
    process: Child,
}

impl LspClient {
    /// Spawn a language server process and start the reader task.
    pub async fn spawn(config: &LanguageServerConfig) -> Result<Self> {
        let mut child = Command::new(&config.binary)
            .args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("Failed to spawn language server: {}", config.binary))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;

        let pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let diagnostics_cache: Arc<Mutex<HashMap<String, Vec<lsp_types::Diagnostic>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let reader_pending = Arc::clone(&pending_requests);
        let reader_diags = Arc::clone(&diagnostics_cache);
        let reader_handle = tokio::spawn(async move {
            Self::reader_task(BufReader::new(stdout), reader_pending, reader_diags).await;
        });

        Ok(Self {
            stdin: Arc::new(tokio::sync::Mutex::new(BufWriter::new(stdin))),
            pending_requests,
            diagnostics_cache,
            next_id: AtomicI64::new(1),
            initialized: false,
            reader_handle,
            process: child,
        })
    }

    /// Initialize the language server with workspace root and capabilities.
    #[allow(
        deprecated,
        reason = "lsp_types::InitializeParams::root_path required by protocol; no non-deprecated alternative"
    )]
    pub async fn initialize(&mut self, workspace_root: &Path) -> Result<()> {
        let uri = path_to_lsp_uri(workspace_root)?;

        let params = lsp_types::InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(uri.clone()),
            capabilities: lsp_types::ClientCapabilities {
                text_document: Some(lsp_types::TextDocumentClientCapabilities {
                    document_symbol: Some(lsp_types::DocumentSymbolClientCapabilities {
                        hierarchical_document_symbol_support: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            workspace_folders: Some(vec![lsp_types::WorkspaceFolder {
                uri,
                name: workspace_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            }]),
            ..Default::default()
        };

        let _result: lsp_types::InitializeResult = self
            .send_request::<lsp_types::request::Initialize>(params)
            .await?;
        self.send_notification::<lsp_types::notification::Initialized>(
            lsp_types::InitializedParams {},
        )
        .await?;
        self.initialized = true;
        Ok(())
    }

    /// Send a typed LSP request and wait for the response (10s timeout).
    async fn send_request<R: lsp_types::request::Request>(
        &self,
        params: R::Params,
    ) -> Result<R::Result> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        self.pending_requests
            .lock()
            .map_err(|_| anyhow::anyhow!("request table poisoned"))?
            .insert(id, tx);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": R::METHOD,
            "params": serde_json::to_value(params)?,
        });

        self.send_message(&request).await?;

        let response = tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .with_context(|| format!("LSP request '{}' timed out", R::METHOD))?
            .with_context(|| format!("LSP reader dropped for '{}'", R::METHOD))?;

        if let Some(error) = response.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("LSP error for '{}': {}", R::METHOD, msg);
        }

        let result = response.get("result").cloned().unwrap_or(Value::Null);
        serde_json::from_value(result)
            .with_context(|| format!("Failed to deserialize '{}' response", R::METHOD))
    }

    /// Send a typed LSP notification (no response expected).
    async fn send_notification<N: lsp_types::notification::Notification>(
        &self,
        params: N::Params,
    ) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": N::METHOD,
            "params": serde_json::to_value(params)?,
        });
        self.send_message(&notification).await
    }

    /// Write a JSON-RPC message with Content-Length header.
    async fn send_message(&self, msg: &Value) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await?;
        stdin.write_all(body.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Request document symbols for a file.
    pub async fn document_symbols(
        &self,
        file: &Path,
    ) -> Result<Option<lsp_types::DocumentSymbolResponse>> {
        let uri = path_to_lsp_uri(file)?;
        let params = lsp_types::DocumentSymbolParams {
            text_document: lsp_types::TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        self.send_request::<lsp_types::request::DocumentSymbolRequest>(params)
            .await
    }

    /// Request go-to-definition at a position.
    pub async fn go_to_definition(
        &self,
        file: &Path,
        position: lsp_types::Position,
    ) -> Result<Option<lsp_types::GotoDefinitionResponse>> {
        let uri = path_to_lsp_uri(file)?;
        let params = lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        self.send_request::<lsp_types::request::GotoDefinition>(params)
            .await
    }

    /// Find all references at a position.
    pub async fn find_references(
        &self,
        file: &Path,
        position: lsp_types::Position,
    ) -> Result<Vec<lsp_types::Location>> {
        let uri = path_to_lsp_uri(file)?;
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position,
            },
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let result = self
            .send_request::<lsp_types::request::References>(params)
            .await?;
        Ok(result.unwrap_or_default())
    }

    /// Request hover information at a position.
    pub async fn hover(
        &self,
        file: &Path,
        position: lsp_types::Position,
    ) -> Result<Option<lsp_types::Hover>> {
        let uri = path_to_lsp_uri(file)?;
        let params = lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
        };
        self.send_request::<lsp_types::request::HoverRequest>(params)
            .await
    }

    /// Request a rename at a position.
    pub async fn rename(
        &self,
        file: &Path,
        position: lsp_types::Position,
        new_name: &str,
    ) -> Result<Option<lsp_types::WorkspaceEdit>> {
        let uri = path_to_lsp_uri(file)?;
        let params = lsp_types::RenameParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position,
            },
            new_name: new_name.to_string(),
            work_done_progress_params: Default::default(),
        };
        self.send_request::<lsp_types::request::Rename>(params)
            .await
    }

    /// Search for workspace symbols matching a query.
    pub async fn workspace_symbols(
        &self,
        query: &str,
    ) -> Result<Option<lsp_types::WorkspaceSymbolResponse>> {
        let params = lsp_types::WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        self.send_request::<lsp_types::request::WorkspaceSymbolRequest>(params)
            .await
    }

    /// Read cached diagnostics for a file (populated from server notifications).
    pub fn cached_diagnostics(&self, file: &Path) -> Vec<lsp_types::Diagnostic> {
        let file_str = file.to_string_lossy().to_string();
        self.diagnostics_cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&file_str)
            .cloned()
            .unwrap_or_default()
    }

    /// Gracefully shut down the language server.
    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self.send_request::<lsp_types::request::Shutdown>(()).await;
        let _ = self
            .send_notification::<lsp_types::notification::Exit>(())
            .await;
        self.reader_handle.abort();
        let _ = self.process.wait().await;
        Ok(())
    }

    /// Check if the language server process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.process.try_wait(), Ok(None))
    }

    /// Background task: reads JSON-RPC messages from stdout and dispatches responses.
    async fn reader_task(
        mut reader: BufReader<tokio::process::ChildStdout>,
        pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
        diagnostics_cache: Arc<Mutex<HashMap<String, Vec<lsp_types::Diagnostic>>>>,
    ) {
        loop {
            // Read headers until blank line
            let mut content_length: usize = 0;
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => return, // EOF
                    Err(e) => {
                        tracing::debug!("LSP reader error: {}", e);
                        return;
                    }
                    Ok(_) => {}
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    break;
                }
                if let Some(len) = parse_content_length(&line) {
                    content_length = len;
                } else {
                    tracing::trace!("Skipping LSP stdout noise: {}", trimmed);
                }
            }

            if content_length == 0 {
                continue;
            }

            // Read body
            let mut body = vec![0u8; content_length];
            if let Err(e) = reader.read_exact(&mut body).await {
                tracing::debug!("LSP reader failed to read body: {}", e);
                return;
            }

            // Parse JSON
            let msg: Value = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("Failed to parse LSP JSON-RPC message: {}", e);
                    continue;
                }
            };

            // Dispatch: response (has "id") vs notification (has "method", no "id")
            if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
                let sender = pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&id);
                if let Some(tx) = sender {
                    let _ = tx.send(msg);
                }
            } else if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
                if method == "textDocument/publishDiagnostics" {
                    if let Some(params) = msg.get("params") {
                        if let Ok(diag_params) = serde_json::from_value::<
                            lsp_types::PublishDiagnosticsParams,
                        >(params.clone())
                        {
                            let file_path = uri_to_file_path(&diag_params.uri);
                            diagnostics_cache
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .insert(file_path, diag_params.diagnostics);
                        }
                    }
                } else {
                    tracing::trace!("LSP notification: {}", method);
                }
            }
        }
    }
}

fn parse_content_length(line: &str) -> Option<usize> {
    let lower = line.to_ascii_lowercase();
    let header = "content-length:";
    let idx = lower.find(header)?;
    let value = line[idx + header.len()..].trim();
    value.split_whitespace().next()?.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_content_length;

    #[test]
    fn test_json_rpc_message_format() {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "textDocument/documentSymbol",
            "params": {
                "textDocument": {
                    "uri": "file:///home/user/main.rs"
                }
            }
        });
        let body = serde_json::to_string(&request).unwrap();
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        assert!(message.starts_with("Content-Length: "));
        assert!(message.contains("\r\n\r\n"));
        let parts: Vec<&str> = message.splitn(2, "\r\n\r\n").collect();
        assert_eq!(parts.len(), 2);
        let declared_len: usize = parts[0]
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(declared_len, parts[1].len());
    }

    #[test]
    fn test_json_rpc_response_parsing() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "result": { "capabilities": {} }
        });

        assert_eq!(response.get("id").unwrap().as_i64(), Some(42));
        assert!(response.get("result").is_some());
        assert!(response.get("error").is_none());
    }

    #[test]
    fn test_json_rpc_error_response() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32600,
                "message": "Invalid request"
            }
        });

        let error = response.get("error").unwrap();
        let msg = error.get("message").unwrap().as_str().unwrap();
        assert_eq!(msg, "Invalid request");
    }

    #[test]
    fn test_json_rpc_notification_has_no_id() {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {}
        });

        assert!(notification.get("id").is_none());
        assert_eq!(
            notification.get("method").unwrap().as_str().unwrap(),
            "textDocument/publishDiagnostics"
        );
    }

    #[test]
    fn test_parse_content_length_matches_header_with_prefix_noise() {
        assert_eq!(
            parse_content_length("booting... Content-Length: 42"),
            Some(42)
        );
    }

    #[test]
    fn test_parse_content_length_is_case_insensitive() {
        assert_eq!(parse_content_length("content-length: 128"), Some(128));
    }

    #[test]
    fn test_parse_content_length_returns_none_for_noise() {
        assert_eq!(parse_content_length("starting up..."), None);
    }
}
