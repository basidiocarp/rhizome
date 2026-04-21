#![allow(clippy::collapsible_if, clippy::empty_line_after_doc_comments)]

use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use anyhow::Result;
use rhizome_core::CodeIntelligence;
use serde_json::{Value, json};

use super::{ToolSchema, tool_response};

static PROJECT_SUMMARY_CACHE: Mutex<Option<(Instant, String)>> = Mutex::new(None);
const CACHE_TTL_SECS: u64 = 300;

pub fn summarize_project_tool(
    backend: &dyn CodeIntelligence,
    _args: &Value,
    project_root: &Path,
) -> Result<Value> {
    // Check cache
    if let Ok(guard) = PROJECT_SUMMARY_CACHE.lock() {
        if let Some((cached_at, ref text)) = *guard {
            if cached_at.elapsed().as_secs() < CACHE_TTL_SECS {
                return Ok(tool_response(text));
            }
        }
    }

    let summary = rhizome_core::summarize_project(project_root, backend)?;
    let text = summary.format_display();

    // Update cache
    if let Ok(mut guard) = PROJECT_SUMMARY_CACHE.lock() {
        *guard = Some((Instant::now(), text.clone()));
    }

    Ok(tool_response(text.trim_end()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Onboarding tool
// ─────────────────────────────────────────────────────────────────────────────

/// Returns onboarding information about the Rhizome code intelligence system.

pub fn rhizome_onboard(project_root: &Path) -> Result<Value> {
    let languages_supported = vec![
        "Rust",
        "Python",
        "JavaScript",
        "TypeScript",
        "Go",
        "Java",
        "C",
        "C++",
        "Ruby",
        "Elixir",
        "Zig",
        "C#",
        "F#",
        "Swift",
        "PHP",
        "Haskell",
        "Bash",
        "Terraform",
        "Kotlin",
        "Dart",
        "Lua",
        "Clojure",
        "OCaml",
        "Julia",
        "Nix",
        "Gleam",
        "Vue",
        "Svelte",
        "Astro",
        "Prisma",
        "Typst",
        "YAML",
    ];

    let tools_available = vec![
        "get_symbols",
        "get_structure",
        "get_definition",
        "search_symbols",
        "go_to_definition",
        "get_signature",
        "get_imports",
        "get_call_sites",
        "get_scope",
        "get_exports",
        "summarize_file",
        "get_tests",
        "get_diff_symbols",
        "get_annotations",
        "get_complexity",
        "get_type_definitions",
        "get_dependencies",
        "get_parameters",
        "get_enclosing_class",
        "get_symbol_body",
        "get_region",
        "get_changed_files",
        "summarize_project",
        "find_references",
        "get_diagnostics",
        "rename_symbol",
        "replace_symbol_body",
        "insert_after_symbol",
        "insert_before_symbol",
        "replace_lines",
        "insert_at_line",
        "delete_lines",
        "create_file",
        "copy_symbol",
        "move_symbol",
        "export_to_hyphae",
        "export_repo_understanding",
        "rhizome_onboard",
    ];

    let quick_start = "Rhizome provides code intelligence across 32 languages via tree-sitter \
        (fast, no deps), LSP (full-featured, auto-selected), and a parserless heuristic \
        fallback for outline-only reads. Start with get_symbols to list symbols in a file, \
        get_structure for a hierarchical view, or get_region to expand one section without \
        reading the full file. Use export_to_hyphae to build a knowledge graph, or \
        export_repo_understanding to capture repo surfaces and incremental update class in a \
        typed understanding artifact.";

    let result = json!({
        "languages_supported": languages_supported,
        "tools_available": tools_available,
        "backend": "tree-sitter + LSP + parserless fallback",
        "project_root": project_root.to_string_lossy(),
        "quick_start": quick_start,
    });

    Ok(tool_response(&result.to_string()))
}

pub fn onboard_schema() -> ToolSchema {
    ToolSchema {
        name: "rhizome_onboard".into(),
        description: "Get a quick overview of the Rhizome code intelligence system for \
            onboarding. Returns supported languages, available tools, active backend, \
            project root, and a quick-start guide. No parameters required."
            .into(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
        annotations: super::ToolAnnotations {
            read_only_hint: false,
            destructive_hint: false,
            idempotent_hint: true,
        },
    }
}
