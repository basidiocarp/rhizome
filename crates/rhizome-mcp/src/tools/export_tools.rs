use std::path::Path;

use anyhow::Result;
use ignore::WalkBuilder;
use rhizome_core::export_cache::ExportCache;
use rhizome_core::graph::{build_graph, merge_graphs, CodeGraph};
use rhizome_core::hyphae;
use rhizome_core::CodeIntelligence;
use serde_json::{json, Value};

use super::{tool_error, tool_response, ToolSchema};

/// Supported file extensions for code graph export.
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "go", "java", "c", "cpp", "cc", "cxx", "rb",
];

fn is_supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext))
}

pub fn tool_schemas() -> Vec<ToolSchema> {
    vec![ToolSchema {
        name: "export_to_hyphae".into(),
        description: "Export code graph to Hyphae for semantic knowledge storage. \
            Walks the project (respecting .gitignore), extracts symbols, builds a concept graph, \
            and sends it to Hyphae. Uses incremental caching to skip unchanged files."
            .into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Optional path to export. Defaults to the project root."
                }
            },
            "required": []
        }),
    }]
}

pub fn export_to_hyphae(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    if !hyphae::is_available() {
        return Ok(tool_error(
            "Hyphae not available. Install hyphae to export code graphs.",
        ));
    }

    let walk_path = args
        .get("path")
        .and_then(|v| v.as_str())
        .map(Path::new)
        .unwrap_or(project_root);

    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let memoir_name = format!("code:{project_name}");

    let mut cache = ExportCache::load(project_root).unwrap_or_default();

    let mut graphs: Vec<CodeGraph> = Vec::new();
    let mut files_processed: usize = 0;
    let mut files_skipped: usize = 0;
    let mut processed_paths: Vec<std::path::PathBuf> = Vec::new();

    let walker = WalkBuilder::new(walk_path)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_path = entry.path();

        if !file_path.is_file() || !is_supported_extension(file_path) {
            continue;
        }

        if !cache.is_stale(file_path) {
            files_skipped += 1;
            continue;
        }

        match backend.get_symbols(file_path) {
            Ok(symbols) => {
                let graph = build_graph(project_name, &symbols, file_path);
                graphs.push(graph);
                processed_paths.push(file_path.to_path_buf());
                files_processed += 1;
            }
            Err(_) => {
                // Skip files that fail to parse rather than aborting the entire export
                continue;
            }
        }
    }

    if files_processed == 0 {
        return Ok(tool_response(&format!(
            "All {files_skipped} files are up to date — nothing to export."
        )));
    }

    let merged = merge_graphs(graphs);
    let graph_json = serde_json::to_value(&merged)?;

    match hyphae::export_graph(&graph_json, &memoir_name) {
        Ok(result) => {
            for path in &processed_paths {
                cache = cache.update(path);
            }
            cache.save(project_root)?;

            Ok(tool_response(&format!(
                "Exported to memoir \"{}\": {} concepts, {} links. \
                 Files processed: {files_processed}, skipped (cached): {files_skipped}.",
                result.memoir_name, result.concepts_created, result.links_created,
            )))
        }
        Err(e) => Ok(tool_error(&format!("Hyphae export failed: {e}"))),
    }
}
