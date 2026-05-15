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

fn detect_language(file: &Path) -> Result<Language> {
    let ext = file.extension().and_then(|e| e.to_str()).ok_or_else(|| {
        RhizomeError::ParseError(format!("Cannot detect language for: {}", file.display()))
    })?;
    Language::from_extension(ext).ok_or_else(|| RhizomeError::UnsupportedLanguage(ext.to_string()))
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
