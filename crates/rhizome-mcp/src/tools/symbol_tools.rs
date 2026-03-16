use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use rhizome_core::{CodeIntelligence, Position, Symbol};
use serde_json::{json, Value};

use super::{tool_response, ToolSchema};

// ---------------------------------------------------------------------------
// Tool schemas
// ---------------------------------------------------------------------------

pub fn tool_schemas() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            name: "get_symbols".into(),
            description: "List all symbols (functions, structs, classes, etc.) in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_structure".into(),
            description: "Show the hierarchical structure of symbols in a file as an indented tree"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "depth": { "type": "number", "description": "Maximum nesting depth to display" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_definition".into(),
            description: "Get the full definition of a symbol including its body".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol to find" },
                    "full": { "type": "boolean", "description": "Show full body even if large (default: false)" }
                },
                "required": ["file", "symbol"]
            }),
        },
        ToolSchema {
            name: "search_symbols".into(),
            description: "Search for symbols matching a pattern across the project".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Pattern to match symbol names (case-insensitive)" },
                    "path": { "type": "string", "description": "Optional directory to search in (defaults to project root)" }
                },
                "required": ["pattern"]
            }),
        },
        ToolSchema {
            name: "find_references".into(),
            description: "Find all references to the symbol at a given position".into(),
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
        ToolSchema {
            name: "go_to_definition".into(),
            description: "Find the definition of the symbol at a given position".into(),
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
        ToolSchema {
            name: "get_signature".into(),
            description: "Get only the signature of a symbol (no body)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol" }
                },
                "required": ["file", "symbol"]
            }),
        },
        ToolSchema {
            name: "get_imports".into(),
            description: "List all import statements in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_call_sites".into(),
            description: "Find all function call expressions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Filter to calls of a specific function" }
                },
                "required": ["file"]
            }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Helper: extract a required string arg
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tool handlers
// ---------------------------------------------------------------------------

/// List all symbols in a file.
pub fn get_symbols(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    let formatted: Vec<Value> = symbols.iter().flat_map(flatten_symbol).collect();
    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

fn flatten_symbol(sym: &Symbol) -> Vec<Value> {
    let mut results = vec![json!({
        "name": sym.name,
        "kind": format!("{:?}", sym.kind),
        "location": {
            "file": &sym.location.file_path,
            "line_start": sym.location.line_start,
            "line_end": sym.location.line_end,
            "column_start": sym.location.column_start,
            "column_end": sym.location.column_end,
        },
        "signature": sym.signature,
    })];

    for child in &sym.children {
        results.extend(flatten_symbol(child));
    }

    results
}

/// Show hierarchical structure of symbols as an indented tree.
pub fn get_structure(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let max_depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    let mut output = String::new();
    for sym in &symbols {
        format_tree(sym, 0, max_depth, &mut output);
    }

    Ok(tool_response(output.trim_end()))
}

fn format_tree(sym: &Symbol, depth: usize, max_depth: Option<usize>, output: &mut String) {
    if let Some(max) = max_depth {
        if depth > max {
            return;
        }
    }

    let indent = "  ".repeat(depth);
    let kind = format!("{:?}", sym.kind);
    let _ = writeln!(output, "{indent}{kind} {}", sym.name);

    for child in &sym.children {
        format_tree(child, depth + 1, max_depth, output);
    }
}

/// Get the definition of a symbol, with optional body truncation.
pub fn get_definition(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;
    let full = args.get("full").and_then(|v| v.as_bool()).unwrap_or(false);

    let path = Path::new(file);
    let sym = backend.get_definition(path, symbol_name)?;

    match sym {
        Some(sym) => {
            let body = read_symbol_body(path, &sym, full)?;
            let result = json!({
                "name": sym.name,
                "kind": format!("{:?}", sym.kind),
                "signature": sym.signature,
                "doc_comment": sym.doc_comment,
                "location": {
                    "file": &sym.location.file_path,
                    "line_start": sym.location.line_start,
                    "line_end": sym.location.line_end,
                },
                "body": body,
            });
            let text = serde_json::to_string_pretty(&result)?;
            Ok(tool_response(&text))
        }
        None => Ok(tool_response(&format!(
            "Symbol '{symbol_name}' not found in {file}"
        ))),
    }
}

fn read_symbol_body(file: &Path, sym: &Symbol, full: bool) -> Result<String> {
    let source = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = source.lines().collect();

    let start = sym.location.line_start as usize;
    let end = sym.location.line_end as usize;

    if start >= lines.len() {
        return Ok(String::new());
    }

    let end = end.min(lines.len().saturating_sub(1));
    let total = end - start + 1;

    if !full && total > 50 {
        // Show signature (first 10 lines) + truncation notice
        let preview_end = (start + 10).min(end);
        let mut body = String::new();
        for line in &lines[start..=preview_end] {
            body.push_str(line);
            body.push('\n');
        }
        let remaining = total - 11;
        let _ = write!(body, "... ({remaining} more lines)");
        Ok(body)
    } else {
        let mut body = String::new();
        for line in &lines[start..=end] {
            body.push_str(line);
            body.push('\n');
        }
        Ok(body)
    }
}

/// Search for symbols matching a pattern across a project.
pub fn search_symbols(
    backend: &dyn CodeIntelligence,
    args: &Value,
    default_root: &Path,
) -> Result<Value> {
    let pattern = required_str(args, "pattern")?;
    let search_path = args
        .get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_root.to_path_buf());

    let symbols = backend.search_symbols(pattern, &search_path)?;

    let formatted: Vec<Value> = symbols
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "kind": format!("{:?}", s.kind),
                "file": &s.location.file_path,
                "line": s.location.line_start,
                "signature": s.signature,
            })
        })
        .collect();

    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

