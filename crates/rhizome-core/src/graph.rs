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

fn language_from_extension(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust".into(),
        Some("py" | "pyi") => "python".into(),
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript".into(),
        Some("ts" | "tsx" | "mts" | "cts") => "typescript".into(),
        Some("go") => "go".into(),
        Some("java") => "java".into(),
        Some("c" | "h") => "c".into(),
        Some("cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh") => "cpp".into(),
        Some("rb" | "rake" | "gemspec" | "ru") => "ruby".into(),
        Some("ex" | "exs") => "elixir".into(),
        Some("zig" | "zon") => "zig".into(),
        Some("cs") => "csharp".into(),
        Some("fs" | "fsi" | "fsx") => "fsharp".into(),
        Some("swift") => "swift".into(),
        Some("php") => "php".into(),
        Some("hs" | "lhs") => "haskell".into(),
        Some("sh" | "bash" | "zsh") => "bash".into(),
        Some("tf" | "tfvars") => "terraform".into(),
        Some("kt" | "kts") => "kotlin".into(),
        Some("dart") => "dart".into(),
        Some("lua") => "lua".into(),
        Some("clj" | "cljs" | "cljc") => "clojure".into(),
        Some("ml" | "mli") => "ocaml".into(),
        Some("jl") => "julia".into(),
        Some("nix") => "nix".into(),
        Some("gleam") => "gleam".into(),
        Some("vue") => "vue".into(),
        Some("svelte") => "svelte".into(),
        Some("astro") => "astro".into(),
        Some("prisma") => "prisma".into(),
        Some("typ" | "typc") => "typst".into(),
        Some("yaml" | "yml") => "yaml".into(),
        // Use the full file path as fallback to prevent node collision
        // when multiple files share the same unrecognized extension.
        _ => path.to_string_lossy().into_owned(),
    }
}

