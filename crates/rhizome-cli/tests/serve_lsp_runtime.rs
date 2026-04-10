use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn write_mock_lsp_script(dir: &Path) -> PathBuf {
    let script = dir.join("mock_lsp.py");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json
import sys

def read_message():
    headers = {}
    while True:
        line = sys.stdin.readline()
        if not line:
            return None
        if line.strip() == "":
            break
        if "Content-Length:" in line:
            headers["Content-Length"] = int(line.split("Content-Length:", 1)[1].strip())
    length = headers.get("Content-Length", 0)
    if length == 0:
        return None
    return json.loads(sys.stdin.read(length))

def send_message(msg):
    body = json.dumps(msg)
    sys.stdout.write(f"Content-Length: {len(body)}\r\n\r\n{body}")
    sys.stdout.flush()

while True:
    msg = read_message()
    if msg is None:
        break

    method = msg.get("method", "")
    msg_id = msg.get("id")

    if method == "initialize":
        send_message({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "capabilities": {
                    "referencesProvider": True,
                    "diagnosticProvider": True
                }
            }
        })
    elif method == "initialized":
        pass
    elif method == "textDocument/references":
        send_message({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": []
        })
    elif method == "shutdown":
        send_message({"jsonrpc": "2.0", "id": msg_id, "result": None})
    elif method == "exit":
        break
"#,
    )
    .unwrap();
    script
}

fn spawn_serve_process(project_root: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_rhizome"))
        .args(["serve", "--expanded", "--project"])
        .arg(project_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn rhizome serve")
}

fn send_json(stdin: &mut ChildStdin, value: &Value) {
    let payload = serde_json::to_string(value).expect("serialize JSON-RPC request");
    writeln!(stdin, "{payload}").expect("write request");
    stdin.flush().expect("flush request");
}

fn read_json_line(stdout: &mut BufReader<ChildStdout>) -> Value {
    let mut line = String::new();
    let bytes = stdout.read_line(&mut line).expect("read response");
    assert!(bytes > 0, "serve process closed stdout unexpectedly");
    serde_json::from_str(line.trim()).expect("parse JSON response")
}

#[test]
fn serve_expanded_handles_lsp_tools_without_runtime_panic() {
    if !python3_available() {
        eprintln!("Skipping serve regression: python3 is not available");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("project");
    let config_dir = project_root.join(".rhizome");
    let source_path = project_root.join("crates/rhizome-treesitter/src/parser.rs");
    let script_path = write_mock_lsp_script(tmp.path());

    fs::create_dir_all(&config_dir).unwrap();
    fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    fs::create_dir_all(project_root.join(".git")).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!(
            r#"
[languages.rust]
server_binary = "python3"
server_args = ["{}"]
"#,
            script_path.display()
        ),
    )
    .unwrap();
    fs::write(&source_path, "pub fn parse() {}\n").unwrap();

    let mut child = spawn_serve_process(&project_root);
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let stderr = child.stderr.take().expect("serve stderr");

    let stderr_handle = std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut collected = String::new();
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => collected.push_str(&line),
                Err(err) => {
                    collected.push_str(&format!("stderr read error: {err}\n"));
                    break;
                }
            }
        }
        collected
    });

    let mut stdout = BufReader::new(stdout);

    send_json(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
    );
    let initialize = read_json_line(&mut stdout);
    assert_eq!(
        initialize.get("id").and_then(|value| value.as_i64()),
        Some(1)
    );
    assert!(
        initialize.get("error").is_none(),
        "initialize should succeed"
    );

    send_json(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "get_diagnostics",
                "arguments": {
                    "file": source_path.display().to_string()
                }
            }
        }),
    );
    let diagnostics = read_json_line(&mut stdout);
    assert_eq!(
        diagnostics.get("id").and_then(|value| value.as_i64()),
        Some(2)
    );
    assert!(
        diagnostics.get("error").is_none(),
        "get_diagnostics should succeed"
    );
    assert!(
        diagnostics
            .get("result")
            .and_then(|result| result.get("content"))
            .and_then(|content| content.as_array())
            .is_some(),
        "get_diagnostics should return a structured MCP result"
    );

    send_json(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "find_references",
                "arguments": {
                    "file": source_path.display().to_string(),
                    "line": 1,
                    "column": 11
                }
            }
        }),
    );
    let references = read_json_line(&mut stdout);
    assert_eq!(
        references.get("id").and_then(|value| value.as_i64()),
        Some(3)
    );
    assert!(
        references.get("error").is_none(),
        "find_references should succeed"
    );
    assert!(
        references
            .get("result")
            .and_then(|result| result.get("content"))
            .and_then(|content| content.as_array())
            .is_some(),
        "find_references should return a structured MCP result"
    );

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    let stderr = stderr_handle.join().unwrap();
    assert!(
        !stderr.contains("Cannot start a runtime from within a runtime"),
        "serve stderr should not contain the runtime nesting panic: {stderr}"
    );
}
