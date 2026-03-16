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
fn test_list_tools_returns_12_tools() {
    let dispatcher = make_dispatcher();
    let tools = dispatcher.list_tools();
    assert_eq!(tools.len(), 12, "Expected 12 tools, got {}", tools.len());

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
        text.contains("LSP required") || text.contains("LSP rename"),
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
        text.contains("LSP required") || text.contains("LSP hover"),
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
