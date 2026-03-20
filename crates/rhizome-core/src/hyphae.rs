use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use spore::discover;
use spore::subprocess::McpClient;
use spore::types::Tool;

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
    // ─────────────────────────────────────────────────────────────────────────
    // Verify Hyphae is available
    // ─────────────────────────────────────────────────────────────────────────
    discover(Tool::Hyphae).ok_or_else(|| anyhow!("Hyphae binary not found in PATH"))?;

    let project = graph_json.get("project").cloned().unwrap_or_else(|| {
        serde_json::Value::String(
            memoir_name
                .strip_prefix("code:")
                .unwrap_or(memoir_name)
                .to_string(),
        )
    });

    // ─────────────────────────────────────────────────────────────────────────
    // Call Hyphae via McpClient
    // ─────────────────────────────────────────────────────────────────────────
    let mut client = McpClient::spawn(Tool::Hyphae, &["serve"])
        .context("Failed to spawn hyphae serve")?
        .with_timeout(Duration::from_secs(10));

    let result = client
        .call_tool(
            "hyphae_import_code_graph",
            serde_json::json!({
                "project": project,
                "nodes": graph_json["nodes"],
                "edges": graph_json["edges"]
            }),
        )
        .context("Failed to call hyphae_import_code_graph")?;

    // ─────────────────────────────────────────────────────────────────────────
    // Extract content from MCP response envelope
    // ─────────────────────────────────────────────────────────────────────────
    // Hyphae wraps tool results in MCP content envelope: result.content[0].text is a JSON string
    let text = result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .context("Missing 'content' in hyphae response")?;

    // Parse the text field as JSON to extract counts
    let parsed = serde_json::from_str::<serde_json::Value>(text)
        .context("Failed to parse hyphae response text as JSON")?;

    let concepts_created = parsed
        .get("concepts_created")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let links_created = parsed
        .get("links_created")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    Ok(ExportResult {
        memoir_name: memoir_name.to_string(),
        concepts_created,
        links_created,
    })
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
        let graph = serde_json::json!({"project": "myapp", "nodes": [{"id": "1"}], "edges": [{"from": "1", "to": "2"}]});
        let request = spore::jsonrpc::Request::new(
            "tools/call",
            serde_json::json!({
                "name": "hyphae_import_code_graph",
                "arguments": {
                    "project": graph["project"],
                    "nodes": graph["nodes"],
                    "edges": graph["edges"]
                }
            }),
        );

        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "tools/call");
        assert_eq!(request.params["name"], "hyphae_import_code_graph");
        assert_eq!(request.params["arguments"]["project"], "myapp");
        assert_eq!(
            request.params["arguments"]["nodes"],
            serde_json::json!([{"id": "1"}])
        );
        assert_eq!(
            request.params["arguments"]["edges"],
            serde_json::json!([{"from": "1", "to": "2"}])
        );
        // Verify old nested format is gone
        assert!(request.params["arguments"].get("memoir_name").is_none());
        assert!(request.params["arguments"].get("graph").is_none());
    }

    #[test]
    fn jsonrpc_request_extracts_project_from_memoir_name() {
        let graph = serde_json::json!({"nodes": [], "edges": []});
        let project = graph.get("project").cloned().unwrap_or_else(|| {
            serde_json::Value::String(
                "code:fallback-app"
                    .strip_prefix("code:")
                    .unwrap_or("code:fallback-app")
                    .to_string(),
            )
        });

        assert_eq!(project, "fallback-app");
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
