pub mod file_tools;
pub mod symbol_tools;

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use rhizome_treesitter::TreeSitterBackend;
use serde::Serialize;
use serde_json::{json, Value};

/// Schema describing an MCP tool for the `tools/list` response.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Routes MCP tool calls to the appropriate backend handler.
pub struct ToolDispatcher {
    treesitter: TreeSitterBackend,
    lsp: Option<rhizome_lsp::LspBackend>,
    project_root: PathBuf,
}

impl ToolDispatcher {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            treesitter: TreeSitterBackend::new(),
            lsp: None,
            project_root,
        }
    }

    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            // Symbol tools (tree-sitter primary)
            "get_symbols" => symbol_tools::get_symbols(&self.treesitter, &args),
            "get_structure" => symbol_tools::get_structure(&self.treesitter, &args),
            "get_definition" => symbol_tools::get_definition(&self.treesitter, &args),
            "search_symbols" => {
                symbol_tools::search_symbols(&self.treesitter, &args, &self.project_root)
            }
            "find_references" => symbol_tools::find_references(&self.treesitter, &args),
            "go_to_definition" => symbol_tools::go_to_definition(&self.treesitter, &args),
            "get_signature" => symbol_tools::get_signature(&self.treesitter, &args),
            "get_imports" => symbol_tools::get_imports(&self.treesitter, &args),
            "get_call_sites" => symbol_tools::get_call_sites(&self.treesitter, &args),

            // File/LSP tools
            "rename_symbol" => file_tools::rename_symbol(self.lsp.as_ref(), &args),
            "get_diagnostics" => {
                file_tools::get_diagnostics(&self.treesitter, self.lsp.as_ref(), &args)
            }
            "get_hover_info" => file_tools::get_hover_info(self.lsp.as_ref(), &args),

            _ => Err(anyhow!("Unknown tool: {name}")),
        }
    }

    pub fn list_tools(&self) -> Vec<ToolSchema> {
        let mut tools = symbol_tools::tool_schemas();
        tools.extend(file_tools::tool_schemas());
        tools
    }
}

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
