pub mod parser;
pub mod queries;
pub mod symbols;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::{anyhow, Result};
use ignore::WalkBuilder;
use lru::LruCache;
use rhizome_core::{
    BackendCapabilities, CodeIntelligence, Diagnostic, Language, Location, Position, Symbol,
    SymbolKind,
};

use crate::parser::ParserPool;
use crate::symbols::extract_symbols;

/// ─────────────────────────────────────────────────────────────────────────
/// SharedParseCache
/// ─────────────────────────────────────────────────────────────────────────
/// Thread-safe LRU cache for parsed trees, shared across backend instances.
/// Key: (canonicalized path, file mtime). Capacity: 100 entries.
type ParseCache = Mutex<LruCache<(PathBuf, SystemTime), (tree_sitter::Tree, Vec<u8>, Language)>>;

fn shared_cache() -> &'static ParseCache {
    // Lazy-initialized static cache - created once per process lifetime
    static CACHE: std::sync::OnceLock<ParseCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(std::num::NonZeroUsize::new(100).unwrap()))
    })
}

/// ─────────────────────────────────────────────────────────────────────────
/// TreeSitterBackend
/// ─────────────────────────────────────────────────────────────────────────
/// Code intelligence backend using tree-sitter parsing with LRU cache.
/// Cache key: (canonicalized path, file mtime) to invalidate on changes.
pub struct TreeSitterBackend {
    parser_pool: ParserPool,
}

impl TreeSitterBackend {
    /// Create a new backend. Uses a shared, process-wide LRU parse tree cache.
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

        // Try to get canonical path and file mtime for cache lookup
        let canonical_path = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
        let metadata = std::fs::metadata(file)?;
        let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        let cache_key = (canonical_path.clone(), mtime);

