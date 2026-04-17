pub mod edit_tools;
pub mod export_tools;
pub mod file_tools;
pub mod symbol_tools;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rhizome_core::{
    BackendSelector, HeuristicBackend, Language, ParserlessBackend, ResolvedBackend, RhizomeConfig,
};
use rhizome_treesitter::TreeSitterBackend;
use serde::Serialize;
use serde_json::{Value, json};

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Annotations describing the semantics of an MCP tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    pub read_only_hint: bool,
    pub destructive_hint: bool,
    pub idempotent_hint: bool,
}

/// Schema describing an MCP tool for the `tools/list` response.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub annotations: ToolAnnotations,
}

/// Internal enum for resolved backend after lazy init.
enum ActiveBackend {
    TreeSitter,
    Lsp,
    Parserless,
    Heuristic,
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
    parserless: ParserlessBackend,
    heuristic: HeuristicBackend,
    lsp: RefCell<Option<rhizome_lsp::LspBackend>>,
    selector: RefCell<BackendSelector>,
    project_root: PathBuf,
}

impl ToolDispatcher {
    pub fn new(project_root: PathBuf) -> Self {
        let config = RhizomeConfig::load(&project_root).unwrap_or_default();
        Self {
            treesitter: TreeSitterBackend::new(),
            parserless: ParserlessBackend::new(),
            heuristic: HeuristicBackend::new(),
            lsp: RefCell::new(None),
            selector: RefCell::new(BackendSelector::new(config)),
            project_root,
        }
    }

    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        // Callers may pass an optional `root` to analyze files in a project
        // other than the server's configured project_root (e.g., when rhizome
        // is registered globally and used across multiple projects).
        let root: PathBuf = args
            .get("root")
            .and_then(|v| v.as_str())
            .map(|s| {
                let p = Path::new(s);
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    self.project_root.join(p)
                }
            })
            .unwrap_or_else(|| self.project_root.clone());

        match name {
            // ── Symbol tools ────────────────────────────────────────────
            "get_symbols" => {
                let ts = &self.treesitter;
                let parserless = &self.parserless;
                let heuristic = &self.heuristic;
                self.dispatch_outline(
                    name,
                    &args,
                    |a| symbol_tools::get_symbols(ts, a, &root),
                    |lsp, a| symbol_tools::get_symbols(lsp, a, &root),
                    |a| symbol_tools::get_parserless_symbols(parserless, a, &root),
                    |a| symbol_tools::get_heuristic_symbols(heuristic, a, &root),
                )
            }
            "get_structure" => {
                let ts = &self.treesitter;
                let parserless = &self.parserless;
                let heuristic = &self.heuristic;
                self.dispatch_outline(
                    name,
                    &args,
                    |a| symbol_tools::get_structure(ts, a, &root),
                    |lsp, a| symbol_tools::get_structure(lsp, a, &root),
                    |a| symbol_tools::get_parserless_structure(parserless, a, &root),
                    |a| symbol_tools::get_heuristic_structure(heuristic, a, &root),
                )
            }
            "get_definition" => symbol_tools::get_definition(&self.treesitter, &args, &root),
            "search_symbols" => symbol_tools::search_symbols(&self.treesitter, &args, &root),
            "go_to_definition" => symbol_tools::go_to_definition(&self.treesitter, &args, &root),
            "get_signature" => symbol_tools::get_signature(&self.treesitter, &args, &root),
            "get_imports" => symbol_tools::get_imports(&self.treesitter, &args, &root),
            "get_call_sites" => symbol_tools::get_call_sites(&self.treesitter, &args, &root),
            "get_scope" => symbol_tools::get_scope(&self.treesitter, &args, &root),
            "get_exports" => symbol_tools::get_exports(&self.treesitter, &args, &root),
            "summarize_file" => symbol_tools::summarize_file(&self.treesitter, &args, &root),
            "get_tests" => symbol_tools::get_tests(&self.treesitter, &args, &root),
            "get_diff_symbols" => symbol_tools::get_diff_symbols(&self.treesitter, &args, &root),
            "get_annotations" => symbol_tools::get_annotations(&self.treesitter, &args, &root),
            "get_complexity" => symbol_tools::get_complexity(&self.treesitter, &args, &root),
            "get_type_definitions" => {
                symbol_tools::get_type_definitions(&self.treesitter, &args, &root)
            }
            "get_dependencies" => symbol_tools::get_dependencies(&self.treesitter, &args, &root),
            "get_parameters" => symbol_tools::get_parameters(&self.treesitter, &args, &root),
            "get_enclosing_class" => {
                symbol_tools::get_enclosing_class(&self.treesitter, &args, &root)
            }
            "get_symbol_body" => symbol_tools::get_symbol_body(&self.treesitter, &args, &root),
            "get_region" => {
                let ts = &self.treesitter;
                let parserless = &self.parserless;
                let heuristic = &self.heuristic;
                self.dispatch_semantic_region(
                    name,
                    &args,
                    |a| symbol_tools::get_region(ts, parserless, heuristic, a, &root),
                    |lsp, a| symbol_tools::get_region(lsp, parserless, heuristic, a, &root),
                )
            }
            "get_changed_files" => symbol_tools::get_changed_files(&self.treesitter, &args, &root),
            "summarize_project" => {
                symbol_tools::summarize_project_tool(&self.treesitter, &args, &root)
            }

            // ── Auto-select tools (prefer LSP when available) ───────────
            "find_references" => {
                let ts = &self.treesitter;
                self.dispatch_auto(
                    name,
                    &args,
                    |a| symbol_tools::find_references(ts, a, &root),
                    |lsp, a| symbol_tools::find_references(lsp, a, &root),
                )
            }
            "analyze_impact" => {
                let ts = &self.treesitter;
                self.dispatch_auto(
                    name,
                    &args,
                    |a| symbol_tools::analyze_impact(ts, a, &root),
                    |lsp, a| symbol_tools::analyze_impact(lsp, a, &root),
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
                file_tools::rename_symbol(Some(lsp), a, &root)
            }),
            "get_hover_info" => self.dispatch_lsp_required(name, &args, |lsp, a| {
                file_tools::get_hover_info(Some(lsp), a)
            }),

            // ── Edit tools ─────────────────────────────────────────────
            "replace_symbol_body" => {
                edit_tools::replace_symbol_body(&self.treesitter, &args, &root)
            }
            "insert_after_symbol" => {
                edit_tools::insert_after_symbol(&self.treesitter, &args, &root)
            }
            "insert_before_symbol" => {
                edit_tools::insert_before_symbol(&self.treesitter, &args, &root)
            }
            "replace_lines" => edit_tools::replace_lines(&args, &root),
            "insert_at_line" => edit_tools::insert_at_line(&args, &root),
            "delete_lines" => edit_tools::delete_lines(&args, &root),
            "create_file" => edit_tools::create_file(&args, &root),
            "copy_symbol" => edit_tools::copy_symbol(&self.treesitter, &args, &root),
            "move_symbol" => edit_tools::move_symbol(&self.treesitter, &args, &root),

            // ── Export tools ────────────────────────────────────────────
            "export_to_hyphae" => export_tools::export_to_hyphae(&self.treesitter, &args, &root),
            "export_repo_understanding" => {
                export_tools::export_repo_understanding(&self.treesitter, &args, &root)
            }

            // ── Onboarding ───────────────────────────────────────────────
            "rhizome_onboard" => symbol_tools::rhizome_onboard(&self.project_root),

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
        tools.push(symbol_tools::onboard_schema());
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

    /// Dispatch for outline tools that can degrade to the heuristic or parserless backend.
    fn dispatch_outline<F, G, H, I>(
        &self,
        tool_name: &str,
        args: &Value,
        ts_fn: F,
        lsp_fn: G,
        parserless_fn: H,
        heuristic_fn: I,
    ) -> Result<Value>
    where
        F: FnOnce(&Value) -> Result<Value>,
        G: FnOnce(&rhizome_lsp::LspBackend, &Value) -> Result<Value>,
        H: FnOnce(&Value) -> Result<Value>,
        I: FnOnce(&Value) -> Result<Value>,
    {
        let lang = self
            .detect_language(args)
            .unwrap_or(Language::Other("unknown".into()));

        match self.resolve_backend(tool_name, args) {
            ActiveBackend::Heuristic => heuristic_fn(args),
            ActiveBackend::Parserless => parserless_fn(args),
            ActiveBackend::Lsp => self.try_lsp_or_heuristic(args, lsp_fn, heuristic_fn),
            ActiveBackend::TreeSitter => match ts_fn(args) {
                Ok(value) => Ok(value),
                Err(_) => match self.selector.borrow_mut().outline_fallback(&lang) {
                    ResolvedBackend::Lsp => {
                        self.try_lsp_or_heuristic(args, lsp_fn, heuristic_fn)
                    }
                    ResolvedBackend::Heuristic => heuristic_fn(args),
                    ResolvedBackend::Parserless => parserless_fn(args),
                    // New variants default to heuristic until an explicit arm is added.
                    _ => heuristic_fn(args),
                },
            },
            ActiveBackend::Error(_) => heuristic_fn(args),
        }
    }

    /// Dispatch for semantic region lookups: tree-sitter first, then LSP, never parserless.
    ///
    /// Parserless `region-*` and heuristic `h-*` region IDs are routed directly
    /// to the function that knows how to resolve them.
    fn dispatch_semantic_region<F, G>(
        &self,
        _tool_name: &str,
        args: &Value,
        ts_fn: F,
        lsp_fn: G,
    ) -> Result<Value>
    where
        F: FnOnce(&Value) -> Result<Value>,
        G: FnOnce(&rhizome_lsp::LspBackend, &Value) -> Result<Value>,
    {
        let lang = self
            .detect_language(args)
            .unwrap_or(Language::Other("unknown".into()));
        let region_id = args
            .get("region_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");

        // Parserless or heuristic region IDs are handled directly by the
        // ts_fn which delegates internally based on the ID prefix.
        if region_id.starts_with("region-") || region_id.starts_with("h-") {
            return ts_fn(args);
        }

        match ts_fn(args) {
            Ok(value) => Ok(value),
            Err(error) => match self.selector.borrow_mut().outline_fallback(&lang) {
                ResolvedBackend::Lsp => self.try_lsp_or_error(args, lsp_fn, error),
                _ => Err(error),
            },
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
            ActiveBackend::Parserless | ActiveBackend::Heuristic => Ok(tool_error(&format!(
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
            ResolvedBackend::Parserless => ActiveBackend::Parserless,
            ResolvedBackend::Heuristic => ActiveBackend::Heuristic,
            ResolvedBackend::LspUnavailable { install_hint, .. } => {
                ActiveBackend::Error(install_hint)
            }
            // New variants default to heuristic until an explicit arm is added.
            _ => ActiveBackend::Heuristic,
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

    fn try_lsp_or_heuristic<G, I>(
        &self,
        args: &Value,
        lsp_fn: G,
        heuristic_fn: I,
    ) -> Result<Value>
    where
        G: FnOnce(&rhizome_lsp::LspBackend, &Value) -> Result<Value>,
        I: FnOnce(&Value) -> Result<Value>,
    {
        self.ensure_lsp();
        let lsp = self.lsp.borrow();
        match lsp.as_ref() {
            Some(backend) => match lsp_fn(backend, args) {
                Ok(value) => Ok(value),
                Err(_) => heuristic_fn(args),
            },
            None => heuristic_fn(args),
        }
    }

    fn try_lsp_or_error<G>(
        &self,
        args: &Value,
        lsp_fn: G,
        original_error: anyhow::Error,
    ) -> Result<Value>
    where
        G: FnOnce(&rhizome_lsp::LspBackend, &Value) -> Result<Value>,
    {
        self.ensure_lsp();
        let lsp = self.lsp.borrow();
        match lsp.as_ref() {
            Some(backend) => lsp_fn(backend, args).or(Err(original_error)),
            None => Err(original_error),
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
