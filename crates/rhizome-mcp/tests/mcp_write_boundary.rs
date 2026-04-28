/// Integration tests for the MCP write boundary and root_override protection.
///
/// These tests verify that ToolDispatcher rejects hostile `root` overrides
/// (e.g., root = "/" or a path that escapes the configured project root) for
/// all tool families — read, write, and export — without requiring a live MCP
/// transport.
///
/// The in-module unit tests in `rhizome_mcp::tools` cover the boundary at the
/// unit level; this file ensures the same protection is visible from the
/// crate's integration test surface.
use rhizome_mcp::tools::ToolDispatcher;
use serde_json::json;

/// Assert that a call is rejected with a root_override error.
fn assert_root_override_rejected(dispatcher: &ToolDispatcher, tool: &str, args: serde_json::Value) {
    match dispatcher.call_tool(tool, args) {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("root_override"),
                "Expected root_override rejection for {tool}, got: {msg}"
            );
        }
        Ok(_) => panic!("{tool}: expected root_override rejection but call succeeded"),
    }
}

#[test]
fn write_boundary_create_file_rejects_slash_root() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ToolDispatcher::new(dir.path().to_path_buf());
    let args = json!({ "path": "pwned.sh", "content": "#!/bin/sh\nrm -rf /", "root": "/" });
    // write_boundary: create_file must not escape the configured project root.
    assert_root_override_rejected(&dispatcher, "create_file", args);
}

#[test]
fn write_boundary_replace_lines_rejects_slash_root() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ToolDispatcher::new(dir.path().to_path_buf());
    let args = json!({
        "file": "etc/passwd",
        "start_line": 1,
        "end_line": 1,
        "new_content": "hacked",
        "root": "/"
    });
    // write_boundary: replace_lines must not accept a root outside the configured root.
    assert_root_override_rejected(&dispatcher, "replace_lines", args);
}

#[test]
fn write_boundary_insert_at_line_rejects_escape() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ToolDispatcher::new(dir.path().to_path_buf());
    // A sibling path that would escape the configured root must be rejected.
    let escape_root = "/tmp";
    let args = json!({
        "file": "evil.rs",
        "line": 1,
        "content": "pub fn pwned() {}",
        "root": escape_root
    });
    assert_root_override_rejected(&dispatcher, "insert_at_line", args);
}

#[test]
fn write_boundary_delete_lines_rejects_slash_root() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ToolDispatcher::new(dir.path().to_path_buf());
    let args = json!({ "file": "src/main.rs", "start_line": 1, "end_line": 5, "root": "/" });
    assert_root_override_rejected(&dispatcher, "delete_lines", args);
}

#[test]
fn read_tools_reject_root_override_slash() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ToolDispatcher::new(dir.path().to_path_buf());
    // Read tools must also enforce the configured project root boundary.
    for tool in &[
        "get_symbols",
        "search_symbols",
        "get_imports",
        "summarize_file",
    ] {
        let args = json!({ "file": "src/main.rs", "root": "/" });
        assert_root_override_rejected(&dispatcher, tool, args);
    }
}

#[test]
fn export_tool_rejects_root_override_slash() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ToolDispatcher::new(dir.path().to_path_buf());
    // Export tools must not escape the configured project root.
    let args = json!({ "root": "/" });
    assert_root_override_rejected(&dispatcher, "export_to_hyphae", args);
}

#[test]
fn subpath_within_configured_root_is_not_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let project_root = dir.path().to_path_buf();
    std::fs::create_dir_all(project_root.join("src")).unwrap();
    let dispatcher = ToolDispatcher::new(project_root.clone());

    // A sub-path of the configured root must pass the root_override boundary check.
    let sub_root = project_root.join("src").display().to_string();
    let args = json!({ "file": "main.rs", "root": sub_root });
    if let Err(e) = dispatcher.call_tool("get_symbols", args) {
        let msg = e.to_string();
        assert!(
            !msg.contains("root_override"),
            "Sub-path of configured root should not be rejected as escape: {msg}"
        );
    }
}
