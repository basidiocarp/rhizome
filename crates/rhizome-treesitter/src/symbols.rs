use anyhow::Result;
use rhizome_core::{Language, Location, Symbol, SymbolKind};
use streaming_iterator::StreamingIterator;

use crate::queries;

pub fn extract_symbols(
    tree: &tree_sitter::Tree,
    source: &[u8],
    file_path: &str,
    language: &Language,
) -> Result<Vec<Symbol>> {
    let query = queries::get_query(language)?;
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source);

    let capture_names = query.capture_names();

    let mut symbols = Vec::new();

    while let Some(m) = matches.next() {
        let mut name_text = String::new();
        let mut def_node: Option<tree_sitter::Node> = None;
        let mut capture_kind = "";

        for cap in m.captures {
            let cap_name = capture_names[cap.index as usize];
            if cap_name == "name" {
                name_text = cap.node.utf8_text(source).unwrap_or_default().to_string();
            } else {
                capture_kind = cap_name;
                def_node = Some(cap.node);
            }
        }

        let Some(node) = def_node else {
            continue;
        };

        // Import nodes don't have a @name capture
        if capture_kind == "import" && name_text.is_empty() {
            name_text = node.utf8_text(source).unwrap_or_default().to_string();
            // Trim to a reasonable length for display
            if name_text.len() > 200 {
                name_text.truncate(200);
            }
        }

        if name_text.is_empty() {
            continue;
        }

        let kind = match capture_kind {
            "function" => SymbolKind::Function,
            "struct_def" => SymbolKind::Struct,
            "enum_def" => SymbolKind::Enum,
            "trait_def" => SymbolKind::Trait,
            "impl_def" => SymbolKind::Struct, // impl blocks map to their struct
            "class_def" => SymbolKind::Class,
            "type_def" => SymbolKind::Type,
            "const_def" | "static_def" => SymbolKind::Constant,
            "variable" => SymbolKind::Variable,
            "import" => SymbolKind::Import,
            _ => SymbolKind::Variable,
        };

        let location = Location {
            file_path: file_path.to_string(),
            line_start: node.start_position().row as u32,
            line_end: node.end_position().row as u32,
            column_start: node.start_position().column as u32,
            column_end: node.end_position().column as u32,
        };

        let signature = extract_signature(node, source, language);
        let doc_comment = extract_doc_comment(node, source);

        let children = if capture_kind == "impl_def" {
            extract_impl_children(node, source, file_path)?
        } else {
            Vec::new()
        };

        symbols.push(Symbol {
            name: name_text,
            kind,
            location,
            signature,
            doc_comment,
            children,
        });
    }

    Ok(symbols)
}

fn extract_signature(
    node: tree_sitter::Node,
    source: &[u8],
    language: &Language,
) -> Option<String> {
    let text = node.utf8_text(source).ok()?;
    let delimiter = match language {
        Language::Python => ':',
        _ => '{',
    };

    let sig = if let Some(pos) = text.find(delimiter) {
        text[..pos].trim()
    } else {
        // Take the first line
        text.lines().next().unwrap_or(text).trim()
    };

    if sig.is_empty() {
        None
    } else {
        Some(sig.to_string())
    }
}

fn extract_doc_comment(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut comment_lines = Vec::new();
    let mut sibling = node.prev_named_sibling();

    while let Some(sib) = sibling {
        let kind = sib.kind();
        if kind == "line_comment" || kind == "comment" || kind == "block_comment" {
            if let Ok(text) = sib.utf8_text(source) {
                comment_lines.push(text.to_string());
            }
            sibling = sib.prev_named_sibling();
        } else if kind == "attribute_item" || kind == "decorator" {
            // Skip past attributes/decorators to find doc comments
            sibling = sib.prev_named_sibling();
        } else if kind == "string" || kind == "expression_statement" {
            // Python docstrings appear as first child, not sibling - skip
            break;
        } else {
            break;
        }
    }

    if comment_lines.is_empty() {
        // Check for Python docstrings (first child string in function/class body)
        if let Some(body) = node.child_by_field_name("body") {
            if let Some(first_child) = body.named_child(0) {
                if first_child.kind() == "expression_statement" {
                    if let Some(string_node) = first_child.named_child(0) {
                        if string_node.kind() == "string" {
                            if let Ok(text) = string_node.utf8_text(source) {
                                return Some(text.to_string());
                            }
                        }
                    }
                }
            }
        }
        return None;
    }

    comment_lines.reverse();
    let combined = comment_lines.join("\n");
    Some(combined)
}

fn extract_impl_children(
    node: tree_sitter::Node,
    source: &[u8],
    file_path: &str,
) -> Result<Vec<Symbol>> {
    let mut methods = Vec::new();

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "declaration_list" {
            let mut inner_cursor = child.walk();
            for item in child.named_children(&mut inner_cursor) {
                if item.kind() == "function_item" {
                    if let Some(name_node) = item.child_by_field_name("name") {
                        let name = name_node.utf8_text(source).unwrap_or_default().to_string();

                        let location = Location {
                            file_path: file_path.to_string(),
                            line_start: item.start_position().row as u32,
                            line_end: item.end_position().row as u32,
                            column_start: item.start_position().column as u32,
                            column_end: item.end_position().column as u32,
                        };

                        let doc_comment = extract_doc_comment(item, source);
                        let signature = extract_signature(item, source, &Language::Rust);

                        methods.push(Symbol {
                            name,
                            kind: SymbolKind::Method,
                            location,
                            signature,
                            doc_comment,
                            children: Vec::new(),
                        });
                    }
                }
            }
        }
    }

    Ok(methods)
}
