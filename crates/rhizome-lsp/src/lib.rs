pub mod client;
pub mod convert;
pub mod edit;
pub mod manager;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rhizome_core::{
    BackendCapabilities, CodeIntelligence, Diagnostic, DiagnosticSeverity, Language, Location,
    Position, Result, Symbol, SymbolKind,
};

use rhizome_core::RhizomeError;
use serde_json::Value;

use crate::convert::{
    lsp_diagnostic_to_diagnostic, lsp_location_to_location, lsp_symbol_info_to_symbol,
    lsp_symbol_to_symbol,
};
use crate::edit::{ApplyResult, PreviewResult, apply_workspace_edit, summarize_workspace_edit};
use crate::manager::LanguageServerManager;

/// LSP-backed code intelligence. Wraps async LSP calls behind the sync
/// `CodeIntelligence` trait using a runtime-safe blocking wrapper.
///
/// Manages multiple LSP clients keyed by (language, workspace_root) for
/// monorepo support. The workspace root is either passed explicitly via
/// root-aware methods or derived from the file path.
///
/// **Important**: `CodeIntelligence` methods are safe to call from Rhizome's
/// async serve path because the implementation uses `block_in_place` when a
/// Tokio runtime is already active. Use the async `LspClient` methods directly
/// only when you need finer-grained control.
type OpenedUrisMap =
    std::collections::HashMap<(String, PathBuf), std::collections::HashSet<String>>;

pub struct LspBackend {
    manager: Arc<tokio::sync::Mutex<LanguageServerManager>>,
    handle: tokio::runtime::Handle,
    /// Default workspace root, used when no per-call root is specified.
    default_root: PathBuf,
    /// Track which URIs have been opened for each workspace.
    opened_uris: Arc<std::sync::Mutex<OpenedUrisMap>>,
}

/// A single completion item returned by the LSP server.
#[derive(serde::Serialize)]
pub struct CompletionItemJson {
    pub label: String,
    pub kind: Option<String>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

/// A call hierarchy item with enough data to pass back to incoming/outgoing calls.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CallHierarchyItemJson {
    pub name: String,
    /// Raw LSP SymbolKind integer for lossless round-tripping.
    pub kind: i32,
    /// Human-readable kind label for display.
    pub kind_label: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    /// Full declaration range start (line).
    pub range_start_line: u32,
    /// Full declaration range start (column).
    pub range_start_column: u32,
    /// Full declaration range end (line).
    pub range_end_line: u32,
    /// Full declaration range end (column).
    pub range_end_column: u32,
    /// Raw LSP item preserved for round-tripping to incoming/outgoing calls.
    pub data: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
pub struct IncomingCallJson {
    pub caller: CallHierarchyItemJson,
    pub ranges: Vec<(u32, u32)>,
}

#[derive(serde::Serialize)]
pub struct OutgoingCallJson {
    pub callee: CallHierarchyItemJson,
    pub ranges: Vec<(u32, u32)>,
}

/// A code action returned by the LSP server.
#[derive(serde::Serialize)]
pub struct CodeActionJson {
    pub title: String,
    pub kind: Option<String>,
    pub is_preferred: Option<bool>,
}

impl LspBackend {
    pub fn new(workspace_root: PathBuf, handle: tokio::runtime::Handle) -> Self {
        Self {
            manager: Arc::new(tokio::sync::Mutex::new(LanguageServerManager::new())),
            handle,
            default_root: workspace_root,
            opened_uris: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn run_blocking<F, T>(&self, future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.handle.block_on(future))
        } else {
            self.handle.block_on(future)
        }
    }

