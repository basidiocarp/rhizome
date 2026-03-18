use std::path::PathBuf;

use rhizome_mcp::tools::ToolDispatcher;
use serde_json::{json, Value};

fn fixture_path(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
        .to_string_lossy()
        .to_string()
}

fn make_dispatcher() -> ToolDispatcher {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    ToolDispatcher::new(project_root)
}

// ---------------------------------------------------------------------------
// list_tools
// ---------------------------------------------------------------------------

#[test]
fn test_list_tools_returns_25_tools() {
    let dispatcher = make_dispatcher();
    let tools = dispatcher.list_tools();
    assert_eq!(tools.len(), 35, "Expected 35 tools, got {}", tools.len());

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"get_symbols"));
    assert!(names.contains(&"get_structure"));
    assert!(names.contains(&"get_definition"));
    assert!(names.contains(&"search_symbols"));
    assert!(names.contains(&"find_references"));
    assert!(names.contains(&"go_to_definition"));
    assert!(names.contains(&"get_signature"));
    assert!(names.contains(&"get_imports"));
    assert!(names.contains(&"get_call_sites"));
    assert!(names.contains(&"rename_symbol"));
    assert!(names.contains(&"get_diagnostics"));
    assert!(names.contains(&"get_hover_info"));
    // Batch 1 tools
    assert!(names.contains(&"get_scope"));
    assert!(names.contains(&"get_exports"));
    assert!(names.contains(&"summarize_file"));
    assert!(names.contains(&"get_tests"));
    assert!(names.contains(&"get_diff_symbols"));
    assert!(names.contains(&"get_annotations"));
    assert!(names.contains(&"get_complexity"));
    assert!(names.contains(&"get_type_definitions"));
    // Batch 2 tools
    assert!(names.contains(&"get_dependencies"));
    assert!(names.contains(&"get_parameters"));
    assert!(names.contains(&"get_enclosing_class"));
    assert!(names.contains(&"get_symbol_body"));
    assert!(names.contains(&"get_changed_files"));
}

#[test]
fn test_tool_schemas_have_required_fields() {
    let dispatcher = make_dispatcher();
    let tools = dispatcher.list_tools();

    for tool in &tools {
        assert!(!tool.name.is_empty(), "Tool name should not be empty");
        assert!(
            !tool.description.is_empty(),
            "Tool {} description should not be empty",
            tool.name
        );
        assert!(
            tool.input_schema.get("type").is_some(),
            "Tool {} should have type in schema",
            tool.name
        );
        assert!(
            tool.input_schema.get("properties").is_some(),
            "Tool {} should have properties in schema",
            tool.name
        );
    }
}

// ---------------------------------------------------------------------------
// get_symbols
// ---------------------------------------------------------------------------

#[test]
fn test_get_symbols_rust() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool("get_symbols", json!({ "file": fixture_path("sample.rs") }))
        .expect("get_symbols should succeed");

    let text = extract_text(&result);
    assert!(text.contains("Config"), "Should find Config symbol");
    assert!(text.contains("process"), "Should find process function");
    assert!(text.contains("MAX_SIZE"), "Should find MAX_SIZE constant");
    assert!(text.contains("Status"), "Should find Status enum");
}

#[test]
fn test_get_symbols_python() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool("get_symbols", json!({ "file": fixture_path("sample.py") }))
        .expect("get_symbols should succeed");

    let text = extract_text(&result);
    assert!(text.contains("Config"), "Should find Config class");
    assert!(text.contains("process"), "Should find process function");
}

// ---------------------------------------------------------------------------
// get_definition
// ---------------------------------------------------------------------------

#[test]
fn test_get_definition_known_symbol() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_definition",
            json!({ "file": fixture_path("sample.rs"), "symbol": "Config" }),
        )
        .expect("get_definition should succeed");

    let text = extract_text(&result);
    assert!(text.contains("Config"), "Should contain Config");
    assert!(text.contains("Struct"), "Should identify kind as Struct");
}

#[test]
fn test_get_definition_missing_symbol() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_definition",
            json!({ "file": fixture_path("sample.rs"), "symbol": "NonExistent" }),
        )
        .expect("get_definition should succeed even for missing symbols");

    let text = extract_text(&result);
    assert!(text.contains("not found"), "Should report symbol not found");
}

// ---------------------------------------------------------------------------
// get_imports
// ---------------------------------------------------------------------------

