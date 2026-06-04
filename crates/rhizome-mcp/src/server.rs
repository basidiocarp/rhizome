use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use serde_json::{Value, json};
use spore::logging::{SpanContext, request_span, tool_span, workflow_span};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::Instrument;
use tracing::{debug, info, warn};

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

        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        run_mcp_over_stream(reader, stdout, &self.dispatcher, self.unified).await
    }

    fn spawn_auto_export(&self) {
        // Gate auto-export with environment variable
        if std::env::var("RHIZOME_AUTO_EXPORT").as_deref() != Ok("1") {
            debug!("Auto-export disabled (set RHIZOME_AUTO_EXPORT=1 to enable)");
            return;
        }

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
                let args = serde_json::json!({});
                let delays = [1_u64, 4, 16];
                let max_idx = delays.len() - 1;

                for (attempt, &delay) in delays.iter().enumerate() {
                    let project_root_clone = project_root.clone();
                    let args_clone = args.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        let backend = rhizome_treesitter::TreeSitterBackend::new();
                        crate::tools::export_tools::export_to_hyphae(
                            &backend,
                            &args_clone,
                            &project_root_clone,
                        )
                    })
                    .await;

                    match result {
                        Ok(Ok(export_result)) => {
                            if let Some(text) = export_result
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
                        Ok(Err(error)) => {
                            debug!(
                                "rhizome: hyphae auto-export attempt {} failed: {error}",
                                attempt + 1
                            );
                            if attempt < max_idx {
                                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                            }
                        }
                        Err(error) => {
                            debug!(
                                "rhizome: hyphae auto-export attempt {} join failed: {error}",
                                attempt + 1
                            );
                            if attempt < max_idx {
                                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                            }
                        }
                    }
                }

                // Final attempt after exhausting all retries
                let project_root_clone = project_root.clone();
                let args_clone = args.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let backend = rhizome_treesitter::TreeSitterBackend::new();
                    crate::tools::export_tools::export_to_hyphae(
                        &backend,
                        &args_clone,
                        &project_root_clone,
                    )
                })
                .await;

                match result {
                    Ok(Ok(export_result)) => {
                        if let Some(text) = export_result
                            .get("content")
                            .and_then(|c| c.as_array())
                            .and_then(|a| a.first())
                            .and_then(|o| o.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            info!("rhizome: hyphae auto-export complete: {text}");
                        }
                    }
                    Ok(Err(error)) => {
                        warn!("rhizome: hyphae auto-export failed after 4 attempts: {error}");
                    }
                    Err(error) => {
                        warn!("rhizome: hyphae auto-export final attempt join failed: {error}");
                    }
                }
            }
            .instrument(workflow_span),
        );
    }

    fn handle_method(&self, method: &str, request: &Value, id: Option<Value>) -> Option<Value> {
        handle_method_impl(method, request, id, &self.dispatcher, self.unified)
    }

    #[allow(dead_code)]
    fn handle_initialize(&self) -> std::result::Result<Value, JsonRpcError> {
        let instructions = "Rhizome provides code intelligence — symbol extraction, definitions, references, diagnostics, impact analysis, and repo-understanding exports. Top tools: get_symbols (file overview), get_definition (symbol source), find_references (cross-file), analyze_impact (change blast radius), search_symbols (global search), get_diagnostics (errors/warnings), and get_region (expand one structural region). Most tools require an absolute file path. Use get_structure for project overview. Use export_to_hyphae to push code graphs to Hyphae for persistent knowledge, or export_repo_understanding for a typed repo-understanding artifact with explicit update class and repo-surface summary.";
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "rhizome", "version": env!("CARGO_PKG_VERSION"), "instructions": instructions }
        }))
    }

    #[allow(dead_code)]
    fn handle_tools_list(&self) -> std::result::Result<Value, JsonRpcError> {
        if self.unified {
            let tool_schema = json!([{
                "name": "rhizome",
                "description": "Code intelligence tool. Commands: get_symbols, get_structure, get_definition, search_symbols, find_references, analyze_impact, go_to_definition, get_signature, get_imports, get_call_sites, get_scope, get_exports, summarize_file, get_tests, get_diff_symbols, get_annotations, get_complexity, get_type_definitions, get_dependencies, get_parameters, get_enclosing_class, get_symbol_body, get_region, get_changed_files, get_chunk_boundaries, rename_symbol, get_diagnostics, rhizome_onboard, rhizome_simulate_change, summarize_project, replace_symbol_body, insert_after_symbol, insert_before_symbol, replace_lines, insert_at_line, delete_lines, create_file, copy_symbol, move_symbol, export_to_hyphae, export_repo_understanding",
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
                        "path": { "type": "string", "description": "Directory path override. For search_symbols, pass an explicit nested-repo path (e.g. a subproject dir) to reach symbols the root index skips — the workspace index respects the root .gitignore, which prunes nested repos." },
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn base_span_context(&self) -> SpanContext {
        SpanContext::for_app("rhizome")
            .with_workspace_root(self.dispatcher.project_root().display().to_string())
    }

    #[allow(dead_code)]
    fn request_context(&self, request_id: Option<&Value>) -> SpanContext {
        let context = self.base_span_context();
        match request_id_from_value(request_id) {
            Some(request_id) => context.with_request_id(request_id),
            None => context,
        }
    }
}

/// Transport-agnostic MCP handler that works over any async reader/writer pair.
/// Handles JSON-RPC 2.0 newline-delimited protocol with idle timeout.
pub async fn run_mcp_over_stream<R, W>(
    mut reader: R,
    mut writer: W,
    dispatcher: &ToolDispatcher,
    unified: bool,
) -> Result<()>
where
    R: tokio::io::AsyncBufRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut line = String::new();

    // RHIZOME_IDLE_TIMEOUT_SECS controls how long the server waits for a
    // request before exiting. Default is 14400s (4 hours), covering a full
    // work session without leaking indefinitely. Set to 0 to disable.
    let idle_timeout_secs = std::env::var("RHIZOME_IDLE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(14400);
    // 0 means no timeout; map to a duration large enough to never fire.
    let idle_timeout = Duration::from_secs(if idle_timeout_secs == 0 {
        u64::MAX / 2
    } else {
        idle_timeout_secs
    });

    loop {
        line.clear();
        let n = match tokio::time::timeout(idle_timeout, reader.read_line(&mut line)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                info!(
                    "rhizome: idle timeout ({} secs) — exiting",
                    idle_timeout_secs
                );
                break;
            }
        };

        if n == 0 {
            info!("rhizome: MCP transport closed — stdin EOF");
            break;
        }

        let line_trimmed = line.trim().to_string();
        if line_trimmed.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line_trimmed) {
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
                write_response_async(&mut writer, &err_response).await?;
                continue;
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let base_context = SpanContext::for_app("rhizome")
            .with_workspace_root(dispatcher.project_root().display().to_string());
        let request_context = match request_id_from_value(id.as_ref()) {
            Some(request_id) => base_context.with_request_id(request_id),
            None => base_context,
        };

        {
            let _request_span = request_span(method, &request_context).entered();
            debug!("Received method: {method}");
        }

        if id.is_none() {
            tracing::debug!(method = %method, "received notification");
            continue;
        }

        let response = handle_method_impl(method, &request, id.clone(), dispatcher, unified);
        if let Some(resp) = response {
            write_response_async(&mut writer, &resp).await?;
        }
    }

    Ok(())
}

fn handle_method_impl(
    method: &str,
    request: &Value,
    id: Option<Value>,
    dispatcher: &ToolDispatcher,
    unified: bool,
) -> Option<Value> {
    let result = match method {
        "initialize" => handle_initialize_impl(),
        "tools/list" => handle_tools_list_impl(dispatcher, unified),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            handle_tools_call_impl(&params, id.as_ref(), dispatcher, unified)
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

fn handle_initialize_impl() -> std::result::Result<Value, JsonRpcError> {
    let instructions = "Rhizome provides code intelligence — symbol extraction, definitions, references, diagnostics, impact analysis, and repo-understanding exports. Top tools: get_symbols (file overview), get_definition (symbol source), find_references (cross-file), analyze_impact (change blast radius), search_symbols (global search), get_diagnostics (errors/warnings), and get_region (expand one structural region). Most tools require an absolute file path. Use get_structure for project overview. Use export_to_hyphae to push code graphs to Hyphae for persistent knowledge, or export_repo_understanding for a typed repo-understanding artifact with explicit update class and repo-surface summary.";
    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "rhizome", "version": env!("CARGO_PKG_VERSION"), "instructions": instructions }
    }))
}

fn handle_tools_list_impl(
    dispatcher: &ToolDispatcher,
    unified: bool,
) -> std::result::Result<Value, JsonRpcError> {
    if unified {
        let tool_schema = json!([{
            "name": "rhizome",
            "description": "Code intelligence tool. Commands: get_symbols, get_structure, get_definition, search_symbols, find_references, analyze_impact, go_to_definition, get_signature, get_imports, get_call_sites, get_scope, get_exports, summarize_file, get_tests, get_diff_symbols, get_annotations, get_complexity, get_type_definitions, get_dependencies, get_parameters, get_enclosing_class, get_symbol_body, get_region, get_changed_files, get_chunk_boundaries, rename_symbol, get_diagnostics, rhizome_onboard, rhizome_simulate_change, summarize_project, replace_symbol_body, insert_after_symbol, insert_before_symbol, replace_lines, insert_at_line, delete_lines, create_file, copy_symbol, move_symbol, export_to_hyphae, export_repo_understanding",
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
                    "path": { "type": "string", "description": "Directory path override. For search_symbols, pass an explicit nested-repo path (e.g. a subproject dir) to reach symbols the root index skips — the workspace index respects the root .gitignore, which prunes nested repos." },
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

    let tools = dispatcher.list_tools();
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

fn handle_tools_call_impl(
    params: &Value,
    request_id: Option<&Value>,
    dispatcher: &ToolDispatcher,
    unified: bool,
) -> std::result::Result<Value, JsonRpcError> {
    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing 'name' in tools/call params".to_string(),
        })?;

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let effective_name = if unified && name == "rhizome" {
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

    let base_context = SpanContext::for_app("rhizome")
        .with_workspace_root(dispatcher.project_root().display().to_string());
    let tool_context = match request_id_from_value(request_id) {
        Some(request_id) => base_context.with_request_id(request_id),
        None => base_context,
    }
    .with_tool(effective_name.clone());
    let _tool_span = tool_span(&effective_name, &tool_context).entered();

    match dispatcher.call_tool(&effective_name, arguments) {
        Ok(result) => Ok(result),
        Err(e) => Ok(json!({
            "isError": true,
            "content": [{ "type": "text", "text": format!("Error: {e}") }]
        })),
    }
}

async fn write_response_async<W>(writer: &mut W, response: &Value) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut buf = serde_json::to_vec(response)?;
    buf.push(b'\n');
    writer.write_all(&buf).await?;
    writer.flush().await?;
    Ok(())
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
        assert!(read_only_tool.annotations.read_only_hint);
        assert!(!read_only_tool.annotations.destructive_hint);
        assert!(read_only_tool.annotations.idempotent_hint);

        // Verify edit tool annotations
        assert!(!edit_tool.annotations.read_only_hint);
        assert!(!edit_tool.annotations.destructive_hint);
        assert!(!edit_tool.annotations.idempotent_hint);

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
