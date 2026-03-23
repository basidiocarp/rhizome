//! Mock LSP server integration test.
//!
//! Spawns a tiny Python script as a fake language server that responds to
//! `initialize` and `textDocument/documentSymbol` with canned JSON-RPC,
//! then verifies `LspClient` parses the responses correctly.

use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;

use rhizome_core::LanguageServerConfig;

/// Create a temporary Python script that acts as a minimal LSP server.
/// It reads JSON-RPC requests from stdin and replies with canned responses.
fn write_mock_server_script(dir: &std::path::Path) -> PathBuf {
    let script = dir.join("mock_lsp.py");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(
        f,
        r#"#!/usr/bin/env python3
"""Minimal mock LSP server that responds to initialize, documentSymbol, and rename."""
import json, sys

NOISY_STDOUT = "--noisy" in sys.argv[1:]
noisy_banner_written = False

def read_message():
    headers = {{}}
    while True:
        line = sys.stdin.readline()
        if not line or line.strip() == "":
            break
        if "Content-Length:" in line:
            headers["Content-Length"] = int(line.split("Content-Length:", 1)[1].strip())
    length = headers.get("Content-Length", 0)
    if length == 0:
        return None
    body = sys.stdin.read(length)
    return json.loads(body)

def send_message(msg):
    global noisy_banner_written
    body = json.dumps(msg)
    if NOISY_STDOUT and not noisy_banner_written:
        sys.stdout.write("mock server booting... ")
        noisy_banner_written = True
    sys.stdout.write(f"Content-Length: {{len(body)}}\r\n\r\n{{body}}")
    sys.stdout.flush()

while True:
    msg = read_message()
    if msg is None:
        break

    method = msg.get("method", "")
    msg_id = msg.get("id")

    if method == "initialize":
        send_message({{
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {{
                "capabilities": {{
                    "documentSymbolProvider": True,
                    "definitionProvider": True,
                }}
            }}
        }})
    elif method == "initialized":
        pass  # notification, no response
    elif method == "textDocument/documentSymbol":
        send_message({{
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": [
                {{
                    "name": "MockStruct",
                    "kind": 23,
                    "range": {{"start": {{"line": 0, "character": 0}}, "end": {{"line": 5, "character": 1}}}},
                    "selectionRange": {{"start": {{"line": 0, "character": 4}}, "end": {{"line": 0, "character": 14}}}},
                    "children": [
                        {{
                            "name": "mock_method",
                            "kind": 6,
                            "range": {{"start": {{"line": 2, "character": 4}}, "end": {{"line": 4, "character": 5}}}},
                            "selectionRange": {{"start": {{"line": 2, "character": 7}}, "end": {{"line": 2, "character": 18}}}},
                            "children": []
                        }}
                    ]
                }},
                {{
                    "name": "mock_function",
                    "kind": 12,
                    "range": {{"start": {{"line": 7, "character": 0}}, "end": {{"line": 9, "character": 1}}}},
                    "selectionRange": {{"start": {{"line": 7, "character": 3}}, "end": {{"line": 7, "character": 16}}}},
                    "children": []
                }}
            ]
        }})
    elif method == "textDocument/rename":
        params = msg.get("params", {{}})
        new_name = params.get("newName", "renamed_symbol")
        text_uri = params.get("textDocument", {{}}).get("uri")
        send_message({{
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {{
                "changes": {{
                    text_uri: [
                        {{
                            "range": {{
                                "start": {{"line": 0, "character": 3}},
                                "end": {{"line": 0, "character": 6}}
                            }},
                            "newText": new_name
                        }}
                    ]
                }}
            }}
        }})
    elif method == "shutdown":
        send_message({{"jsonrpc": "2.0", "id": msg_id, "result": None}})
    elif method == "exit":
        break
"#
    )
    .unwrap();
    script
}

