#![allow(
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    unused_imports
)]

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
            let _paren_pos = i;
            let mut name_end = i;

            // Skip any whitespace between name and paren
            while name_end > 0 && bytes[name_end - 1] == b' ' {
                name_end -= 1;
            }

            if name_end > 0 {
                let ch = line[..name_end].chars().last().unwrap_or(' ');
                if is_ident_char(ch) {
                    // Collect (byte_offset, char) pairs so we can walk backward
                    // without ever landing mid-codepoint.
                    let char_positions: Vec<(usize, char)> = line.char_indices().collect();

                    // Find the index in char_positions of the last char whose byte
                    // offset is strictly less than name_end (i.e., the char just
                    // before the opening paren or any whitespace we skipped).
                    let name_end_char_idx = char_positions
                        .iter()
                        .rposition(|(b, _)| *b < name_end)
                        .map(|i| i + 1)
                        .unwrap_or(0);

                    // Walk backward over chars to find the start of the identifier.
                    let mut name_start_char_idx = name_end_char_idx;
                    while name_start_char_idx > 0 {
                        let (_, prev_ch) = char_positions[name_start_char_idx - 1];
                        if !is_ident_char(prev_ch) {
                            break;
                        }
                        name_start_char_idx -= 1;
                    }

                    let name_start = char_positions
                        .get(name_start_char_idx)
                        .map(|(b, _)| *b)
                        .unwrap_or(0);

                    let name = &line[name_start..name_end];

                    // Skip language keywords
                    if !is_keyword(name) {
                        // Always return the byte offset of the first character of
                        // the function name (name_start), regardless of whether a
                        // filter is active.  Using paren_pos in the filtered branch
                        // was inconsistent and displaced editor cursors by the length
                        // of the function name.
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
                                "column": name_start as u32,
                                "context": line.trim(),
                            }));
                        }
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

/// Case-insensitive substring search that returns a byte offset into `line`
/// that is always a valid char boundary.
///
/// Unlike searching in `line.to_uppercase()`, this function never creates a
/// string whose byte length differs from `line` due to Unicode case-folding
/// (e.g., ß→SS, ı→I).  It slides a char-at-a-time window over the original
/// string and compares uppercased prefixes.
fn find_tag_in_line_ci(line: &str, tag: &str) -> Option<usize> {
    let tag_char_count = tag.chars().count();
    let tag_upper = tag.to_uppercase();
    let mut char_indices = line.char_indices().peekable();
    while let Some((byte_pos, _)) = char_indices.peek().copied() {
        let remaining = &line[byte_pos..];
        let prefix: String = remaining.chars().take(tag_char_count).collect();
        if prefix.to_uppercase() == tag_upper {
            return Some(byte_pos);
        }
        char_indices.next();
    }
    None
}

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
        for tag in &tags {
            // Case-insensitive search that stays within the original string's
            // byte coordinates, avoiding the uppercase copy entirely.
            // `to_uppercase` is not length-preserving for all Unicode characters
            // (e.g., ß→SS, ı→I), so using find() on an uppercased copy produces
            // byte offsets that are not valid boundaries in the original string.
            if let Some(tag_byte_pos) = find_tag_in_line_ci(line, tag) {
                let after_tag_pos = tag_byte_pos + tag.len();
                let after_tag = &line[after_tag_pos..];
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
