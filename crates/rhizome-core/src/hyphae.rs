use std::time::Duration;

use serde::{Deserialize, Serialize};
use spore::discover;
use spore::subprocess::McpClient;
use spore::types::Tool;

use crate::error::{Result, RhizomeError};
use crate::export_cache::ExportIdentity;

const CODE_GRAPH_SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub memoir_name: String,
    pub concepts_created: usize,
    pub links_created: usize,
}

#[derive(Debug, Deserialize)]
struct ExportSummary {
    memoir: String,
    concepts: ExportCountSummary,
    links: ExportCountSummary,
}

#[derive(Debug, Deserialize)]
struct ExportCountSummary {
    created: usize,
    #[serde(rename = "updated")]
    _updated: usize,
    #[serde(rename = "unchanged")]
    _unchanged: usize,
    #[serde(default, rename = "pruned")]
    _pruned: usize,
}

fn parse_export_result(text: &str) -> Result<ExportResult> {
    let parsed: ExportSummary = serde_json::from_str(text).map_err(|error| {
        RhizomeError::Other(format!(
            "Hyphae export response must be structured JSON: {error}"
        ))
    })?;

    Ok(ExportResult {
        memoir_name: parsed.memoir,
        concepts_created: parsed.concepts.created,
        links_created: parsed.links.created,
    })
}

/// Check whether the `hyphae` binary is available in PATH.
/// The result is cached after the first call via spore's discovery cache.
pub fn is_available() -> bool {
    discover(Tool::Hyphae).is_some()
}

/// Export a code graph to Hyphae by spawning `hyphae serve` and sending a
/// JSON-RPC request over its stdio transport.
pub fn export_graph(
    graph_json: &serde_json::Value,
    identity: &ExportIdentity,
) -> Result<ExportResult> {
    // ─────────────────────────────────────────────────────────────────────────
    // Verify Hyphae is available
    // ─────────────────────────────────────────────────────────────────────────
    discover(Tool::Hyphae)
        .ok_or_else(|| RhizomeError::Other("Hyphae binary not found in PATH".to_string()))?;

    let project = graph_json
        .get("project")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::String(identity.project.clone()));

    // ─────────────────────────────────────────────────────────────────────────
    // Call Hyphae via McpClient
    // ─────────────────────────────────────────────────────────────────────────
    let mut client = McpClient::spawn(Tool::Hyphae, &["serve"])
        .map_err(|e| RhizomeError::Other(format!("Failed to spawn hyphae serve: {}", e)))?
        .with_timeout(Duration::from_secs(10));

    let result = client
        .call_tool(
            "hyphae_import_code_graph",
            build_import_arguments(&project, graph_json, identity),
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

    parse_export_result(text)
}

fn build_import_arguments(
    project: &serde_json::Value,
    graph_json: &serde_json::Value,
    identity: &ExportIdentity,
) -> serde_json::Value {
    let mut arguments = serde_json::Map::from_iter([
        (
            "schema_version".to_string(),
            serde_json::Value::String(CODE_GRAPH_SCHEMA_VERSION.to_string()),
        ),
        ("project".to_string(), project.clone()),
        ("nodes".to_string(), graph_json["nodes"].clone()),
        ("edges".to_string(), graph_json["edges"].clone()),
    ]);

    arguments.insert(
        "project_root".to_string(),
        serde_json::Value::String(identity.project_root.clone()),
    );

    if let Some(worktree_id) = &identity.worktree_id {
        arguments.insert(
            "worktree_id".to_string(),
            serde_json::Value::String(worktree_id.clone()),
        );
    }

    serde_json::Value::Object(arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity() -> ExportIdentity {
        ExportIdentity {
            project: "myapp".to_string(),
            memoir_name: "code:myapp".to_string(),
            project_root: "/repo/myapp".to_string(),
            worktree_id: Some("wt-alpha".to_string()),
        }
    }

    #[test]
    fn is_available_returns_bool_without_panic() {
        // In CI/test environments hyphae is typically not installed,
        // but the function must not panic regardless.
        let _result: bool = is_available();
    }

    #[test]
    fn jsonrpc_request_format() {
        let graph = serde_json::json!({"project": "myapp", "nodes": [{"id": "1"}], "edges": [{"from": "1", "to": "2"}]});
        let identity = test_identity();
        let request = spore::jsonrpc::Request::new(
            "tools/call",
            serde_json::json!({
                "name": "hyphae_import_code_graph",
                "arguments": build_import_arguments(&graph["project"], &graph, &identity)
            }),
        );

        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "tools/call");
        assert_eq!(request.params["name"], "hyphae_import_code_graph");
        assert_eq!(request.params["arguments"]["schema_version"], "1.0");
        assert_eq!(request.params["arguments"]["project"], "myapp");
        assert_eq!(
            request.params["arguments"]["nodes"],
            serde_json::json!([{"id": "1"}])
        );
        assert_eq!(
            request.params["arguments"]["edges"],
            serde_json::json!([{"from": "1", "to": "2"}])
        );
        assert_eq!(request.params["arguments"]["project_root"], "/repo/myapp");
        assert_eq!(request.params["arguments"]["worktree_id"], "wt-alpha");
        // Verify old nested format is gone
        assert!(request.params["arguments"].get("memoir_name").is_none());
        assert!(request.params["arguments"].get("graph").is_none());
    }

    #[test]
    fn jsonrpc_request_includes_project_root_when_worktree_id_missing() {
        let graph = serde_json::json!({"project": "myapp", "nodes": [], "edges": []});
        let identity = ExportIdentity {
            project: "myapp".to_string(),
            memoir_name: "code:myapp".to_string(),
            project_root: "/repo/myapp".to_string(),
            worktree_id: None,
        };

        let arguments = build_import_arguments(&graph["project"], &graph, &identity);

        assert_eq!(arguments["schema_version"], "1.0");
        assert_eq!(arguments["project"], "myapp");
        assert_eq!(arguments["project_root"], "/repo/myapp");
        assert!(arguments.get("worktree_id").is_none());
    }

    #[test]
    fn jsonrpc_request_extracts_project_from_identity_when_graph_omits_it() {
        let graph = serde_json::json!({"nodes": [], "edges": []});
        let identity = ExportIdentity {
            project: "fallback-app".to_string(),
            memoir_name: "code:fallback-app".to_string(),
            project_root: "/repo/fallback-app".to_string(),
            worktree_id: Some("main".to_string()),
        };
        let project = graph
            .get("project")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::String(identity.project.clone()));

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
        let identity = ExportIdentity {
            project: "test".to_string(),
            memoir_name: "code:test".to_string(),
            project_root: "/repo/test".to_string(),
            worktree_id: Some("main".to_string()),
        };
        let result = export_graph(&graph, &identity);
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
        let result = parse_export_result(text).unwrap();
        assert_eq!(result.memoir_name, "code:myapp");
        assert_eq!(result.concepts_created, 3);
        assert_eq!(result.links_created, 4);
    }

    #[test]
    fn parse_export_result_rejects_legacy_compact_summary() {
        let text = "Imported code:myapp: concepts +3/2/1 pruned=0 links +4/1/0";
        let err = parse_export_result(text).unwrap_err();
        assert!(err.to_string().contains("structured JSON"));
    }

    #[test]
    fn parse_export_result_rejects_flat_legacy_json_shape() {
        let text = r#"{"memoir":"code:myapp","concepts_created":3,"links_created":4}"#;
        let err = parse_export_result(text).unwrap_err();
        assert!(err.to_string().contains("structured JSON"));
    }
}
