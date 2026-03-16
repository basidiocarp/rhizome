pub mod parser;
pub mod queries;
pub mod symbols;

use std::path::Path;

use anyhow::{anyhow, Result};
use ignore::WalkBuilder;
use rhizome_core::{
    BackendCapabilities, CodeIntelligence, Diagnostic, Language, Location, Position, Symbol,
    SymbolKind,
};

use crate::parser::ParserPool;
use crate::symbols::extract_symbols;

pub struct TreeSitterBackend {
    parser_pool: ParserPool,
}

impl TreeSitterBackend {
    pub fn new() -> Self {
        Self {
            parser_pool: ParserPool::new(),
        }
    }

    fn detect_language(file: &Path) -> Result<Language> {
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow!("No file extension: {}", file.display()))?;

        Language::from_extension(ext).ok_or_else(|| anyhow!("Unsupported extension: {}", ext))
    }

    fn parse_file(&mut self, file: &Path) -> Result<(tree_sitter::Tree, Vec<u8>, Language)> {
        let language = Self::detect_language(file)?;
        let source = std::fs::read(file)?;
        let parser = self.parser_pool.get_parser(&language)?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| anyhow!("Failed to parse: {}", file.display()))?;
        Ok((tree, source, language))
    }
}

impl Default for TreeSitterBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeIntelligence for TreeSitterBackend {
    fn get_symbols(&self, file: &Path) -> Result<Vec<Symbol>> {
        // We need a mutable reference for the parser pool, so use interior
        // approach via a fresh pool per call for trait compatibility.
        let mut backend = TreeSitterBackend::new();
        let (tree, source, language) = backend.parse_file(file)?;
        let file_path = file.to_string_lossy().to_string();
        extract_symbols(&tree, &source, &file_path, &language)
    }

    fn get_definition(&self, file: &Path, name: &str) -> Result<Option<Symbol>> {
        let symbols = self.get_symbols(file)?;
        Ok(find_symbol_by_name(&symbols, name))
    }

    fn find_references(&self, file: &Path, position: &Position) -> Result<Vec<Location>> {
        let mut backend = TreeSitterBackend::new();
        let (tree, source, _language) = backend.parse_file(file)?;
        let file_path = file.to_string_lossy().to_string();

        // Find the identifier at the given position
        let point = tree_sitter::Point::new(position.line as usize, position.column as usize);
        let target_node = tree
            .root_node()
            .descendant_for_point_range(point, point)
            .ok_or_else(|| anyhow!("No node at position {}:{}", position.line, position.column))?;

        let target_name = target_node.utf8_text(&source)?.to_string();
        if target_name.is_empty() {
            return Ok(Vec::new());
        }

        // Find all matching identifiers in the file
        let mut locations = Vec::new();
        collect_references(
            tree.root_node(),
            &source,
            &target_name,
            &file_path,
            &mut locations,
        );

        Ok(locations)
    }

    fn search_symbols(&self, pattern: &str, project_root: &Path) -> Result<Vec<Symbol>> {
        let pattern_lower = pattern.to_lowercase();
        let mut all_symbols = Vec::new();

        let walker = WalkBuilder::new(project_root)
            .hidden(true)
            .git_ignore(true)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if Self::detect_language(path).is_err() {
                continue;
            }

            match self.get_symbols(path) {
                Ok(syms) => {
                    for sym in syms {
                        collect_matching_symbols(&sym, &pattern_lower, &mut all_symbols);
                    }
                }
                Err(_) => continue,
            }
        }