#[tokio::test]
async fn test_mock_lsp_initialize_and_document_symbols() {
    // Check that python3 is available
    let python_check = std::process::Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if python_check.is_err() || !python_check.unwrap().success() {
        eprintln!("Skipping mock LSP test: python3 not available");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let script_path = write_mock_server_script(tmp.path());

    let config = LanguageServerConfig {
        binary: "python3".to_string(),
        args: vec![script_path.to_string_lossy().to_string()],
        initialization_options: None,
    };

    // Spawn the mock server via LspClient
    let mut client = rhizome_lsp::client::LspClient::spawn(&config)
        .await
        .expect("Failed to spawn mock server");

    // Initialize
    let workspace = tmp.path().to_path_buf();
    client
        .initialize(&workspace)
        .await
        .expect("Initialize failed");

    // Request document symbols for a fake file
    let fake_file = tmp.path().join("test.rs");
    std::fs::write(&fake_file, "// placeholder").unwrap();

    let response = client
        .document_symbols(&fake_file)
        .await
        .expect("documentSymbol request failed");

    assert!(response.is_some(), "Expected document symbols response");

    match response.unwrap() {
        lsp_types::DocumentSymbolResponse::Nested(symbols) => {
            assert_eq!(symbols.len(), 2, "Expected 2 top-level symbols");

            assert_eq!(symbols[0].name, "MockStruct");
            assert_eq!(symbols[0].kind, lsp_types::SymbolKind::STRUCT);
            assert_eq!(symbols[0].children.as_ref().map(|c| c.len()), Some(1));
            assert_eq!(symbols[0].children.as_ref().unwrap()[0].name, "mock_method");

            assert_eq!(symbols[1].name, "mock_function");
            assert_eq!(symbols[1].kind, lsp_types::SymbolKind::FUNCTION);
        }
        lsp_types::DocumentSymbolResponse::Flat(_) => {
            panic!("Expected nested document symbols, got flat");
        }
    }

    // Shut down
    client.shutdown().await.expect("Shutdown failed");
}

#[tokio::test]
async fn test_mock_lsp_handles_noisy_stdout_before_first_response() {
    // Check that python3 is available
    let python_check = std::process::Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if python_check.is_err() || !python_check.unwrap().success() {
        eprintln!("Skipping mock LSP test: python3 not available");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let script_path = write_mock_server_script(tmp.path());

    let config = LanguageServerConfig {
        binary: "python3".to_string(),
        args: vec![
            script_path.to_string_lossy().to_string(),
            "--noisy".to_string(),
        ],
        initialization_options: None,
    };

    let mut client = rhizome_lsp::client::LspClient::spawn(&config)
        .await
        .expect("Failed to spawn noisy mock server");

    let workspace = tmp.path().to_path_buf();
    client
        .initialize(&workspace)
        .await
        .expect("Initialize failed");

    let fake_file = tmp.path().join("test.rs");
    std::fs::write(&fake_file, "// placeholder").unwrap();

    let response = client
        .document_symbols(&fake_file)
        .await
        .expect("documentSymbol request failed");

    assert!(response.is_some(), "Expected document symbols response");
    client.shutdown().await.expect("Shutdown failed");
}

#[tokio::test]
async fn test_mock_lsp_rename_returns_workspace_edit_that_can_be_applied() {
    let python_check = std::process::Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if python_check.is_err() || !python_check.unwrap().success() {
        eprintln!("Skipping mock LSP test: python3 not available");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let script_path = write_mock_server_script(tmp.path());

    let config = LanguageServerConfig {
        binary: "python3".to_string(),
        args: vec![script_path.to_string_lossy().to_string()],
        initialization_options: None,
    };

    let mut client = rhizome_lsp::client::LspClient::spawn(&config)
        .await
        .expect("Failed to spawn mock server");

    client
        .initialize(tmp.path())
        .await
        .expect("Initialize failed");

    let fake_file = tmp.path().join("test.rs");
    std::fs::write(&fake_file, "fn old() {}\n").unwrap();

    let edit = client
        .rename(
            &fake_file,
            lsp_types::Position {
                line: 0,
                character: 3,
            },
            "new_name",
        )
        .await
        .expect("rename request failed")
        .expect("rename should return a workspace edit");

    let apply_result = rhizome_lsp::edit::apply_workspace_edit(&edit).unwrap();
    assert_eq!(apply_result.files_modified, 1);
    assert_eq!(apply_result.edits_applied, 1);
    assert_eq!(
        std::fs::read_to_string(&fake_file).unwrap(),
        "fn new_name() {}\n"
    );

    client.shutdown().await.expect("Shutdown failed");
}
