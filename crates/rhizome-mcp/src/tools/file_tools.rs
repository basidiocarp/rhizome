use std::path::Path;

use anyhow::Result;
use rhizome_core::CodeIntelligence;
use serde_json::{json, Value};

use super::{tool_error, tool_response, ToolSchema};

// ---------------------------------------------------------------------------
// Tool schemas
// ---------------------------------------------------------------------------

pub fn tool_schemas() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            name: "rename_symbol".into(),
            description: "Rename a symbol across the project (requires LSP)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" },
                    "column": { "type": "number", "description": "Column number (0-based)" },
                    "new_name": { "type": "string", "description": "New name for the symbol" }
                },
                "required": ["file", "line", "column", "new_name"]
            }),
        },
        ToolSchema {
            name: "get_diagnostics".into(),
            description: "Get compiler diagnostics (errors, warnings) for a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_hover_info".into(),
            description: "Get hover information (type info, docs) for a position (requires LSP)"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" },
                    "column": { "type": "number", "description": "Column number (0-based)" }
                },
                "required": ["file", "line", "column"]
            }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Tool handlers
// ---------------------------------------------------------------------------

/// Rename a symbol (LSP only).
pub fn rename_symbol(lsp: Option<&rhizome_lsp::LspBackend>, _args: &Value) -> Result<Value> {
    match lsp {
        Some(_lsp_backend) => {
            // Full rename implementation would use the LSP rename request.
            // For now, indicate this requires an active LSP connection.
            Ok(tool_error(
                "LSP rename is not yet fully wired. \
                 Install rust-analyzer for Rust or pyright for Python support.",
            ))
        }
        None => Ok(lsp_required_error("rename_symbol")),
    }
}

/// Get diagnostics for a file. Uses LSP if available, falls back to tree-sitter (empty).
pub fn get_diagnostics(
    treesitter: &dyn CodeIntelligence,
    lsp: Option<&rhizome_lsp::LspBackend>,
    args: &Value,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;

    let path = Path::new(file);

    // Prefer LSP diagnostics if available
    let backend: &dyn CodeIntelligence = match lsp {
        Some(b) => b,
        None => treesitter,
    };

    let diagnostics = backend.get_diagnostics(path)?;

    let formatted: Vec<Value> = diagnostics
        .iter()
        .map(|d| {
            json!({
                "message": d.message,
                "severity": format!("{:?}", d.severity),
                "location": {
                    "file": &d.location.file_path,
                    "line_start": d.location.line_start,
                    "line_end": d.location.line_end,
                    "column_start": d.location.column_start,
                    "column_end": d.location.column_end,
                }
            })
        })
        .collect();

    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

/// Get hover info at a position (LSP only).
pub fn get_hover_info(lsp: Option<&rhizome_lsp::LspBackend>, _args: &Value) -> Result<Value> {
    match lsp {
        Some(_lsp_backend) => {
            // Full hover implementation would use the LSP hover request.
            Ok(tool_error(
                "LSP hover is not yet fully wired. \
                 Install rust-analyzer for Rust or pyright for Python support.",
            ))
        }
        None => Ok(lsp_required_error("get_hover_info")),
    }
}

fn lsp_required_error(tool_name: &str) -> Value {
    let suggestion = match tool_name {
        "rename_symbol" => {
            "LSP required for this operation. \
             Install rust-analyzer for Rust or pyright for Python support."
        }
        "get_hover_info" => {
            "LSP required for this operation. \
             Install rust-analyzer for Rust or pyright for Python support."
        }
        _ => "LSP required for this operation.",
    };

    tool_error(suggestion)
}