    /// Ensure a file has been opened with the language server by sending didOpen notification.
    /// Returns the URI as a string for tracking purposes.
    async fn ensure_did_open(
        &self,
        file: &Path,
        lang: &Language,
        workspace_root: &Path,
    ) -> Result<String> {
        let uri = crate::convert::path_to_lsp_uri(file)
            .map_err(|e| RhizomeError::LspError(e.to_string()))?;
        let uri_str = uri.to_string();
        let key = (lang.to_string(), workspace_root.to_path_buf());

        {
            let tracker = self
                .opened_uris
                .lock()
                .map_err(|_| RhizomeError::LspError("opened_uris lock poisoned".to_string()))?;
            if let Some(opened) = tracker.get(&key)
                && opened.contains(&uri_str)
            {
                return Ok(uri_str);
            }
        }

        // File not opened yet; acquire the manager lock and re-check before sending didOpen.
        // A concurrent caller may have won the race between the first check and here.
        let file_path = file.to_path_buf();
        let content = tokio::task::spawn_blocking(move || std::fs::read_to_string(&file_path))
            .await
            .map_err(|e| RhizomeError::LspError(format!("Failed to spawn read task: {}", e)))?
            .map_err(|e| RhizomeError::LspError(format!("Failed to read file: {}", e)))?;
        let language_id = lang.lsp_language_id();

        let mut mgr = self.manager.lock().await;

        // Re-check under manager lock to close the TOCTOU window.
        {
            let tracker = self
                .opened_uris
                .lock()
                .map_err(|_| RhizomeError::LspError("opened_uris lock poisoned".to_string()))?;
            if let Some(opened) = tracker.get(&key)
                && opened.contains(&uri_str)
            {
                return Ok(uri_str);
            }
        }

        let client = mgr
            .get_client(lang, workspace_root)
            .await
            .map_err(|e| RhizomeError::LspError(e.to_string()))?;

        match client.did_open(&uri, &language_id, &content).await {
            Ok(()) => {
                // Track that this URI is now open only on confirmed success
                let mut tracker = self
                    .opened_uris
                    .lock()
                    .map_err(|_| RhizomeError::LspError("opened_uris lock poisoned".to_string()))?;
                tracker
                    .entry(key)
                    .or_insert_with(std::collections::HashSet::new)
                    .insert(uri_str.clone());
                Ok(uri_str)
            }
            Err(e) => {
                // did_open failed or timed out; do not insert URI
                Err(RhizomeError::LspError(format!("didOpen failed: {}", e)))
            }
        }
    }

    /// Force-restart one or all LSP server clients.
    ///
    /// Takes an optional JSON object with `language` (string) and `root` (string) fields.
    /// When both are absent, restarts all clients.
    /// When a language is provided, restarts that language's client for the given root
    /// (or default root if `root` is not provided).
    pub fn restart_client(&self, args: &Value) -> anyhow::Result<Value> {
        let language_str = args.get("language").and_then(|v| v.as_str());

        let root = args
            .get("root")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.default_root.clone());