        // ─────────────────────────────────────────────────────────────────
        // Check shared cache for hit
        // ─────────────────────────────────────────────────────────────────
        {
            let mut cache = shared_cache().lock().unwrap();
            if let Some((tree, source, lang)) = cache.get(&cache_key) {
                return Ok((tree.clone(), source.clone(), lang.clone()));
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // Cache miss: parse and insert into shared cache
        // ─────────────────────────────────────────────────────────────────
        let source = std::fs::read(file)?;
        let parser = self.parser_pool.get_parser(&language)?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| anyhow!("Failed to parse: {}", file.display()))?;

        {
            let mut cache = shared_cache().lock().unwrap();
            cache.put(cache_key, (tree.clone(), source.clone(), language.clone()));
        }

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
        // Use shared cache via parse_file. Create a new pool (lightweight) for
        // this call, but the parse tree results come from the process-wide cache.
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

        // 5ms in release, 50ms tolerance in debug mode (CI shared runners are slow)
        let threshold = if cfg!(debug_assertions) { 50.0 } else { 5.0 };
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

    // ─────────────────────────────────────────────────────────────────────────
    // Java
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_java_symbols() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.java");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get Java symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"UserService"),
            "Should find UserService class: {names:?}"
        );
        assert!(
            names.contains(&"Repository"),
            "Should find Repository interface: {names:?}"
        );
        assert!(
            names.contains(&"Status"),
            "Should find Status enum: {names:?}"
        );
    }

    #[test]
    fn test_java_symbol_kinds() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.java");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get Java symbols");

        let class_sym = symbols
            .iter()
            .find(|s| s.name == "UserService" && s.kind == SymbolKind::Class);
        assert!(class_sym.is_some(), "UserService should be a Class");

        let iface_sym = symbols
            .iter()
            .find(|s| s.name == "Repository" && s.kind == SymbolKind::Trait);
        assert!(
            iface_sym.is_some(),
            "Repository should be a Trait (interface)"
        );

        let enum_sym = symbols
            .iter()
            .find(|s| s.name == "Status" && s.kind == SymbolKind::Enum);
        assert!(enum_sym.is_some(), "Status should be an Enum");
    }

    #[test]
    fn test_java_methods_and_fields() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.java");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get Java symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"getName"),
            "Should find getName method: {names:?}"
        );
        assert!(
            names.contains(&"setAge"),
            "Should find setAge method: {names:?}"
        );
    }

    #[test]
    fn test_java_imports() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.java");
        let imports = backend
            .get_imports(&path)
            .expect("Failed to get Java imports");

        assert!(
            imports.len() >= 2,
            "Should find at least 2 imports, found {}",
            imports.len()
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // C
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_c_symbols() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.c");
        let symbols = backend.get_symbols(&path).expect("Failed to get C symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"process"),
            "Should find process function: {names:?}"
        );
        assert!(
            names.contains(&"calculate"),
            "Should find calculate function: {names:?}"
        );
        assert!(
            names.contains(&"Config"),
            "Should find Config struct: {names:?}"
        );
        assert!(
            names.contains(&"Status"),
            "Should find Status enum: {names:?}"
        );
    }

    #[test]
    fn test_c_symbol_kinds() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.c");
        let symbols = backend.get_symbols(&path).expect("Failed to get C symbols");

        let fn_sym = symbols
            .iter()
            .find(|s| s.name == "process" && s.kind == SymbolKind::Function);
        assert!(fn_sym.is_some(), "process should be a Function");

        let struct_sym = symbols
            .iter()
            .find(|s| s.name == "Config" && s.kind == SymbolKind::Struct);
        assert!(struct_sym.is_some(), "Config should be a Struct");

        let enum_sym = symbols
            .iter()
            .find(|s| s.name == "Status" && s.kind == SymbolKind::Enum);
        assert!(enum_sym.is_some(), "Status should be an Enum");
    }

    #[test]
    fn test_c_typedef() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.c");
        let symbols = backend.get_symbols(&path).expect("Failed to get C symbols");

        let typedef_sym = symbols
            .iter()
            .find(|s| s.name == "usize" && s.kind == SymbolKind::Type);
        assert!(typedef_sym.is_some(), "usize should be a Type (typedef)");
    }

    #[test]
    fn test_c_macro() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.c");
        let symbols = backend.get_symbols(&path).expect("Failed to get C symbols");

        let macro_sym = symbols
            .iter()
            .find(|s| s.name == "SQUARE" && s.kind == SymbolKind::Function);
        assert!(macro_sym.is_some(), "SQUARE should be a Function (macro)");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // C++
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_cpp_symbols() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.cpp");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get C++ symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"UserService"),
            "Should find UserService class: {names:?}"
        );
        assert!(
            names.contains(&"myapp"),
            "Should find myapp namespace: {names:?}"
        );
        assert!(
            names.contains(&"globalFunction"),
            "Should find globalFunction: {names:?}"
        );
    }

    #[test]
    fn test_cpp_symbol_kinds() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.cpp");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get C++ symbols");

        let class_sym = symbols
            .iter()
            .find(|s| s.name == "UserService" && s.kind == SymbolKind::Class);
        assert!(class_sym.is_some(), "UserService should be a Class");

        let ns_sym = symbols
            .iter()
            .find(|s| s.name == "myapp" && s.kind == SymbolKind::Type);
        assert!(ns_sym.is_some(), "myapp should be a Type (namespace)");

        let struct_sym = symbols
            .iter()
            .find(|s| s.name == "Config" && s.kind == SymbolKind::Struct);
        assert!(struct_sym.is_some(), "Config should be a Struct");

        let enum_sym = symbols
            .iter()
            .find(|s| s.name == "Status" && s.kind == SymbolKind::Enum);
        assert!(enum_sym.is_some(), "Status should be an Enum");
    }

    #[test]
    fn test_cpp_functions() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.cpp");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get C++ symbols");

        let fn_sym = symbols
            .iter()
            .find(|s| s.name == "globalFunction" && s.kind == SymbolKind::Function);
        assert!(fn_sym.is_some(), "globalFunction should be a Function");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Ruby
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_ruby_symbols() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rb");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get Ruby symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"Networking"),
            "Should find Networking module: {names:?}"
        );
        assert!(
            names.contains(&"HttpClient"),
            "Should find HttpClient class: {names:?}"
        );
        assert!(
            names.contains(&"Response"),
            "Should find Response class: {names:?}"
        );
    }

    #[test]
    fn test_ruby_symbol_kinds() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rb");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get Ruby symbols");

        let module_sym = symbols
            .iter()
            .find(|s| s.name == "Networking" && s.kind == SymbolKind::Type);
        assert!(module_sym.is_some(), "Networking should be a Type (module)");

        let class_sym = symbols
            .iter()
            .find(|s| s.name == "HttpClient" && s.kind == SymbolKind::Class);
        assert!(class_sym.is_some(), "HttpClient should be a Class");
    }

    #[test]
    fn test_ruby_methods() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rb");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get Ruby symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"initialize"),
            "Should find initialize method: {names:?}"
        );
        assert!(names.contains(&"get"), "Should find get method: {names:?}");
        assert!(
            names.contains(&"post"),
            "Should find post method: {names:?}"
        );
    }

    #[test]
    fn test_ruby_singleton_method() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.rb");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get Ruby symbols");

        let singleton_sym = symbols
            .iter()
            .find(|s| s.name == "default_client" && s.kind == SymbolKind::Function);
        assert!(
            singleton_sym.is_some(),
            "default_client should be a Function (singleton method)"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // PHP
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_php_symbols() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.php");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get PHP symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"UserService"),
            "Should find UserService class: {names:?}"
        );
        assert!(
            names.contains(&"Repository"),
            "Should find Repository interface: {names:?}"
        );
        assert!(
            names.contains(&"Loggable"),
            "Should find Loggable trait: {names:?}"
        );
        assert!(
            names.contains(&"processUsers"),
            "Should find processUsers function: {names:?}"
        );
    }

    #[test]
    fn test_php_symbol_kinds() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.php");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get PHP symbols");

        let class_sym = symbols
            .iter()
            .find(|s| s.name == "UserService" && s.kind == SymbolKind::Class);
        assert!(class_sym.is_some(), "UserService should be a Class");

        let iface_sym = symbols
            .iter()
            .find(|s| s.name == "Repository" && s.kind == SymbolKind::Trait);
        assert!(
            iface_sym.is_some(),
            "Repository should be a Trait (interface)"
        );

        let trait_sym = symbols
            .iter()
            .find(|s| s.name == "Loggable" && s.kind == SymbolKind::Trait);
        assert!(trait_sym.is_some(), "Loggable should be a Trait");

        let fn_sym = symbols
            .iter()
            .find(|s| s.name == "processUsers" && s.kind == SymbolKind::Function);
        assert!(fn_sym.is_some(), "processUsers should be a Function");
    }

    #[test]
    fn test_php_methods() {
        let backend = TreeSitterBackend::new();
        let path = fixture_path("sample.php");
        let symbols = backend
            .get_symbols(&path)
            .expect("Failed to get PHP symbols");

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"findAll"),
            "Should find findAll method: {names:?}"
        );
        assert!(
            names.contains(&"create"),
            "Should find create method: {names:?}"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Parse Cache Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_cache_hit() {
        let path = fixture_path("sample.rs");
        let backend = TreeSitterBackend::new();

        // First call: cache miss, parse and store
        let start = std::time::Instant::now();
        let symbols1 = backend.get_symbols(&path).expect("First call failed");
        let first_duration = start.elapsed();

        // Second call: cache hit, should be much faster
        let start = std::time::Instant::now();
        let symbols2 = backend.get_symbols(&path).expect("Second call failed");
        let second_duration = start.elapsed();

        // Verify results are identical (same cache)
        assert_eq!(symbols1.len(), symbols2.len(), "Symbol count should match");
        for (s1, s2) in symbols1.iter().zip(symbols2.iter()) {
            assert_eq!(s1.name, s2.name, "Symbol names should match");
            assert_eq!(s1.kind, s2.kind, "Symbol kinds should match");
        }

        // Cache hit should be at least 2x faster (not strict - debug builds slow)
        if !cfg!(debug_assertions) {
            assert!(
                second_duration < first_duration / 2,
                "Cache hit ({:?}) should be at least 2x faster than cache miss ({:?})",
                second_duration,
                first_duration
            );
        }
    }

    #[test]
    fn test_parse_cache_mtime_invalidation() {
        use std::fs::File;
        use std::io::Write;

        // Create a temporary test file
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("rhizome_test_cache.rs");

        // Write initial content
        {
            let mut file = File::create(&test_file).expect("Failed to create temp file");
            file.write_all(b"fn test_func() {}")
                .expect("Failed to write to temp file");
        }

        let backend = TreeSitterBackend::new();

        // First parse
        let symbols1 = backend
            .get_symbols(&test_file)
            .expect("First parse failed");
        assert_eq!(symbols1.len(), 1, "Should find one function");

        // Modify file (change mtime)
        std::thread::sleep(std::time::Duration::from_millis(10));
        {
            let mut file = File::create(&test_file).expect("Failed to update temp file");
            file.write_all(b"fn func1() {} fn func2() {}")
                .expect("Failed to write to temp file");
        }

        // Second parse should see the new content (mtime changed, cache invalidated)
        let symbols2 = backend
            .get_symbols(&test_file)
            .expect("Second parse failed");
        assert_eq!(symbols2.len(), 2, "Should find two functions after modification");

        // Clean up
        let _ = std::fs::remove_file(test_file);
    }
}
