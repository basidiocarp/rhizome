use std::path::PathBuf;

use rhizome_core::backend::CodeIntelligence;
use rhizome_core::graph::build_graph;
use rhizome_treesitter::TreeSitterBackend;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn test_build_graph_from_rust_fixture() {
    let backend = TreeSitterBackend::new();
    let path = fixture_path("sample.rs");
    let symbols = backend.get_symbols(&path).expect("Should extract symbols");
    assert!(!symbols.is_empty(), "Should extract at least one symbol");

    let graph = build_graph("test-project", &symbols, &path);

    assert_eq!(graph.project, "test-project");
    assert!(
        graph.nodes.len() >= symbols.len(),
        "Graph should have at least as many nodes as top-level symbols (nodes={}, symbols={})",
        graph.nodes.len(),
        symbols.len()
    );

    let names: Vec<&str> = graph.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.iter().any(|n| n.ends_with("::Config")),
        "Should contain Config struct: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n.ends_with("::process")),
        "Should contain process function: {:?}",
        names
    );
}

#[test]
fn test_build_graph_from_python_fixture() {
    let backend = TreeSitterBackend::new();
    let path = fixture_path("sample.py");
    let symbols = backend.get_symbols(&path).expect("Should extract symbols");
    assert!(!symbols.is_empty(), "Should extract at least one symbol");

    let graph = build_graph("test-project", &symbols, &path);

    assert_eq!(graph.project, "test-project");
    assert!(
        graph.nodes.len() >= symbols.len(),
        "Graph should have at least as many nodes as top-level symbols"
    );

    // Verify language metadata is set correctly
    for node in &graph.nodes {
        assert_eq!(
            node.metadata.get("language").and_then(|v| v.as_str()),
            Some("python"),
            "Python fixture nodes should have language=python"
        );
    }
}

#[test]
fn test_build_graph_from_typescript_fixture() {
    let backend = TreeSitterBackend::new();
    let path = fixture_path("sample.ts");
    let symbols = backend
        .get_symbols(&path)
        .expect("Should extract TypeScript symbols");
    assert!(!symbols.is_empty(), "Should extract at least one symbol");

    let graph = build_graph("test-project", &symbols, &path);

    let names: Vec<&str> = graph.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.iter().any(|n| n.ends_with("::HttpClient")),
        "Should contain HttpClient class: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n.ends_with("::createClient")),
        "Should contain createClient function: {:?}",
        names
    );

    for node in &graph.nodes {
        assert_eq!(
            node.metadata.get("language").and_then(|v| v.as_str()),
            Some("typescript"),
            "TypeScript fixture nodes should have language=typescript"
        );
    }
}
