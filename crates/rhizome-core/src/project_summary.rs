use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use crate::backend::CodeIntelligence;
use crate::error::Result;
use crate::language::Language;
use crate::symbol::{Symbol, SymbolKind};

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub name: String,
    pub root: PathBuf,
    pub languages: Vec<(String, usize)>,
    pub total_files: usize,
    pub total_symbols: usize,
    pub entry_points: Vec<EntryPoint>,
    pub key_types: Vec<String>,
    pub modules: Vec<ModuleSummary>,
    pub test_files: usize,
    pub test_functions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPoint {
    pub file: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSummary {
    pub name: String,
    pub files: usize,
    pub functions: usize,
    pub description: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const MAX_FILE_LINES: usize = 10_000;
const TOP_TYPES_COUNT: usize = 10;

// ─────────────────────────────────────────────────────────────────────────────
// Entry point detection
// ─────────────────────────────────────────────────────────────────────────────

fn detect_entry_point(rel_path: &str, symbols: &[Symbol]) -> Option<EntryPoint> {
    let file_name = Path::new(rel_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Check for main() function
    for sym in symbols {
        if sym.name == "main" && matches!(sym.kind, SymbolKind::Function) {
            return Some(EntryPoint {
                file: rel_path.to_string(),
                kind: "main()".to_string(),
            });
        }
    }

    // Check for known entry point files
    match file_name {
        "lib.rs" => Some(EntryPoint {
            file: rel_path.to_string(),
            kind: "library root".to_string(),
        }),
        "index.ts" | "index.tsx" | "index.js" | "index.jsx" => Some(EntryPoint {
            file: rel_path.to_string(),
            kind: "module index".to_string(),
        }),
        "app.py" | "wsgi.py" | "asgi.py" => Some(EntryPoint {
            file: rel_path.to_string(),
            kind: "application entry".to_string(),
        }),
        "main.go" => Some(EntryPoint {
            file: rel_path.to_string(),
            kind: "main package".to_string(),
        }),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Symbol counting helpers
// ─────────────────────────────────────────────────────────────────────────────

fn is_test_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("test") || lower.contains("spec") || lower.contains("_test.")
}

fn is_test_function(sym: &Symbol) -> bool {
    let name = &sym.name;
    name.starts_with("test_")
        || name.starts_with("it_")
        || name.starts_with("should_")
        || name.ends_with("_test")
        || sym
            .doc_comment
            .as_deref()
            .is_some_and(|d| d.contains("#[test]") || d.contains("@Test"))
}

fn count_functions(symbols: &[Symbol]) -> usize {
    symbols
        .iter()
        .map(|s| {
            let self_count =
                usize::from(matches!(s.kind, SymbolKind::Function | SymbolKind::Method));
            self_count + count_functions(&s.children)
        })
        .sum()
}

fn count_test_functions(symbols: &[Symbol]) -> usize {
    symbols
        .iter()
        .map(|s| {
            let self_count = usize::from(
                matches!(s.kind, SymbolKind::Function | SymbolKind::Method) && is_test_function(s),
            );
            self_count + count_test_functions(&s.children)
        })
        .sum()
}

fn collect_type_names(symbols: &[Symbol], counts: &mut HashMap<String, usize>) {
    for sym in symbols {
        match sym.kind {
            SymbolKind::Struct
            | SymbolKind::Class
            | SymbolKind::Enum
            | SymbolKind::Interface
            | SymbolKind::Trait
            | SymbolKind::Type => {
                *counts.entry(sym.name.clone()).or_insert(0) += 1;
            }
            _ => {}
        }
        collect_type_names(&sym.children, counts);
    }
}

fn count_all_symbols(symbols: &[Symbol]) -> usize {
    symbols
        .iter()
        .map(|s| 1 + count_all_symbols(&s.children))
        .sum()
}

fn file_line_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|c| c.lines().count())
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Symbol kind breakdown
// ─────────────────────────────────────────────────────────────────────────────

fn count_by_kind(symbols: &[Symbol], counts: &mut HashMap<String, usize>) {
    for sym in symbols {
        let label = match sym.kind {
            SymbolKind::Function => "functions",
            SymbolKind::Method => "methods",
            SymbolKind::Struct => "structs",
            SymbolKind::Class => "classes",
            SymbolKind::Enum => "enums",
            SymbolKind::Trait => "traits",
            SymbolKind::Interface => "interfaces",
            SymbolKind::Type => "types",
            SymbolKind::Constant => "constants",
            SymbolKind::Variable => "variables",
            SymbolKind::Module => "modules",
            SymbolKind::Import => "imports",
            SymbolKind::Property => "properties",
            SymbolKind::Field => "fields",
        };
        *counts.entry(label.to_string()).or_insert(0) += 1;
        count_by_kind(&sym.children, counts);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

pub fn summarize_project(root: &Path, backend: &dyn CodeIntelligence) -> Result<ProjectSummary> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    let project_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let walker = WalkBuilder::new(&root)
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut lang_counts: HashMap<String, usize> = HashMap::new();
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut kind_counts: HashMap<String, usize> = HashMap::new();
    let mut module_data: HashMap<String, (usize, usize)> = HashMap::new();
    let mut entry_points = Vec::new();
    let mut total_files: usize = 0;
    let mut total_symbols: usize = 0;
    let mut test_files: usize = 0;
    let mut test_functions: usize = 0;

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_string(),
            None => continue,
        };

        let lang = match Language::from_extension(&ext) {
            Some(l) => l,
            None => continue,
        };

        // Skip overly large files
        if file_line_count(path) > MAX_FILE_LINES {
            continue;
        }

        total_files += 1;
        *lang_counts.entry(lang.to_string()).or_insert(0) += 1;

        let rel_path = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Track test files
        if is_test_file(&rel_path) {
            test_files += 1;
        }

        // Determine top-level module (first directory component)
        let module_name = Path::new(&rel_path).components().next().and_then(|c| {
            let s = c.as_os_str().to_string_lossy().to_string();
            if s.contains('.') {
                None // top-level file, not a directory
            } else {
                Some(s)
            }
        });

        // Extract symbols
        let symbols = match backend.get_symbols(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let sym_count = count_all_symbols(&symbols);
        total_symbols += sym_count;

        count_by_kind(&symbols, &mut kind_counts);
        collect_type_names(&symbols, &mut type_counts);

        let fn_count = count_functions(&symbols);
        test_functions += count_test_functions(&symbols);

        if let Some(ep) = detect_entry_point(&rel_path, &symbols) {
            entry_points.push(ep);
        }

        if let Some(mod_name) = module_name {
            let entry = module_data.entry(mod_name).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += fn_count;
        }
    }

    // Sort languages by file count descending
    let mut languages: Vec<(String, usize)> = lang_counts.into_iter().collect();
    languages.sort_by(|a, b| b.1.cmp(&a.1));

    // Top referenced types
    let mut type_list: Vec<(String, usize)> = type_counts.into_iter().collect();
    type_list.sort_by(|a, b| b.1.cmp(&a.1));
    let key_types: Vec<String> = type_list
        .into_iter()
        .take(TOP_TYPES_COUNT)
        .map(|(name, _)| name)
        .collect();

    // Module summaries sorted by file count
    let mut modules: Vec<ModuleSummary> = module_data
        .into_iter()
        .map(|(name, (files, functions))| ModuleSummary {
            name,
            files,
            functions,
            description: String::new(),
        })
        .collect();
    modules.sort_by(|a, b| b.files.cmp(&a.files));

    Ok(ProjectSummary {
        name: project_name,
        root,
        languages,
        total_files,
        total_symbols,
        entry_points,
        key_types,
        modules,
        test_files,
        test_functions,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Display formatting
// ─────────────────────────────────────────────────────────────────────────────

impl ProjectSummary {
    pub fn format_display(&self) -> String {
        let mut out = String::new();

        let _ = writeln!(out, "Project: {}", self.name);
        let _ = writeln!(out, "Root: {}", self.root.display());
        let _ = writeln!(out);

        // Languages
        let lang_parts: Vec<String> = self
            .languages
            .iter()
            .map(|(lang, count)| {
                if *count == 1 {
                    format!("{lang} (1 file)")
                } else {
                    format!("{lang} ({count} files)")
                }
            })
            .collect();
        let _ = writeln!(out, "Languages: {}", lang_parts.join(", "));

        // Symbols
        let _ = write!(out, "Symbols: {} total", self.total_symbols);
        let _ = writeln!(out);

        // Tests
        let _ = writeln!(
            out,
            "Tests: {} functions across {} files",
            self.test_functions, self.test_files,
        );

        // Entry points
        if !self.entry_points.is_empty() {
            let _ = writeln!(out);
            let _ = writeln!(out, "Entry Points:");
            for ep in &self.entry_points {
                let _ = writeln!(out, "  {} ({})", ep.file, ep.kind);
            }
        }

        // Key types
        if !self.key_types.is_empty() {
            let _ = writeln!(out);
            let _ = writeln!(out, "Key Types (most referenced):");
            let _ = writeln!(out, "  {}", self.key_types.join(", "));
        }

        // Modules
        if !self.modules.is_empty() {
            let _ = writeln!(out);
            let _ = writeln!(out, "Modules:");
            for m in &self.modules {
                let _ = writeln!(
                    out,
                    "  {}/ \u{2014} {} files, {} functions",
                    m.name, m.files, m.functions,
                );
            }
        }

        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{BackendCapabilities, Diagnostic};
    use crate::symbol::Location;

    struct MockBackend {
        symbols: Vec<Symbol>,
    }

    impl MockBackend {
        fn new(symbols: Vec<Symbol>) -> Self {
            Self { symbols }
        }
    }

    impl CodeIntelligence for MockBackend {
        fn get_symbols(&self, _file: &Path) -> Result<Vec<Symbol>> {
            Ok(self.symbols.clone())
        }

        fn get_definition(&self, _file: &Path, _name: &str) -> Result<Option<Symbol>> {
            Ok(None)
        }

        fn find_references(
            &self,
            _file: &Path,
            _position: &crate::backend::Position,
        ) -> Result<Vec<Location>> {
            Ok(vec![])
        }

        fn search_symbols(&self, _pattern: &str, _root: &Path) -> Result<Vec<Symbol>> {
            Ok(vec![])
        }

        fn get_imports(&self, _file: &Path) -> Result<Vec<Symbol>> {
            Ok(vec![])
        }

        fn get_diagnostics(&self, _file: &Path) -> Result<Vec<Diagnostic>> {
            Ok(vec![])
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

    fn make_symbol(name: &str, kind: SymbolKind) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind,
            location: Location {
                file_path: String::new(),
                line_start: 0,
                line_end: 0,
                column_start: 0,
                column_end: 0,
            },
            scope_path: vec![],
            signature: None,
            doc_comment: None,
            children: vec![],
        }
    }

    #[test]
    fn test_detect_entry_point_main() {
        let symbols = vec![make_symbol("main", SymbolKind::Function)];
        let ep = detect_entry_point("src/main.rs", &symbols);
        assert!(ep.is_some());
        let ep = ep.unwrap();
        assert_eq!(ep.kind, "main()");
    }

    #[test]
    fn test_detect_entry_point_lib() {
        let symbols = vec![make_symbol("foo", SymbolKind::Function)];
        let ep = detect_entry_point("src/lib.rs", &symbols);
        assert!(ep.is_some());
        assert_eq!(ep.unwrap().kind, "library root");
    }

    #[test]
    fn test_detect_entry_point_none() {
        let symbols = vec![make_symbol("foo", SymbolKind::Function)];
        let ep = detect_entry_point("src/utils.rs", &symbols);
        assert!(ep.is_none());
    }

    #[test]
    fn test_is_test_file() {
        assert!(is_test_file("src/tests/foo.rs"));
        assert!(is_test_file("src/foo.test.ts"));
        assert!(is_test_file("src/foo_spec.rb"));
        assert!(!is_test_file("src/main.rs"));
    }

    #[test]
    fn test_is_test_function() {
        let sym = make_symbol("test_something", SymbolKind::Function);
        assert!(is_test_function(&sym));

        let sym = make_symbol("run_server", SymbolKind::Function);
        assert!(!is_test_function(&sym));
    }

    #[test]
    fn test_count_functions() {
        let symbols = vec![
            make_symbol("foo", SymbolKind::Function),
            make_symbol("Bar", SymbolKind::Struct),
            make_symbol("baz", SymbolKind::Method),
        ];
        assert_eq!(count_functions(&symbols), 2);
    }

    #[test]
    fn test_count_all_symbols() {
        let mut parent = make_symbol("Foo", SymbolKind::Struct);
        parent.children = vec![make_symbol("bar", SymbolKind::Method)];
        let symbols = vec![parent, make_symbol("baz", SymbolKind::Function)];
        assert_eq!(count_all_symbols(&symbols), 3);
    }

    #[test]
    fn test_collect_type_names() {
        let symbols = vec![
            make_symbol("Config", SymbolKind::Struct),
            make_symbol("Config", SymbolKind::Struct),
            make_symbol("run", SymbolKind::Function),
            make_symbol("Error", SymbolKind::Enum),
        ];
        let mut counts = HashMap::new();
        collect_type_names(&symbols, &mut counts);
        assert_eq!(counts.get("Config"), Some(&2));
        assert_eq!(counts.get("Error"), Some(&1));
        assert!(!counts.contains_key("run"));
    }

    #[test]
    fn test_format_display() {
        let summary = ProjectSummary {
            name: "test-project".to_string(),
            root: PathBuf::from("/tmp/test-project"),
            languages: vec![("Rust".to_string(), 10), ("Python".to_string(), 3)],
            total_files: 13,
            total_symbols: 200,
            entry_points: vec![EntryPoint {
                file: "src/main.rs".to_string(),
                kind: "main()".to_string(),
            }],
            key_types: vec!["Config".to_string(), "Server".to_string()],
            modules: vec![ModuleSummary {
                name: "src".to_string(),
                files: 10,
                functions: 50,
                description: String::new(),
            }],
            test_files: 2,
            test_functions: 15,
        };

        let output = summary.format_display();
        assert!(output.contains("Project: test-project"));
        assert!(output.contains("Rust (10 files)"));
        assert!(output.contains("Python (3 files)"));
        assert!(output.contains("200 total"));
        assert!(output.contains("15 functions across 2 files"));
        assert!(output.contains("src/main.rs (main())"));
        assert!(output.contains("Config, Server"));
        assert!(output.contains("src/"));
    }

    #[test]
    fn test_summarize_project_with_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create a simple Rust file
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("main.rs"), "fn main() {}\nstruct Config {}\n").unwrap();

        let backend = MockBackend::new(vec![
            make_symbol("main", SymbolKind::Function),
            make_symbol("Config", SymbolKind::Struct),
        ]);

        let summary = summarize_project(root, &backend).unwrap();
        assert_eq!(summary.total_files, 1);
        assert_eq!(summary.total_symbols, 2);
        assert!(!summary.entry_points.is_empty());
        assert!(summary.key_types.contains(&"Config".to_string()));
    }
}
