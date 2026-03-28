use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::symbol::{Symbol, SymbolKind};

pub const WEIGHT_CONTAINS: f32 = 1.0;
pub const WEIGHT_IMPORTS: f32 = 0.5;
pub const WEIGHT_IMPLEMENTS: f32 = 0.8;
pub const WEIGHT_CALLS: f32 = 0.3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptNode {
    pub name: String,
    pub labels: Vec<String>,
    pub description: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraph {
    pub project: String,
    pub nodes: Vec<ConceptNode>,
    pub edges: Vec<ConceptEdge>,
}

fn language_from_extension(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py" | "pyi") => "python",
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript",
        Some("ts" | "tsx" | "mts" | "cts") => "typescript",
        Some("go") => "go",
        Some("java") => "java",
        Some("c" | "h") => "c",
        Some("cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh") => "cpp",
        Some("rb" | "rake" | "gemspec" | "ru") => "ruby",
        Some("ex" | "exs") => "elixir",
        Some("zig" | "zon") => "zig",
        Some("cs") => "csharp",
        Some("fs" | "fsi" | "fsx") => "fsharp",
        Some("swift") => "swift",
        Some("php") => "php",
        Some("hs" | "lhs") => "haskell",
        Some("sh" | "bash" | "zsh") => "bash",
        Some("tf" | "tfvars") => "terraform",
        Some("kt" | "kts") => "kotlin",
        Some("dart") => "dart",
        Some("lua") => "lua",
        Some("clj" | "cljs" | "cljc") => "clojure",
        Some("ml" | "mli") => "ocaml",
        Some("jl") => "julia",
        Some("nix") => "nix",
        Some("gleam") => "gleam",
        Some("vue") => "vue",
        Some("svelte") => "svelte",
        Some("astro") => "astro",
        Some("prisma") => "prisma",
        Some("typ" | "typc") => "typst",
        Some("yaml" | "yml") => "yaml",
        _ => "unknown",
    }
}

fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn labels_for_kind(kind: &SymbolKind) -> Vec<String> {
    match kind {
        SymbolKind::Function => vec!["function".into()],
        SymbolKind::Method => vec!["method".into()],
        SymbolKind::Struct => vec!["struct".into(), "type".into()],
        SymbolKind::Enum => vec!["enum".into(), "type".into()],
        SymbolKind::Trait => vec!["trait".into(), "interface".into()],
        SymbolKind::Class => vec!["class".into(), "type".into()],
        SymbolKind::Interface => vec!["interface".into()],
        SymbolKind::Type => vec!["type".into()],
        SymbolKind::Constant => vec!["constant".into()],
        SymbolKind::Variable => vec!["variable".into()],
        SymbolKind::Import => vec!["import".into()],
        SymbolKind::Module => vec!["module".into()],
        SymbolKind::Property => vec!["property".into()],
        SymbolKind::Field => vec!["field".into()],
    }
}

fn build_description(signature: &Option<String>, doc_comment: &Option<String>) -> String {
    match (signature, doc_comment) {
        (Some(sig), Some(doc)) => format!("{sig}\n\n{doc}"),
        (Some(sig), None) => sig.clone(),
        (None, Some(doc)) => doc.clone(),
        (None, None) => String::new(),
    }
}

fn process_symbols(
    symbols: &[Symbol],
    file_path_str: &str,
    language: &str,
    module_name: &str,
    nodes: &mut Vec<ConceptNode>,
    edges: &mut Vec<ConceptEdge>,
    parent_name: Option<&str>,
) {
    for symbol in symbols {
        let mut labels = labels_for_kind(&symbol.kind);

        if let Some(sig) = &symbol.signature {
            if sig.starts_with("pub") {
                labels.push("public".into());
            }
            if sig.contains("async") {
                labels.push("async".into());
            }
        }

        let description = build_description(&symbol.signature, &symbol.doc_comment);

        let mut metadata = HashMap::new();
        metadata.insert("file_path".into(), file_path_str.to_string());
        metadata.insert("line_start".into(), symbol.location.line_start.to_string());
        metadata.insert("line_end".into(), symbol.location.line_end.to_string());
        metadata.insert("language".into(), language.to_string());

        nodes.push(ConceptNode {
            name: symbol.name.clone(),
            labels,
            description,
            metadata,
        });

        if let Some(parent) = parent_name {
            edges.push(ConceptEdge {
                source: parent.to_string(),
                target: symbol.name.clone(),
                relation: "contains".into(),
                weight: WEIGHT_CONTAINS,
            });
        }

        if symbol.kind == SymbolKind::Import {
            edges.push(ConceptEdge {
                source: module_name.to_string(),
                target: symbol.name.clone(),
                relation: "imports".into(),
                weight: WEIGHT_IMPORTS,
            });
        }

        if !symbol.children.is_empty() {
            process_symbols(
                &symbol.children,
                file_path_str,
                language,
                module_name,
                nodes,
                edges,
                Some(&symbol.name),
            );
        }
    }
}

