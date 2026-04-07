pub mod analysis;
pub mod git;
pub mod inspection;
pub mod navigation;
pub mod onboard;
pub mod params;
pub mod query;

pub(crate) use super::{ToolSchema, tool_error, tool_response};
pub(crate) use params::{required_str, required_u32};

pub(crate) use analysis::*;
pub(crate) use git::*;
pub(crate) use inspection::*;
pub(crate) use navigation::*;
pub(crate) use onboard::{onboard_schema, rhizome_onboard, summarize_project_tool};
pub(crate) use query::*;

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

pub(crate) fn resolve_project_path(file: &str, project_root: &Path) -> Result<PathBuf> {
    super::edit_tools::resolve_path(file, project_root)
}

pub fn tool_schemas() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            name: "get_symbols".into(),
            description: "List all symbols (functions, structs, classes, etc.) in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file (absolute, or relative to root)" },
                    "root": { "type": "string", "description": "Optional project root to resolve relative paths against. Use when working in a project other than the server's configured root." }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_structure".into(),
            description: "Show the hierarchical structure of symbols in a file as an indented tree"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file (absolute, or relative to root)" },
                    "depth": { "type": "number", "description": "Maximum nesting depth to display" },
                    "root": { "type": "string", "description": "Optional project root to resolve relative paths against. Use when working in a project other than the server's configured root." }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_definition".into(),
            description: "Get the full definition of a symbol including its body".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol to find" },
                    "full": { "type": "boolean", "description": "Show full body even if large (default: false)" }
                },
                "required": ["file", "symbol"]
            }),
        },
        ToolSchema {
            name: "search_symbols".into(),
            description: "Search for symbols matching a pattern across the project".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Pattern to match symbol names (case-insensitive)" },
                    "path": { "type": "string", "description": "Optional directory to search in (defaults to project root)" }
                },
                "required": ["pattern"]
            }),
        },
        ToolSchema {
            name: "find_references".into(),
            description: "Find all references to the symbol at a given position".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" },
                    "column": { "type": "number", "description": "Column number (0-based)" }
                },
                "required": ["file", "line", "column"]
            }),
        },
        ToolSchema {
            name: "analyze_impact".into(),
            description: "Estimate change impact for the symbol at a given position".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" },
                    "column": { "type": "number", "description": "Column number (0-based)" }
                },
                "required": ["file", "line", "column"]
            }),
        },
        ToolSchema {
            name: "go_to_definition".into(),
            description: "Find the definition of the symbol at a given position".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" },
                    "column": { "type": "number", "description": "Column number (0-based)" }
                },
                "required": ["file", "line", "column"]
            }),
        },
        ToolSchema {
            name: "get_signature".into(),
            description: "Get only the signature of a symbol (no body)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol" }
                },
                "required": ["file", "symbol"]
            }),
        },
        ToolSchema {
            name: "get_imports".into(),
            description: "List all import statements in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_call_sites".into(),
            description: "Find all function call expressions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Filter to calls of a specific function" }
                },
                "required": ["file"]
            }),
        },
        // --- New tools ---
        ToolSchema {
            name: "get_scope".into(),
            description: "Get the enclosing scope (function, class, module) at a given line".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" }
                },
                "required": ["file", "line"]
            }),
        },
        ToolSchema {
            name: "get_exports".into(),
            description: "List only public/exported symbols in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "summarize_file".into(),
            description: "Compact file summary showing only public signatures, no bodies".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_tests".into(),
            description: "Find test functions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_diff_symbols".into(),
            description:
                "Show which symbols were modified in uncommitted changes or between commits".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Limit to a specific file" },
                    "ref1": { "type": "string", "description": "First git ref (default: HEAD)" },
                    "ref2": { "type": "string", "description": "Second git ref (default: working tree)" }
                },
                "required": []
            }),
        },
        ToolSchema {
            name: "get_annotations".into(),
            description: "Find TODO, FIXME, HACK, and other annotation comments in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Specific tags to search for (default: TODO, FIXME, HACK, XXX, NOTE, WARN)"
                    }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_complexity".into(),
            description: "Calculate cyclomatic complexity for functions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Analyze only this function" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_type_definitions".into(),
            description:
                "List type definitions (structs, enums, interfaces, type aliases) in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        // --- Batch 2 tools ---
        ToolSchema {
            name: "get_dependencies".into(),
            description: "Map which functions call which within a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_parameters".into(),
            description: "Extract function parameters with types".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Filter to a specific function" }
                },
                "required": ["file"]
            }),
        },
        ToolSchema {
            name: "get_enclosing_class".into(),
            description: "Get the parent class/struct and all sibling methods for a given method"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "method": { "type": "string", "description": "Name of the method to find" }
                },
                "required": ["file", "method"]
            }),
        },
        ToolSchema {
            name: "get_symbol_body".into(),
            description: "Get the source code body of a specific symbol by name and optional line"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol" },
                    "line": { "type": "number", "description": "Line number to disambiguate (0-based)" }
                },
                "required": ["file", "symbol"]
            }),
        },
        ToolSchema {
            name: "get_region".into(),
            description:
                "Return the full text for a parserless region_id or semantic stable_id".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "region_id": { "type": "string", "description": "Parserless region_id (region-<line>) or semantic stable_id" }
                },
                "required": ["file", "region_id"]
            }),
        },
        ToolSchema {
            name: "get_changed_files".into(),
            description: "List files with uncommitted changes and their modified symbol counts"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ref1": { "type": "string", "description": "Git ref for diff start" },
                    "ref2": { "type": "string", "description": "Git ref for diff end" }
                },
                "required": []
            }),
        },
        ToolSchema {
            name: "summarize_project".into(),
            description: "Summarize project structure: entry points, key types, modules, language breakdown, test counts".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Helper: extract a required string arg
// ---------------------------------------------------------------------------
