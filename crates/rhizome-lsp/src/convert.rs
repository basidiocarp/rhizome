use rhizome_core::{Diagnostic, DiagnosticSeverity, Location, Symbol, SymbolKind};

pub fn lsp_symbol_kind_to_symbol_kind(kind: lsp_types::SymbolKind) -> SymbolKind {
    if kind == lsp_types::SymbolKind::FUNCTION {
        SymbolKind::Function
    } else if kind == lsp_types::SymbolKind::METHOD || kind == lsp_types::SymbolKind::CONSTRUCTOR {
        SymbolKind::Method
    } else if kind == lsp_types::SymbolKind::CLASS {
        SymbolKind::Class
    } else if kind == lsp_types::SymbolKind::STRUCT {
        SymbolKind::Struct
    } else if kind == lsp_types::SymbolKind::ENUM {
        SymbolKind::Enum
    } else if kind == lsp_types::SymbolKind::INTERFACE {
        SymbolKind::Interface
    } else if kind == lsp_types::SymbolKind::CONSTANT {
        SymbolKind::Constant
    } else if kind == lsp_types::SymbolKind::VARIABLE {
        SymbolKind::Variable
    } else if kind == lsp_types::SymbolKind::MODULE
        || kind == lsp_types::SymbolKind::NAMESPACE
        || kind == lsp_types::SymbolKind::PACKAGE
    {
        SymbolKind::Module
    } else if kind == lsp_types::SymbolKind::PROPERTY {
        SymbolKind::Property
    } else if kind == lsp_types::SymbolKind::FIELD {
        SymbolKind::Field
    } else if kind == lsp_types::SymbolKind::TYPE_PARAMETER {
        SymbolKind::Type
    } else {
        SymbolKind::Variable
    }
}

/// Convert an LSP range + file path to a rhizome Location.
/// LSP uses 0-based lines/columns; rhizome uses 1-based.
pub fn lsp_range_to_location(range: &lsp_types::Range, file_path: &str) -> Location {
    Location {
        file_path: file_path.to_string(),
        line_start: range.start.line + 1,
        line_end: range.end.line + 1,
        column_start: range.start.character + 1,
        column_end: range.end.character + 1,
    }
}

pub fn lsp_symbol_to_symbol(sym: &lsp_types::DocumentSymbol, file_path: &str) -> Symbol {
    let children = sym
        .children
        .as_ref()
        .map(|c| {
            c.iter()
                .map(|child| lsp_symbol_to_symbol(child, file_path))
                .collect()
        })
        .unwrap_or_default();

    Symbol {
        name: sym.name.clone(),
        kind: lsp_symbol_kind_to_symbol_kind(sym.kind),
        location: lsp_range_to_location(&sym.range, file_path),
        signature: sym.detail.clone(),
        doc_comment: None,
        children,
    }
}

pub fn lsp_location_to_location(loc: &lsp_types::Location) -> Location {
    let file_path = uri_to_file_path(&loc.uri);
    lsp_range_to_location(&loc.range, &file_path)
}

pub fn lsp_symbol_info_to_symbol(sym: &lsp_types::SymbolInformation) -> Symbol {
    Symbol {
        name: sym.name.clone(),
        kind: lsp_symbol_kind_to_symbol_kind(sym.kind),
        location: lsp_location_to_location(&sym.location),
        signature: sym.container_name.clone(),
        doc_comment: None,
        children: vec![],
    }
}

pub fn lsp_diagnostic_to_diagnostic(diag: &lsp_types::Diagnostic, file_path: &str) -> Diagnostic {
    let severity = diag
        .severity
        .map(|s| {
            if s == lsp_types::DiagnosticSeverity::ERROR {
                DiagnosticSeverity::Error
            } else if s == lsp_types::DiagnosticSeverity::WARNING {
                DiagnosticSeverity::Warning
            } else if s == lsp_types::DiagnosticSeverity::INFORMATION {
                DiagnosticSeverity::Information
            } else {
                DiagnosticSeverity::Hint
            }
        })
        .unwrap_or(DiagnosticSeverity::Warning);

    Diagnostic {
        message: diag.message.clone(),
        severity,
        location: lsp_range_to_location(&diag.range, file_path),
    }
}

/// Convert an lsp_types::Uri to a file path string.
pub fn uri_to_file_path(uri: &lsp_types::Uri) -> String {
    let s = uri.as_str();
    if let Ok(url) = url::Url::parse(s) {
        if let Ok(path) = url.to_file_path() {
            return path.to_string_lossy().to_string();
        }
    }
    s.to_string()
}

