pub mod client;
pub mod convert;
pub mod manager;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rhizome_core::{
    BackendCapabilities, CodeIntelligence, Diagnostic, Language, Location, Position, Result,
    Symbol, SymbolKind,
};

use rhizome_core::RhizomeError;

use crate::convert::{
    lsp_diagnostic_to_diagnostic, lsp_location_to_location, lsp_symbol_info_to_symbol,
    lsp_symbol_to_symbol,
};
use crate::manager::LanguageServerManager;

/// LSP-backed code intelligence. Wraps async LSP calls behind the sync
/// `CodeIntelligence` trait using `Handle::block_on`.
///
/// Manages multiple LSP clients keyed by (language, workspace_root) for
/// monorepo support. The workspace root is either passed explicitly via
/// root-aware methods or derived from the file path.
///
/// **Important**: `CodeIntelligence` methods use `block_on` internally, so they
/// must NOT be called from within a tokio async context. Use `spawn_blocking`
/// from async code, or call the async methods on `LspClient` directly.
pub struct LspBackend {
    manager: Arc<tokio::sync::Mutex<LanguageServerManager>>,
    handle: tokio::runtime::Handle,
    /// Default workspace root, used when no per-call root is specified.
    default_root: PathBuf,
}

impl LspBackend {
    pub fn new(workspace_root: PathBuf, handle: tokio::runtime::Handle) -> Self {
        Self {
            manager: Arc::new(tokio::sync::Mutex::new(LanguageServerManager::new())),
            handle,
            default_root: workspace_root,
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
        self.handle.block_on(async {
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
            line: position.line.saturating_sub(1),
            character: position.column.saturating_sub(1),
        };
        self.handle.block_on(async {
            let lang = detect_language(&file)?;
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
        self.handle.block_on(async {
            let lang = detect_language(&file)?;
            let mut mgr = self.manager.lock().await;
            let client = mgr
                .get_client(&lang, &root)
                .await
                .map_err(|e| RhizomeError::LspError(e.to_string()))?;
            let file_str = file.to_string_lossy().to_string();
            let diags = client.cached_diagnostics(&file);
            Ok(diags
                .iter()
                .map(|d| lsp_diagnostic_to_diagnostic(d, &file_str))
                .collect())
        })
    }
}

fn detect_language(file: &Path) -> Result<Language> {
    let ext = file.extension().and_then(|e| e.to_str()).ok_or_else(|| {
        RhizomeError::ParseError(format!("Cannot detect language for: {}", file.display()))
    })?;
    Language::from_extension(ext).ok_or_else(|| RhizomeError::UnsupportedLanguage(ext.to_string()))
}

/// Recursively search a symbol tree for a symbol matching `name`.
fn find_symbol_by_name(symbols: &[Symbol], name: &str) -> Option<Symbol> {
    for sym in symbols {
        if sym.name == name {
            return Some(sym.clone());
        }
        if let Some(found) = find_symbol_by_name(&sym.children, name) {
            return Some(found);
        }
    }
    None
}

impl CodeIntelligence for LspBackend {
    fn get_symbols(&self, file: &Path) -> Result<Vec<Symbol>> {
        self.get_symbols_with_root(file, &self.default_root)
    }

    fn get_definition(&self, file: &Path, name: &str) -> Result<Option<Symbol>> {
        let file = file.to_path_buf();
        let name = name.to_string();
        let root = self.default_root.clone();
        self.handle.block_on(async {
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

            Ok(find_symbol_by_name(&symbols, &name))
        })
    }

    fn find_references(&self, file: &Path, position: &Position) -> Result<Vec<Location>> {
        self.find_references_with_root(file, position, &self.default_root)
    }

    fn search_symbols(&self, pattern: &str, _project_root: &Path) -> Result<Vec<Symbol>> {
        let pattern = pattern.to_string();
        let root = self.default_root.clone();
        self.handle.block_on(async {
            let mut mgr = self.manager.lock().await;
            let languages: Vec<Language> = [
                Language::Rust,
                Language::TypeScript,
                Language::Python,
                Language::Go,
            ]
            .into();

            for lang in &languages {
                if let Ok(client) = mgr.get_client(lang, &root).await {
                    if let Ok(Some(response)) = client.workspace_symbols(&pattern).await {
                        let symbols: Vec<Symbol> = match response {
                            lsp_types::WorkspaceSymbolResponse::Flat(infos) => {
                                infos.iter().map(lsp_symbol_info_to_symbol).collect()
                            }
                            lsp_types::WorkspaceSymbolResponse::Nested(ws_syms) => ws_syms
                                .iter()
                                .map(|ws| {
                                    let location = match &ws.location {
                                        lsp_types::OneOf::Left(loc) => {
                                            lsp_location_to_location(loc)
                                        }
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
                                        kind: crate::convert::lsp_symbol_kind_to_symbol_kind(
                                            ws.kind,
                                        ),
                                        location,
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
