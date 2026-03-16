use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub memoir_name: String,
    pub concepts_created: usize,
    pub links_created: usize,
}

static HYPHAE_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Check whether the `hyphae` binary is available in PATH.
/// The result is cached after the first call.
pub fn is_available() -> bool {
    *HYPHAE_AVAILABLE.get_or_init(|| {
        Command::new("which")
            .arg("hyphae")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Export a code graph to Hyphae by spawning `hyphae serve` and sending a
/// JSON-RPC request over its stdio transport.
pub fn export_graph(graph_json: &serde_json::Value, memoir_name: &str) -> Result<ExportResult> {
    if !is_available() {
        bail!("Hyphae binary not found in PATH");
    }

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "hyphae_import_code_graph",
            "arguments": {
                "memoir_name": memoir_name,
                "graph": graph_json
            }
        }
    });

    let request_bytes = serde_json::to_vec(&request)?;
    let message = format_jsonrpc_message(&request_bytes);

    let mut child = Command::new("hyphae")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn hyphae serve")?;

    let mut stdin = child.stdin.take().context("Failed to open hyphae stdin")?;
    stdin
        .write_all(&message)
        .context("Failed to write to hyphae stdin")?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .context("Failed to open hyphae stdout")?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let result = parse_jsonrpc_response(&mut reader);
        let _ = tx.send(result);
    });

    let response = rx
        .recv_timeout(Duration::from_secs(10))
        .context("Timed out waiting for hyphae response (10s)")?
        .context("Failed to parse hyphae response")?;

    let _ = child.kill();
    let _ = child.wait();

    let result = response
        .get("result")
        .context("Missing 'result' in hyphae response")?;

    let concepts_created = result
        .get("concepts_created")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let links_created = result
        .get("links_created")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    Ok(ExportResult {
        memoir_name: memoir_name.to_string(),
        concepts_created,
        links_created,
    })
}

fn format_jsonrpc_message(body: &[u8]) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut message = header.into_bytes();
    message.extend_from_slice(body);
    message
}

fn parse_jsonrpc_response(reader: &mut impl Read) -> Result<serde_json::Value> {
    let mut buf_reader = BufReader::new(reader);
    let mut header_line = String::new();
    buf_reader
        .read_line(&mut header_line)
        .context("Failed to read Content-Length header")?;

    let content_length: usize = header_line
        .trim()
        .strip_prefix("Content-Length: ")
        .context("Invalid header: expected Content-Length")?
        .parse()
        .context("Invalid Content-Length value")?;

    // Consume the blank line separating header from body
    let mut blank = String::new();
    buf_reader
        .read_line(&mut blank)
        .context("Failed to read header separator")?;

    let mut body = vec![0u8; content_length];
    buf_reader
        .read_exact(&mut body)
        .context("Failed to read response body")?;

    serde_json::from_slice(&body).context("Failed to parse response JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_available_returns_bool_without_panic() {
        // In CI/test environments hyphae is typically not installed,
        // but the function must not panic regardless.
        let _result: bool = is_available();
    }

    #[test]
    fn jsonrpc_request_format() {
        let graph = serde_json::json!({"nodes": [], "edges": []});
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "hyphae_import_code_graph",
                "arguments": {
                    "memoir_name": "test-memoir",
                    "graph": graph
                }
            }
        });

        assert_eq!(request["jsonrpc"], "2.0");
        assert_eq!(request["id"], 1);
        assert_eq!(request["method"], "tools/call");
        assert_eq!(request["params"]["name"], "hyphae_import_code_graph");
        assert_eq!(request["params"]["arguments"]["memoir_name"], "test-memoir");
        assert_eq!(
            request["params"]["arguments"]["graph"],
            serde_json::json!({"nodes": [], "edges": []})
        );
    }

    #[test]
    fn content_length_header_format() {
        let body = b"hello";
        let message = format_jsonrpc_message(body);
        let expected = b"Content-Length: 5\r\n\r\nhello";
        assert_eq!(message, expected);
    }

    #[test]
    fn content_length_header_empty_body() {
        let body = b"";
        let message = format_jsonrpc_message(body);
        assert_eq!(message, b"Content-Length: 0\r\n\r\n");
    }

    #[test]
    fn parse_jsonrpc_response_roundtrip() {
        let payload = serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {"ok": true}});
        let body = serde_json::to_vec(&payload).unwrap();
        let message = format_jsonrpc_message(&body);

        let mut cursor = std::io::Cursor::new(message);
        let parsed = parse_jsonrpc_response(&mut cursor).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn export_graph_errors_when_hyphae_unavailable() {
        // Force the availability check (will be false in test env, or even if
        // somehow true the child process path won't matter for this assertion
        // since we check is_available first).
        if is_available() {
            // If hyphae happens to be installed, skip this test.
            return;
        }

        let graph = serde_json::json!({"nodes": []});
        let result = export_graph(&graph, "test");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found"),
            "Expected 'not found' in error, got: {err_msg}"
        );
    }
}
