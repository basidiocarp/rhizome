use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use spore::{discover, Tool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub memoir_name: String,
    pub concepts_created: usize,
    pub links_created: usize,
}

/// Check whether the `hyphae` binary is available in PATH.
/// The result is cached after the first call via spore's discovery cache.
pub fn is_available() -> bool {
    discover(Tool::Hyphae).is_some()
}

/// Export a code graph to Hyphae by spawning `hyphae serve` and sending a
/// JSON-RPC request over its stdio transport.
pub fn export_graph(graph_json: &serde_json::Value, memoir_name: &str) -> Result<ExportResult> {
    if !is_available() {
        bail!("Hyphae binary not found in PATH");
    }

    let request = spore::jsonrpc::Request::new(
        "tools/call",
        serde_json::json!({
            "name": "hyphae_import_code_graph",
            "arguments": {
                "memoir_name": memoir_name,
                "graph": graph_json
            }
        }),
    );

    // Hyphae's MCP server reads line-delimited JSON (one JSON object per line),
    // so we serialize without Content-Length framing.
    let message =
        serde_json::to_string(&request).context("Failed to serialize JSON-RPC request")? + "\n";

    let mut child = Command::new("hyphae")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn hyphae serve")?;

    let mut stdin = child.stdin.take().context("Failed to open hyphae stdin")?;
    stdin
        .write_all(message.as_bytes())
        .context("Failed to write to hyphae stdin")?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .context("Failed to open hyphae stdout")?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let result = read_line_delimited_response(reader);
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

/// Read a single line-delimited JSON response, skipping empty lines.
fn read_line_delimited_response(reader: impl BufRead) -> Result<serde_json::Value> {
    for line in reader.lines() {
        let line = line.context("Failed to read line from hyphae stdout")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return serde_json::from_str(trimmed).context("Failed to parse response JSON");
    }
    bail!("No response received from hyphae")
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
        let request = spore::jsonrpc::Request::new(
            "tools/call",
            serde_json::json!({
                "name": "hyphae_import_code_graph",
                "arguments": {
                    "memoir_name": "test-memoir",
                    "graph": graph
                }
            }),
        );

        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "tools/call");
        assert_eq!(request.params["name"], "hyphae_import_code_graph");
        assert_eq!(request.params["arguments"]["memoir_name"], "test-memoir");
        assert_eq!(
            request.params["arguments"]["graph"],
            serde_json::json!({"nodes": [], "edges": []})
        );
    }

    #[test]
    fn line_delimited_response_parsing() {
        let payload = serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {"ok": true}});
        let line = serde_json::to_string(&payload).unwrap() + "\n";

        let cursor = std::io::Cursor::new(line);
        let reader = BufReader::new(cursor);
        let parsed = read_line_delimited_response(reader).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn line_delimited_response_skips_empty_lines() {
        let payload = serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": null});
        let input = format!("\n\n{}\n", serde_json::to_string(&payload).unwrap());

        let cursor = std::io::Cursor::new(input);
        let reader = BufReader::new(cursor);
        let parsed = read_line_delimited_response(reader).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn line_delimited_response_errors_on_empty_input() {
        let cursor = std::io::Cursor::new("");
        let reader = BufReader::new(cursor);
        let result = read_line_delimited_response(reader);
        assert!(result.is_err());
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