/// Find all references to the symbol at a given position.
pub fn find_references(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let line = required_u32(args, "line")?;
    let column = required_u32(args, "column")?;

    let path = Path::new(file);
    let pos = Position { line, column };
    let locations = backend.find_references(path, &pos)?;

    let formatted: Vec<Value> = locations
        .iter()
        .map(|loc| {
            json!({
                "file": &loc.file_path,
                "line_start": loc.line_start,
                "line_end": loc.line_end,
                "column_start": loc.column_start,
                "column_end": loc.column_end,
            })
        })
        .collect();

    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

/// Find the definition of the symbol at a given position.
/// Uses tree-sitter to identify the symbol name at the position, then calls get_definition.
pub fn go_to_definition(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let line = required_u32(args, "line")?;
    let column = required_u32(args, "column")?;

    let path = Path::new(file);
    let pos = Position { line, column };

    // Find the identifier at the given position by looking at references
    let refs = backend.find_references(path, &pos)?;
    if refs.is_empty() {
        return Ok(tool_response("No symbol found at the given position"));
    }

    // Read the source to get the symbol name at the position
    let source = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = line as usize;

    if line_idx >= lines.len() {
        return Ok(tool_response("Position is beyond end of file"));
    }

    let line_text = lines[line_idx];
    let col = column as usize;

    // Extract the identifier at the given column
    let name = extract_identifier_at(line_text, col);
    if name.is_empty() {
        return Ok(tool_response("No identifier at the given position"));
    }

    let sym = backend.get_definition(path, &name)?;
    match sym {
        Some(sym) => {
            let result = json!({
                "name": sym.name,
                "kind": format!("{:?}", sym.kind),
                "file": &sym.location.file_path,
                "line_start": sym.location.line_start,
                "line_end": sym.location.line_end,
                "column_start": sym.location.column_start,
                "column_end": sym.location.column_end,
                "signature": sym.signature,
            });
            let text = serde_json::to_string_pretty(&result)?;
            Ok(tool_response(&text))
        }
        None => Ok(tool_response(&format!("Definition not found for '{name}'"))),
    }
}

fn extract_identifier_at(line: &str, col: usize) -> String {
    let bytes = line.as_bytes();
    if col >= bytes.len() {
        return String::new();
    }

    // Check if we're on an identifier character
    if !is_ident_char(bytes[col]) {
        return String::new();
    }

    // Walk backward to find start
    let mut start = col;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }

    // Walk forward to find end
    let mut end = col;
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }

    line[start..end].to_string()
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Get only the signature of a symbol.
pub fn get_signature(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;

    let path = Path::new(file);
    let sym = backend.get_definition(path, symbol_name)?;

    match sym {
        Some(sym) => {
            let sig = sym
                .signature
                .unwrap_or_else(|| format!("{} (no signature available)", sym.name));
            Ok(tool_response(&sig))
        }
        None => Ok(tool_response(&format!(
            "Symbol '{symbol_name}' not found in {file}"
        ))),
    }
}

