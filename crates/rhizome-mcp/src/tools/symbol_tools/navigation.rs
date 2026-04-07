use std::path::Path;

use anyhow::Result;
use rhizome_core::{CodeIntelligence, ParserlessBackend, Position, Symbol};
use serde_json::{Value, json};

use super::{ToolSchema, required_str, required_u32, resolve_project_path, tool_response};

pub fn go_to_definition(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let line = required_u32(args, "line")?;
    let column = required_u32(args, "column")?;

    let path = resolve_project_path(file, project_root)?;
    let pos = Position { line, column };

    // Find the identifier at the given position by looking at references
    let refs = backend.find_references(&path, &pos)?;
    if refs.is_empty() {
        return Ok(tool_response("No symbol found at the given position"));
    }

    // Read the source to get the symbol name at the position
    let source = std::fs::read_to_string(&path)?;
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

    let sym = backend.get_definition(&path, &name)?;
    match sym {
        Some(sym) => {
            let result = json!({
                "name": sym.name,
                "qualified_name": sym.qualified_name(),
                "stable_id": sym.stable_id(),
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

pub(crate) fn extract_identifier_at(line: &str, col: usize) -> String {
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

pub(crate) fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Get only the signature of a symbol.

pub fn get_scope(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let line = required_u32(args, "line")?;

    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

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

pub(crate) fn find_innermost_scope(symbols: &[Symbol], line: u32) -> Option<&Symbol> {
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

pub fn get_enclosing_class(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let method_name = required_str(args, "method")?;

    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

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

pub fn get_symbol_body(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;
    let line_hint = args.get("line").and_then(|v| v.as_u64()).map(|v| v as u32);

    let path = resolve_project_path(file, project_root)?;

    // Find all matching symbols
    let symbols = backend.get_symbols(&path)?;
    let mut matches = Vec::new();
    collect_symbols_by_name(&symbols, symbol_name, &mut matches);

    let sym = if matches.is_empty() {
        // Try get_definition as fallback
        match backend.get_definition(&path, symbol_name)? {
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

    let body = read_location_body(&path, &sym.location)?;

    let result = json!({
        "name": sym.name,
        "qualified_name": sym.qualified_name(),
        "stable_id": sym.stable_id(),
        "kind": format!("{:?}", sym.kind),
        "body": body,
    });
    let text = serde_json::to_string_pretty(&result)?;
    Ok(tool_response(&text))
}

pub fn get_region(
    backend: &dyn CodeIntelligence,
    parserless: &ParserlessBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let region_id = required_str(args, "region_id")?;
    let path = resolve_project_path(file, project_root)?;

    if region_id.starts_with("region-") {
        return match parserless.get_region_text(&path, region_id) {
            Ok(body) => {
                let result = json!({
                    "region_id": region_id,
                    "backend": "parserless",
                    "heuristic": true,
                    "body": body,
                });
                Ok(tool_response(&serde_json::to_string_pretty(&result)?))
            }
            Err(_) => Ok(tool_response(&format!(
                "Region '{region_id}' not found in {file}"
            ))),
        };
    }

    let symbols = backend.get_symbols(&path)?;
    let Some(sym) = find_symbol_by_stable_id(&symbols, region_id) else {
        return Ok(tool_response(&format!(
            "Region '{region_id}' not found in {file}"
        )));
    };

    let body = read_location_body(&path, &sym.location)?;
    let result = json!({
        "name": sym.name,
        "qualified_name": sym.qualified_name(),
        "stable_id": sym.stable_id(),
        "backend": "semantic",
        "body": body,
    });
    Ok(tool_response(&serde_json::to_string_pretty(&result)?))
}

fn collect_symbols_by_name(symbols: &[Symbol], name: &str, out: &mut Vec<Symbol>) {
    for sym in symbols {
        if sym.name == name {
            out.push(sym.clone());
        }
        collect_symbols_by_name(&sym.children, name, out);
    }
}

fn find_symbol_by_stable_id(symbols: &[Symbol], stable_id: &str) -> Option<Symbol> {
    for sym in symbols {
        if sym.stable_id() == stable_id {
            return Some(sym.clone());
        }
        if let Some(child) = find_symbol_by_stable_id(&sym.children, stable_id) {
            return Some(child);
        }
    }
    None
}

fn read_location_body(path: &Path, location: &rhizome_core::Location) -> Result<String> {
    let source = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = source.lines().collect();
    let start = location.line_start as usize;
    let end = (location.line_end as usize).min(lines.len().saturating_sub(1));

    let mut body = String::new();
    if start < lines.len() {
        for line in &lines[start..=end] {
            body.push_str(line);
            body.push('\n');
        }
    }
    Ok(body)
}
