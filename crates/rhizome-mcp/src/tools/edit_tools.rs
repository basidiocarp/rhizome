use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use rhizome_core::CodeIntelligence;
use serde_json::{json, Value};

use super::{tool_error, tool_response, ToolSchema};

// ─────────────────────────────────────────────────────────────────────────────
// Param helpers
// ─────────────────────────────────────────────────────────────────────────────

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: {key}"))
}

fn required_u32(args: &Value, key: &str) -> Result<u32> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| anyhow!("Missing required parameter: {key}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativePosition {
    Before,
    After,
}

fn required_position(args: &Value, key: &str) -> Result<RelativePosition> {
    match required_str(args, key)? {
        "before" => Ok(RelativePosition::Before),
        "after" => Ok(RelativePosition::After),
        value => Err(anyhow!(
            "Invalid value for {key}: {value}. Expected 'before' or 'after'"
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool schemas
// ─────────────────────────────────────────────────────────────────────────────

pub fn tool_schemas() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            name: "replace_symbol_body".into(),
            description: "Replace the entire body of a symbol (function, struct, class, etc.) \
                with new content. Uses tree-sitter to locate the symbol precisely."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol to replace" },
                    "new_body": { "type": "string", "description": "New content to replace the symbol body with" }
                },
                "required": ["file", "symbol", "new_body"]
            }),
        },
        ToolSchema {
            name: "insert_after_symbol".into(),
            description: "Insert content after a symbol (function, struct, class, etc.). \
                A blank line is added for separation."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol to insert after" },
                    "content": { "type": "string", "description": "Content to insert after the symbol" }
                },
                "required": ["file", "symbol", "content"]
            }),
        },
        ToolSchema {
            name: "insert_before_symbol".into(),
            description: "Insert content before a symbol (function, struct, class, etc.). \
                A blank line is added for separation."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol to insert before" },
                    "content": { "type": "string", "description": "Content to insert before the symbol" }
                },
                "required": ["file", "symbol", "content"]
            }),
        },
        ToolSchema {
            name: "replace_lines".into(),
            description: "Replace a range of lines in a file with new content. \
                Line numbers are 1-based and inclusive."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "start_line": { "type": "number", "description": "First line to replace (1-based)" },
                    "end_line": { "type": "number", "description": "Last line to replace (1-based, inclusive)" },
                    "content": { "type": "string", "description": "New content to replace the lines with" }
                },
                "required": ["file", "start_line", "end_line", "content"]
            }),
        },
        ToolSchema {
            name: "insert_at_line".into(),
            description: "Insert content at a specific line number. \
                Existing content at that line is pushed down. Line number is 1-based."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number to insert at (1-based)" },
                    "content": { "type": "string", "description": "Content to insert" }
                },
                "required": ["file", "line", "content"]
            }),
        },
        ToolSchema {
            name: "delete_lines".into(),
            description: "Delete a range of lines from a file. \
                Line numbers are 1-based and inclusive."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "start_line": { "type": "number", "description": "First line to delete (1-based)" },
                    "end_line": { "type": "number", "description": "Last line to delete (1-based, inclusive)" }
                },
                "required": ["file", "start_line", "end_line"]
            }),
        },
        ToolSchema {
            name: "create_file".into(),
            description: "Create a new file with content. Creates parent directories \
                automatically. Refuses to overwrite unless overwrite=true."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "File path relative to project root" },
                    "content": { "type": "string", "description": "Content to write to the file" },
                    "overwrite": { "type": "boolean", "description": "Allow overwriting existing files (default: false)", "default": false }
                },
                "required": ["file", "content"]
            }),
        },
        ToolSchema {
            name: "copy_symbol".into(),
            description: "Copy a symbol's full source block to a position before or after \
                another symbol. Safe MVP: text-preserving, tree-sitter-located symbol movement."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source_file": { "type": "string", "description": "Path to the source file containing the symbol" },
                    "symbol": { "type": "string", "description": "Name of the source symbol to copy" },
                    "target_file": { "type": "string", "description": "Path to the target file" },
                    "target_symbol": { "type": "string", "description": "Name of the symbol to insert relative to" },
                    "position": { "type": "string", "enum": ["before", "after"], "description": "Whether to insert before or after the target symbol" }
                },
                "required": ["source_file", "symbol", "target_file", "target_symbol", "position"]
            }),
        },
        ToolSchema {
            name: "move_symbol".into(),
            description: "Move a symbol's full source block to a position before or after \
                another symbol in a different file. Same-file moves are rejected in this MVP."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source_file": { "type": "string", "description": "Path to the source file containing the symbol" },
                    "symbol": { "type": "string", "description": "Name of the source symbol to move" },
                    "target_file": { "type": "string", "description": "Path to the target file" },
                    "target_symbol": { "type": "string", "description": "Name of the symbol to insert relative to" },
                    "position": { "type": "string", "enum": ["before", "after"], "description": "Whether to insert before or after the target symbol" }
                },
                "required": ["source_file", "symbol", "target_file", "target_symbol", "position"]
            }),
        },
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// File reading/writing helpers
// ─────────────────────────────────────────────────────────────────────────────