/// Convert a filesystem path to an lsp_types::Uri.
pub fn path_to_lsp_uri(path: &std::path::Path) -> anyhow::Result<lsp_types::Uri> {
    let url = url::Url::from_file_path(path)
        .map_err(|_| anyhow::anyhow!("Invalid file path: {}", path.display()))?;
    url.as_str()
        .parse::<lsp_types::Uri>()
        .map_err(|e| anyhow::anyhow!("Failed to parse URI: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_mapping() {
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::FUNCTION),
            SymbolKind::Function
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::METHOD),
            SymbolKind::Method
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::CONSTRUCTOR),
            SymbolKind::Method
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::CLASS),
            SymbolKind::Class
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::STRUCT),
            SymbolKind::Struct
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::ENUM),
            SymbolKind::Enum
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::INTERFACE),
            SymbolKind::Interface
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::CONSTANT),
            SymbolKind::Constant
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::VARIABLE),
            SymbolKind::Variable
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::MODULE),
            SymbolKind::Module
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::NAMESPACE),
            SymbolKind::Module
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::PACKAGE),
            SymbolKind::Module
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::PROPERTY),
            SymbolKind::Property
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::FIELD),
            SymbolKind::Field
        );
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::TYPE_PARAMETER),
            SymbolKind::Type
        );
        // Fallback
        assert_eq!(
            lsp_symbol_kind_to_symbol_kind(lsp_types::SymbolKind::STRING),
            SymbolKind::Variable
        );
    }

    #[test]
    fn test_lsp_range_to_location() {
        let range = lsp_types::Range {
            start: lsp_types::Position {
                line: 0,
                character: 0,
            },
            end: lsp_types::Position {
                line: 5,
                character: 10,
            },
        };
        let loc = lsp_range_to_location(&range, "src/main.rs");
        assert_eq!(loc.file_path, "src/main.rs");
        assert_eq!(loc.line_start, 1);
        assert_eq!(loc.line_end, 6);
        assert_eq!(loc.column_start, 1);
        assert_eq!(loc.column_end, 11);
    }

    #[test]
    fn test_lsp_location_to_location() {
        let uri: lsp_types::Uri = "file:///home/user/project/src/main.rs".parse().unwrap();
        let loc = lsp_types::Location {
            uri,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 10,
                    character: 20,
                },
            },
        };
        let result = lsp_location_to_location(&loc);
        assert_eq!(result.file_path, "/home/user/project/src/main.rs");
        assert_eq!(result.line_start, 11);
        assert_eq!(result.line_end, 11);
        assert_eq!(result.column_start, 5);
        assert_eq!(result.column_end, 21);
    }

    #[allow(
        deprecated,
        reason = "lsp_types::DocumentSymbol construction; test compatibility"
    )]
    #[test]
    fn test_lsp_symbol_to_symbol_with_children() {
        let child = lsp_types::DocumentSymbol {
            name: "field_a".to_string(),
            detail: Some("u32".to_string()),
            kind: lsp_types::SymbolKind::FIELD,
            tags: None,
            deprecated: None,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 2,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 2,
                    character: 20,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 2,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 2,
                    character: 11,
                },
            },
            children: None,
        };

        let parent = lsp_types::DocumentSymbol {
            name: "MyStruct".to_string(),
            detail: None,
            kind: lsp_types::SymbolKind::STRUCT,
            tags: None,
            deprecated: None,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 5,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 11,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 19,
                },
            },
            children: Some(vec![child]),
        };

        let symbol = lsp_symbol_to_symbol(&parent, "src/lib.rs");
        assert_eq!(symbol.name, "MyStruct");
        assert_eq!(symbol.kind, SymbolKind::Struct);
        assert_eq!(symbol.location.line_start, 1);
        assert_eq!(symbol.location.line_end, 6);
        assert!(symbol.signature.is_none());
        assert_eq!(symbol.children.len(), 1);

        let child_sym = &symbol.children[0];
        assert_eq!(child_sym.name, "field_a");
        assert_eq!(child_sym.kind, SymbolKind::Field);
        assert_eq!(child_sym.signature, Some("u32".to_string()));
        assert!(child_sym.children.is_empty());
    }

    #[test]
    fn test_lsp_diagnostic_to_diagnostic() {
        let diag = lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 3,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 3,
                    character: 15,
                },
            },
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            message: "unused variable".to_string(),
            ..Default::default()
        };

        let result = lsp_diagnostic_to_diagnostic(&diag, "src/main.rs");
        assert_eq!(result.message, "unused variable");
        assert_eq!(result.severity, DiagnosticSeverity::Error);
        assert_eq!(result.location.line_start, 4);
        assert_eq!(result.location.file_path, "src/main.rs");
    }

    #[test]
    fn test_lsp_diagnostic_severity_mapping() {
        let make_diag = |sev: Option<lsp_types::DiagnosticSeverity>| lsp_types::Diagnostic {
            range: lsp_types::Range::default(),
            severity: sev,
            message: "test".to_string(),
            ..Default::default()
        };

        assert_eq!(
            lsp_diagnostic_to_diagnostic(
                &make_diag(Some(lsp_types::DiagnosticSeverity::ERROR)),
                ""
            )
            .severity,
            DiagnosticSeverity::Error
        );
        assert_eq!(
            lsp_diagnostic_to_diagnostic(
                &make_diag(Some(lsp_types::DiagnosticSeverity::WARNING)),
                ""
            )
            .severity,
            DiagnosticSeverity::Warning
        );
        assert_eq!(
            lsp_diagnostic_to_diagnostic(
                &make_diag(Some(lsp_types::DiagnosticSeverity::INFORMATION)),
                ""
            )
            .severity,
            DiagnosticSeverity::Information
        );
        assert_eq!(
            lsp_diagnostic_to_diagnostic(&make_diag(Some(lsp_types::DiagnosticSeverity::HINT)), "")
                .severity,
            DiagnosticSeverity::Hint
        );
        // None severity defaults to Warning
        assert_eq!(
            lsp_diagnostic_to_diagnostic(&make_diag(None), "").severity,
            DiagnosticSeverity::Warning
        );
    }

    #[test]
    fn test_symbol_info_to_symbol() {
        let uri: lsp_types::Uri = "file:///project/src/lib.rs".parse().unwrap();
        #[allow(
            deprecated,
            reason = "lsp_types::SymbolInformation construction; test compatibility"
        )]
        let info = lsp_types::SymbolInformation {
            name: "my_function".to_string(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            location: lsp_types::Location {
                uri,
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 10,
                        character: 0,
                    },
                    end: lsp_types::Position {
                        line: 20,
                        character: 1,
                    },
                },
            },
            container_name: Some("my_module".to_string()),
        };

        let sym = lsp_symbol_info_to_symbol(&info);
        assert_eq!(sym.name, "my_function");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert_eq!(sym.signature, Some("my_module".to_string()));
        assert!(sym.children.is_empty());
    }
}
