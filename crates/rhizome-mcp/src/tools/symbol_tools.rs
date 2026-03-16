use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};
use rhizome_core::{CodeIntelligence, Position, Symbol, SymbolKind};
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
        // --- New tools ---
        ToolSchema {
            name: "get_scope".into(),
            description: "Get the enclosing scope (function, class, module) at a given line".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" }
                },
                "required": ["file", "line"]
            }),
        },
        ToolSchema {
            name: "get_exports".into(),
            description: "List only public/exported symbols in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "summarize_file".into(),
            description: "Compact file summary showing only public signatures, no bodies".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_tests".into(),
            description: "Find test functions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_diff_symbols".into(),
            description:
                "Show which symbols were modified in uncommitted changes or between commits".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Limit to a specific file" },
                    "ref1": { "type": "string", "description": "First git ref (default: HEAD)" },
                    "ref2": { "type": "string", "description": "Second git ref (default: working tree)" }
                },
                "required": []
            }),
        },
        ToolSchema {
            name: "get_annotations".into(),
            description: "Find TODO, FIXME, HACK, and other annotation comments in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Specific tags to search for (default: TODO, FIXME, HACK, XXX, NOTE, WARN)"
                    }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_complexity".into(),
            description: "Calculate cyclomatic complexity for functions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Analyze only this function" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_type_definitions".into(),
            description:
                "List type definitions (structs, enums, interfaces, type aliases) in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        // --- Batch 2 tools ---
        ToolSchema {
            name: "get_dependencies".into(),
            description: "Map which functions call which within a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_parameters".into(),
            description: "Extract function parameters with types".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Filter to a specific function" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_enclosing_class".into(),
            description: "Get the parent class/struct and all sibling methods for a given method"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "method": { "type": "string", "description": "Name of the method to find" }
                },
                "required": ["file", "method"]
            }),
        },
        ToolSchema {
            name: "get_symbol_body".into(),
            description: "Get the source code body of a specific symbol by name and optional line"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol" },
                    "line": { "type": "number", "description": "Line number to disambiguate (0-based)" }
                },
                "required": ["file", "symbol"]
            }),
        },
        ToolSchema {
            name: "get_changed_files".into(),
            description: "List files with uncommitted changes and their modified symbol counts"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ref1": { "type": "string", "description": "Git ref for diff start" },
                    "ref2": { "type": "string", "description": "Git ref for diff end" }
                },
                "required": []
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

// ---------------------------------------------------------------------------
// Tool 1: get_scope
// ---------------------------------------------------------------------------

/// Get the enclosing scope at a given line.
pub fn get_scope(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let line = required_u32(args, "line")?;

    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    let scope = find_innermost_scope(&symbols, line);

    match scope {
        Some(sym) => {
            let result = json!({
                "name": sym.name,
                "kind": format!("{:?}", sym.kind),
                "line_start": sym.location.line_start,
                "line_end": sym.location.line_end,
            });
            let text = serde_json::to_string_pretty(&result)?;
            Ok(tool_response(&text))
        }
        None => Ok(tool_response("Top-level scope")),
    }
}

fn find_innermost_scope(symbols: &[Symbol], line: u32) -> Option<&Symbol> {
    let mut best: Option<&Symbol> = None;

    for sym in symbols {
        if sym.location.line_start <= line && line <= sym.location.line_end {
            // This symbol contains the line; check if it's more specific than current best
            let is_better = match best {
                None => true,
                Some(prev) => {
                    let prev_span = prev.location.line_end - prev.location.line_start;
                    let this_span = sym.location.line_end - sym.location.line_start;
                    this_span < prev_span
                }
            };
            if is_better {
                best = Some(sym);
            }

            // Check children for a tighter scope
            if let Some(child) = find_innermost_scope(&sym.children, line) {
                let child_span = child.location.line_end - child.location.line_start;
                let is_child_better = match best {
                    None => true,
                    Some(prev) => {
                        let prev_span = prev.location.line_end - prev.location.line_start;
                        child_span < prev_span
                    }
                };
                if is_child_better {
                    best = Some(child);
                }
            }
        }
    }

    best
}

// ---------------------------------------------------------------------------
// Tool 2: get_exports
// ---------------------------------------------------------------------------

/// List only public/exported symbols in a file.
pub fn get_exports(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let mut exports = Vec::new();
    collect_exports(&symbols, ext, &mut exports);

    let text = serde_json::to_string_pretty(&exports)?;
    Ok(tool_response(&text))
}