fn read_lines(path: &Path) -> Result<Vec<String>> {
    let content =
        fs::read_to_string(path).map_err(|e| anyhow!("Failed to read {}: {e}", path.display()))?;
    Ok(content.lines().map(String::from).collect())
}

fn write_lines(path: &Path, lines: &[String]) -> Result<()> {
    let content = if lines.is_empty() {
        String::new()
    } else {
        let mut out = lines.join("\n");
        out.push('\n');
        out
    };
    fs::write(path, content).map_err(|e| anyhow!("Failed to write {}: {e}", path.display()))
}

/// Resolve a file path, validating it stays within project_root to prevent path traversal.
fn resolve_path(file: &str, project_root: &Path) -> Result<std::path::PathBuf> {
    let p = Path::new(file);
    let resolved = if p.is_absolute() {
        p.to_path_buf()
    } else {
        project_root.join(p)
    };

    // Canonicalize to resolve symlinks and ../ components.
    // For non-existent paths, canonicalize the parent and re-attach the filename.
    let canonical = if resolved.exists() {
        resolved.canonicalize()?
    } else {
        // Path doesn't exist. Normalize it by canonicalizing the parent and re-attaching the name.
        if let Some(parent) = resolved.parent() {
            match parent.canonicalize() {
                Ok(canonical_parent) => {
                    canonical_parent.join(resolved.file_name().unwrap_or_default())
                }
                Err(_) => {
                    // Parent path can't be canonicalized (may not exist yet).
                    // Use dunce to normalize .. components without I/O.
                    use std::path::Component;
                    let mut normalized = project_root.to_path_buf();
                    for component in resolved.components() {
                        match component {
                            Component::ParentDir => {
                                normalized.pop();
                            }
                            Component::Normal(name) => {
                                normalized.push(name);
                            }
                            _ => {}
                        }
                    }
                    normalized
                }
            }
        } else {
            resolved.clone()
        }
    };

    let canonical_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());

    if !canonical.starts_with(&canonical_root) {
        anyhow::bail!(
            "Path traversal denied: {} is outside project root {}",
            canonical.display(),
            canonical_root.display()
        );
    }

    Ok(resolved)
}

/// Look up a symbol's location in a file via the backend.
fn find_symbol_location(
    backend: &dyn CodeIntelligence,
    path: &Path,
    symbol_name: &str,
) -> Result<(u32, u32)> {
    let sym = backend.get_definition(path, symbol_name)?;
    match sym {
        Some(s) => Ok((s.location.line_start, s.location.line_end)),
        None => Err(anyhow!(
            "Symbol '{}' not found in {}",
            symbol_name,
            path.display()
        )),
    }
}

fn extract_symbol_lines(path: &Path, line_start: u32, line_end: u32) -> Result<Vec<String>> {
    let lines = read_lines(path)?;
    let start = line_start as usize;
    let end = (line_end as usize + 1).min(lines.len());
    Ok(lines[start..end].to_vec())
}

fn insert_lines_relative_to_symbol(
    backend: &dyn CodeIntelligence,
    target_path: &Path,
    target_symbol: &str,
    position: RelativePosition,
    content_lines: &[String],
) -> Result<(usize, usize)> {
    let (target_start, target_end) = find_symbol_location(backend, target_path, target_symbol)?;
    let lines = read_lines(target_path)?;

    let insert_idx = match position {
        RelativePosition::Before => target_start as usize,
        RelativePosition::After => (target_end as usize + 1).min(lines.len()),
    };

    let lines_inserted = content_lines.len() + 1;
    let mut result_lines = Vec::with_capacity(lines.len() + lines_inserted);

    result_lines.extend_from_slice(&lines[..insert_idx]);
    match position {
        RelativePosition::Before => {
            result_lines.extend(content_lines.iter().cloned());
            result_lines.push(String::new());
        }
        RelativePosition::After => {
            result_lines.push(String::new());
            result_lines.extend(content_lines.iter().cloned());
        }
    }
    if insert_idx < lines.len() {
        result_lines.extend_from_slice(&lines[insert_idx..]);
    }

    write_lines(target_path, &result_lines)?;
    Ok((insert_idx + 1, lines_inserted))
}

