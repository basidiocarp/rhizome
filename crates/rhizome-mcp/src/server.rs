use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{json, Value};
use tracing::{debug, error, info};

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
        tokio::spawn(async move {
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
                Err(e) => {
                    debug!("rhizome: hyphae auto-export failed: {e}");
                }
            }
        });
    }

    fn handle_method(&self, method: &str, request: &Value, id: Option<Value>) -> Option<Value> {
        let result = match method {
            "initialize" => self.handle_initialize(),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                self.handle_tools_call(&params)
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
        let instructions = "Rhizome provides code intelligence — symbol extraction, definitions, references, diagnostics, and impact analysis. Top tools: get_symbols (file overview), get_definition (symbol source), find_references (cross-file), analyze_impact (change blast radius), search_symbols (global search), get_diagnostics (errors/warnings). Most tools require an absolute file path. Use get_structure for project overview. Use export_to_hyphae to push code graphs to Hyphae for persistent knowledge.";
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "rhizome", "version": "0.4.0", "instructions": instructions }
        }))
    }

    fn handle_tools_list(&self) -> std::result::Result<Value, JsonRpcError> {
        if self.unified {
            let tool_schema = json!([{
                "name": "rhizome",
                "description": "Code intelligence tool. Commands: get_symbols, get_structure, get_definition, search_symbols, find_references, analyze_impact, go_to_definition, get_signature, get_imports, get_call_sites, get_scope, get_exports, summarize_file, get_tests, get_diff_symbols, get_annotations, get_complexity, get_type_definitions, get_dependencies, get_parameters, get_enclosing_class, get_symbol_body, get_changed_files, rename_symbol, get_diagnostics, get_hover_info, replace_symbol_body, insert_after_symbol, insert_before_symbol, replace_lines, insert_at_line, delete_lines, create_file, copy_symbol, move_symbol, export_to_hyphae",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "The command to run (e.g. get_symbols, get_definition, summarize_file)" },
                        "file": { "type": "string", "description": "Path to source file" },
                        "symbol": { "type": "string", "description": "Symbol name (for get_definition, get_signature, etc.)" },
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
                        "memoir": { "type": "string", "description": "Override memoir name for export" },
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
                    "inputSchema": t.input_schema
                })
            })
            .collect();

        Ok(json!({ "tools": tool_schemas }))
    }

    fn handle_tools_call(&self, params: &Value) -> std::result::Result<Value, JsonRpcError> {
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
}

struct JsonRpcError {
    code: i32,
    message: String,
}