pub fn build_graph(project: &str, symbols: &[Symbol], file_path: &Path) -> CodeGraph {
    let module_name = module_name_from_path(file_path);
    let language = language_from_extension(file_path);
    let file_path_str = file_path.to_string_lossy();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    process_symbols(
        symbols,
        &file_path_str,
        language,
        &module_name,
        &mut nodes,
        &mut edges,
        None,
    );

    CodeGraph {
        project: project.to_string(),
        nodes,
        edges,
    }
}

pub fn merge_graphs(graphs: Vec<CodeGraph>) -> CodeGraph {
    let project = graphs
        .first()
        .map(|g| g.project.clone())
        .unwrap_or_default();

    let mut seen_names = std::collections::HashSet::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for graph in graphs {
        for node in graph.nodes {
            if seen_names.insert(node.name.clone()) {
                nodes.push(node);
            }
        }
        edges.extend(graph.edges);
    }

    edges.retain(|edge| seen_names.contains(&edge.source) && seen_names.contains(&edge.target));

    CodeGraph {
        project,
        nodes,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::Location;

    fn make_location() -> Location {
        Location {
            file_path: "src/server.rs".into(),
            line_start: 1,
            line_end: 10,
            column_start: 0,
            column_end: 0,
        }
    }

    fn make_symbol(name: &str, kind: SymbolKind) -> Symbol {
        Symbol {
            name: name.into(),
            kind,
            location: make_location(),
            scope_path: vec![],
            signature: None,
            doc_comment: None,
            children: vec![],
        }
    }

    #[test]
    fn test_build_graph_node_and_edge_counts() {
        let symbols = vec![
            make_symbol("handle_request", SymbolKind::Function),
            make_symbol("Server", SymbolKind::Struct),
        ];

        let graph = build_graph("myproject", &symbols, Path::new("src/server.rs"));

        assert_eq!(graph.project, "myproject");
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 0);
    }

    #[test]
    fn test_labels_mapping() {
        let symbols = vec![
            make_symbol("foo", SymbolKind::Function),
            make_symbol("Bar", SymbolKind::Struct),
            make_symbol("Baz", SymbolKind::Enum),
            make_symbol("Quux", SymbolKind::Trait),
            make_symbol("MyClass", SymbolKind::Class),
            make_symbol("IFace", SymbolKind::Interface),
            make_symbol("MyType", SymbolKind::Type),
            make_symbol("MAX", SymbolKind::Constant),
            make_symbol("x", SymbolKind::Variable),
            make_symbol("serde", SymbolKind::Import),
            make_symbol("mymod", SymbolKind::Module),
            make_symbol("prop", SymbolKind::Property),
            make_symbol("field", SymbolKind::Field),
        ];

        let graph = build_graph("test", &symbols, Path::new("src/lib.rs"));

        assert_eq!(graph.nodes[0].labels, vec!["function"]);
        assert_eq!(graph.nodes[1].labels, vec!["struct", "type"]);
        assert_eq!(graph.nodes[2].labels, vec!["enum", "type"]);
        assert_eq!(graph.nodes[3].labels, vec!["trait", "interface"]);
        assert_eq!(graph.nodes[4].labels, vec!["class", "type"]);
        assert_eq!(graph.nodes[5].labels, vec!["interface"]);
        assert_eq!(graph.nodes[6].labels, vec!["type"]);
        assert_eq!(graph.nodes[7].labels, vec!["constant"]);
        assert_eq!(graph.nodes[8].labels, vec!["variable"]);
        assert_eq!(graph.nodes[9].labels, vec!["import"]);
        assert_eq!(graph.nodes[10].labels, vec!["module"]);
        assert_eq!(graph.nodes[11].labels, vec!["property"]);
        assert_eq!(graph.nodes[12].labels, vec!["field"]);
    }

    #[test]
    fn test_metadata() {
        let symbols = vec![make_symbol("foo", SymbolKind::Function)];
        let graph = build_graph("proj", &symbols, Path::new("src/server.rs"));

        let node = &graph.nodes[0];
        assert_eq!(node.metadata.get("file_path").unwrap(), "src/server.rs");
        assert_eq!(node.metadata.get("line_start").unwrap(), "1");
        assert_eq!(node.metadata.get("line_end").unwrap(), "10");
        assert_eq!(node.metadata.get("language").unwrap(), "rust");
    }

    #[test]
    fn test_public_label() {
        let mut sym = make_symbol("serve", SymbolKind::Function);
        sym.signature = Some("pub fn serve()".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        assert!(graph.nodes[0].labels.contains(&"public".to_string()));
    }

    #[test]
    fn test_async_label() {
        let mut sym = make_symbol("fetch", SymbolKind::Function);
        sym.signature = Some("pub async fn fetch()".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        assert!(graph.nodes[0].labels.contains(&"async".to_string()));
        assert!(graph.nodes[0].labels.contains(&"public".to_string()));
    }

    #[test]
    fn test_description_both_present() {
        let mut sym = make_symbol("foo", SymbolKind::Function);
        sym.signature = Some("fn foo()".into());
        sym.doc_comment = Some("Does foo things".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        assert_eq!(graph.nodes[0].description, "fn foo()\n\nDoes foo things");
    }

    #[test]
    fn test_description_only_signature() {
        let mut sym = make_symbol("foo", SymbolKind::Function);
        sym.signature = Some("fn foo()".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        assert_eq!(graph.nodes[0].description, "fn foo()");
    }

    #[test]
    fn test_description_only_doc_comment() {
        let mut sym = make_symbol("foo", SymbolKind::Function);
        sym.doc_comment = Some("Does foo things".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        assert_eq!(graph.nodes[0].description, "Does foo things");
    }

    #[test]
    fn test_description_neither() {
        let sym = make_symbol("foo", SymbolKind::Function);
        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        assert_eq!(graph.nodes[0].description, "");
    }

    #[test]
    fn test_import_edge_creation() {
        let symbols = vec![make_symbol("serde", SymbolKind::Import)];
        let graph = build_graph("proj", &symbols, Path::new("src/server.rs"));

        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].source, "server");
        assert_eq!(graph.edges[0].target, "serde");
        assert_eq!(graph.edges[0].relation, "imports");
        assert_eq!(graph.edges[0].weight, WEIGHT_IMPORTS);
    }

    #[test]
    fn test_contains_edge_for_children() {
        let child = make_symbol("method_a", SymbolKind::Method);
        let mut parent = make_symbol("MyStruct", SymbolKind::Struct);
        parent.children = vec![child];

        let graph = build_graph("proj", &[parent], Path::new("src/lib.rs"));

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].source, "MyStruct");
        assert_eq!(graph.edges[0].target, "method_a");
        assert_eq!(graph.edges[0].relation, "contains");
        assert_eq!(graph.edges[0].weight, WEIGHT_CONTAINS);
    }

    #[test]
    fn test_merge_graphs_deduplication() {
        let g1 = CodeGraph {
            project: "proj".into(),
            nodes: vec![ConceptNode {
                name: "Foo".into(),
                labels: vec!["struct".into()],
                description: "first".into(),
                metadata: HashMap::new(),
            }],
            edges: vec![],
        };

        let g2 = CodeGraph {
            project: "proj".into(),
            nodes: vec![
                ConceptNode {
                    name: "Foo".into(),
                    labels: vec!["struct".into()],
                    description: "second".into(),
                    metadata: HashMap::new(),
                },
                ConceptNode {
                    name: "Bar".into(),
                    labels: vec!["function".into()],
                    description: String::new(),
                    metadata: HashMap::new(),
                },
            ],
            edges: vec![ConceptEdge {
                source: "Foo".into(),
                target: "Bar".into(),
                relation: "contains".into(),
                weight: 1.0,
            }],
        };

        let merged = merge_graphs(vec![g1, g2]);

        assert_eq!(merged.project, "proj");
        assert_eq!(merged.nodes.len(), 2);
        assert_eq!(merged.nodes[0].description, "first");
        assert_eq!(merged.edges.len(), 1);
    }

    #[test]
    fn test_merge_graphs_drops_edges_with_missing_endpoints() {
        let graph = CodeGraph {
            project: "proj".into(),
            nodes: vec![ConceptNode {
                name: "serde".into(),
                labels: vec!["import".into()],
                description: String::new(),
                metadata: HashMap::new(),
            }],
            edges: vec![ConceptEdge {
                source: "integration_tests".into(),
                target: "serde".into(),
                relation: "imports".into(),
                weight: WEIGHT_IMPORTS,
            }],
        };

        let merged = merge_graphs(vec![graph]);
        assert_eq!(merged.nodes.len(), 1);
        assert!(merged.edges.is_empty());
    }

    #[test]
    fn test_language_detection() {
        let symbols = vec![make_symbol("foo", SymbolKind::Function)];

        let g = build_graph("p", &symbols, Path::new("src/app.py"));
        assert_eq!(g.nodes[0].metadata.get("language").unwrap(), "python");

        let g = build_graph("p", &symbols, Path::new("src/app.ts"));
        assert_eq!(g.nodes[0].metadata.get("language").unwrap(), "typescript");

        let g = build_graph("p", &symbols, Path::new("src/app.go"));
        assert_eq!(g.nodes[0].metadata.get("language").unwrap(), "go");
    }
}
