use std::fmt::Write;
use std::path::Path;

use anyhow::Result;
use rhizome_core::{CodeIntelligence, Symbol, SymbolKind};
use serde_json::{Value, json};

use super::navigation::is_ident_char;
use super::{ToolSchema, required_str, resolve_project_path, tool_response};

pub fn get_call_sites(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let function_filter = args.get("function").and_then(|v| v.as_str());

    let path = resolve_project_path(file, project_root)?;
    let source = std::fs::read_to_string(&path)?;
    let lines: Vec<&str> = source.lines().collect();

    // Use the backend to get symbols which validates the file is parseable
    let _symbols = backend.get_symbols(&path)?;

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

pub fn get_annotations(
    _backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
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

    let path = resolve_project_path(file, project_root)?;
    let source = std::fs::read_to_string(&path)?;
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

pub fn get_tests(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Read source for attribute checking
    let source = std::fs::read_to_string(&path)?;
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

pub fn get_type_definitions(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

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

pub fn summarize_file(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

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