#[test]
fn test_get_imports_rust() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool("get_imports", json!({ "file": fixture_path("sample.rs") }))
        .expect("get_imports should succeed");

    let text = extract_text(&result);
    // sample.rs has: use std::collections::HashMap; use std::path::PathBuf;
    assert!(
        text.contains("HashMap") || text.contains("PathBuf") || text.contains("std"),
        "Should find import statements: {text}"
    );
}

#[test]
fn test_get_imports_python() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool("get_imports", json!({ "file": fixture_path("sample.py") }))
        .expect("get_imports should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("os") || text.contains("Path") || text.contains("pathlib"),
        "Should find import statements: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_structure
// ---------------------------------------------------------------------------

#[test]
fn test_get_structure() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_structure",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_structure should succeed");

    let text = extract_text(&result);
    assert!(text.contains("Struct Config"), "Should show Config struct");
    assert!(
        text.contains("Function process"),
        "Should show process function"
    );
}

// ---------------------------------------------------------------------------
// get_signature
// ---------------------------------------------------------------------------

#[test]
fn test_get_signature() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_signature",
            json!({ "file": fixture_path("sample.rs"), "symbol": "process" }),
        )
        .expect("get_signature should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("fn process"),
        "Should contain function signature: {text}"
    );
}

// ---------------------------------------------------------------------------
// find_references
// ---------------------------------------------------------------------------

#[test]
fn test_find_references() {
    let dispatcher = make_dispatcher();
    // "Config" is at line 2, column 11 (0-based) in sample.rs
    let result = dispatcher
        .call_tool(
            "find_references",
            json!({ "file": fixture_path("sample.rs"), "line": 2, "column": 11 }),
        )
        .expect("find_references should succeed");

    let text = extract_text(&result);
    // Should find multiple references to Config
    assert!(
        text.contains("line_start"),
        "Should return reference locations: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_diagnostics (tree-sitter: empty)
// ---------------------------------------------------------------------------

#[test]
fn test_get_diagnostics_treesitter_empty() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_diagnostics",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_diagnostics should succeed");

    let text = extract_text(&result);
    // Tree-sitter returns no diagnostics
    assert_eq!(
        text.trim(),
        "[]",
        "Tree-sitter should return empty diagnostics"
    );
}

// ---------------------------------------------------------------------------
// LSP-only tools: proper error message
// ---------------------------------------------------------------------------

#[test]
fn test_rename_symbol_no_lsp() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "rename_symbol",
            json!({ "file": fixture_path("sample.rs"), "line": 2, "column": 11, "new_name": "Settings" }),
        )
        .expect("rename_symbol should succeed (returning error message)");

    assert!(
        result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "Should return isError=true"
    );
    let text = extract_text(&result);
    assert!(
        text.contains("LSP") && text.contains("require"),
        "Should indicate LSP is required: {text}"
    );
}

#[test]
fn test_get_hover_info_no_lsp() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_hover_info",
            json!({ "file": fixture_path("sample.rs"), "line": 2, "column": 11 }),
        )
        .expect("get_hover_info should succeed (returning error message)");

    assert!(
        result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "Should return isError=true"
    );
    let text = extract_text(&result);
    assert!(
        text.contains("LSP") && text.contains("require"),
        "Should indicate LSP is required: {text}"
    );
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[test]
fn test_missing_file_error() {
    let dispatcher = make_dispatcher();
    let result = dispatcher.call_tool(
        "get_symbols",
        json!({ "file": "/nonexistent/path/to/file.rs" }),
    );

    assert!(result.is_err(), "Should return error for missing file");
}

#[test]
fn test_unknown_tool_error() {
    let dispatcher = make_dispatcher();
    let result = dispatcher.call_tool("nonexistent_tool", json!({}));
    assert!(result.is_err(), "Should return error for unknown tool");
}

#[test]
fn test_missing_required_param() {
    let dispatcher = make_dispatcher();
    // get_symbols requires "file" param
    let result = dispatcher.call_tool("get_symbols", json!({}));
    assert!(
        result.is_err(),
        "Should return error for missing required param"
    );
}

// ---------------------------------------------------------------------------
// get_call_sites
// ---------------------------------------------------------------------------

#[test]
fn test_get_call_sites() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_call_sites",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_call_sites should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("format") || text.contains("Config"),
        "Should find at least some call sites: {text}"
    );
}

// ---------------------------------------------------------------------------
// go_to_definition
// ---------------------------------------------------------------------------