        // Resolve the optional language. Absent language => restart all clients.
        // A *present but unrecognized* language is a hard error: silently falling
        // back to None would restart every client, the opposite of the caller's
        // intent on a typo or unsupported language.
        let target = match language_str {
            None => None,
            Some(lang_str) => {
                let lang = Language::from_extension(lang_str)
                    .or_else(|| Language::from_name(lang_str))
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Unrecognized language '{lang_str}'. \
                             Pass a known language name (e.g. 'rust', 'typescript') \
                             or file extension (e.g. 'rs', 'ts'), or omit `language` to restart all clients."
                        )
                    })?;
                Some((lang, root.clone()))
            }
        };

        let result = self.run_blocking(async {
            let mut mgr = self.manager.lock().await;
            let results = mgr.restart_client(target).await;

            // A restarted client is a fresh process with no documents open. Drop the
            // stale opened_uris tracking for every restarted key (success or failure —
            // the old client was force-dropped either way) so the next request re-sends
            // didOpen instead of short-circuiting against a server that never received it.
            {
                let mut tracker = self
                    .opened_uris
                    .lock()
                    .map_err(|_| RhizomeError::LspError("opened_uris lock poisoned".to_string()))?;
                for (key, _) in &results {
                    tracker.remove(&(key.0.to_string(), key.1.clone()));
                }
            }

            // Build response with success and failure summaries
            let mut restarted = Vec::new();
            let mut failed = Vec::new();

            for (key, result) in results {
                let key_str = format!("{:?} at {}", key.0, key.1.display());
                match result {
                    Ok(()) => restarted.push(key_str),
                    Err(e) => failed.push(format!("{}: {}", key_str, e)),
                }
            }

            let response_msg = if failed.is_empty() {
                format!(
                    "Restarted {} LSP client(s): {}",
                    restarted.len(),
                    restarted.join(", ")
                )
            } else {
                format!(
                    "Restarted {} LSP client(s): {}\nFailed to restart: {}",
                    restarted.len(),
                    if restarted.is_empty() {
                        "(none)".to_string()
                    } else {
                        restarted.join(", ")
                    },
                    failed.join("; ")
                )
            };

            Ok(serde_json::json!({
                "restarted": restarted,
                "failed": failed,
                "message": response_msg
            }))
        });

        result.map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Shut down all managed language servers.
    pub async fn shutdown(&self) -> Result<()> {
        self.manager
            .lock()
            .await
            .shutdown_all()
            .await
            .map_err(|e| RhizomeError::LspError(e.to_string()))
    }

    // ─────────────────────────────────────────────────────────────────────
    // Root-aware methods for the ToolDispatcher
    // ─────────────────────────────────────────────────────────────────────

    /// Get symbols using a specific workspace root.
    pub fn get_symbols_with_root(&self, file: &Path, workspace_root: &Path) -> Result<Vec<Symbol>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let response = client
                .document_symbols(&file)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let file_str = file.to_string_lossy().to_string();

            Ok(match response {
                Some(lsp_types::DocumentSymbolResponse::Nested(syms)) => syms
                    .iter()
                    .map(|s| lsp_symbol_to_symbol(s, &file_str))
                    .collect(),
                Some(lsp_types::DocumentSymbolResponse::Flat(infos)) => {
                    infos.iter().map(lsp_symbol_info_to_symbol).collect()
                }
                None => vec![],
            })
        })
    }

    /// Find references using a specific workspace root.
    pub fn find_references_with_root(
        &self,
        file: &Path,
        position: &Position,
        workspace_root: &Path,
    ) -> Result<Vec<Location>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let refs = client
                .find_references(&file, lsp_pos)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(refs.iter().map(lsp_location_to_location).collect())
        })
    }

    /// Get hover documentation for a symbol using a specific workspace root.
    pub fn hover_with_root(
        &self,
        file: &Path,
        position: &Position,
        workspace_root: &Path,
    ) -> Result<Option<String>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let hover = client
                .hover(&file, lsp_pos)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(hover.map(|h| extract_hover_text(&h)))
        })
    }

    /// Get implementation locations for a symbol using a specific workspace root.
    pub fn go_to_implementation_with_root(
        &self,
        file: &Path,
        position: &Position,
        workspace_root: &Path,
    ) -> Result<Vec<Location>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let locs = client
                .go_to_implementation(&file, lsp_pos)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(locs
                .unwrap_or_default()
                .iter()
                .map(lsp_location_to_location)
                .collect())
        })
    }

    /// Get type definition locations for a symbol using a specific workspace root.
    pub fn go_to_type_definition_with_root(
        &self,
        file: &Path,
        position: &Position,
        workspace_root: &Path,
    ) -> Result<Vec<Location>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let locs = client
                .go_to_type_definition(&file, lsp_pos)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(locs
                .unwrap_or_default()
                .iter()
                .map(lsp_location_to_location)
                .collect())
        })
    }

    /// Get completion items at a position using a specific workspace root.
    pub fn completion_with_root(
        &self,
        file: &Path,
        position: &Position,
        trigger_character: Option<char>,
        workspace_root: &Path,
    ) -> Result<Vec<CompletionItemJson>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let response = client
                .completion(&file, lsp_pos, trigger_character)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let items: Vec<lsp_types::CompletionItem> = match response {
                Some(lsp_types::CompletionResponse::Array(items)) => items,
                Some(lsp_types::CompletionResponse::List(list)) => list.items,
                None => vec![],
            };
            Ok(items
                .into_iter()
                .take(50)
                .map(|item| CompletionItemJson {
                    label: item.label,
                    kind: item.kind.map(completion_kind_to_string),
                    detail: item.detail,
                    documentation: item.documentation.map(doc_to_string),
                })
                .collect())
        })
    }

    /// Get signature help at a position using a specific workspace root.
    pub fn signature_help_with_root(
        &self,
        file: &Path,
        position: &Position,
        workspace_root: &Path,
    ) -> Result<Option<serde_json::Value>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let help = client
                .signature_help(&file, lsp_pos)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(help.map(|h| {
                let signatures: Vec<String> =
                    h.signatures.iter().map(|sig| sig.label.clone()).collect();
                serde_json::json!({
                    "signatures": signatures,
                    "active_signature": h.active_signature,
                    "active_parameter": h.active_parameter,
                })
            }))
        })
    }

    /// Prepare call hierarchy items at a position using a specific workspace root.
    pub fn prepare_call_hierarchy_with_root(
        &self,
        file: &Path,
        position: &Position,
        workspace_root: &Path,
    ) -> Result<Vec<CallHierarchyItemJson>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let items = client
                .prepare_call_hierarchy(&file, lsp_pos)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(items
                .unwrap_or_default()
                .iter()
                .map(lsp_call_hierarchy_item_to_json)
                .collect())
        })
    }

    /// Get incoming callers for a call hierarchy item.
    pub fn incoming_calls_with_root(
        &self,
        item_json: &CallHierarchyItemJson,
        workspace_root: &Path,
    ) -> Result<Vec<IncomingCallJson>> {
        let root = workspace_root.to_path_buf();
        let lsp_item = json_to_lsp_call_hierarchy_item(item_json)?;
        self.run_blocking(async {
            let mut mgr = self.manager.lock().await;
            // Use a heuristic language for the manager lookup based on file extension
            let file = std::path::PathBuf::from(&item_json.file.replace("file://", ""));
            let lang = detect_language(&file).unwrap_or(Language::Rust);
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let calls = client
                .incoming_calls(lsp_item)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(calls
                .unwrap_or_default()
                .into_iter()
                .map(|c| IncomingCallJson {
                    caller: lsp_call_hierarchy_item_to_json(&c.from),
                    ranges: c
                        .from_ranges
                        .iter()
                        .map(|r| (r.start.line, r.start.character))
                        .collect(),
                })
                .collect())
        })
    }

    /// Get outgoing callees for a call hierarchy item.
    pub fn outgoing_calls_with_root(
        &self,
        item_json: &CallHierarchyItemJson,
        workspace_root: &Path,
    ) -> Result<Vec<OutgoingCallJson>> {
        let root = workspace_root.to_path_buf();
        let lsp_item = json_to_lsp_call_hierarchy_item(item_json)?;
        self.run_blocking(async {
            let mut mgr = self.manager.lock().await;
            let file = std::path::PathBuf::from(&item_json.file.replace("file://", ""));
            let lang = detect_language(&file).unwrap_or(Language::Rust);
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let calls = client
                .outgoing_calls(lsp_item)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(calls
                .unwrap_or_default()
                .into_iter()
                .map(|c| OutgoingCallJson {
                    callee: lsp_call_hierarchy_item_to_json(&c.to),
                    ranges: c
                        .from_ranges
                        .iter()
                        .map(|r| (r.start.line, r.start.character))
                        .collect(),
                })
                .collect())
        })
    }

    /// Get code actions for a range using a specific workspace root.
    pub fn code_actions_with_root(
        &self,
        file: &Path,
        start: &Position,
        end: &Position,
        workspace_root: &Path,
    ) -> Result<Vec<CodeActionJson>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let range = lsp_types::Range {
            start: lsp_types::Position {
                line: start.line,
                character: start.column,
            },
            end: lsp_types::Position {
                line: end.line,
                character: end.column,
            },
        };
        let context = lsp_types::CodeActionContext {
            diagnostics: vec![],
            only: None,
            trigger_kind: None,
        };
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let actions = client
                .code_actions(&file, range, context)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            Ok(actions
                .unwrap_or_default()
                .into_iter()
                .filter_map(|a| match a {
                    lsp_types::CodeActionOrCommand::CodeAction(action) => Some(CodeActionJson {
                        title: action.title,
                        kind: action.kind.map(|k| k.as_str().to_string()),
                        is_preferred: action.is_preferred,
                    }),
                    lsp_types::CodeActionOrCommand::Command(_) => None,
                })
                .collect())
        })
    }

    /// Get diagnostics using a specific workspace root.
    pub fn get_diagnostics_with_root(
        &self,
        file: &Path,
        workspace_root: &Path,
    ) -> Result<Vec<Diagnostic>> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let file_str = file.to_string_lossy().to_string();
            let diags = client.cached_diagnostics(&file);

            // If cache is empty, return an informative hint rather than silently empty
            if diags.is_empty() {
                return Ok(vec![Diagnostic {
                    message: "No cached diagnostics. The LSP server publishes diagnostics in response to document open/change notifications. Diagnostics will populate after the language server indexes the file.".to_string(),
                    severity: DiagnosticSeverity::Hint,
                    location: Location {
                        file_path: file_str,
                        line_start: 0,
                        line_end: 0,
                        column_start: 0,
                        column_end: 0,
                    },
                }]);
            }

            Ok(diags
                .iter()
                .map(|d| lsp_diagnostic_to_diagnostic(d, &file_str))
                .collect())
        })
    }

    /// Rename a symbol using LSP and apply the resulting workspace edit.
    pub fn rename_with_root(
        &self,
        file: &Path,
        position: &Position,
        new_name: &str,
        workspace_root: &Path,
    ) -> Result<ApplyResult> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let new_name = new_name.to_string();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };

        self.run_blocking(async {
            let lang = detect_language(&file)?;
            self.ensure_did_open(&file, &lang, &root).await?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;

            let edit = client
                .rename(&file, lsp_pos, &new_name)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;

            let edit = edit.ok_or_else(|| {
                RhizomeError::LspError(
                    "language server returned no workspace edit for rename".into(),
                )
            })?;

            apply_workspace_edit(&edit).map_err(|e| RhizomeError::LspError(e.to_string()))
        })
    }

    /// Request an LSP rename and return a summary without applying the workspace edit.
    pub fn preview_rename_with_root(
        &self,
        file: &Path,
        position: &Position,
        new_name: &str,
        workspace_root: &Path,
    ) -> Result<PreviewResult> {
        let file = file.to_path_buf();
        let root = workspace_root.to_path_buf();
        let new_name = new_name.to_string();
        let lsp_pos = lsp_types::Position {
            line: position.line,
            character: position.column,
        };

        self.run_blocking(async {
            let lang = detect_language(&file)?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;

            let edit = client
                .rename(&file, lsp_pos, &new_name)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;

            let edit = edit.ok_or_else(|| {
                RhizomeError::LspError(
                    "language server returned no workspace edit for rename".into(),
                )
            })?;

            summarize_workspace_edit(&edit).map_err(|e| RhizomeError::LspError(e.to_string()))
        })
    }
}

