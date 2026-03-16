use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{json, Value};
use tracing::{debug, error};

use crate::tools::ToolDispatcher;

/// MCP JSON-RPC 2.0 server that reads newline-delimited JSON from stdin
/// and writes responses to stdout.
pub struct McpServer {
    dispatcher: ToolDispatcher,
}

impl McpServer {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            dispatcher: ToolDispatcher::new(project_root),
        }
    }

    /// Run the server loop reading from stdin, writing to stdout.
    pub async fn run(&mut self) -> Result<()> {
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
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "rhizome", "version": "0.1.0" }
        }))
    }

    fn handle_tools_list(&self) -> std::result::Result<Value, JsonRpcError> {
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

        match self.dispatcher.call_tool(name, arguments) {
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
}

struct JsonRpcError {
    code: i32,
    message: String,
}
