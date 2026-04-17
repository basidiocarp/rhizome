use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{Value, json};
use spore::logging::{SpanContext, request_span, tool_span, workflow_span};
use tracing::Instrument;
use tracing::{debug, error, info, warn};

use crate::tools::ToolDispatcher;

/// MCP JSON-RPC 2.0 server that reads newline-delimited JSON from stdin
/// and writes responses to stdout.
pub struct McpServer {
    dispatcher: ToolDispatcher,
    unified: bool,
}

impl McpServer {
    pub fn new(project_root: PathBuf, unified: bool) -> Self {
        Self {
            dispatcher: ToolDispatcher::new(project_root),
            unified,
        }
    }

    /// Run the server loop reading from stdin, writing to stdout.
    pub async fn run(&mut self) -> Result<()> {
        self.spawn_auto_export();

        let stdin = io::stdin();
        let stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to read stdin: {e}");
                    break;
                }
            };

            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            let request: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    let err_response = json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {e}")
                        }
                    });
                    Self::write_response(&stdout, &err_response)?;
                    continue;
                }
            };

            let id = request.get("id").cloned();
            let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
            let request_context = self.request_context(id.as_ref());
            let _request_span = request_span(method, &request_context).entered();

            debug!("Received method: {method}");

            // Notifications have no id and need no response
            if method == "notifications/initialized" {
                continue;
            }

            let response = self.handle_method(method, &request, id.clone());
            if let Some(resp) = response {
                Self::write_response(&stdout, &resp)?;
            }
        }

        Ok(())
    }

    fn spawn_auto_export(&self) {
        let project_root = self.dispatcher.project_root().to_path_buf();
        let span_context =
            SpanContext::for_app("rhizome").with_workspace_root(project_root.display().to_string());
        let workflow_span = workflow_span("auto_export_to_hyphae", &span_context);

        tokio::spawn(
            async move {
                if !rhizome_core::hyphae::is_available() {
                    debug!("Hyphae not available, skipping auto-export");
                    return;
                }

                let config = rhizome_core::RhizomeConfig::load(&project_root).unwrap_or_default();
                if !config.auto_export() {
                    debug!("Auto-export disabled in config");
                    return;
                }

                info!("rhizome: starting auto-export to hyphae");
                let backend = rhizome_treesitter::TreeSitterBackend::new();
                let args = serde_json::json!({});
                let backoff_seconds = [1_u64, 4, 16];

                for (attempt_idx, delay_seconds) in backoff_seconds.iter().copied().enumerate() {
                    match crate::tools::export_tools::export_to_hyphae(
                        &backend,
                        &args,
                        &project_root,
                    ) {
                        Ok(result) => {
                            if let Some(text) = result
                                .get("content")
                                .and_then(|c| c.as_array())
                                .and_then(|a| a.first())
                                .and_then(|o| o.get("text"))
                                .and_then(|t| t.as_str())
                            {
                                info!("rhizome: hyphae auto-export complete: {text}");
                            }
                            return;
                        }
                        Err(error) => {
                            debug!(
                                "rhizome: hyphae auto-export attempt {} failed: {error}",
                                attempt_idx + 1
                            );
                            tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds))
                                .await;
                        }
                    }
                }

                match crate::tools::export_tools::export_to_hyphae(&backend, &args, &project_root) {
                    Ok(result) => {
                        if let Some(text) = result
                            .get("content")
                            .and_then(|c| c.as_array())
                            .and_then(|a| a.first())
                            .and_then(|o| o.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            info!("rhizome: hyphae auto-export complete: {text}");
                        }
                    }
                    Err(error) => {
                        warn!("rhizome: hyphae auto-export failed after 4 attempts: {error}");
                    }
                }
            }
            .instrument(workflow_span),
        );
    }

    fn handle_method(&self, method: &str, request: &Value, id: Option<Value>) -> Option<Value> {
        let result = match method {
            "initialize" => self.handle_initialize(),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                self.handle_tools_call(&params, id.as_ref())
            }
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {method}"),
            }),
        };

        let id = id.unwrap_or(Value::Null);

        Some(match result {
            Ok(result_value) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result_value
            }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": e.code,
                    "message": e.message
                }
            }),
        })
    }

    fn handle_initialize(&self) -> std::result::Result<Value, JsonRpcError> {
        let instructions = "Rhizome provides code intelligence — symbol extraction, definitions, references, diagnostics, impact analysis, and repo-understanding exports. Top tools: get_symbols (file overview), get_definition (symbol source), find_references (cross-file), analyze_impact (change blast radius), search_symbols (global search), get_diagnostics (errors/warnings), and get_region (expand one structural region). Most tools require an absolute file path. Use get_structure for project overview. Use export_to_hyphae to push code graphs to Hyphae for persistent knowledge, or export_repo_understanding for a typed repo-understanding artifact with explicit update class and repo-surface summary.";
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "rhizome", "version": env!("CARGO_PKG_VERSION"), "instructions": instructions }
        }))
    }

    fn handle_tools_list(&self) -> std::result::Result<Value, JsonRpcError> {
        if self.unified {
            let tool_schema = json!([{
                "name": "rhizome",
                "description": "Code intelligence tool. Commands: get_symbols, get_structure, get_definition, search_symbols, find_references, analyze_impact, go_to_definition, get_signature, get_imports, get_call_sites, get_scope, get_exports, summarize_file, get_tests, get_diff_symbols, get_annotations, get_complexity, get_type_definitions, get_dependencies, get_parameters, get_enclosing_class, get_symbol_body, get_region, get_changed_files, rename_symbol, get_diagnostics, get_hover_info, replace_symbol_body, insert_after_symbol, insert_before_symbol, replace_lines, insert_at_line, delete_lines, create_file, copy_symbol, move_symbol, export_to_hyphae, export_repo_understanding",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "The command to run (e.g. get_symbols, get_definition, summarize_file)" },
                        "file": { "type": "string", "description": "Path to source file" },
                        "symbol": { "type": "string", "description": "Symbol name (for get_definition, get_signature, etc.)" },
                        "region_id": { "type": "string", "description": "Structural region id for get_region" },
                        "pattern": { "type": "string", "description": "Search pattern (for search_symbols)" },
                        "line": { "type": "number", "description": "Line number, 0-based" },
                        "column": { "type": "number", "description": "Column number, 0-based" },
                        "function": { "type": "string", "description": "Function name filter" },
                        "method": { "type": "string", "description": "Method name (for get_enclosing_class)" },
                        "path": { "type": "string", "description": "Directory path override" },
                        "depth": { "type": "number", "description": "Max nesting depth" },
                        "full": { "type": "boolean", "description": "Show full body" },
                        "new_name": { "type": "string", "description": "New name for rename" },
                        "ref1": { "type": "string", "description": "Git ref for diff start" },
                        "ref2": { "type": "string", "description": "Git ref for diff end" },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Annotation tags to search for" },
                        "new_body": { "type": "string", "description": "New content for replace_symbol_body" },
                        "content": { "type": "string", "description": "Content for insert/replace/create operations" },
                        "source_file": { "type": "string", "description": "Path to the source file for copy_symbol or move_symbol" },
                        "target_file": { "type": "string", "description": "Path to the target file for copy_symbol or move_symbol" },
                        "target_symbol": { "type": "string", "description": "Target symbol name for copy_symbol or move_symbol" },
                        "position": { "type": "string", "description": "Insert position relative to target symbol: before or after" },
                        "start_line": { "type": "number", "description": "Start line (1-based) for line operations" },
                        "end_line": { "type": "number", "description": "End line (1-based, inclusive) for line operations" },
                        "overwrite": { "type": "boolean", "description": "Allow overwriting existing files in create_file" }
                    },
                    "required": ["command"]
                }
            }]);
            return Ok(json!({ "tools": tool_schema }));
        }

        let tools = self.dispatcher.list_tools();
        let tool_schemas: Vec<Value> = tools
            .into_iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                    "annotations": t.annotations
                })
            })
            .collect();

        Ok(json!({ "tools": tool_schemas }))
    }

    fn handle_tools_call(
        &self,
        params: &Value,
        request_id: Option<&Value>,
    ) -> std::result::Result<Value, JsonRpcError> {
        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing 'name' in tools/call params".to_string(),
            })?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        // In unified mode, the tool name is "rhizome" and the actual command
        // is in the "command" argument.
        let effective_name = if self.unified && name == "rhizome" {
            arguments
                .get("command")
                .and_then(|c| c.as_str())
                .ok_or_else(|| JsonRpcError {
                    code: -32602,
                    message: "Missing 'command' in rhizome tool arguments".to_string(),
                })?
                .to_string()
        } else {
            name.to_string()
        };

        let tool_context = self
            .request_context(request_id)
            .with_tool(effective_name.clone());
        let _tool_span = tool_span(&effective_name, &tool_context).entered();

        match self.dispatcher.call_tool(&effective_name, arguments) {
            Ok(result) => Ok(result),
            Err(e) => Ok(json!({
                "isError": true,
                "content": [{ "type": "text", "text": format!("Error: {e}") }]
            })),
        }
    }

    fn write_response(stdout: &io::Stdout, response: &Value) -> Result<()> {
        let mut out = stdout.lock();
        serde_json::to_writer(&mut out, response)?;
        out.write_all(b"\n")?;
        out.flush()?;
        Ok(())
    }

    /// Process a single JSON-RPC request and return the response.
    /// Exposed for integration testing.
    pub fn handle_request_for_test(&self, request: &Value) -> Value {
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        match self.handle_method(method, request, id.clone()) {
            Some(resp) => resp,
            None => json!({"jsonrpc": "2.0", "id": id, "result": null}),
        }
    }

    fn base_span_context(&self) -> SpanContext {
        SpanContext::for_app("rhizome")
            .with_workspace_root(self.dispatcher.project_root().display().to_string())
    }

    fn request_context(&self, request_id: Option<&Value>) -> SpanContext {
        let context = self.base_span_context();
        match request_id_from_value(request_id) {
            Some(request_id) => context.with_request_id(request_id),
            None => context,
        }
    }
}