/// Generate a unique, stable node identifier for a symbol.
/// Combines file path with the full qualified name (built from scope path + symbol name).
fn node_id_for_symbol(symbol: &Symbol, file_path_str: &str, current_scope: &[String]) -> String {
    let mut parts = current_scope.to_vec();
    parts.push(symbol.name.clone());
    format!("{}::{}", file_path_str, parts.join("::"))
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
    nodes: &mut Vec<ConceptNode>,
    edges: &mut Vec<ConceptEdge>,
    parent_id: Option<&str>,
    current_scope: &[String],
) {
    for symbol in symbols {
        let mut labels = labels_for_kind(&symbol.kind);

        if let Some(sig) = &symbol.signature {
            if sig.starts_with("pub ") || sig.starts_with("pub(") {
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

        let node_id = node_id_for_symbol(symbol, file_path_str, current_scope);

        nodes.push(ConceptNode {
            name: node_id.clone(),
            labels,
            description,
            metadata,
        });

        if let Some(parent) = parent_id {
            edges.push(ConceptEdge {
                source: parent.to_string(),
                target: node_id.clone(),
                relation: "contains".into(),
                weight: WEIGHT_CONTAINS,
            });
        }

        if symbol.kind == SymbolKind::Import {
            edges.push(ConceptEdge {
                source: file_path_str.to_string(),
                target: node_id.clone(),
                relation: "imports".into(),
                weight: WEIGHT_IMPORTS,
            });
        }

        if !symbol.children.is_empty() {
            let mut child_scope = current_scope.to_vec();
            child_scope.push(symbol.name.clone());
            process_symbols(
                &symbol.children,
                file_path_str,
                language,
                nodes,
                edges,
                Some(&node_id),
                &child_scope,
            );
        }
    }
}

pub fn build_graph(project: &str, symbols: &[Symbol], file_path: &Path) -> CodeGraph {
    let language = language_from_extension(file_path);
    let file_path_str = file_path.to_string_lossy();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Add a synthetic file-level node to anchor imports edges
    let mut file_metadata = HashMap::new();
    file_metadata.insert("file_path".into(), file_path_str.to_string());
    file_metadata.insert("line_start".into(), "0".to_string());
    file_metadata.insert("line_end".into(), "0".to_string());
    file_metadata.insert("language".into(), language.clone());

    nodes.push(ConceptNode {
        name: file_path_str.to_string(),
        labels: vec!["file".into()],
        description: String::new(),
        metadata: file_metadata,
    });

    process_symbols(
        symbols,
        &file_path_str,
        &language,
        &mut nodes,
        &mut edges,
        None,
        &[],
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
        assert_eq!(graph.nodes.len(), 3); // file-level node + 2 symbols
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

        // File-level node is at index 0, symbols start at index 1
        assert_eq!(graph.nodes[1].labels, vec!["function"]);
        assert_eq!(graph.nodes[2].labels, vec!["struct", "type"]);
        assert_eq!(graph.nodes[3].labels, vec!["enum", "type"]);
        assert_eq!(graph.nodes[4].labels, vec!["trait", "interface"]);
        assert_eq!(graph.nodes[5].labels, vec!["class", "type"]);
        assert_eq!(graph.nodes[6].labels, vec!["interface"]);
        assert_eq!(graph.nodes[7].labels, vec!["type"]);
        assert_eq!(graph.nodes[8].labels, vec!["constant"]);
        assert_eq!(graph.nodes[9].labels, vec!["variable"]);
        assert_eq!(graph.nodes[10].labels, vec!["import"]);
        assert_eq!(graph.nodes[11].labels, vec!["module"]);
        assert_eq!(graph.nodes[12].labels, vec!["property"]);
        assert_eq!(graph.nodes[13].labels, vec!["field"]);
    }

    #[test]
    fn test_metadata() {
        let symbols = vec![make_symbol("foo", SymbolKind::Function)];
        let graph = build_graph("proj", &symbols, Path::new("src/server.rs"));

        // File-level node is at index 0, function symbol is at index 1
        let node = &graph.nodes[1];
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
        // File-level node is at index 0, function symbol is at index 1
        assert!(graph.nodes[1].labels.contains(&"public".to_string()));
    }

    #[test]
    fn test_async_label() {
        let mut sym = make_symbol("fetch", SymbolKind::Function);
        sym.signature = Some("pub async fn fetch()".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        // File-level node is at index 0, function symbol is at index 1
        assert!(graph.nodes[1].labels.contains(&"async".to_string()));
        assert!(graph.nodes[1].labels.contains(&"public".to_string()));
    }

    #[test]
    fn test_description_both_present() {
        let mut sym = make_symbol("foo", SymbolKind::Function);
        sym.signature = Some("fn foo()".into());
        sym.doc_comment = Some("Does foo things".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        // File-level node is at index 0, function symbol is at index 1
        assert_eq!(graph.nodes[1].description, "fn foo()\n\nDoes foo things");
    }

    #[test]
    fn test_description_only_signature() {
        let mut sym = make_symbol("foo", SymbolKind::Function);
        sym.signature = Some("fn foo()".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        // File-level node is at index 0, function symbol is at index 1
        assert_eq!(graph.nodes[1].description, "fn foo()");
    }

    #[test]
    fn test_description_only_doc_comment() {
        let mut sym = make_symbol("foo", SymbolKind::Function);
        sym.doc_comment = Some("Does foo things".into());

        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        // File-level node is at index 0, function symbol is at index 1
        assert_eq!(graph.nodes[1].description, "Does foo things");
    }

    #[test]
    fn test_description_neither() {
        let sym = make_symbol("foo", SymbolKind::Function);
        let graph = build_graph("proj", &[sym], Path::new("src/lib.rs"));
        // File-level node is at index 0, function symbol is at index 1
        assert_eq!(graph.nodes[1].description, "");
    }

    #[test]
    fn test_import_edge_creation() {
        let symbols = vec![make_symbol("serde", SymbolKind::Import)];
        let graph = build_graph("proj", &symbols, Path::new("src/server.rs"));

        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].source, "src/server.rs");
        assert_eq!(graph.edges[0].target, "src/server.rs::serde");
        assert_eq!(graph.edges[0].relation, "imports");
        assert_eq!(graph.edges[0].weight, WEIGHT_IMPORTS);
    }

    #[test]
    fn test_contains_edge_for_children() {
        let child = make_symbol("method_a", SymbolKind::Method);
        let mut parent = make_symbol("MyStruct", SymbolKind::Struct);
        parent.children = vec![child];

        let graph = build_graph("proj", &[parent], Path::new("src/lib.rs"));

        // File-level node + parent + child = 3 nodes
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].source, "src/lib.rs::MyStruct");
        assert_eq!(graph.edges[0].target, "src/lib.rs::MyStruct::method_a");
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
    fn test_merge_graphs_preserves_imports_edges() {
        // Verify that imports edges survive merge when the file-level node is present
        let graph = CodeGraph {
            project: "proj".into(),
            nodes: vec![
                ConceptNode {
                    name: "src/server.rs".into(),
                    labels: vec!["file".into()],
                    description: String::new(),
                    metadata: HashMap::new(),
                },
                ConceptNode {
                    name: "src/server.rs::serde".into(),
                    labels: vec!["import".into()],
                    description: String::new(),
                    metadata: HashMap::new(),
                },
            ],
            edges: vec![ConceptEdge {
                source: "src/server.rs".into(),
                target: "src/server.rs::serde".into(),
                relation: "imports".into(),
                weight: WEIGHT_IMPORTS,
            }],
        };

        let merged = merge_graphs(vec![graph]);
        assert_eq!(merged.nodes.len(), 2);
        assert_eq!(merged.edges.len(), 1);
        assert_eq!(merged.edges[0].source, "src/server.rs");
        assert_eq!(merged.edges[0].target, "src/server.rs::serde");
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

    #[test]
    fn test_same_function_name_different_files_no_collision() {
        // Bug 1: Two functions with the same name in different files should produce
        // distinct nodes with their own file paths and line numbers.
        let symbols_file1 = vec![make_symbol("new", SymbolKind::Function)];
        let symbols_file2 = vec![make_symbol("new", SymbolKind::Function)];

        let graph1 = build_graph("proj", &symbols_file1, Path::new("src/file1.rs"));
        let graph2 = build_graph("proj", &symbols_file2, Path::new("src/file2.rs"));

        let merged = merge_graphs(vec![graph1, graph2]);

        // Should have 3 nodes: 2 file-level nodes + 2 function nodes
        assert_eq!(merged.nodes.len(), 4);

        // Extract the function nodes (skip file-level nodes at index 0 and 2)
        let func_nodes: Vec<_> = merged
            .nodes
            .iter()
            .filter(|n| n.labels.contains(&"function".to_string()))
            .collect();

        assert_eq!(func_nodes.len(), 2, "Should have 2 function nodes for 'new'");

        // Verify they have different identities (include file path)
        let names: Vec<_> = func_nodes.iter().map(|n| n.name.as_str()).collect();
        assert_ne!(names[0], names[1], "Function nodes should have different names");

        // Verify they retain correct file paths
        assert_eq!(
            func_nodes[0].metadata.get("file_path").unwrap(),
            "src/file1.rs"
        );
        assert_eq!(
            func_nodes[1].metadata.get("file_path").unwrap(),
            "src/file2.rs"
        );
    }

    #[test]
    fn test_public_label_not_applied_to_similar_names() {
        // Bug 3: Symbols named "publish" or "pub_key" should not be marked public.
        let mut sym_publish = make_symbol("publish", SymbolKind::Function);
        sym_publish.signature = Some("fn publish()".into());

        let mut sym_pub_key = make_symbol("pub_key", SymbolKind::Function);
        sym_pub_key.signature = Some("fn pub_key()".into());

        let mut sym_public_api = make_symbol("public_api", SymbolKind::Function);
        sym_public_api.signature = Some("fn public_api()".into());

        let graph = build_graph("proj", &[sym_publish, sym_pub_key, sym_public_api], Path::new("src/lib.rs"));

        // File-level node is at index 0, function symbols start at index 1
        let publish_node = &graph.nodes[1];
        let pub_key_node = &graph.nodes[2];
        let public_api_node = &graph.nodes[3];

        // None of these should have the "public" label
        assert!(
            !publish_node.labels.contains(&"public".to_string()),
            "publish() should not be labeled public"
        );
        assert!(
            !pub_key_node.labels.contains(&"public".to_string()),
            "pub_key() should not be labeled public"
        );
        assert!(
            !public_api_node.labels.contains(&"public".to_string()),
            "public_api() should not be labeled public"
        );
    }

    #[test]
    fn test_actual_public_keyword_adds_label() {
        // Verify that actual "pub " and "pub(" keywords DO add the public label.
        let mut sym_pub_fn = make_symbol("my_public_fn", SymbolKind::Function);
        sym_pub_fn.signature = Some("pub fn my_public_fn()".into());

        let mut sym_pub_crate_fn = make_symbol("my_crate_fn", SymbolKind::Function);
        sym_pub_crate_fn.signature = Some("pub(crate) fn my_crate_fn()".into());

        let graph = build_graph("proj", &[sym_pub_fn, sym_pub_crate_fn], Path::new("src/lib.rs"));

        let pub_fn_node = &graph.nodes[1];
        let pub_crate_fn_node = &graph.nodes[2];

        // Both should have the "public" label
        assert!(
            pub_fn_node.labels.contains(&"public".to_string()),
            "pub fn should be labeled public"
        );
        assert!(
            pub_crate_fn_node.labels.contains(&"public".to_string()),
            "pub(crate) fn should be labeled public"
        );
    }
}
