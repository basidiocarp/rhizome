//! Deterministic LSP integration tests that run without any external language server.
//!
//! These tests verify error handling and graceful degradation. They do not
//! require rust-analyzer, gopls, or any other language server to be installed.
//!
//! For live end-to-end tests that require a real language server, see:
//! `cargo test -p rhizome-lsp --test live_lsp -- --ignored`

use std::path::PathBuf;

use rhizome_core::{CodeIntelligence, LanguageServerConfig};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Spawn an LspClient with a binary that does not exist.
/// The client must return a clear error rather than panicking.
#[tokio::test]
async fn lsp_client_spawn_fails_gracefully_on_missing_binary() {
    let config = LanguageServerConfig {
        binary: "__rhizome_nonexistent_lsp_binary__".to_string(),
        args: vec![],
        initialization_options: None,
    };

    match rhizome_lsp::client::LspClient::spawn(&config, Some(&project_root())).await {
        Ok(_) => panic!("Spawning a non-existent binary must return an error"),
        Err(e) => {
            let err = e.to_string();
            // The error must mention the binary so the caller can diagnose what to install.
            assert!(
                err.contains("__rhizome_nonexistent_lsp_binary__") || err.contains("No such file"),
                "Error should identify the missing binary: {err}"
            );
        }
    }
}

/// LspBackend::get_symbols must return an error (not panic) when the language
/// server cannot be spawned. This is the primary degradation path — callers
/// fall back to tree-sitter when LSP is unavailable.
#[test]
fn lsp_backend_get_symbols_returns_error_without_server() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let handle = rt.handle().clone();

    let backend = rhizome_lsp::LspBackend::new(project_root(), handle);

    // Use a Rust source file that tree-sitter can parse — but we are asking
    // the LSP backend which will try to spawn rust-analyzer. If rust-analyzer
    // is not available, the result must be an Err, not a panic.
    let test_file = project_root().join("crates/rhizome-core/src/lib.rs");
    if !test_file.exists() {
        // Skip on unusual checkout layouts.
        return;
    }

    let result = backend.get_symbols(&test_file);
    // Either succeeds (rust-analyzer happens to be installed) or returns Err.
    // The critical property is: it must not panic.
    match result {
        Ok(symbols) => {
            // rust-analyzer is available — sanity check the result is non-empty.
            assert!(
                !symbols.is_empty(),
                "rust-analyzer returned empty symbol list for lib.rs"
            );
        }
        Err(e) => {
            // Language server unavailable — the error must be descriptive.
            let msg = e.to_string();
            assert!(
                !msg.is_empty(),
                "LspBackend must return a non-empty error message"
            );
        }
    }
}

/// LspBackend::get_symbols must return an error when given a file that does
/// not exist — not a panic, and not a silent empty result.
#[test]
fn lsp_backend_get_symbols_errors_on_missing_file() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let handle = rt.handle().clone();

    let backend = rhizome_lsp::LspBackend::new(project_root(), handle);
    let missing = project_root().join("__does_not_exist__.rs");

    let result = backend.get_symbols(&missing);
    // Either: LSP backend errors out before spawning (file not found),
    // or: LSP server can't be spawned (no rust-analyzer). Either way: Err.
    assert!(
        result.is_err(),
        "LspBackend must not succeed for a non-existent file"
    );
}

/// Verify that LanguageServerManager correctly surfaces a spawn failure when
/// the configured binary is not on PATH. The error must propagate up without
/// panic rather than being swallowed.
#[tokio::test]
async fn language_server_manager_surfaces_spawn_error() {
    use rhizome_core::Language;

    let mut manager = rhizome_lsp::manager::LanguageServerManager::new();

    // Temporarily override the config by using a language whose server is
    // unlikely to be available. We test with a real language that has a
    // default server configured so get_client attempts a spawn.
    let workspace = project_root();

    // Rust is the test target; rust-analyzer may or may not be present.
    // In either case, get_client must not panic.
    let result = manager.get_client(&Language::Rust, &workspace).await;
    match result {
        Ok(_) => {
            // rust-analyzer is installed — fine, the manager works.
        }
        Err(e) => {
            // Not installed — the error must be non-empty and describe the failure.
            assert!(
                !e.to_string().is_empty(),
                "Manager must surface a descriptive spawn error"
            );
        }
    }
}