struct JsonRpcError {
    code: i32,
    message: String,
}

fn request_id_from_value(request_id: Option<&Value>) -> Option<String> {
    match request_id? {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        value => Some(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_id_from_value_supports_jsonrpc_scalars() {
        assert_eq!(
            request_id_from_value(Some(&json!("req-42"))),
            Some("req-42".into())
        );
        assert_eq!(request_id_from_value(Some(&json!(42))), Some("42".into()));
        assert_eq!(
            request_id_from_value(Some(&json!(true))),
            Some("true".into())
        );
        assert_eq!(request_id_from_value(Some(&Value::Null)), None);
        assert_eq!(request_id_from_value(None), None);
    }

    #[test]
    fn request_context_carries_workspace_root_and_request_id() {
        let server = McpServer::new(PathBuf::from("/tmp/project"), true);
        let context = server.request_context(Some(&json!(7)));

        assert_eq!(context.service.as_deref(), Some("rhizome"));
        assert_eq!(context.request_id.as_deref(), Some("7"));
        assert_eq!(context.workspace_root.as_deref(), Some("/tmp/project"));
    }

    #[test]
    fn tool_schemas_include_annotations() {
        let server = McpServer::new(PathBuf::from("/tmp/project"), false);
        let tools = server.dispatcher.list_tools();

        // Verify we have tools
        assert!(!tools.is_empty(), "Should have at least one tool");

        // Find a read-only tool and an edit tool
        let read_only_tool = tools
            .iter()
            .find(|t| t.name == "get_symbols")
            .expect("get_symbols tool should exist");

        let edit_tool = tools
            .iter()
            .find(|t| t.name == "replace_symbol_body")
            .expect("replace_symbol_body tool should exist");

        // Verify read-only tool annotations
        assert_eq!(read_only_tool.annotations.read_only_hint, true);
        assert_eq!(read_only_tool.annotations.destructive_hint, false);
        assert_eq!(read_only_tool.annotations.idempotent_hint, true);

        // Verify edit tool annotations
        assert_eq!(edit_tool.annotations.read_only_hint, false);
        assert_eq!(edit_tool.annotations.destructive_hint, false);
        assert_eq!(edit_tool.annotations.idempotent_hint, false);

        // Verify that annotations serialize to JSON correctly
        let serialized = serde_json::to_value(&read_only_tool.annotations)
            .expect("Should serialize annotations to JSON");
        assert_eq!(
            serialized.get("readOnlyHint").and_then(|v| v.as_bool()),
            Some(true),
            "readOnlyHint should be present in JSON"
        );
    }
}