#[test]
fn test_go_to_definition() {
    let dispatcher = make_dispatcher();
    // "Config" at line 2, column 11 in sample.rs
    let result = dispatcher
        .call_tool(
            "go_to_definition",
            json!({ "file": fixture_path("sample.rs"), "line": 2, "column": 11 }),
        )
        .expect("go_to_definition should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("Config"),
        "Should find Config definition: {text}"
    );
}

// ---------------------------------------------------------------------------
// search_symbols
// ---------------------------------------------------------------------------

#[test]
fn test_search_symbols() {
    let dispatcher = make_dispatcher();
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .to_string_lossy()
        .to_string();

    let result = dispatcher
        .call_tool(
            "search_symbols",
            json!({ "pattern": "Config", "path": fixtures_dir }),
        )
        .expect("search_symbols should succeed");

    let text = extract_text(&result);
    assert!(text.contains("Config"), "Should find Config: {text}");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

// ---------------------------------------------------------------------------
// get_scope
// ---------------------------------------------------------------------------

#[test]
fn test_get_scope_inside_function() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_scope",
            json!({ "file": fixture_path("sample.rs"), "line": 21 }),
        )
        .expect("get_scope should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("process"),
        "Should find process function as scope: {text}"
    );
}

#[test]
fn test_get_scope_top_level() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_scope",
            json!({ "file": fixture_path("sample.rs"), "line": 24 }),
        )
        .expect("get_scope should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("Top-level") || text.contains("MAX_SIZE"),
        "Should return top-level or constant scope: {text}"
    );
}

#[test]
fn test_get_scope_inside_impl() {
    let dispatcher = make_dispatcher();
    // Line 10 is inside Config::new method
    let result = dispatcher
        .call_tool(
            "get_scope",
            json!({ "file": fixture_path("sample.rs"), "line": 10 }),
        )
        .expect("get_scope should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("new"),
        "Should find the new method as innermost scope: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_exports
// ---------------------------------------------------------------------------

#[test]
fn test_get_exports_rust() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool("get_exports", json!({ "file": fixture_path("sample.rs") }))
        .expect("get_exports should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("Config"),
        "Should find pub struct Config: {text}"
    );
    assert!(
        text.contains("process"),
        "Should find pub fn process: {text}"
    );
    assert!(
        text.contains("Status"),
        "Should find pub enum Status: {text}"
    );
    // internal_helper is not pub, should not appear
    assert!(
        !text.contains("internal_helper"),
        "Should not include private internal_helper: {text}"
    );
}

#[test]
fn test_get_exports_python() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool("get_exports", json!({ "file": fixture_path("sample.py") }))
        .expect("get_exports should succeed for Python");

    let text = extract_text(&result);
    assert!(text.contains("Config"), "Should find Config class: {text}");
    assert!(
        text.contains("process"),
        "Should find process function: {text}"
    );
}

// ---------------------------------------------------------------------------
// summarize_file
// ---------------------------------------------------------------------------

#[test]
fn test_summarize_file() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "summarize_file",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("summarize_file should succeed");

    let text = extract_text(&result);
    assert!(text.contains("Config"), "Should include Config: {text}");
    assert!(text.contains("process"), "Should include process: {text}");
    // Should be compact - no function bodies
    assert!(
        !text.contains("format!"),
        "Should not include function body details: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_tests
// ---------------------------------------------------------------------------

#[test]
fn test_get_tests_finds_rust_tests() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool("get_tests", json!({ "file": fixture_path("sample.rs") }))
        .expect("get_tests should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("test_config_new"),
        "Should find test_config_new: {text}"
    );
    assert!(
        text.contains("test_process"),
        "Should find test_process: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_annotations
// ---------------------------------------------------------------------------

#[test]
fn test_get_annotations_finds_todos() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_annotations",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_annotations should succeed");

    let text = extract_text(&result);
    assert!(text.contains("TODO"), "Should find TODO comment: {text}");
    assert!(text.contains("FIXME"), "Should find FIXME comment: {text}");
}

#[test]
fn test_get_annotations_custom_tags() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_annotations",
            json!({ "file": fixture_path("sample.rs"), "tags": ["FIXME"] }),
        )
        .expect("get_annotations should succeed");

    let text = extract_text(&result);
    assert!(text.contains("FIXME"), "Should find FIXME: {text}");
    // TODO should not appear when only filtering for FIXME
    assert!(
        !text.contains("TODO"),
        "Should not find TODO when filtering for FIXME only: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_complexity
// ---------------------------------------------------------------------------

#[test]
fn test_get_complexity() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_complexity",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_complexity should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("complex_logic"),
        "Should analyze complex_logic: {text}"
    );
    assert!(
        text.contains("complexity"),
        "Should include complexity scores: {text}"
    );
}