fn collect_exports(symbols: &[Symbol], ext: &str, out: &mut Vec<Value>) {
    for sym in symbols {
        if is_exported(sym, ext) {
            out.push(json!({
                "name": sym.name,
                "kind": format!("{:?}", sym.kind),
                "signature": sym.signature,
            }));
        }
        // Check children (e.g. pub methods inside impl blocks)
        collect_exports(&sym.children, ext, out);
    }
}

fn is_exported(sym: &Symbol, ext: &str) -> bool {
    match ext {
        "rs" => sym
            .signature
            .as_deref()
            .map(|s| s.starts_with("pub ") || s.starts_with("pub("))
            .unwrap_or(false),
        "py" => !sym.name.starts_with('_'),
        "js" | "ts" | "jsx" | "tsx" | "mjs" => sym
            .signature
            .as_deref()
            .map(|s| s.contains("export"))
            .unwrap_or(false),
        "go" => sym
            .name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false),
        _ => true, // can't determine visibility, return all
    }
}

// ---------------------------------------------------------------------------
// Tool 3: summarize_file
// ---------------------------------------------------------------------------

/// Compact file summary showing only public signatures, no bodies.
pub fn summarize_file(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    let mut output = String::new();
    for sym in &symbols {
        format_summary(sym, 0, &mut output);
    }

    if output.is_empty() {
        Ok(tool_response("(no symbols found)"))
    } else {
        Ok(tool_response(output.trim_end()))
    }
}

fn format_summary(sym: &Symbol, depth: usize, output: &mut String) {
    let indent = "  ".repeat(depth);
    let doc_line = sym
        .doc_comment
        .as_deref()
        .and_then(|d| d.lines().next())
        .map(|l| l.trim());

    if let Some(doc) = doc_line {
        let _ = writeln!(output, "{indent}/// {doc}");
    }

    if let Some(sig) = &sym.signature {
        let _ = writeln!(output, "{indent}{sig}");
    } else {
        let kind = format!("{:?}", sym.kind);
        let _ = writeln!(output, "{indent}{kind} {}", sym.name);
    }

    for child in &sym.children {
        format_summary(child, depth + 1, output);
    }
}

// ---------------------------------------------------------------------------
// Tool 4: get_tests
// ---------------------------------------------------------------------------

/// Find test functions in a file.
pub fn get_tests(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Read source for attribute checking
    let source = std::fs::read_to_string(path)?;
    let source_lines: Vec<&str> = source.lines().collect();

    let mut tests = Vec::new();
    collect_tests(&symbols, ext, &source_lines, false, &mut tests);

    let text = serde_json::to_string_pretty(&tests)?;
    Ok(tool_response(&text))
}

fn collect_tests(
    symbols: &[Symbol],
    ext: &str,
    source_lines: &[&str],
    in_test_module: bool,
    out: &mut Vec<Value>,
) {
    for sym in symbols {
        let is_test_mod = matches!(sym.kind, SymbolKind::Module) && sym.name == "tests";
        let inside_tests = in_test_module || is_test_mod;

        let is_test_fn = match ext {
            "rs" => {
                if !matches!(sym.kind, SymbolKind::Function | SymbolKind::Method) {
                    false
                } else {
                    // Check doc_comment for #[test]
                    let has_test_attr = sym
                        .doc_comment
                        .as_deref()
                        .map(|d| d.contains("#[test]"))
                        .unwrap_or(false);

                    // Check source lines above the function for #[test]
                    let has_test_in_source = if sym.location.line_start > 0 {
                        let start = sym.location.line_start.saturating_sub(5) as usize;
                        let end = sym.location.line_start as usize;
                        (start..end).any(|i| {
                            i < source_lines.len() && source_lines[i].trim().starts_with("#[test]")
                        })
                    } else {
                        false
                    };

                    has_test_attr || has_test_in_source || inside_tests
                }
            }
            "py" => {
                (matches!(sym.kind, SymbolKind::Function | SymbolKind::Method)
                    && sym.name.starts_with("test_"))
                    || (matches!(sym.kind, SymbolKind::Class) && sym.name.starts_with("Test"))
            }
            "js" | "ts" | "jsx" | "tsx" | "mjs" => {
                matches!(sym.kind, SymbolKind::Function | SymbolKind::Method)
                    && (sym.name.contains("test")
                        || sym.name.contains("it")
                        || sym.name.contains("describe"))
            }
            _ => sym.name.starts_with("test"),
        };

        if is_test_fn {
            out.push(json!({
                "name": sym.name,
                "kind": format!("{:?}", sym.kind),
                "file": &sym.location.file_path,
                "line_start": sym.location.line_start,
                "line_end": sym.location.line_end,
            }));
        }

        collect_tests(&sym.children, ext, source_lines, inside_tests, out);
    }
}