        Ok(all_symbols)
    }

    fn get_imports(&self, file: &Path) -> Result<Vec<Symbol>> {
        let symbols = self.get_symbols(file)?;
        Ok(symbols
            .into_iter()
            .filter(|s| s.kind == SymbolKind::Import)
            .collect())
    }

    fn get_diagnostics(&self, _file: &Path) -> Result<Vec<Diagnostic>> {
        // Tree-sitter is a parser, not a type checker — no diagnostics
        Ok(Vec::new())
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

fn find_symbol_by_name(symbols: &[Symbol], name: &str) -> Option<Symbol> {
    for sym in symbols {
        if sym.name == name {
            return Some(sym.clone());
        }
        if let Some(child) = find_symbol_by_name(&sym.children, name) {
            return Some(child);
        }
    }
    None
}

fn collect_matching_symbols(symbol: &Symbol, pattern_lower: &str, results: &mut Vec<Symbol>) {
    if symbol.name.to_lowercase().contains(pattern_lower) {
        results.push(symbol.clone());
    }
    for child in &symbol.children {
        collect_matching_symbols(child, pattern_lower, results);
    }
}

fn collect_references(
    node: tree_sitter::Node,
    source: &[u8],
    target_name: &str,
    file_path: &str,
    locations: &mut Vec<Location>,
) {
    if node.kind() == "identifier" || node.kind() == "type_identifier" {
        if let Ok(text) = node.utf8_text(source) {
            if text == target_name {
                locations.push(Location {
                    file_path: file_path.to_string(),
                    line_start: node.start_position().row as u32,
                    line_end: node.end_position().row as u32,
                    column_start: node.start_position().column as u32,
                    column_end: node.end_position().column as u32,
                });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_references(child, source, target_name, file_path, locations);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn test_parse_rust_symbols() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");
        let symbols = backend.get_symbols(&path).expect("Failed to get symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"Config"),
            "Should find Config struct: {names:?}"
        );
        assert!(
            names.contains(&"process"),
            "Should find process function: {names:?}"
        );
        assert!(
            names.contains(&"MAX_SIZE"),
            "Should find MAX_SIZE constant: {names:?}"
        );
        assert!(
            names.contains(&"Status"),
            "Should find Status enum: {names:?}"
        );
        assert!(
            names.contains(&"Processor"),
            "Should find Processor trait: {names:?}"
        );
    }

    #[test]
    fn test_rust_symbol_kinds() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");
        let symbols = backend.get_symbols(&path).expect("Failed to get symbols");

        let struct_sym = symbols
            .iter()
            .find(|s| s.name == "Config" && s.kind == SymbolKind::Struct);
        assert!(struct_sym.is_some(), "Config should be a Struct");

        let fn_sym = symbols
            .iter()
            .find(|s| s.name == "process" && s.kind == SymbolKind::Function);
        assert!(fn_sym.is_some(), "process should be a Function");

        let enum_sym = symbols
            .iter()
            .find(|s| s.name == "Status" && s.kind == SymbolKind::Enum);
        assert!(enum_sym.is_some(), "Status should be an Enum");

        let trait_sym = symbols
            .iter()
            .find(|s| s.name == "Processor" && s.kind == SymbolKind::Trait);
        assert!(trait_sym.is_some(), "Processor should be a Trait");

        let const_sym = symbols
            .iter()
            .find(|s| s.name == "MAX_SIZE" && s.kind == SymbolKind::Constant);
        assert!(const_sym.is_some(), "MAX_SIZE should be a Constant");
    }

    #[test]
    fn test_rust_impl_methods() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");
        let symbols = backend.get_symbols(&path).expect("Failed to get symbols");

        // Find the impl block for Config
        let impl_sym = symbols
            .iter()
            .find(|s| s.name == "Config" && !s.children.is_empty());
        assert!(impl_sym.is_some(), "Should find impl Config with children");

        let impl_sym = impl_sym.unwrap();
        let method_names: Vec<&str> = impl_sym.children.iter().map(|c| c.name.as_str()).collect();
        assert!(
            method_names.contains(&"new"),
            "Should find new method: {method_names:?}"
        );
        assert!(
            method_names.contains(&"value"),
            "Should find value method: {method_names:?}"
        );

        for child in &impl_sym.children {
            assert_eq!(
                child.kind,
                SymbolKind::Method,
                "{} should be a Method",
                child.name
            );
        }
    }

    #[test]
    fn test_rust_imports() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");
        let imports = backend.get_imports(&path).expect("Failed to get imports");

        assert!(
            imports.len() >= 2,
            "Should find at least 2 imports, found {}",
            imports.len()
        );
        assert!(imports.iter().all(|s| s.kind == SymbolKind::Import));
    }

    #[test]
    fn test_get_definition() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");

        let config = backend.get_definition(&path, "Config").expect("Failed");
        assert!(config.is_some(), "Should find Config");
        let config = config.unwrap();
        assert_eq!(config.name, "Config");

        let missing = backend
            .get_definition(&path, "NonExistent")
            .expect("Failed");
        assert!(missing.is_none(), "Should not find NonExistent");
    }

    #[test]
    fn test_find_references() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");

        // "Config" type_identifier is on line 3 (0-indexed: row 2), at "pub struct Config"
        // column 11 points to the 'C' in Config
        let pos = Position {
            line: 2,
            column: 11,
        };

        let refs = backend.find_references(&path, &pos).expect("Failed");
        // Config appears multiple times: struct def, impl, fn param, fn body
        assert!(
            refs.len() >= 2,
            "Should find at least 2 references to Config, found {}",
            refs.len()
        );
    }

    #[test]
    fn test_parse_python_symbols() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.py");
        let symbols = backend.get_symbols(&path).expect("Failed to get symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"Config"),
            "Should find Config class: {names:?}"
        );
        assert!(
            names.contains(&"process"),
            "Should find process function: {names:?}"
        );
        assert!(
            names.contains(&"Status"),
            "Should find Status class: {names:?}"
        );
    }

    #[test]
    fn test_python_symbol_kinds() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.py");
        let symbols = backend.get_symbols(&path).expect("Failed to get symbols");

        let class_sym = symbols
            .iter()
            .find(|s| s.name == "Config" && s.kind == SymbolKind::Class);
        assert!(class_sym.is_some(), "Config should be a Class");

        let fn_sym = symbols
            .iter()
            .find(|s| s.name == "process" && s.kind == SymbolKind::Function);
        assert!(fn_sym.is_some(), "process should be a Function");
    }

    #[test]
    fn test_python_imports() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.py");
        let imports = backend.get_imports(&path).expect("Failed to get imports");

        assert!(
            imports.len() >= 2,
            "Should find at least 2 imports, found {}",
            imports.len()
        );
    }

    #[test]
    fn test_doc_comments() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");
        let symbols = backend.get_symbols(&path).expect("Failed");

        let config = symbols
            .iter()
            .find(|s| s.name == "Config" && s.kind == SymbolKind::Struct);
        assert!(config.is_some());
        let config = config.unwrap();
        assert!(
            config.doc_comment.is_some(),
            "Config should have a doc comment"
        );
        assert!(
            config
                .doc_comment
                .as_ref()
                .unwrap()
                .contains("sample struct"),
            "Doc comment should contain 'sample struct'"
        );
    }

    #[test]
    fn test_signatures() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");
        let symbols = backend.get_symbols(&path).expect("Failed");

        let process = symbols
            .iter()
            .find(|s| s.name == "process" && s.kind == SymbolKind::Function);
        assert!(process.is_some());
        let process = process.unwrap();
        assert!(
            process.signature.is_some(),
            "process should have a signature"
        );
        let sig = process.signature.as_ref().unwrap();
        assert!(
            sig.contains("fn process"),
            "Signature should contain 'fn process': {sig}"
        );
    }

    #[test]
    fn test_capabilities() {
        let backend = TreeSitterBackend::new();
        let caps = backend.capabilities();
        assert!(!caps.cross_file_references);
        assert!(!caps.rename);
        assert!(!caps.type_info);
        assert!(!caps.diagnostics);
    }

    #[test]
    fn test_parse_large_file_under_5ms() {
        let path = fixture_path("large_sample.rs");
        let source = std::fs::read(&path).expect("Failed to read fixture");
        let language = Language::Rust;

        // Benchmark just the parse + extract step (excluding parser allocation)
        let mut pool = crate::parser::ParserPool::new();
        let parser = pool.get_parser(&language).expect("Failed to get parser");

        // Warm up
        let _ = parser.parse(&source, None);

        let iterations = 20;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let tree = parser.parse(&source, None).expect("Parse failed");
            let file_path = path.to_string_lossy().to_string();
            let symbols = crate::symbols::extract_symbols(&tree, &source, &file_path, &language)
                .expect("Extract failed");
            assert!(!symbols.is_empty());
        }
        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;

        // 5ms in release, 20ms tolerance in debug mode
        let threshold = if cfg!(debug_assertions) { 20.0 } else { 5.0 };
        assert!(
            avg_ms < threshold,
            "Parsing ~1000-line Rust file should take <{threshold}ms, took {avg_ms:.2}ms average",
        );
    }

    #[test]
    fn test_large_file_symbol_count() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("large_sample.rs");
        let symbols = backend.get_symbols(&path).expect("Failed");

        // The large fixture has ~20 structs, ~10 enums, ~10 traits, ~15 functions, etc.
        assert!(
            symbols.len() >= 30,
            "Large file should have at least 30 top-level symbols, found {}",
            symbols.len()
        );
    }

    #[test]
    fn test_diagnostics_empty() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rs");
        let diags = backend.get_diagnostics(&path).expect("Failed");
        assert!(
            diags.is_empty(),
            "Tree-sitter should not produce diagnostics"
        );
    }
}