fn extract_hover_text(hover: &lsp_types::Hover) -> String {
    match &hover.contents {
        lsp_types::HoverContents::Scalar(markup) => markup_to_string(markup),
        lsp_types::HoverContents::Array(markups) => markups
            .iter()
            .map(markup_to_string)
            .collect::<Vec<_>>()
            .join("\n\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value.clone(),
    }
}

fn markup_to_string(markup: &lsp_types::MarkedString) -> String {
    match markup {
        lsp_types::MarkedString::String(s) => s.clone(),
        lsp_types::MarkedString::LanguageString(ls) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}

fn detect_language(file: &Path) -> Result<Language> {
    let ext = file.extension().and_then(|e| e.to_str()).ok_or_else(|| {
        RhizomeError::ParseError(format!("Cannot detect language for: {}", file.display()))
    })?;
    Language::from_extension(ext).ok_or_else(|| RhizomeError::UnsupportedLanguage(ext.to_string()))
}

fn completion_kind_to_string(kind: lsp_types::CompletionItemKind) -> String {
    match kind {
        lsp_types::CompletionItemKind::TEXT => "Text",
        lsp_types::CompletionItemKind::METHOD => "Method",
        lsp_types::CompletionItemKind::FUNCTION => "Function",
        lsp_types::CompletionItemKind::CONSTRUCTOR => "Constructor",
        lsp_types::CompletionItemKind::FIELD => "Field",
        lsp_types::CompletionItemKind::VARIABLE => "Variable",
        lsp_types::CompletionItemKind::CLASS => "Class",
        lsp_types::CompletionItemKind::INTERFACE => "Interface",
        lsp_types::CompletionItemKind::MODULE => "Module",
        lsp_types::CompletionItemKind::PROPERTY => "Property",
        lsp_types::CompletionItemKind::UNIT => "Unit",
        lsp_types::CompletionItemKind::VALUE => "Value",
        lsp_types::CompletionItemKind::ENUM => "Enum",
        lsp_types::CompletionItemKind::KEYWORD => "Keyword",
        lsp_types::CompletionItemKind::SNIPPET => "Snippet",
        lsp_types::CompletionItemKind::COLOR => "Color",
        lsp_types::CompletionItemKind::FILE => "File",
        lsp_types::CompletionItemKind::REFERENCE => "Reference",
        lsp_types::CompletionItemKind::FOLDER => "Folder",
        lsp_types::CompletionItemKind::ENUM_MEMBER => "EnumMember",
        lsp_types::CompletionItemKind::CONSTANT => "Constant",
        lsp_types::CompletionItemKind::STRUCT => "Struct",
        lsp_types::CompletionItemKind::EVENT => "Event",
        lsp_types::CompletionItemKind::OPERATOR => "Operator",
        lsp_types::CompletionItemKind::TYPE_PARAMETER => "TypeParameter",
        _ => "Unknown",
    }
    .to_string()
}

fn doc_to_string(doc: lsp_types::Documentation) -> String {
    match doc {
        lsp_types::Documentation::String(s) => s,
        lsp_types::Documentation::MarkupContent(m) => m.value,
    }
}

fn lsp_call_hierarchy_item_to_json(item: &lsp_types::CallHierarchyItem) -> CallHierarchyItemJson {
    let kind = serde_json::to_value(item.kind)
        .ok()
        .and_then(|v| v.as_i64())
        .and_then(|v| i32::try_from(v).ok())
        .unwrap_or(12); // SymbolKind::FUNCTION
    CallHierarchyItemJson {
        name: item.name.clone(),
        kind,
        kind_label: format!("{:?}", item.kind),
        file: item.uri.to_string(),
        line: item.selection_range.start.line,
        column: item.selection_range.start.character,
        range_start_line: item.range.start.line,
        range_start_column: item.range.start.character,
        range_end_line: item.range.end.line,
        range_end_column: item.range.end.character,
        data: item.data.clone(),
    }
}

fn json_to_lsp_call_hierarchy_item(
    item: &CallHierarchyItemJson,
) -> Result<lsp_types::CallHierarchyItem> {
    let uri = item
        .file
        .parse::<lsp_types::Uri>()
        .map_err(|_| RhizomeError::LspError(format!("Invalid file URI: {}", item.file)))?;
    let selection_pos = lsp_types::Position {
        line: item.line,
        character: item.column,
    };
    let range = lsp_types::Range {
        start: lsp_types::Position {
            line: item.range_start_line,
            character: item.range_start_column,
        },
        end: lsp_types::Position {
            line: item.range_end_line,
            character: item.range_end_column,
        },
    };
    let kind: lsp_types::SymbolKind = serde_json::from_value(serde_json::json!(item.kind))
        .unwrap_or(lsp_types::SymbolKind::FUNCTION);

    Ok(lsp_types::CallHierarchyItem {
        name: item.name.clone(),
        kind,
        tags: None,
        detail: None,
        uri,
        range,
        selection_range: lsp_types::Range {
            start: selection_pos,
            end: selection_pos,
        },
        data: item.data.clone(),
    })
}

impl CodeIntelligence for LspBackend {
    fn get_symbols(&self, file: &Path) -> Result<Vec<Symbol>> {
        self.get_symbols_with_root(file, &self.default_root)
    }

    fn get_definition(&self, file: &Path, name: &str) -> Result<Option<Symbol>> {
        let file = file.to_path_buf();
        let name = name.to_string();
        let root = self.default_root.clone();
        self.run_blocking(async {
            let lang = detect_language(&file)?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let response = client
                .document_symbols(&file)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let file_str = file.to_string_lossy().to_string();

            let symbols: Vec<Symbol> = match response {
                Some(lsp_types::DocumentSymbolResponse::Nested(syms)) => syms
                    .iter()
                    .map(|s| lsp_symbol_to_symbol(s, &file_str))
                    .collect(),
                Some(lsp_types::DocumentSymbolResponse::Flat(infos)) => {
                    infos.iter().map(lsp_symbol_info_to_symbol).collect()
                }
                None => vec![],
            };

            // Find all matching symbols to detect disambiguation cases
            let mut candidates = Vec::new();
            fn collect_by_name(symbols: &[Symbol], target_name: &str, results: &mut Vec<Symbol>) {
                for sym in symbols {
                    if sym.name == target_name {
                        results.push(sym.clone());
                    }
                    collect_by_name(&sym.children, target_name, results);
                }
            }
            collect_by_name(&symbols, &name, &mut candidates);

            match candidates.len() {
                0 => Ok(None),
                1 => Ok(candidates.into_iter().next()),
                _ => {
                    let uris = candidates
                        .iter()
                        .map(|c| format!("{}:{}", c.location.file_path, c.location.line_start))
                        .collect::<Vec<_>>()
                        .join(", ");
                    Err(RhizomeError::LspError(format!(
                        "ambiguous symbol '{}': {} definitions found at {}",
                        name,
                        candidates.len(),
                        uris
                    )))
                }
            }
        })
    }

    fn find_references(&self, file: &Path, position: &Position) -> Result<Vec<Location>> {
        self.find_references_with_root(file, position, &self.default_root)
    }

    fn search_symbols(&self, pattern: &str, _project_root: &Path) -> Result<Vec<Symbol>> {
        let pattern = pattern.to_string();
        let root = self.default_root.clone();
        self.run_blocking(async {
            let mut mgr = self.manager.lock().await;
            let languages: Vec<Language> = [
                Language::Rust,
                Language::TypeScript,
                Language::Python,
                Language::Go,
            ]
            .into();

            for lang in &languages {
                match mgr.get_client(lang, &root).await {
                    Ok(client) => {
                        if let Ok(Some(response)) = client.workspace_symbols(&pattern).await {
                            let symbols: Vec<Symbol> = match response {
                                lsp_types::WorkspaceSymbolResponse::Flat(infos) => {
                                    infos.iter().map(lsp_symbol_info_to_symbol).collect()
                                }
                                lsp_types::WorkspaceSymbolResponse::Nested(ws_syms) => ws_syms
                                    .iter()
                                    .map(|ws| {
                                        let location = match &ws.location {
                                            lsp_types::OneOf::Left(loc) => lsp_location_to_location(loc),
                                            lsp_types::OneOf::Right(ws_loc) => {
                                                let file_path =
                                                    crate::convert::uri_to_file_path(&ws_loc.uri);
                                                Location {
                                                    file_path,
                                                    line_start: 0,
                                                    line_end: 0,
                                                    column_start: 0,
                                                    column_end: 0,
                                                }
                                            }
                                        };
                                        Symbol {
                                            name: ws.name.clone(),
                                            kind: crate::convert::lsp_symbol_kind_to_symbol_kind(ws.kind),
                                            location,
                                            scope_path: ws
                                                .container_name
                                                .clone()
                                                .map(|container| vec![container])
                                                .unwrap_or_default(),
                                            signature: ws.container_name.clone(),
                                            doc_comment: None,
                                            children: vec![],
                                        }
                                    })
                                    .collect(),
                            };
                            if !symbols.is_empty() {
                                return Ok(symbols);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(lang = %lang, error = %e, "search_symbols: LSP client unavailable, skipping");
                        continue;
                    }
                }
            }
            Ok(vec![])
        })
    }

    fn get_imports(&self, file: &Path) -> Result<Vec<Symbol>> {
        let symbols = self.get_symbols(file)?;
        Ok(symbols
            .into_iter()
            .filter(|s| s.kind == SymbolKind::Import)
            .collect())
    }

    fn get_diagnostics(&self, file: &Path) -> Result<Vec<Diagnostic>> {
        self.get_diagnostics_with_root(file, &self.default_root)
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            cross_file_references: true,
            rename: true,
            type_info: true,
            diagnostics: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_hierarchy_item_round_trips_kind_and_range() {
        let uri = "file:///src/lib.rs".parse::<lsp_types::Uri>().unwrap();
        let item = lsp_types::CallHierarchyItem {
            name: "my_method".to_string(),
            kind: lsp_types::SymbolKind::METHOD,
            tags: None,
            detail: None,
            uri,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 25,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 10,
                    character: 13,
                },
            },
            data: Some(serde_json::json!({"server_data": 42})),
        };

        let json = lsp_call_hierarchy_item_to_json(&item);

        // Verify the kind serialized to the expected numeric value.
        let expected_kind_int = serde_json::to_value(lsp_types::SymbolKind::METHOD)
            .unwrap()
            .as_i64()
            .unwrap() as i32;
        assert_eq!(json.kind, expected_kind_int);
        assert_eq!(json.range_start_line, 10);
        assert_eq!(json.range_start_column, 0);
        assert_eq!(json.range_end_line, 25);
        assert_eq!(json.range_end_column, 1);
        assert_eq!(json.line, 10);
        assert_eq!(json.column, 4);

        let restored = json_to_lsp_call_hierarchy_item(&json).unwrap();
        assert_eq!(restored.kind, lsp_types::SymbolKind::METHOD);
        assert_eq!(restored.range.start.line, 10);
        assert_eq!(restored.range.start.character, 0);
        assert_eq!(restored.range.end.line, 25);
        assert_eq!(restored.range.end.character, 1);
        assert_eq!(restored.data, Some(serde_json::json!({"server_data": 42})));
    }
}