fn delete_symbol_lines(path: &Path, line_start: u32, line_end: u32) -> Result<(usize, usize)> {
    let lines = read_lines(path)?;
    let lines_before = lines.len();
    let start = line_start as usize;
    let end = (line_end as usize + 1).min(lines.len());

    let mut result_lines = Vec::with_capacity(lines.len());
    result_lines.extend_from_slice(&lines[..start]);
    if end < lines.len() {
        result_lines.extend_from_slice(&lines[end..]);
    }

    let lines_after = result_lines.len();
    write_lines(path, &result_lines)?;
    Ok((lines_before, lines_after))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Replace the entire body of a symbol with new content.
pub fn replace_symbol_body(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;
    let new_body = required_str(args, "new_body")?;

    let path = resolve_path(file, project_root)?;

    let (line_start, line_end) = match find_symbol_location(backend, &path, symbol_name) {
        Ok(loc) => loc,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let lines = match read_lines(&path) {
        Ok(l) => l,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let lines_before = lines.len();

    // line_start and line_end are 0-based from tree-sitter
    let start = line_start as usize;
    let end = (line_end as usize + 1).min(lines.len());

    let new_body_lines: Vec<String> = new_body.lines().map(String::from).collect();

    let mut result_lines = Vec::with_capacity(lines.len());
    result_lines.extend_from_slice(&lines[..start]);
    result_lines.extend(new_body_lines);
    if end < lines.len() {
        result_lines.extend_from_slice(&lines[end..]);
    }

    let lines_after = result_lines.len();

    if let Err(e) = write_lines(&path, &result_lines) {
        return Ok(tool_error(&e.to_string()));
    }

    let text = serde_json::to_string_pretty(&json!({
        "file": file,
        "symbol": symbol_name,
        "lines_before": lines_before,
        "lines_after": lines_after,
    }))?;
    Ok(tool_response(&text))
}

/// Insert content after a symbol.
pub fn insert_after_symbol(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;
    let content = required_str(args, "content")?;

    let path = resolve_path(file, project_root)?;

    let (_line_start, line_end) = match find_symbol_location(backend, &path, symbol_name) {
        Ok(loc) => loc,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let lines = match read_lines(&path) {
        Ok(l) => l,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    // Insert after line_end (0-based), so at index line_end + 1
    let insert_idx = (line_end as usize + 1).min(lines.len());

    let content_lines: Vec<String> = content.lines().map(String::from).collect();
    let lines_inserted = content_lines.len() + 1; // +1 for blank separator

    let mut result_lines = Vec::with_capacity(lines.len() + lines_inserted);
    result_lines.extend_from_slice(&lines[..insert_idx]);
    result_lines.push(String::new()); // blank line separator
    result_lines.extend(content_lines);
    if insert_idx < lines.len() {
        result_lines.extend_from_slice(&lines[insert_idx..]);
    }

    if let Err(e) = write_lines(&path, &result_lines) {
        return Ok(tool_error(&e.to_string()));
    }

    let text = serde_json::to_string_pretty(&json!({
        "file": file,
        "symbol": symbol_name,
        "inserted_at_line": insert_idx + 1,
        "lines_inserted": lines_inserted,
    }))?;
    Ok(tool_response(&text))
}

/// Insert content before a symbol.
pub fn insert_before_symbol(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;
    let content = required_str(args, "content")?;

    let path = resolve_path(file, project_root)?;

    let (line_start, _line_end) = match find_symbol_location(backend, &path, symbol_name) {
        Ok(loc) => loc,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let lines = match read_lines(&path) {
        Ok(l) => l,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let insert_idx = line_start as usize;

    let content_lines: Vec<String> = content.lines().map(String::from).collect();
    let lines_inserted = content_lines.len() + 1; // +1 for blank separator

    let mut result_lines = Vec::with_capacity(lines.len() + lines_inserted);
    result_lines.extend_from_slice(&lines[..insert_idx]);
    result_lines.extend(content_lines);
    result_lines.push(String::new()); // blank line separator
    result_lines.extend_from_slice(&lines[insert_idx..]);

    if let Err(e) = write_lines(&path, &result_lines) {
        return Ok(tool_error(&e.to_string()));
    }

    let text = serde_json::to_string_pretty(&json!({
        "file": file,
        "symbol": symbol_name,
        "inserted_at_line": insert_idx + 1,
        "lines_inserted": lines_inserted,
    }))?;
    Ok(tool_response(&text))
}

/// Replace a range of lines (1-based, inclusive) with new content.
pub fn replace_lines(args: &Value, project_root: &Path) -> Result<Value> {
    let file = required_str(args, "file")?;
    let start_line = required_u32(args, "start_line")?;
    let end_line = required_u32(args, "end_line")?;
    let content = required_str(args, "content")?;

    if start_line == 0 || end_line == 0 {
        return Ok(tool_error("Line numbers are 1-based; 0 is not valid."));
    }
    if start_line > end_line {
        return Ok(tool_error(&format!(
            "start_line ({start_line}) must be <= end_line ({end_line})"
        )));
    }

    let path = resolve_path(file, project_root)?;
    let lines = match read_lines(&path) {
        Ok(l) => l,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let lines_before = lines.len();
    let total = lines_before as u32;

    if start_line > total || end_line > total {
        return Ok(tool_error(&format!(
            "Line range {start_line}-{end_line} is out of bounds (file has {total} lines)"
        )));
    }

    let start_idx = (start_line - 1) as usize;
    let end_idx = end_line as usize; // inclusive end -> exclusive index

    let new_content_lines: Vec<String> = content.lines().map(String::from).collect();

    let mut result_lines = Vec::with_capacity(lines.len());
    result_lines.extend_from_slice(&lines[..start_idx]);
    result_lines.extend(new_content_lines);
    if end_idx < lines.len() {
        result_lines.extend_from_slice(&lines[end_idx..]);
    }

    let lines_after = result_lines.len();

    if let Err(e) = write_lines(&path, &result_lines) {
        return Ok(tool_error(&e.to_string()));
    }

    let text = serde_json::to_string_pretty(&json!({
        "file": file,
        "lines_before": lines_before,
        "lines_after": lines_after,
    }))?;
    Ok(tool_response(&text))
}

/// Insert content at a specific line (1-based).
pub fn insert_at_line(args: &Value, project_root: &Path) -> Result<Value> {
    let file = required_str(args, "file")?;
    let line = required_u32(args, "line")?;
    let content = required_str(args, "content")?;

    if line == 0 {
        return Ok(tool_error("Line number is 1-based; 0 is not valid."));
    }

    let path = resolve_path(file, project_root)?;
    let lines = match read_lines(&path) {
        Ok(l) => l,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let lines_before = lines.len();
    let total = lines_before as u32;

    // Allow inserting at total+1 (appending at end)
    if line > total + 1 {
        return Ok(tool_error(&format!(
            "Line {line} is out of bounds (file has {total} lines, max insert at {})",
            total + 1
        )));
    }

    let insert_idx = (line - 1) as usize;
    let content_lines: Vec<String> = content.lines().map(String::from).collect();

    let mut result_lines = Vec::with_capacity(lines.len() + content_lines.len());
    result_lines.extend_from_slice(&lines[..insert_idx]);
    result_lines.extend(content_lines);
    if insert_idx < lines.len() {
        result_lines.extend_from_slice(&lines[insert_idx..]);
    }

    let lines_after = result_lines.len();

    if let Err(e) = write_lines(&path, &result_lines) {
        return Ok(tool_error(&e.to_string()));
    }

    let text = serde_json::to_string_pretty(&json!({
        "file": file,
        "lines_before": lines_before,
        "lines_after": lines_after,
    }))?;
    Ok(tool_response(&text))
}

/// Delete a range of lines (1-based, inclusive).
pub fn delete_lines(args: &Value, project_root: &Path) -> Result<Value> {
    let file = required_str(args, "file")?;
    let start_line = required_u32(args, "start_line")?;
    let end_line = required_u32(args, "end_line")?;

    if start_line == 0 || end_line == 0 {
        return Ok(tool_error("Line numbers are 1-based; 0 is not valid."));
    }
    if start_line > end_line {
        return Ok(tool_error(&format!(
            "start_line ({start_line}) must be <= end_line ({end_line})"
        )));
    }

    let path = resolve_path(file, project_root)?;
    let lines = match read_lines(&path) {
        Ok(l) => l,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let lines_before = lines.len();
    let total = lines_before as u32;

    if start_line > total || end_line > total {
        return Ok(tool_error(&format!(
            "Line range {start_line}-{end_line} is out of bounds (file has {total} lines)"
        )));
    }

    let start_idx = (start_line - 1) as usize;
    let end_idx = end_line as usize;

    let mut result_lines = Vec::with_capacity(lines.len());
    result_lines.extend_from_slice(&lines[..start_idx]);
    if end_idx < lines.len() {
        result_lines.extend_from_slice(&lines[end_idx..]);
    }

    let lines_after = result_lines.len();

    if let Err(e) = write_lines(&path, &result_lines) {
        return Ok(tool_error(&e.to_string()));
    }

    let text = serde_json::to_string_pretty(&json!({
        "file": file,
        "lines_before": lines_before,
        "lines_after": lines_after,
    }))?;
    Ok(tool_response(&text))
}

/// Create a new file with content. Creates parent directories automatically.
pub fn create_file(args: &Value, project_root: &Path) -> Result<Value> {
    let file = required_str(args, "file")?;
    let content = required_str(args, "content")?;
    let overwrite = args
        .get("overwrite")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let path = resolve_path(file, project_root)?;

    if path.exists() && !overwrite {
        return Ok(tool_error(&format!(
            "File already exists: {}. Set overwrite=true to replace it.",
            path.display()
        )));
    }

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| anyhow!("Failed to create directories: {e}"))?;
        }
    }

    fs::write(&path, content).map_err(|e| anyhow!("Failed to write {}: {e}", path.display()))?;

    let lines = content.lines().count();
    let bytes = content.len();

    let text = serde_json::to_string_pretty(&json!({
        "file": file,
        "lines": lines,
        "bytes": bytes,
    }))?;
    Ok(tool_response(&text))
}

/// Copy a symbol's source block relative to another symbol.
pub fn copy_symbol(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let source_file = required_str(args, "source_file")?;
    let symbol_name = required_str(args, "symbol")?;
    let target_file = required_str(args, "target_file")?;
    let target_symbol = required_str(args, "target_symbol")?;
    let position = required_position(args, "position")?;

    let source_path = resolve_path(source_file, project_root)?;
    let target_path = resolve_path(target_file, project_root)?;

    let (line_start, line_end) = match find_symbol_location(backend, &source_path, symbol_name) {
        Ok(loc) => loc,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };
    let content_lines = match extract_symbol_lines(&source_path, line_start, line_end) {
        Ok(lines) => lines,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let (inserted_at_line, lines_inserted) = match insert_lines_relative_to_symbol(
        backend,
        &target_path,
        target_symbol,
        position,
        &content_lines,
    ) {
        Ok(result) => result,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let text = serde_json::to_string_pretty(&json!({
        "source_file": source_file,
        "target_file": target_file,
        "symbol": symbol_name,
        "target_symbol": target_symbol,
        "position": match position {
            RelativePosition::Before => "before",
            RelativePosition::After => "after",
        },
        "inserted_at_line": inserted_at_line,
        "lines_copied": content_lines.len(),
        "lines_inserted": lines_inserted,
    }))?;
    Ok(tool_response(&text))
}

/// Move a symbol's source block relative to another symbol in a different file.
pub fn move_symbol(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let source_file = required_str(args, "source_file")?;
    let symbol_name = required_str(args, "symbol")?;
    let target_file = required_str(args, "target_file")?;
    let target_symbol = required_str(args, "target_symbol")?;
    let position = required_position(args, "position")?;

    let source_path = resolve_path(source_file, project_root)?;
    let target_path = resolve_path(target_file, project_root)?;

    if source_path == target_path {
        return Ok(tool_error(
            "Same-file move_symbol is not supported in this MVP. Use copy_symbol plus delete_lines or replace_symbol_body instead.",
        ));
    }

    let (line_start, line_end) = match find_symbol_location(backend, &source_path, symbol_name) {
        Ok(loc) => loc,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };
    let content_lines = match extract_symbol_lines(&source_path, line_start, line_end) {
        Ok(lines) => lines,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let (inserted_at_line, lines_inserted) = match insert_lines_relative_to_symbol(
        backend,
        &target_path,
        target_symbol,
        position,
        &content_lines,
    ) {
        Ok(result) => result,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let (lines_before, lines_after) = match delete_symbol_lines(&source_path, line_start, line_end)
    {
        Ok(result) => result,
        Err(e) => return Ok(tool_error(&e.to_string())),
    };

    let text = serde_json::to_string_pretty(&json!({
        "source_file": source_file,
        "target_file": target_file,
        "symbol": symbol_name,
        "target_symbol": target_symbol,
        "position": match position {
            RelativePosition::Before => "before",
            RelativePosition::After => "after",
        },
        "inserted_at_line": inserted_at_line,
        "lines_moved": content_lines.len(),
        "lines_inserted": lines_inserted,
        "source_lines_before": lines_before,
        "source_lines_after": lines_after,
    }))?;
    Ok(tool_response(&text))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rhizome_core::symbol::{Location, Symbol, SymbolKind};
    use rhizome_core::{BackendCapabilities, CodeIntelligence, Diagnostic, Position};
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Minimal mock backend that returns a single symbol for get_definition.
    struct MockBackend {
        symbols: Vec<Symbol>,
    }

    impl MockBackend {
        fn with_symbol(name: &str, file: &str, line_start: u32, line_end: u32) -> Self {
            Self::with_symbols(vec![(name, file, line_start, line_end)])
        }

        fn with_symbols(entries: Vec<(&str, &str, u32, u32)>) -> Self {
            Self {
                symbols: entries
                    .into_iter()
                    .map(|(name, file, line_start, line_end)| Symbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        location: Location {
                            file_path: file.to_string(),
                            line_start,
                            line_end,
                            column_start: 0,
                            column_end: 0,
                        },
                        scope_path: vec![],
                        signature: None,
                        doc_comment: None,
                        children: vec![],
                    })
                    .collect(),
            }
        }
    }

    impl CodeIntelligence for MockBackend {
        fn get_symbols(&self, _file: &Path) -> rhizome_core::Result<Vec<Symbol>> {
            Ok(vec![])
        }

        fn get_definition(&self, _file: &Path, name: &str) -> rhizome_core::Result<Option<Symbol>> {
            Ok(self
                .symbols
                .iter()
                .find(|symbol| symbol.name == name)
                .cloned())
        }

        fn find_references(
            &self,
            _file: &Path,
            _pos: &Position,
        ) -> rhizome_core::Result<Vec<Location>> {
            Ok(vec![])
        }

        fn search_symbols(
            &self,
            _pattern: &str,
            _root: &Path,
        ) -> rhizome_core::Result<Vec<Symbol>> {
            Ok(vec![])
        }

        fn get_imports(&self, _file: &Path) -> rhizome_core::Result<Vec<Symbol>> {
            Ok(vec![])
        }

        fn get_diagnostics(&self, _file: &Path) -> rhizome_core::Result<Vec<Diagnostic>> {
            Ok(vec![])
        }

        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities {
                cross_file_references: false,
                rename: false,
                type_info: false,
                diagnostics: false,
            }
        }
    }

    fn write_test_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_replace_symbol_body() {
        let dir = TempDir::new().unwrap();
        let content = "fn hello() {\n    println!(\"hello\");\n}\n\nfn other() {}\n";
        let path = write_test_file(&dir, "test.rs", content);

        let backend = MockBackend::with_symbol("hello", path.to_str().unwrap(), 0, 2);

        let args = json!({
            "file": path.to_str().unwrap(),
            "symbol": "hello",
            "new_body": "fn hello() {\n    println!(\"world\");\n}"
        });

        let result = replace_symbol_body(&backend, &args, dir.path()).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["lines_before"], 5);
        assert_eq!(parsed["lines_after"], 5);

        let new_content = fs::read_to_string(&path).unwrap();
        assert!(new_content.contains("println!(\"world\")"));
        assert!(new_content.contains("fn other()"));
    }

    #[test]
    fn test_insert_after_symbol() {
        let dir = TempDir::new().unwrap();
        let content = "fn hello() {\n    println!(\"hello\");\n}\n";
        let path = write_test_file(&dir, "test.rs", content);

        let backend = MockBackend::with_symbol("hello", path.to_str().unwrap(), 0, 2);

        let args = json!({
            "file": path.to_str().unwrap(),
            "symbol": "hello",
            "content": "fn goodbye() {\n    println!(\"bye\");\n}"
        });

        let result = insert_after_symbol(&backend, &args, dir.path()).unwrap();
        assert!(result.get("isError").is_none());

        let new_content = fs::read_to_string(&path).unwrap();
        assert!(new_content.contains("fn hello()"));
        assert!(new_content.contains("fn goodbye()"));
    }

    #[test]
    fn test_insert_before_symbol() {
        let dir = TempDir::new().unwrap();
        let content = "fn hello() {\n    println!(\"hello\");\n}\n";
        let path = write_test_file(&dir, "test.rs", content);

        let backend = MockBackend::with_symbol("hello", path.to_str().unwrap(), 0, 2);

        let args = json!({
            "file": path.to_str().unwrap(),
            "symbol": "hello",
            "content": "/// A greeting function"
        });

        let result = insert_before_symbol(&backend, &args, dir.path()).unwrap();
        assert!(result.get("isError").is_none());

        let new_content = fs::read_to_string(&path).unwrap();
        assert!(new_content.starts_with("/// A greeting function\n"));
        assert!(new_content.contains("fn hello()"));
    }

    #[test]
    fn test_replace_lines() {
        let dir = TempDir::new().unwrap();
        let content = "line1\nline2\nline3\nline4\nline5\n";
        let path = write_test_file(&dir, "test.txt", content);

        let args = json!({
            "file": path.to_str().unwrap(),
            "start_line": 2,
            "end_line": 3,
            "content": "replaced"
        });

        let result = replace_lines(&args, dir.path()).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["lines_before"], 5);
        assert_eq!(parsed["lines_after"], 4);

        let new_content = fs::read_to_string(&path).unwrap();
        assert_eq!(new_content, "line1\nreplaced\nline4\nline5\n");
    }

    #[test]
    fn test_replace_lines_validation() {
        let dir = TempDir::new().unwrap();
        let content = "line1\nline2\n";
        let path = write_test_file(&dir, "test.txt", content);

        // start > end
        let args = json!({
            "file": path.to_str().unwrap(),
            "start_line": 3,
            "end_line": 1,
            "content": "x"
        });
        let result = replace_lines(&args, dir.path()).unwrap();
        assert!(result["isError"].as_bool().unwrap_or(false));

        // out of bounds
        let args = json!({
            "file": path.to_str().unwrap(),
            "start_line": 1,
            "end_line": 10,
            "content": "x"
        });
        let result = replace_lines(&args, dir.path()).unwrap();
        assert!(result["isError"].as_bool().unwrap_or(false));
    }

    #[test]
    fn test_insert_at_line() {
        let dir = TempDir::new().unwrap();
        let content = "line1\nline2\nline3\n";
        let path = write_test_file(&dir, "test.txt", content);

        let args = json!({
            "file": path.to_str().unwrap(),
            "line": 2,
            "content": "inserted"
        });

        let result = insert_at_line(&args, dir.path()).unwrap();
        assert!(result.get("isError").is_none());

        let new_content = fs::read_to_string(&path).unwrap();
        assert_eq!(new_content, "line1\ninserted\nline2\nline3\n");
    }

    #[test]
    fn test_delete_lines() {
        let dir = TempDir::new().unwrap();
        let content = "line1\nline2\nline3\nline4\n";
        let path = write_test_file(&dir, "test.txt", content);

        let args = json!({
            "file": path.to_str().unwrap(),
            "start_line": 2,
            "end_line": 3
        });

        let result = delete_lines(&args, dir.path()).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["lines_before"], 4);
        assert_eq!(parsed["lines_after"], 2);

        let new_content = fs::read_to_string(&path).unwrap();
        assert_eq!(new_content, "line1\nline4\n");
    }

    #[test]
    fn test_create_file() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let args = json!({
            "file": "subdir/new_file.txt",
            "content": "hello world\nsecond line"
        });

        let result = create_file(&args, &root).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["lines"], 2);
        assert_eq!(parsed["bytes"], 23);

        let new_content = fs::read_to_string(dir.path().join("subdir/new_file.txt")).unwrap();
        assert_eq!(new_content, "hello world\nsecond line");
    }

    #[test]
    fn test_create_file_refuses_overwrite() {
        let dir = TempDir::new().unwrap();
        write_test_file(&dir, "existing.txt", "original");

        let args = json!({
            "file": "existing.txt",
            "content": "overwritten"
        });

        let result = create_file(&args, dir.path()).unwrap();
        assert!(result["isError"].as_bool().unwrap_or(false));

        // Original content preserved
        let content = fs::read_to_string(dir.path().join("existing.txt")).unwrap();
        assert_eq!(content, "original");
    }

    #[test]
    fn test_create_file_allows_overwrite() {
        let dir = TempDir::new().unwrap();
        write_test_file(&dir, "existing.txt", "original");

        let args = json!({
            "file": "existing.txt",
            "content": "overwritten",
            "overwrite": true
        });

        let result = create_file(&args, dir.path()).unwrap();
        assert!(result.get("isError").is_none());

        let content = fs::read_to_string(dir.path().join("existing.txt")).unwrap();
        assert_eq!(content, "overwritten");
    }

    #[test]
    fn test_copy_symbol_to_another_file_after_target() {
        let dir = TempDir::new().unwrap();
        let source_path = write_test_file(&dir, "source.rs", "fn alpha() {\n    1\n}\n");
        let target_path = write_test_file(&dir, "target.rs", "fn beta() {\n    2\n}\n");

        let backend = MockBackend::with_symbols(vec![
            ("alpha", source_path.to_str().unwrap(), 0, 2),
            ("beta", target_path.to_str().unwrap(), 0, 2),
        ]);

        let args = json!({
            "source_file": source_path.to_str().unwrap(),
            "symbol": "alpha",
            "target_file": target_path.to_str().unwrap(),
            "target_symbol": "beta",
            "position": "after"
        });

        let result = copy_symbol(&backend, &args, dir.path()).unwrap();
        assert!(result.get("isError").is_none());

        let target_content = fs::read_to_string(&target_path).unwrap();
        assert!(target_content.contains("fn beta()"));
        assert!(target_content.contains("fn alpha()"));
        assert_eq!(
            fs::read_to_string(&source_path).unwrap(),
            "fn alpha() {\n    1\n}\n"
        );
    }

    #[test]
    fn test_move_symbol_to_another_file_before_target() {
        let dir = TempDir::new().unwrap();
        let source_path = write_test_file(&dir, "source.rs", "fn alpha() {\n    1\n}\n");
        let target_path = write_test_file(&dir, "target.rs", "fn beta() {\n    2\n}\n");

        let backend = MockBackend::with_symbols(vec![
            ("alpha", source_path.to_str().unwrap(), 0, 2),
            ("beta", target_path.to_str().unwrap(), 0, 2),
        ]);

        let args = json!({
            "source_file": source_path.to_str().unwrap(),
            "symbol": "alpha",
            "target_file": target_path.to_str().unwrap(),
            "target_symbol": "beta",
            "position": "before"
        });

        let result = move_symbol(&backend, &args, dir.path()).unwrap();
        assert!(result.get("isError").is_none());

        assert_eq!(fs::read_to_string(&source_path).unwrap(), "");
        let target_content = fs::read_to_string(&target_path).unwrap();
        assert!(target_content.starts_with("fn alpha() {\n    1\n}\n\n"));
        assert!(target_content.contains("fn beta()"));
    }

    #[test]
    fn test_move_symbol_same_file_is_rejected() {
        let dir = TempDir::new().unwrap();
        let path = write_test_file(&dir, "test.rs", "fn alpha() {}\nfn beta() {}\n");

        let backend = MockBackend::with_symbols(vec![
            ("alpha", path.to_str().unwrap(), 0, 0),
            ("beta", path.to_str().unwrap(), 1, 1),
        ]);

        let args = json!({
            "source_file": path.to_str().unwrap(),
            "symbol": "alpha",
            "target_file": path.to_str().unwrap(),
            "target_symbol": "beta",
            "position": "after"
        });

        let result = move_symbol(&backend, &args, dir.path()).unwrap();
        assert!(result["isError"].as_bool().unwrap_or(false));
    }

    #[test]
    fn test_symbol_not_found() {
        let dir = TempDir::new().unwrap();
        let content = "fn hello() {}\n";
        let _path = write_test_file(&dir, "test.rs", content);

        let backend = MockBackend { symbols: vec![] };

        let args = json!({
            "file": "test.rs",
            "symbol": "nonexistent",
            "new_body": "fn nonexistent() {}"
        });

        let result = replace_symbol_body(&backend, &args, dir.path()).unwrap();
        assert!(result["isError"].as_bool().unwrap_or(false));
    }
}