#[test]
fn test_get_complexity_single_function() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_complexity",
            json!({ "file": fixture_path("sample.rs"), "function": "complex_logic" }),
        )
        .expect("get_complexity should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("complex_logic"),
        "Should find complex_logic: {text}"
    );

    // complex_logic has multiple if/else/match/for/|| branches, should be > 1
    let parsed: Vec<Value> = serde_json::from_str(&text).unwrap_or_default();
    assert!(!parsed.is_empty(), "Should return at least one result");
    if let Some(complexity) = parsed[0].get("complexity").and_then(|v| v.as_u64()) {
        assert!(
            complexity > 1,
            "complex_logic should have complexity > 1, got {complexity}"
        );
    }
}

// ---------------------------------------------------------------------------
// get_type_definitions
// ---------------------------------------------------------------------------

#[test]
fn test_get_type_definitions() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_type_definitions",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_type_definitions should succeed");

    let text = extract_text(&result);
    assert!(text.contains("Config"), "Should find Config struct: {text}");
    assert!(text.contains("Status"), "Should find Status enum: {text}");
    assert!(
        text.contains("Processor"),
        "Should find Processor trait: {text}"
    );
}

#[test]
fn test_get_type_definitions_excludes_functions() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_type_definitions",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_type_definitions should succeed");

    let text = extract_text(&result);
    // Functions should not appear in type definitions
    assert!(
        !text.contains("\"process\""),
        "Should not include process function: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_diff_symbols
// ---------------------------------------------------------------------------

#[test]
fn test_get_diff_symbols_runs() {
    let dispatcher = make_dispatcher();
    // Just verify it doesn't error - actual diff depends on git state
    let result = dispatcher
        .call_tool("get_diff_symbols", json!({}))
        .expect("get_diff_symbols should succeed");

    let text = extract_text(&result);
    // Should return some output (either changes or "No changes found")
    assert!(!text.is_empty(), "Should return some output: {text}");
}

// ---------------------------------------------------------------------------
// get_dependencies
// ---------------------------------------------------------------------------

#[test]
fn test_get_dependencies() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_dependencies",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_dependencies should succeed");

    let text = extract_text(&result);
    // Should return a JSON map with function names as keys
    let parsed: Value = serde_json::from_str(&text).unwrap();
    assert!(parsed.is_object(), "Should return a JSON object: {text}");
    // complex_logic is a function in the file
    assert!(
        parsed.get("complex_logic").is_some() || parsed.get("process").is_some(),
        "Should include known functions: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_parameters
// ---------------------------------------------------------------------------

#[test]
fn test_get_parameters_all() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_parameters",
            json!({ "file": fixture_path("sample.rs") }),
        )
        .expect("get_parameters should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("process") || text.contains("complex_logic"),
        "Should include function names: {text}"
    );
}

#[test]
fn test_get_parameters_single_function() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_parameters",
            json!({ "file": fixture_path("sample.rs"), "function": "complex_logic" }),
        )
        .expect("get_parameters should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("complex_logic"),
        "Should include complex_logic: {text}"
    );
    // complex_logic has params x: i32, y: i32
    assert!(text.contains("x"), "Should find param x: {text}");
    assert!(text.contains("y"), "Should find param y: {text}");
}

// ---------------------------------------------------------------------------
// get_enclosing_class
// ---------------------------------------------------------------------------

#[test]
fn test_get_enclosing_class_found() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_enclosing_class",
            json!({ "file": fixture_path("sample.rs"), "method": "new" }),
        )
        .expect("get_enclosing_class should succeed");

    let text = extract_text(&result);
    // "new" is inside impl Config
    assert!(
        text.contains("Config"),
        "Should find Config as parent: {text}"
    );
    assert!(
        text.contains("value"),
        "Should include sibling method 'value': {text}"
    );
}

#[test]
fn test_get_enclosing_class_not_found() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_enclosing_class",
            json!({ "file": fixture_path("sample.rs"), "method": "nonexistent_method" }),
        )
        .expect("get_enclosing_class should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("No enclosing"),
        "Should report not found: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_symbol_body
// ---------------------------------------------------------------------------

#[test]
fn test_get_symbol_body() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_symbol_body",
            json!({ "file": fixture_path("sample.rs"), "symbol": "process" }),
        )
        .expect("get_symbol_body should succeed");

    let text = extract_text(&result);
    assert!(text.contains("process"), "Should find process: {text}");
    assert!(text.contains("body"), "Should include body field: {text}");
    // The body should contain the actual source code
    assert!(
        text.contains("format!") || text.contains("config"),
        "Should include actual source code: {text}"
    );
}

