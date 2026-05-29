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
            title: Some("Rename Symbol".to_string()),
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
            title: Some("Get Diagnostics".to_string()),
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
            name: "get_hover".into(),
            title: Some("Get Hover".to_string()),
            description: "Get hover documentation for a symbol at a position (requires LSP)".into(),
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
        ToolSchema {
            name: "go_to_implementation".into(),
            title: Some("Go To Implementation".to_string()),
            description: "Find implementations of an interface or abstract method (requires LSP)".into(),
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
        ToolSchema {
            name: "go_to_type_definition".into(),
            title: Some("Go To Type Definition".to_string()),
            description: "Jump to the type definition of a symbol (requires LSP)".into(),
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
        ToolSchema {
            name: "get_completions".into(),
            title: Some("Get Completions".to_string()),
            description: "Get completion items at a position (requires LSP). Returns at most 50 items.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" },
                    "column": { "type": "number", "description": "Column number (0-based)" },
                    "trigger_character": {
                        "type": "string",
                        "description": "Single character that triggered completion (e.g. '.', '::')",
                        "maxLength": 1
                    }
                },
                "required": ["file", "line", "column"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_signature_help".into(),
            title: Some("Get Signature Help".to_string()),
            description: "Get parameter information for a function call at a position (requires LSP)".into(),
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
        ToolSchema {
            name: "prepare_call_hierarchy".into(),
            title: Some("Prepare Call Hierarchy".to_string()),
            description: "Prepare call hierarchy items at a position. Use the returned items with get_incoming_calls or get_outgoing_calls (requires LSP).".into(),
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
        ToolSchema {
            name: "get_incoming_calls".into(),
            title: Some("Get Incoming Calls".to_string()),
            description: "Get callers of a function from a call hierarchy item (requires LSP).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "item": {
                        "type": "object",
                        "description": "A CallHierarchyItem returned by prepare_call_hierarchy"
                    }
                },
                "required": ["item"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_outgoing_calls".into(),
            title: Some("Get Outgoing Calls".to_string()),
            description: "Get functions called by a function from a call hierarchy item (requires LSP).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "item": {
                        "type": "object",
                        "description": "A CallHierarchyItem returned by prepare_call_hierarchy"
                    }
                },
                "required": ["item"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_code_actions".into(),
            title: Some("Get Code Actions".to_string()),
            description: "Get available code actions (quick fixes, refactors) for a range (requires LSP). Filters to CodeAction objects only.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "start_line": { "type": "number", "description": "Start line (0-based)" },
                    "start_column": { "type": "number", "description": "Start column (0-based)" },
                    "end_line": { "type": "number", "description": "End line (0-based)" },
                    "end_column": { "type": "number", "description": "End column (0-based)" }
                },
                "required": ["file", "start_line", "start_column", "end_line", "end_column"]
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
    project_root: &Path,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;

    let path_buf = resolve_path(file, project_root)?;
    let path = path_buf.as_path();

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

pub fn get_hover(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;
    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: line"))? as u32;
    let column = args
        .get("column")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: column"))? as u32;

    let path_buf = resolve_path(file, project_root)?;
    let position = Position { line, column };

    match lsp.hover_with_root(&path_buf, &position, project_root)? {
        Some(text) => Ok(tool_response(&text)),
        None => Ok(tool_response("No hover information available at this position.")),
    }
}

pub fn go_to_implementation(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;
    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: line"))? as u32;
    let column = args
        .get("column")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: column"))? as u32;
    let path_buf = resolve_path(file, project_root)?;
    let position = Position { line, column };
    let locs = lsp.go_to_implementation_with_root(&path_buf, &position, project_root)?;
    let formatted: Vec<Value> = locs
        .iter()
        .map(|l| {
            json!({
                "file": l.file_path,
                "line_start": l.line_start,
                "line_end": l.line_end,
                "column_start": l.column_start,
                "column_end": l.column_end,
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

pub fn go_to_type_definition(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;
    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: line"))? as u32;
    let column = args
        .get("column")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: column"))? as u32;
    let path_buf = resolve_path(file, project_root)?;
    let position = Position { line, column };
    let locs = lsp.go_to_type_definition_with_root(&path_buf, &position, project_root)?;
    let formatted: Vec<Value> = locs
        .iter()
        .map(|l| {
            json!({
                "file": l.file_path,
                "line_start": l.line_start,
                "line_end": l.line_end,
                "column_start": l.column_start,
                "column_end": l.column_end,
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

pub fn get_completions(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;
    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: line"))? as u32;
    let column = args
        .get("column")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: column"))? as u32;
    let trigger_character = args
        .get("trigger_character")
        .and_then(|v| v.as_str())
        .and_then(|s| s.chars().next());
    let path_buf = resolve_path(file, project_root)?;
    let position = Position { line, column };
    let items = lsp.completion_with_root(&path_buf, &position, trigger_character, project_root)?;
    let text = serde_json::to_string_pretty(&items)?;
    Ok(tool_response(&text))
}

pub fn get_signature_help(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;
    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: line"))? as u32;
    let column = args
        .get("column")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: column"))? as u32;
    let path_buf = resolve_path(file, project_root)?;
    let position = Position { line, column };
    match lsp.signature_help_with_root(&path_buf, &position, project_root)? {
        Some(help) => {
            let text = serde_json::to_string_pretty(&help)?;
            Ok(tool_response(&text))
        }
        None => Ok(tool_response("No signature help available at this position.")),
    }
}

pub fn prepare_call_hierarchy(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;
    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: line"))? as u32;
    let column = args
        .get("column")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: column"))? as u32;
    let path_buf = resolve_path(file, project_root)?;
    let position = Position { line, column };
    let items = lsp.prepare_call_hierarchy_with_root(&path_buf, &position, project_root)?;
    let text = serde_json::to_string_pretty(&items)?;
    Ok(tool_response(&text))
}

pub fn get_incoming_calls(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let item: rhizome_lsp::CallHierarchyItemJson = serde_json::from_value(
        args.get("item")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: item"))?,
    )?;
    let calls = lsp.incoming_calls_with_root(&item, project_root)?;
    let text = serde_json::to_string_pretty(&calls)?;
    Ok(tool_response(&text))
}

pub fn get_outgoing_calls(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let item: rhizome_lsp::CallHierarchyItemJson = serde_json::from_value(
        args.get("item")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: item"))?,
    )?;
    let calls = lsp.outgoing_calls_with_root(&item, project_root)?;
    let text = serde_json::to_string_pretty(&calls)?;
    Ok(tool_response(&text))
}

pub fn get_code_actions(
    lsp: &rhizome_lsp::LspBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file"))?;
    let start_line = args
        .get("start_line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: start_line"))? as u32;
    let start_column = args
        .get("start_column")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: start_column"))? as u32;
    let end_line = args
        .get("end_line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: end_line"))? as u32;
    let end_column = args
        .get("end_column")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: end_column"))? as u32;
    let path_buf = resolve_path(file, project_root)?;
    let start = Position { line: start_line, column: start_column };
    let end = Position { line: end_line, column: end_column };
    let actions = lsp.code_actions_with_root(&path_buf, &start, &end, project_root)?;
    let text = serde_json::to_string_pretty(&actions)?;
    Ok(tool_response(&text))
}

fn lsp_required_error(tool_name: &str) -> Value {
    let suggestion = match tool_name {
        "rename_symbol" | "get_hover" | "go_to_implementation" | "go_to_type_definition"
        | "get_completions" | "get_signature_help" | "prepare_call_hierarchy"
        | "get_incoming_calls" | "get_outgoing_calls" | "get_code_actions" => {
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