// ---------------------------------------------------------------------------
// Tool 5: get_diff_symbols
// ---------------------------------------------------------------------------

/// Show which symbols were modified in uncommitted changes or between commits.
pub fn get_diff_symbols(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file_filter = args.get("file").and_then(|v| v.as_str());
    let ref1 = args.get("ref1").and_then(|v| v.as_str());
    let ref2 = args.get("ref2").and_then(|v| v.as_str());

    let mut cmd = Command::new("git");
    cmd.current_dir(project_root);

    match (ref1, ref2) {
        (Some(r1), Some(r2)) => {
            cmd.args(["diff", "--unified=0", r1, r2]);
        }
        (Some(r1), None) => {
            cmd.args(["diff", "--unified=0", r1]);
        }
        _ => {
            cmd.args(["diff", "--unified=0", "HEAD"]);
        }
    }

    if let Some(f) = file_filter {
        cmd.arg("--").arg(f);
    }

    let output = cmd.output()?;
    if !output.status.success() && output.stdout.is_empty() {
        // git diff returns 0 even with no changes; non-zero may mean bad ref
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            return Ok(tool_response(&format!("git diff error: {stderr}")));
        }
    }

    let diff_text = String::from_utf8_lossy(&output.stdout);
    if diff_text.is_empty() {
        return Ok(tool_response("No changes found"));
    }

    let changed_files = parse_diff_hunks(&diff_text);
    let mut results = Vec::new();

    for (rel_path, changed_lines) in &changed_files {
        let abs_path = if Path::new(rel_path).is_absolute() {
            PathBuf::from(rel_path)
        } else {
            project_root.join(rel_path)
        };

        if !abs_path.exists() {
            // File was deleted
            for &line in changed_lines {
                results.push(json!({
                    "name": "(deleted)",
                    "kind": "Unknown",
                    "file": rel_path,
                    "line": line,
                    "status": "deleted",
                }));
            }
            continue;
        }

        let symbols = match backend.get_symbols(&abs_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut matched_symbols = std::collections::HashSet::new();
        for &line in changed_lines {
            if let Some(sym) = find_innermost_scope(&symbols, line) {
                if matched_symbols.insert(sym.name.clone()) {
                    results.push(json!({
                        "name": sym.name,
                        "kind": format!("{:?}", sym.kind),
                        "file": rel_path,
                        "line_start": sym.location.line_start,
                        "line_end": sym.location.line_end,
                        "status": "modified",
                    }));
                }
            }
        }
    }

    let text = serde_json::to_string_pretty(&results)?;
    Ok(tool_response(&text))
}