#[test]
fn test_get_symbol_body_not_found() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool(
            "get_symbol_body",
            json!({ "file": fixture_path("sample.rs"), "symbol": "nonexistent" }),
        )
        .expect("get_symbol_body should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("not found"),
        "Should report not found: {text}"
    );
}

// ---------------------------------------------------------------------------
// get_changed_files
// ---------------------------------------------------------------------------

#[test]
fn test_get_changed_files_runs() {
    let dispatcher = make_dispatcher();
    let result = dispatcher
        .call_tool("get_changed_files", json!({}))
        .expect("get_changed_files should succeed");

    let text = extract_text(&result);
    assert!(!text.is_empty(), "Should return some output: {text}");
}

// ---------------------------------------------------------------------------
// Unified mode (McpServer)
// ---------------------------------------------------------------------------

#[test]
fn test_unified_mode_tools_list_returns_one_tool() {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = rhizome_mcp::McpServer::new(project_root, true);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    let response = server.handle_request_for_test(&request);
    let tools = response
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .expect("Should have tools array");
    assert_eq!(tools.len(), 1, "Unified mode should return exactly 1 tool");
    assert_eq!(
        tools[0].get("name").and_then(|n| n.as_str()),
        Some("rhizome"),
        "The single tool should be named 'rhizome'"
    );
}

#[test]
fn test_expanded_mode_tools_list_returns_25_tools() {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = rhizome_mcp::McpServer::new(project_root, false);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    let response = server.handle_request_for_test(&request);
    let tools = response
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .expect("Should have tools array");
    assert_eq!(
        tools.len(),
        35,
        "Expanded mode should return 35 tools, got {}",
        tools.len()
    );
}

#[test]
fn test_unified_mode_call_via_rhizome_tool() {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = rhizome_mcp::McpServer::new(project_root, true);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "rhizome",
            "arguments": {
                "command": "get_symbols",
                "file": fixture_path("sample.rs")
            }
        }
    });

    let response = server.handle_request_for_test(&request);
    let result = response.get("result").expect("Should have result");
    // Should not be an error
    assert!(
        result.get("isError").is_none()
            || !result
                .get("isError")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        "Should not be an error: {:?}",
        result
    );
    let text = result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    assert!(
        text.contains("Config"),
        "Should find Config symbol via unified mode: {text}"
    );
}

// ── Hyphae export integration tests ──

#[test]
fn test_export_to_hyphae() {
    let dispatcher = make_dispatcher();
    let result = dispatcher.call_tool("export_to_hyphae", json!({})).unwrap();
    let text = extract_text(&result);
    if rhizome_core::hyphae::is_available() {
        // Hyphae installed: should succeed or report cached files
        assert!(
            text.contains("concepts") || text.contains("up to date"),
            "Should report export result: {text}"
        );
    } else {
        // Hyphae not installed: should return an error
        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(is_error, "Should return error when Hyphae not available");
        assert!(
            text.contains("Hyphae not available"),
            "Error should mention Hyphae: {text}"
        );
    }
}

#[test]
fn test_export_tool_in_list() {
    let dispatcher = make_dispatcher();
    let tools = dispatcher.list_tools();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names.contains(&"export_to_hyphae"),
        "export_to_hyphae should be in tool list: {:?}",
        names
    );
}

#[test]
fn test_export_unified_mode() {
    use rhizome_mcp::McpServer;

    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = McpServer::new(project_root, true);

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "rhizome",
            "arguments": {
                "command": "export_to_hyphae"
            }
        }
    });

    let response = server.handle_request_for_test(&request);
    let result = response.get("result").expect("Should have result");
    let text = result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|o| o.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if rhizome_core::hyphae::is_available() {
        assert!(
            text.contains("concepts") || text.contains("up to date"),
            "Should report export result in unified mode: {text}"
        );
    } else {
        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(is_error, "Should return error in unified mode too");
    }
}

#[test]
#[ignore]
fn test_export_to_hyphae_e2e() {
    // This test requires `hyphae` to be installed and available in PATH
    let dispatcher = make_dispatcher();
    let result = dispatcher.call_tool("export_to_hyphae", json!({})).unwrap();
    let is_error = result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(!is_error, "Should succeed when Hyphae is available");
    let text = extract_text(&result);
    assert!(
        text.contains("concepts"),
        "Should report concept count: {text}"
    );
}
