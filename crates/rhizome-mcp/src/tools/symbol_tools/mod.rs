pub mod analysis;
pub mod blast_radius;
pub mod git;
pub mod inspection;
pub mod navigation;
pub mod onboard;
pub mod params;
pub mod query;

pub(crate) use super::{ToolAnnotations, ToolSchema, tool_response};
pub(crate) use params::{required_str, required_u32};

pub(crate) use analysis::*;
pub(crate) use blast_radius::{simulate_change, simulate_change_schema};
pub(crate) use git::*;
pub(crate) use inspection::*;
pub(crate) use navigation::*;
pub(crate) use onboard::{onboard_schema, rhizome_onboard, summarize_project_tool};
pub(crate) use query::*;

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::json;

pub(crate) fn resolve_project_path(file: &str, project_root: &Path) -> Result<PathBuf> {
    super::edit_tools::resolve_path(file, project_root)
}

pub fn tool_schemas() -> Vec<ToolSchema> {
    vec![
        ToolSchema {
            name: "get_symbols".into(),
            title: Some("Get Symbols".to_string()),
            description: "List all symbols (functions, structs, classes, etc.) in a file. \
                When the file type has no tree-sitter or LSP support, output falls back to \
                heuristic analysis with region_id/label fields instead of name/qualified_name/stable_id."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file (absolute, or relative to root)" },
                    "root": { "type": "string", "description": "Optional project root to resolve relative paths against. Use when working in a project other than the server's configured root." }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_structure".into(),
            title: Some("Get File Structure".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_definition".into(),
            title: Some("Get Definition".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "search_symbols".into(),
            title: Some("Search Symbols".to_string()),
            description: "Search for symbols matching a pattern across the project".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Pattern to match symbol names (case-insensitive)" },
                    "path": { "type": "string", "description": "Optional directory to search in (defaults to project root). Pass an explicit nested-repo path (e.g. a subproject dir) to reach symbols the root index skips — the workspace index respects the root .gitignore, which prunes nested repos." }
                },
                "required": ["pattern"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "find_references".into(),
            title: Some("Find References".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "analyze_impact".into(),
            title: Some("Analyze Impact".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "go_to_definition".into(),
            title: Some("Go to Definition".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_signature".into(),
            title: Some("Get Signature".to_string()),
            description: "Get only the signature of a symbol (no body)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "symbol": { "type": "string", "description": "Name of the symbol" }
                },
                "required": ["file", "symbol"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_imports".into(),
            title: Some("Get Imports".to_string()),
            description: "List all import statements in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_call_sites".into(),
            title: Some("Get Call Sites".to_string()),
            description: "Find all function call expressions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Filter to calls of a specific function" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        // --- New tools ---
        ToolSchema {
            name: "get_scope".into(),
            title: Some("Get Scope".to_string()),
            description: "Get the enclosing scope (function, class, module) at a given line".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "line": { "type": "number", "description": "Line number (0-based)" }
                },
                "required": ["file", "line"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_exports".into(),
            title: Some("Get Exports".to_string()),
            description: "List only public/exported symbols in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "summarize_file".into(),
            title: Some("Summarize File".to_string()),
            description: "Compact file summary showing only public signatures, no bodies".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_tests".into(),
            title: Some("Get Tests".to_string()),
            description: "Find test functions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_diff_symbols".into(),
            title: Some("Get Diff Symbols".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_annotations".into(),
            title: Some("Get Annotations".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_complexity".into(),
            title: Some("Get Complexity".to_string()),
            description: "Calculate cyclomatic complexity for functions in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Analyze only this function" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_type_definitions".into(),
            title: Some("Get Type Definitions".to_string()),
            description:
                "List type definitions (structs, enums, interfaces, type aliases) in a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        // --- Batch 2 tools ---
        ToolSchema {
            name: "get_dependencies".into(),
            title: Some("Get Dependencies".to_string()),
            description: "Map which functions call which within a file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_parameters".into(),
            title: Some("Get Parameters".to_string()),
            description: "Extract function parameters with types".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "function": { "type": "string", "description": "Filter to a specific function" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_enclosing_class".into(),
            title: Some("Get Enclosing Class".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_symbol_body".into(),
            title: Some("Get Symbol Body".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_region".into(),
            title: Some("Get Region".to_string()),
            description:
                "Return the full text for a heuristic (h-*), parserless (region-*), or semantic stable_id region".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "region_id": { "type": "string", "description": "Heuristic region_id (h-<hash>-<line>), parserless region_id (region-<line>), or semantic stable_id" }
                },
                "required": ["file", "region_id"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_changed_files".into(),
            title: Some("Get Changed Files".to_string()),
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
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "summarize_project".into(),
            title: Some("Summarize Project".to_string()),
            description: "Summarize project structure: entry points, key types, modules, language breakdown, test counts".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        ToolSchema {
            name: "get_chunk_boundaries".into(),
            title: Some("Get Chunk Boundaries".to_string()),
            description: "Get AST-based chunk boundaries for a file using tree-sitter".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the source file" },
                    "strategy": { "type": "string", "enum": ["Function", "Class", "TopLevel", "Semantic"], "description": "Chunking strategy (default: Function)" }
                },
                "required": ["file"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: false,
                idempotent_hint: true,
            },
        },
        simulate_change_schema(),
    ]
}

// ---------------------------------------------------------------------------
// Helper: extract a required string arg
// ---------------------------------------------------------------------------