/// List all import statements in a file.
pub fn get_imports(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = Path::new(file);
    let imports = backend.get_imports(path)?;

    let formatted: Vec<Value> = imports
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "location": {
                    "file": &s.location.file_path,
                    "line_start": s.location.line_start,
                    "line_end": s.location.line_end,
                },
                "signature": s.signature,
            })
        })
        .collect();

    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

/// Find function call expressions in a file.
/// Uses tree-sitter to parse and find call_expression nodes.
pub fn get_call_sites(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let function_filter = args.get("function").and_then(|v| v.as_str());

    let path = Path::new(file);
    let source = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = source.lines().collect();

    // Use the backend to get symbols which validates the file is parseable
    let _symbols = backend.get_symbols(path)?;

    // Parse call sites from the source text by scanning for function call patterns
    let mut call_sites = Vec::new();
    for (line_idx, line_text) in lines.iter().enumerate() {
        let trimmed = line_text.trim();
        // Skip comments and empty lines
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with("///")
        {
            continue;
        }

        extract_calls_from_line(
            line_text,
            line_idx as u32,
            file,
            function_filter,
            &mut call_sites,
        );
    }

    let text = serde_json::to_string_pretty(&call_sites)?;
    Ok(tool_response(&text))
}

fn extract_calls_from_line(
    line: &str,
    line_num: u32,
    file: &str,
    function_filter: Option<&str>,
    results: &mut Vec<Value>,
) {
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Look for '(' preceded by an identifier
        if bytes[i] == b'(' && i > 0 {
            // Walk backward to find the function name
            let paren_pos = i;
            let mut name_end = i;

            // Skip any whitespace between name and paren
            while name_end > 0 && bytes[name_end - 1] == b' ' {
                name_end -= 1;
            }

            if name_end > 0 && is_ident_char(bytes[name_end - 1]) {
                let mut name_start = name_end - 1;
                while name_start > 0 && is_ident_char(bytes[name_start - 1]) {
                    name_start -= 1;
                }

                let name = &line[name_start..name_end];

                // Skip language keywords
                if !is_keyword(name) {
                    if let Some(filter) = function_filter {
                        if name == filter {
                            results.push(json!({
                                "function": name,
                                "file": file,
                                "line": line_num,
                                "column": name_start as u32,
                                "context": line.trim(),
                            }));
                        }
                    } else {
                        results.push(json!({
                            "function": name,
                            "file": file,
                            "line": line_num,
                            "column": paren_pos as u32,
                            "context": line.trim(),
                        }));
                    }
                }
            }
        }
        i += 1;
    }
}

fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "else"
            | "for"
            | "while"
            | "match"
            | "return"
            | "let"
            | "mut"
            | "fn"
            | "pub"
            | "use"
            | "mod"
            | "struct"
            | "enum"
            | "impl"
            | "trait"
            | "type"
            | "const"
            | "static"
            | "where"
            | "async"
            | "await"
            | "loop"
            | "break"
            | "continue"
            | "self"
            | "super"
            | "crate"
            | "as"
            | "in"
            | "def"
            | "class"
            | "import"
            | "from"
            | "with"
            | "try"
            | "except"
            | "raise"
            | "pass"
            | "assert"
            | "yield"
            | "not"
            | "and"
            | "or"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_identifier_at() {
        assert_eq!(extract_identifier_at("fn hello() {}", 3), "hello");
        assert_eq!(extract_identifier_at("fn hello() {}", 5), "hello");
        assert_eq!(extract_identifier_at("fn hello() {}", 8), "");
        assert_eq!(extract_identifier_at("my_var = 42", 0), "my_var");
    }

    #[test]
    fn test_is_keyword() {
        assert!(is_keyword("if"));
        assert!(is_keyword("fn"));
        assert!(!is_keyword("hello"));
        assert!(!is_keyword("process"));
    }
}
