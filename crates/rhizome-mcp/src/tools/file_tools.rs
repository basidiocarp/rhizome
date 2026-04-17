use std::path::Path;

use anyhow::Result;
use rhizome_core::{CodeIntelligence, Language, Position, detect_workspace_root};
use serde_json::{Value, json};

use super::{ToolAnnotations, ToolSchema, edit_tools::resolve_path, tool_error, tool_response};

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
                    "new_name": { "type": "string", "description": "New name for the symbol" },
                    "preview": {
                        "type": "boolean",
                        "description": "When true, return a dry-run preview instead of applying edits"
                    }
                },
                "required": ["file", "line", "column", "new_name"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: false,
                idempotent_hint: false,
            },
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// Tool handlers
// ---------------------------------------------------------------------------

/// Rename a symbol (LSP only).
pub fn rename_symbol(
    lsp: Option<&rhizome_lsp::LspBackend>,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    match lsp {
        Some(lsp_backend) => {
            let file = args
                .get("file")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;
            let line = args
                .get("line")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("Missing required parameter: line"))?;
            let column = args
                .get("column")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("Missing required parameter: column"))?;
            let new_name = args
                .get("new_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required parameter: new_name"))?;
            let preview = args
                .get("preview")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let path_buf = resolve_path(file, project_root)?;
            let path = path_buf.as_path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .ok_or_else(|| anyhow::anyhow!("Cannot detect language for {}", path.display()))?;
            let language = Language::from_extension(ext)
                .ok_or_else(|| anyhow::anyhow!("Unsupported language extension: {ext}"))?;
            let workspace_root = detect_workspace_root(path, &language, project_root);
            let position = Position {
                line: line as u32,
                column: column as u32,
            };

            match if preview {
                lsp_backend
                    .preview_rename_with_root(path, &position, new_name, &workspace_root)
                    .map(|result| rename_preview_response(path, line, column, new_name, &result))
            } else {
                lsp_backend
                    .rename_with_root(path, &position, new_name, &workspace_root)
                    .map(|result| {
                        tool_response(&format!(
                            "Renamed symbol at {}:{}:{} to {}.\nfiles_modified: {}\nedits_applied: {}",
                            path.display(),
                            line,
                            column,
                            new_name,
                            result.files_modified,
                            result.edits_applied
                        ))
                    })
            } {
                Ok(result) => Ok(result),
                Err(err) => Ok(tool_error(&format!("rename failed: {err}"))),
            }
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

fn rename_preview_response(
    path: &Path,
    line: u64,
    column: u64,
    new_name: &str,
    preview: &rhizome_lsp::edit::PreviewResult,
) -> Value {
    let paths = if preview.affected_paths.is_empty() {
        String::new()
    } else {
        format!(
            "\naffected_paths:\n{}",
            preview
                .affected_paths
                .iter()
                .map(|path| format!("- {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Preview rename at {}:{}:{} to {}.\nfiles_modified: {}\nedits_applied: {}{}",
                path.display(),
                line,
                column,
                new_name,
                preview.files_modified,
                preview.edits_applied,
                paths
            )
        }],
        "preview": {
            "files_modified": preview.files_modified,
            "edits_applied": preview.edits_applied,
            "affected_paths": preview.affected_paths,
        }
    })
}
