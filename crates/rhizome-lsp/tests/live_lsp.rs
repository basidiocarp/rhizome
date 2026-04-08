//! Live LSP integration tests.
//!
//! These tests require an actual language server installed (e.g. rust-analyzer).
//! Run with: `cargo test -p rhizome-lsp --test live_lsp -- --ignored`

use std::path::PathBuf;
use std::process::Stdio;

use rhizome_core::{CodeIntelligence, LanguageServerConfig, SymbolKind};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[tokio::test]
#[ignore]
async fn test_rust_analyzer_go_to_definition() {
    if !rust_analyzer_available() {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    }

    let config = LanguageServerConfig {
        binary: "rust-analyzer".to_string(),
        args: vec![],
        initialization_options: None,
    };

    let workspace = project_root();
    let mut client = rhizome_lsp::client::LspClient::spawn(&config, Some(&workspace))
        .await
        .expect("Failed to spawn rust-analyzer");

    client
        .initialize(&workspace)
        .await
        .expect("Initialize failed");

    // Give rust-analyzer a moment to index
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Request document symbols for our own backend.rs
    let file = workspace.join("crates/rhizome-core/src/backend.rs");
    let response = client
        .document_symbols(&file)
        .await
        .expect("documentSymbol failed");

    assert!(
        response.is_some(),
        "rust-analyzer should return document symbols"
    );

    match response.unwrap() {
        lsp_types::DocumentSymbolResponse::Nested(symbols) => {
            let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
            assert!(
                names.contains(&"CodeIntelligence"),
                "Should find CodeIntelligence trait: {names:?}"
            );
        }
        lsp_types::DocumentSymbolResponse::Flat(infos) => {
            let names: Vec<&str> = infos.iter().map(|s| s.name.as_str()).collect();
            assert!(
                names.contains(&"CodeIntelligence"),
                "Should find CodeIntelligence trait: {names:?}"
            );
        }
    }

    client.shutdown().await.expect("Shutdown failed");
}

#[tokio::test]
#[ignore]
async fn test_lsp_backend_get_symbols() {
    if !rust_analyzer_available() {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    }

    let workspace = project_root();
    let handle = tokio::runtime::Handle::current();
    let backend = rhizome_lsp::LspBackend::new(workspace.clone(), handle);

    // Use spawn_blocking since LspBackend::get_symbols uses block_on internally
    let file = workspace.join("crates/rhizome-core/src/symbol.rs");
    let symbols = tokio::task::spawn_blocking(move || backend.get_symbols(&file))
        .await
        .expect("spawn_blocking failed")
        .expect("get_symbols failed");

    assert!(!symbols.is_empty(), "Should find symbols in symbol.rs");

    let has_symbol_kind = symbols
        .iter()
        .any(|s| s.name == "SymbolKind" && s.kind == SymbolKind::Enum);
    assert!(
        has_symbol_kind,
        "Should find SymbolKind enum: {:?}",
        symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}