fn parse_diff_hunks(diff: &str) -> Vec<(String, Vec<u32>)> {
    let mut result: Vec<(String, Vec<u32>)> = Vec::new();
    let mut current_file: Option<String> = None;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            current_file = Some(rest.to_string());
        } else if line.starts_with("+++ /dev/null") {
            current_file = None;
        } else if line.starts_with("@@ ") {
            if let Some(ref file) = current_file {
                // Parse the +c,d part from "@@ -a,b +c,d @@"
                if let Some(plus_part) = line.split('+').nth(1) {
                    let range_part = plus_part.split(' ').next().unwrap_or("");
                    let parts: Vec<&str> = range_part.split(',').collect();
                    if let Ok(start) = parts[0].parse::<u32>() {
                        let count = parts
                            .get(1)
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(1);
                        // Convert 1-based to 0-based
                        let start_0 = start.saturating_sub(1);
                        let entry = result.iter_mut().find(|(f, _)| f == file);
                        let lines = if let Some((_, lines)) = entry {
                            lines
                        } else {
                            result.push((file.clone(), Vec::new()));
                            &mut result.last_mut().unwrap().1
                        };
                        for i in 0..count {
                            lines.push(start_0 + i);
                        }
                    }
                }
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tool 6: get_annotations
// ---------------------------------------------------------------------------

/// Find TODO, FIXME, HACK, and other annotation comments in a file.
pub fn get_annotations(_backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let tags_arg = args.get("tags").and_then(|v| v.as_array());

    let default_tags = vec!["TODO", "FIXME", "HACK", "XXX", "NOTE", "WARN"];
    let custom_tags: Vec<String>;
    let tags: Vec<&str> = if let Some(arr) = tags_arg {
        custom_tags = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        custom_tags.iter().map(|s| s.as_str()).collect()
    } else {
        default_tags
    };

    let source = std::fs::read_to_string(file)?;
    let mut annotations = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let upper = line.to_uppercase();
        for tag in &tags {
            let tag_upper = tag.to_uppercase();
            if let Some(pos) = upper.find(&tag_upper) {
                // Extract the message after the tag
                let after_tag = &line[pos + tag.len()..];
                let message = after_tag
                    .trim_start_matches([':', ' ', ']'])
                    .trim_end_matches(['*', '/'])
                    .trim();

                annotations.push(json!({
                    "tag": *tag,
                    "message": message,
                    "file": file,
                    "line": line_idx,
                }));
                break; // Only match one tag per line
            }
        }
    }

    let text = serde_json::to_string_pretty(&annotations)?;
    Ok(tool_response(&text))
}

// ---------------------------------------------------------------------------
// Tool 7: get_complexity
// ---------------------------------------------------------------------------

/// Calculate cyclomatic complexity for functions in a file.
pub fn get_complexity(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let function_filter = args.get("function").and_then(|v| v.as_str());

    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;
    let source = std::fs::read_to_string(path)?;
    let source_lines: Vec<&str> = source.lines().collect();

    let mut results = Vec::new();
    collect_complexity(&symbols, &source_lines, function_filter, &mut results);

    let text = serde_json::to_string_pretty(&results)?;
    Ok(tool_response(&text))
}

fn collect_complexity(
    symbols: &[Symbol],
    source_lines: &[&str],
    function_filter: Option<&str>,
    out: &mut Vec<Value>,
) {
    for sym in symbols {
        if matches!(sym.kind, SymbolKind::Function | SymbolKind::Method) {
            if let Some(filter) = function_filter {
                if sym.name != filter {
                    collect_complexity(&sym.children, source_lines, function_filter, out);
                    continue;
                }
            }

            let start = sym.location.line_start as usize;
            let end = (sym.location.line_end as usize).min(source_lines.len().saturating_sub(1));

            let mut complexity: u32 = 1; // base complexity
            let branch_keywords = [
                "if ", "if(", "else if", "elif ", "match ", "for ", "for(", "while ", "while(",
                "loop ", "loop{", "&&", "||", "catch ", "catch(", "case ",
            ];

            for line_idx in start..=end {
                if line_idx >= source_lines.len() {
                    break;
                }
                let line = source_lines[line_idx].trim();
                // Skip comments
                if line.starts_with("//") || line.starts_with('#') || line.starts_with("/*") {
                    continue;
                }
                for kw in &branch_keywords {
                    // Count each occurrence of the keyword in this line
                    let mut search_from = 0;
                    while let Some(pos) = line[search_from..].find(kw) {
                        complexity += 1;
                        search_from += pos + kw.len();
                    }
                }
            }

            let rating = match complexity {
                1..=5 => "simple",
                6..=10 => "moderate",
                11..=20 => "complex",
                _ => "very complex",
            };

            out.push(json!({
                "name": sym.name,
                "complexity": complexity,
                "rating": rating,
                "line_start": sym.location.line_start,
                "line_end": sym.location.line_end,
            }));
        }

        collect_complexity(&sym.children, source_lines, function_filter, out);
    }
}

// ---------------------------------------------------------------------------
// Tool 8: get_type_definitions
// ---------------------------------------------------------------------------

/// List type definitions (structs, enums, interfaces, type aliases) in a file.
pub fn get_type_definitions(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    let mut type_defs = Vec::new();
    collect_type_definitions(&symbols, &mut type_defs);

    let text = serde_json::to_string_pretty(&type_defs)?;
    Ok(tool_response(&text))
}

fn collect_type_definitions(symbols: &[Symbol], out: &mut Vec<Value>) {
    for sym in symbols {
        if matches!(
            sym.kind,
            SymbolKind::Struct
                | SymbolKind::Enum
                | SymbolKind::Interface
                | SymbolKind::Trait
                | SymbolKind::Type
                | SymbolKind::Class
        ) {
            let doc_line = sym
                .doc_comment
                .as_deref()
                .and_then(|d| d.lines().next())
                .map(|l| l.trim().to_string());

            let children_info: Vec<Value> = sym
                .children
                .iter()
                .map(|c| {
                    json!({
                        "name": c.name,
                        "kind": format!("{:?}", c.kind),
                    })
                })
                .collect();

            let mut entry = json!({
                "name": sym.name,
                "kind": format!("{:?}", sym.kind),
                "signature": sym.signature,
                "line_start": sym.location.line_start,
                "line_end": sym.location.line_end,
            });

            if let Some(doc) = doc_line {
                entry["doc_comment"] = json!(doc);
            }

            if !children_info.is_empty() {
                entry["members"] = json!(children_info);
            }

            out.push(entry);
        }

        collect_type_definitions(&sym.children, out);
    }
}

// ---------------------------------------------------------------------------
// Tool 9: get_dependencies
// ---------------------------------------------------------------------------

/// Map which functions call which within a file.
pub fn get_dependencies(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;
    let source = std::fs::read_to_string(path)?;
    let source_lines: Vec<&str> = source.lines().collect();

    // Collect all function/method names with their line ranges
    let mut functions: Vec<(&str, usize, usize)> = Vec::new();
    collect_function_ranges(&symbols, &mut functions);

    let function_names: Vec<&str> = functions.iter().map(|(name, _, _)| *name).collect();

    let mut deps: serde_json::Map<String, Value> = serde_json::Map::new();

    for &(name, start, end) in &functions {
        let mut calls = Vec::new();
        let end = end.min(source_lines.len().saturating_sub(1));
        for line_idx in start..=end {
            if line_idx >= source_lines.len() {
                break;
            }
            let line = source_lines[line_idx];
            for &target in &function_names {
                if target == name {
                    continue;
                }
                // Look for the target name followed by '(' in this line
                if let Some(pos) = line.find(target) {
                    let after = pos + target.len();
                    // Check there's a '(' after (possibly with whitespace)
                    let rest = line[after..].trim_start();
                    if rest.starts_with('(') {
                        // Also check it's not part of a larger identifier
                        let before_ok = pos == 0 || !is_ident_char(line.as_bytes()[pos - 1]);
                        let after_ok = after >= line.len()
                            || !line.as_bytes()[after].is_ascii_alphanumeric()
                                && line.as_bytes()[after] != b'_';
                        if before_ok && after_ok && !calls.contains(&target.to_string()) {
                            calls.push(target.to_string());
                        }
                    }
                }
            }
        }
        deps.insert(name.to_string(), json!(calls));
    }

    let text = serde_json::to_string_pretty(&Value::Object(deps))?;
    Ok(tool_response(&text))
}

fn collect_function_ranges<'a>(symbols: &'a [Symbol], out: &mut Vec<(&'a str, usize, usize)>) {
    for sym in symbols {
        if matches!(sym.kind, SymbolKind::Function | SymbolKind::Method) {
            out.push((
                &sym.name,
                sym.location.line_start as usize,
                sym.location.line_end as usize,
            ));
        }
        collect_function_ranges(&sym.children, out);
    }
}

// ---------------------------------------------------------------------------
// Tool 10: get_parameters
// ---------------------------------------------------------------------------

/// Extract function parameters with types.
pub fn get_parameters(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let function_filter = args.get("function").and_then(|v| v.as_str());

    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    let mut results = Vec::new();
    collect_parameters(&symbols, function_filter, &mut results);

    let text = serde_json::to_string_pretty(&results)?;
    Ok(tool_response(&text))
}

fn collect_parameters(symbols: &[Symbol], filter: Option<&str>, out: &mut Vec<Value>) {
    for sym in symbols {
        if matches!(sym.kind, SymbolKind::Function | SymbolKind::Method) {
            if let Some(f) = filter {
                if sym.name != f {
                    collect_parameters(&sym.children, Some(f), out);
                    continue;
                }
            }

            let params = parse_params_from_signature(sym.signature.as_deref());
            out.push(json!({
                "function": sym.name,
                "parameters": params,
            }));
        }
        collect_parameters(&sym.children, filter, out);
    }
}

fn parse_params_from_signature(sig: Option<&str>) -> Vec<Value> {
    let sig = match sig {
        Some(s) => s,
        None => return Vec::new(),
    };

    // Find the parameter list between the first '(' and its matching ')'
    let open = match sig.find('(') {
        Some(i) => i,
        None => return Vec::new(),
    };

    let mut depth = 0;
    let mut close = None;
    for (i, c) in sig[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }

    let close = match close {
        Some(c) => c,
        None => return Vec::new(),
    };

    let params_str = &sig[open + 1..close];
    if params_str.trim().is_empty() {
        return Vec::new();
    }

    // Split by commas (respecting nested generics)
    let mut params = Vec::new();
    let mut current = String::new();
    let mut angle_depth = 0;
    let mut paren_depth = 0;

    for c in params_str.chars() {
        match c {
            '<' => {
                angle_depth += 1;
                current.push(c);
            }
            '>' => {
                angle_depth -= 1;
                current.push(c);
            }
            '(' => {
                paren_depth += 1;
                current.push(c);
            }
            ')' => {
                paren_depth -= 1;
                current.push(c);
            }
            ',' if angle_depth == 0 && paren_depth == 0 => {
                params.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        params.push(current);
    }

    params
        .iter()
        .filter_map(|p| {
            let p = p.trim();
            if p.is_empty() {
                return None;
            }

            // Skip self/&self/&mut self
            if p == "self" || p == "&self" || p == "&mut self" {
                return None;
            }

            // Rust: name: Type
            if let Some(colon_pos) = p.find(':') {
                let name = p[..colon_pos].trim();
                let ty = p[colon_pos + 1..].trim();
                Some(json!({ "name": name, "type": ty }))
            } else {
                // Python/JS: just the name
                Some(json!({ "name": p, "type": null }))
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tool 11: get_enclosing_class
// ---------------------------------------------------------------------------

/// Get the parent class/struct and all sibling methods for a given method.
pub fn get_enclosing_class(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let method_name = required_str(args, "method")?;

    let path = Path::new(file);
    let symbols = backend.get_symbols(path)?;

    if let Some((parent, methods)) = find_parent_with_method(&symbols, method_name) {
        let methods_json: Vec<Value> = methods
            .iter()
            .map(|m| {
                json!({
                    "name": m.name,
                    "signature": m.signature,
                })
            })
            .collect();

        let result = json!({
            "parent": parent.name,
            "kind": format!("{:?}", parent.kind),
            "methods": methods_json,
        });
        let text = serde_json::to_string_pretty(&result)?;
        Ok(tool_response(&text))
    } else {
        Ok(tool_response(&format!(
            "No enclosing class/struct found for method '{method_name}'"
        )))
    }
}

fn find_parent_with_method<'a>(
    symbols: &'a [Symbol],
    method_name: &str,
) -> Option<(&'a Symbol, Vec<&'a Symbol>)> {
    for sym in symbols {
        // Check if this symbol has children containing the method
        let has_method = sym.children.iter().any(|c| c.name == method_name);
        if has_method {
            let methods: Vec<&Symbol> = sym.children.iter().collect();
            return Some((sym, methods));
        }

        // Recurse
        if let Some(result) = find_parent_with_method(&sym.children, method_name) {
            return Some(result);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tool 12: get_symbol_body
// ---------------------------------------------------------------------------

/// Get the source code body of a specific symbol by name and optional line.
pub fn get_symbol_body(backend: &dyn CodeIntelligence, args: &Value) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;
    let line_hint = args.get("line").and_then(|v| v.as_u64()).map(|v| v as u32);

    let path = Path::new(file);

    // Find all matching symbols
    let symbols = backend.get_symbols(path)?;
    let mut matches = Vec::new();
    collect_symbols_by_name(&symbols, symbol_name, &mut matches);

    let sym = if matches.is_empty() {
        // Try get_definition as fallback
        match backend.get_definition(path, symbol_name)? {
            Some(s) => s,
            None => {
                return Ok(tool_response(&format!(
                    "Symbol '{symbol_name}' not found in {file}"
                )));
            }
        }
    } else if matches.len() == 1 {
        matches.into_iter().next().unwrap()
    } else if let Some(hint) = line_hint {
        // Disambiguate: find the closest match to the hint line
        matches
            .into_iter()
            .min_by_key(|s| {
                let start = s.location.line_start;
                hint.abs_diff(start)
            })
            .unwrap()
    } else {
        // Return the first match
        matches.into_iter().next().unwrap()
    };

    let source = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = source.lines().collect();
    let start = sym.location.line_start as usize;
    let end = (sym.location.line_end as usize).min(lines.len().saturating_sub(1));

    let mut body = String::new();
    if start < lines.len() {
        for line in &lines[start..=end] {
            body.push_str(line);
            body.push('\n');
        }
    }

    let result = json!({
        "name": sym.name,
        "kind": format!("{:?}", sym.kind),
        "body": body,
    });
    let text = serde_json::to_string_pretty(&result)?;
    Ok(tool_response(&text))
}

fn collect_symbols_by_name(symbols: &[Symbol], name: &str, out: &mut Vec<Symbol>) {
    for sym in symbols {
        if sym.name == name {
            out.push(sym.clone());
        }
        collect_symbols_by_name(&sym.children, name, out);
    }
}

// ---------------------------------------------------------------------------
// Tool 13: get_changed_files
// ---------------------------------------------------------------------------

/// List files with uncommitted changes and their modified symbol counts.
pub fn get_changed_files(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let ref1 = args.get("ref1").and_then(|v| v.as_str());
    let ref2 = args.get("ref2").and_then(|v| v.as_str());

    // Get list of changed files
    let mut name_cmd = Command::new("git");
    name_cmd.current_dir(project_root);
    match (ref1, ref2) {
        (Some(r1), Some(r2)) => {
            name_cmd.args(["diff", "--name-only", r1, r2]);
        }
        (Some(r1), None) => {
            name_cmd.args(["diff", "--name-only", r1]);
        }
        _ => {
            name_cmd.args(["diff", "--name-only", "HEAD"]);
        }
    }

    let name_output = name_cmd.output()?;
    let names_text = String::from_utf8_lossy(&name_output.stdout);

    // Get stat info
    let mut stat_cmd = Command::new("git");
    stat_cmd.current_dir(project_root);
    match (ref1, ref2) {
        (Some(r1), Some(r2)) => {
            stat_cmd.args(["diff", "--stat", r1, r2]);
        }
        (Some(r1), None) => {
            stat_cmd.args(["diff", "--stat", r1]);
        }
        _ => {
            stat_cmd.args(["diff", "--stat", "HEAD"]);
        }
    }
    let stat_output = stat_cmd.output()?;
    let stat_text = String::from_utf8_lossy(&stat_output.stdout);

    // Parse stat lines into a map of file -> lines_changed
    let stat_map = parse_stat_lines(&stat_text);

    let supported_exts = [
        "rs", "py", "js", "ts", "jsx", "tsx", "mjs", "go", "java", "c", "cpp", "h", "hpp",
    ];

    let mut results = Vec::new();

    for line in names_text.lines() {
        let file = line.trim();
        if file.is_empty() {
            continue;
        }

        let ext = Path::new(file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let lines_changed = stat_map
            .get(file)
            .cloned()
            .unwrap_or_else(|| "?".to_string());

        if supported_exts.contains(&ext) {
            let abs_path = if Path::new(file).is_absolute() {
                PathBuf::from(file)
            } else {
                project_root.join(file)
            };

            let symbol_count = if abs_path.exists() {
                backend
                    .get_symbols(&abs_path)
                    .map(|syms| count_all_symbols(&syms))
                    .unwrap_or(0)
            } else {
                0
            };

            results.push(json!({
                "file": file,
                "symbols": symbol_count,
                "lines_changed": lines_changed,
            }));
        } else {
            results.push(json!({
                "file": file,
                "symbols": 0,
                "lines_changed": lines_changed,
            }));
        }
    }

    if results.is_empty() {
        return Ok(tool_response("No changed files found"));
    }

    let text = serde_json::to_string_pretty(&results)?;
    Ok(tool_response(&text))
}

fn parse_stat_lines(stat: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in stat.lines() {
        // Format: " path/to/file.rs | 10 ++++----"
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        if parts.len() == 2 {
            let file = parts[0].trim().to_string();
            let stats = parts[1].trim();
            // Extract the +N/-M pattern
            let plus_count = stats.matches('+').count();
            let minus_count = stats.matches('-').count();
            map.insert(file, format!("+{plus_count}/-{minus_count}"));
        }
    }
    map
}

fn count_all_symbols(symbols: &[Symbol]) -> usize {
    let mut count = symbols.len();
    for sym in symbols {
        count += count_all_symbols(&sym.children);
    }
    count
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
