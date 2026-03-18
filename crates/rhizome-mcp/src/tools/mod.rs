pub mod edit_tools;
pub mod export_tools;
pub mod file_tools;
pub mod symbol_tools;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use rhizome_core::{BackendSelector, Language, ResolvedBackend, RhizomeConfig};
use rhizome_treesitter::TreeSitterBackend;
use serde::Serialize;
use serde_json::{json, Value};

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Schema describing an MCP tool for the `tools/list` response.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Internal enum for resolved backend after lazy init.
enum ActiveBackend {
    TreeSitter,
    Lsp,
    Error(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// ToolDispatcher
// ─────────────────────────────────────────────────────────────────────────────

/// Routes MCP tool calls to the appropriate backend handler.
///
/// Uses `RefCell` for the LSP backend and selector to allow lazy initialization
/// and caching while keeping `call_tool(&self, ...)` unchanged. This is safe
/// because the MCP server loop is single-threaded.
pub struct ToolDispatcher {
    treesitter: TreeSitterBackend,
    lsp: RefCell<Option<rhizome_lsp::LspBackend>>,
    selector: RefCell<BackendSelector>,
    project_root: PathBuf,
}

impl ToolDispatcher {
    pub fn new(project_root: PathBuf) -> Self {
        let config = RhizomeConfig::load(&project_root).unwrap_or_default();
        Self {
            treesitter: TreeSitterBackend::new(),
            lsp: RefCell::new(None),
            selector: RefCell::new(BackendSelector::new(config)),
            project_root,
        }
    }

    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            // ── Symbol tools (tree-sitter) ──────────────────────────────
            "get_symbols" => symbol_tools::get_symbols(&self.treesitter, &args),
            "get_structure" => symbol_tools::get_structure(&self.treesitter, &args),
            "get_definition" => symbol_tools::get_definition(&self.treesitter, &args),
            "search_symbols" => {
                symbol_tools::search_symbols(&self.treesitter, &args, &self.project_root)
            }
            "go_to_definition" => symbol_tools::go_to_definition(&self.treesitter, &args),
            "get_signature" => symbol_tools::get_signature(&self.treesitter, &args),
            "get_imports" => symbol_tools::get_imports(&self.treesitter, &args),
            "get_call_sites" => symbol_tools::get_call_sites(&self.treesitter, &args),
            "get_scope" => symbol_tools::get_scope(&self.treesitter, &args),
            "get_exports" => symbol_tools::get_exports(&self.treesitter, &args),
            "summarize_file" => symbol_tools::summarize_file(&self.treesitter, &args),
            "get_tests" => symbol_tools::get_tests(&self.treesitter, &args),
            "get_diff_symbols" => {
                symbol_tools::get_diff_symbols(&self.treesitter, &args, &self.project_root)
            }
            "get_annotations" => symbol_tools::get_annotations(&self.treesitter, &args),
            "get_complexity" => symbol_tools::get_complexity(&self.treesitter, &args),
            "get_type_definitions" => symbol_tools::get_type_definitions(&self.treesitter, &args),
            "get_dependencies" => symbol_tools::get_dependencies(&self.treesitter, &args),
            "get_parameters" => symbol_tools::get_parameters(&self.treesitter, &args),
            "get_enclosing_class" => symbol_tools::get_enclosing_class(&self.treesitter, &args),
            "get_symbol_body" => symbol_tools::get_symbol_body(&self.treesitter, &args),
            "get_changed_files" => {
                symbol_tools::get_changed_files(&self.treesitter, &args, &self.project_root)
            }
            "summarize_project" => {
                symbol_tools::summarize_project_tool(&self.treesitter, &args, &self.project_root)
            }

            // ── Auto-select tools (prefer LSP when available) ───────────
            "find_references" => {
                let ts = &self.treesitter;
                self.dispatch_auto(
                    name,
                    &args,
                    |a| symbol_tools::find_references(ts, a),
                    |lsp, a| symbol_tools::find_references(lsp, a),
                )
            }
            "get_diagnostics" => {
                let ts = &self.treesitter;
                self.dispatch_auto(
                    name,
                    &args,
                    |a| file_tools::get_diagnostics(ts, None, a),
                    |lsp, a| file_tools::get_diagnostics(ts, Some(lsp), a),
                )
            }

            // ── LSP-required tools ──────────────────────────────────────
            "rename_symbol" => self.dispatch_lsp_required(name, &args, |lsp, a| {
                file_tools::rename_symbol(Some(lsp), a)
            }),
            "get_hover_info" => self.dispatch_lsp_required(name, &args, |lsp, a| {
                file_tools::get_hover_info(Some(lsp), a)
            }),

            // ── Edit tools ─────────────────────────────────────────────
            "replace_symbol_body" => {
                edit_tools::replace_symbol_body(&self.treesitter, &args, &self.project_root)
            }
            "insert_after_symbol" => {
                edit_tools::insert_after_symbol(&self.treesitter, &args, &self.project_root)
            }
            "insert_before_symbol" => {
                edit_tools::insert_before_symbol(&self.treesitter, &args, &self.project_root)
            }
            "replace_lines" => edit_tools::replace_lines(&args, &self.project_root),
            "insert_at_line" => edit_tools::insert_at_line(&args, &self.project_root),
            "delete_lines" => edit_tools::delete_lines(&args, &self.project_root),
            "create_file" => edit_tools::create_file(&args, &self.project_root),

            // ── Export tools ────────────────────────────────────────────
            "export_to_hyphae" => {
                export_tools::export_to_hyphae(&self.treesitter, &args, &self.project_root)
            }

            _ => Err(anyhow!("Unknown tool: {name}")),
        }
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn selector(&self) -> &RefCell<BackendSelector> {
        &self.selector
    }

    pub fn list_tools(&self) -> Vec<ToolSchema> {
        let mut tools = symbol_tools::tool_schemas();
        tools.extend(file_tools::tool_schemas());
        tools.extend(edit_tools::tool_schemas());
        tools.extend(export_tools::tool_schemas());
        tools
    }

    // ─────────────────────────────────────────────────────────────────────
    // Backend dispatch helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Dispatch for tools that prefer LSP but fall back to tree-sitter.
    fn dispatch_auto<F, G>(
        &self,
        tool_name: &str,
        args: &Value,
        ts_fn: F,
        lsp_fn: G,
    ) -> Result<Value>
    where
        F: FnOnce(&Value) -> Result<Value>,
        G: FnOnce(&rhizome_lsp::LspBackend, &Value) -> Result<Value>,
    {
        match self.resolve_backend(tool_name, args) {
            ActiveBackend::Lsp => {
                self.ensure_lsp();
                let lsp = self.lsp.borrow();
                match lsp.as_ref() {
                    Some(backend) => lsp_fn(backend, args),
                    None => ts_fn(args),
                }
            }
            _ => ts_fn(args),
        }
    }

    /// Dispatch for tools that require LSP — error if unavailable.
    fn dispatch_lsp_required<F>(&self, tool_name: &str, args: &Value, lsp_fn: F) -> Result<Value>
    where
        F: FnOnce(&rhizome_lsp::LspBackend, &Value) -> Result<Value>,
    {
        match self.resolve_backend(tool_name, args) {
            ActiveBackend::Lsp => {
                self.ensure_lsp();
                let lsp = self.lsp.borrow();
                match lsp.as_ref() {
                    Some(backend) => lsp_fn(backend, args),
                    None => Ok(tool_error(&format!(
                        "{tool_name} requires LSP but initialization failed. \
                         Run `rhizome status` to check server availability."
                    ))),
                }
            }
            ActiveBackend::Error(msg) => Ok(tool_error(&msg)),
            ActiveBackend::TreeSitter => Ok(tool_error(&format!(
                "{tool_name} requires an LSP server. Run `rhizome status` to check availability."
            ))),
        }
    }

    /// Resolve which backend to use for a tool call.
    fn resolve_backend(&self, tool_name: &str, args: &Value) -> ActiveBackend {
        let lang = self
            .detect_language(args)
            .unwrap_or(Language::Other("unknown".into()));

        let resolved = self.selector.borrow_mut().select(tool_name, &lang);

        match resolved {
            ResolvedBackend::TreeSitter => ActiveBackend::TreeSitter,
            ResolvedBackend::Lsp => ActiveBackend::Lsp,
            ResolvedBackend::LspUnavailable { install_hint, .. } => {
                ActiveBackend::Error(install_hint)
            }
        }
    }

    fn detect_language(&self, args: &Value) -> Option<Language> {
        let file = args.get("file").and_then(|v| v.as_str())?;
        let ext = Path::new(file).extension()?.to_str()?;
        Language::from_extension(ext)
    }

    fn ensure_lsp(&self) {
        if self.lsp.borrow().is_some() {
            return;
        }
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                let backend = rhizome_lsp::LspBackend::new(self.project_root.clone(), handle);
                *self.lsp.borrow_mut() = Some(backend);
            }
            Err(_) => {
                tracing::debug!("No tokio runtime available for LSP backend initialization");
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Response helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a successful MCP tool response.
pub(crate) fn tool_response(text: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }]
    })
}

/// Build an error MCP tool response.
pub(crate) fn tool_error(message: &str) -> Value {
    json!({
        "isError": true,
        "content": [{ "type": "text", "text": message }]
    })
}
