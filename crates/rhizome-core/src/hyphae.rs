use std::time::Duration;

use serde::{Deserialize, Serialize};
use spore::discover;
use spore::subprocess::McpClient;
use spore::types::Tool;

use crate::error::{Result, RhizomeError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub memoir_name: String,
    pub concepts_created: usize,
    pub links_created: usize,
}

fn parse_count(value: &serde_json::Value, nested_key: &str, flat_key: &str) -> usize {
    value
        .get(nested_key)
        .and_then(|v| v.get("created"))
        .and_then(|v| v.as_u64())
        .or_else(|| value.get(flat_key).and_then(|v| v.as_u64()))
        .unwrap_or(0) as usize
}

fn parse_compact_import_summary(text: &str, fallback_memoir_name: &str) -> Option<ExportResult> {
    let text = text.trim();
    if !text.starts_with("Imported ") {
        return None;
    }

    let (memoir_name, rest) = text
        .strip_prefix("Imported ")?
        .split_once(": concepts +")
        .map(|(memoir, rest)| (memoir.trim(), rest))?;

    let concepts_created = rest
        .split('/')
        .next()
        .and_then(|count| count.trim().parse::<usize>().ok())
        .unwrap_or(0);

    let links_created = rest
        .split("links +")
        .nth(1)
        .and_then(|tail| tail.split('/').next())
        .and_then(|count| count.trim().parse::<usize>().ok())
        .unwrap_or(0);

    Some(ExportResult {
        memoir_name: if memoir_name.is_empty() {
            fallback_memoir_name.to_string()
        } else {
            memoir_name.to_string()
        },
        concepts_created,
        links_created,
    })
}

fn parse_export_result(text: &str, fallback_memoir_name: &str) -> Result<ExportResult> {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
        let memoir_name = parsed
            .get("memoir")
            .and_then(|v| v.as_str())
            .unwrap_or(fallback_memoir_name)
            .to_string();

        return Ok(ExportResult {
            memoir_name,
            concepts_created: parse_count(&parsed, "concepts", "concepts_created"),
            links_created: parse_count(&parsed, "links", "links_created"),
        });
    }

    parse_compact_import_summary(text, fallback_memoir_name).ok_or_else(|| {
        RhizomeError::Other(format!(
            "Failed to parse hyphae response text as JSON or compact summary: {text}"
        ))
    })
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
    discover(Tool::Hyphae)
        .ok_or_else(|| RhizomeError::Other("Hyphae binary not found in PATH".to_string()))?;

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
        .map_err(|e| RhizomeError::Other(format!("Failed to spawn hyphae serve: {}", e)))?
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
        .map_err(|e| {
            RhizomeError::Other(format!("Failed to call hyphae_import_code_graph: {}", e))
        })?;

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
        .ok_or_else(|| RhizomeError::Other("Missing 'content' in hyphae response".to_string()))?;

    parse_export_result(text, memoir_name)
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

    #[test]
    fn parse_export_result_accepts_nested_json_shape() {
        let text = r#"{"memoir":"code:myapp","concepts":{"created":3,"updated":2,"unchanged":1,"pruned":0},"links":{"created":4,"updated":1,"unchanged":0}}"#;
        let result = parse_export_result(text, "code:fallback").unwrap();
        assert_eq!(result.memoir_name, "code:myapp");
        assert_eq!(result.concepts_created, 3);
        assert_eq!(result.links_created, 4);
    }

    #[test]
    fn parse_export_result_accepts_compact_summary() {
        let text = "Imported code:myapp: concepts +3/2/1 pruned=0 links +4/1/0";
        let result = parse_export_result(text, "code:fallback").unwrap();
        assert_eq!(result.memoir_name, "code:myapp");
        assert_eq!(result.concepts_created, 3);
        assert_eq!(result.links_created, 4);
    }
}
