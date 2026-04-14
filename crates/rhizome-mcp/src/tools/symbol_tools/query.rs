#![allow(clippy::collapsible_if, clippy::empty_line_after_doc_comments)]

use std::fmt::Write;
use std::path::Path;

use anyhow::Result;
use rhizome_core::{
    CodeIntelligence, HeuristicBackend, HeuristicRegion, ParserlessBackend, ParserlessRegion,
    Symbol,
};
use serde_json::{Value, json};

use super::{required_str, resolve_project_path, tool_response};

pub fn get_symbols(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

    let formatted: Vec<Value> = symbols.iter().flat_map(flatten_symbol).collect();
    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

pub fn get_parserless_symbols(
    backend: &ParserlessBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let regions = backend.outline(&path)?;
    let text = serde_json::to_string_pretty(&parserless_region_values(&regions))?;
    Ok(tool_response(&text))
}

fn flatten_symbol(sym: &Symbol) -> Vec<Value> {
    let mut results = vec![json!({
        "name": sym.name,
        "qualified_name": sym.qualified_name(),
        "stable_id": sym.stable_id(),
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

pub fn get_structure(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let max_depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

    let mut output = String::new();
    for sym in &symbols {
        format_tree(sym, 0, max_depth, &mut output);
    }

    Ok(tool_response(output.trim_end()))
}

pub fn get_parserless_structure(
    backend: &ParserlessBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let regions = backend.outline(&path)?;
    let response = json!({
        "backend": "parserless",
        "heuristic": true,
        "regions": parserless_region_values(&regions),
    });
    Ok(tool_response(&serde_json::to_string_pretty(&response)?))
}

pub fn get_heuristic_symbols(
    backend: &HeuristicBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let regions = backend.outline(&path)?;
    let text = serde_json::to_string_pretty(&heuristic_region_values(&regions))?;
    Ok(tool_response(&text))
}

pub fn get_heuristic_structure(
    backend: &HeuristicBackend,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let regions = backend.outline(&path)?;
    let response = json!({
        "backend": "heuristic",
        "heuristic": true,
        "regions": heuristic_region_values(&regions),
    });
    Ok(tool_response(&serde_json::to_string_pretty(&response)?))
}

fn heuristic_region_values(regions: &[HeuristicRegion]) -> Vec<Value> {
    regions
        .iter()
        .map(|region| {
            json!({
                "region_id": region.region_id,
                "line": region.line,
                "line_end": region.line_end,
                "depth": region.depth,
                "label": region.label,
                "backend": "heuristic",
                "heuristic": true,
            })
        })
        .collect()
}

fn parserless_region_values(regions: &[ParserlessRegion]) -> Vec<Value> {
    regions
        .iter()
        .map(|region| {
            json!({
                "region_id": region.region_id,
                "line": region.line,
                "line_end": region.line_end,
                "depth": region.depth,
                "label": region.label,
                "backend": "parserless",
                "heuristic": true,
            })
        })
        .collect()
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

pub fn get_definition(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;
    let full = args.get("full").and_then(|v| v.as_bool()).unwrap_or(false);

    let path = resolve_project_path(file, project_root)?;
    let sym = backend.get_definition(&path, symbol_name)?;

    match sym {
        Some(sym) => {
            let body = read_symbol_body(&path, &sym, full)?;
            let result = json!({
                "name": sym.name,
                "qualified_name": sym.qualified_name(),
                "stable_id": sym.stable_id(),
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
    project_root: &Path,
) -> Result<Value> {
    let pattern = required_str(args, "pattern")?;
    let search_path = args
        .get("path")
        .and_then(|v| v.as_str())
        .map(|path| resolve_project_path(path, project_root))
        .transpose()?
        .unwrap_or_else(|| project_root.to_path_buf());

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

pub fn get_signature(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let symbol_name = required_str(args, "symbol")?;

    let path = resolve_project_path(file, project_root)?;
    let sym = backend.get_definition(&path, symbol_name)?;

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

pub fn get_imports(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let imports = backend.get_imports(&path)?;

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

/// List only public/exported symbols in a file.
pub fn get_exports(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

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
        _ => true,
    }
}
