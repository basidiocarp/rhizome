use std::path::Path;

use anyhow::Result;
use ignore::WalkBuilder;
use rhizome_core::export_cache::ExportCache;
use rhizome_core::graph::{build_graph, merge_graphs, CodeGraph};
use rhizome_core::hyphae;
use rhizome_core::{derive_export_identity, CodeIntelligence, Language};
use serde::Serialize;
use serde_json::{json, Value};

use super::{tool_error, ToolSchema};

fn is_supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .and_then(Language::from_extension)
        .is_some_and(|language| language.tree_sitter_supported())
}

#[derive(Debug, Serialize)]
struct ExportSummary {
    status: &'static str,
    project: String,
    memoir_name: String,
    export_root: String,
    supported_files: usize,
    files_processed: usize,
    files_skipped_cached: usize,
    files_failed: usize,
    failure_samples: Vec<String>,
    warnings: Vec<String>,
}

struct PreparedExport {
    graphs: Vec<CodeGraph>,
    processed_paths: Vec<std::path::PathBuf>,
    summary: ExportSummary,
    cache: ExportCache,
}

fn export_response(text: &str, summary: &ExportSummary) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "export": summary,
    })
}

fn export_error(message: &str, summary: &ExportSummary) -> Value {
    json!({
        "isError": true,
        "content": [{ "type": "text", "text": message }],
        "export": summary,
    })
}

fn resolve_export_root(args: &Value, project_root: &Path) -> Result<std::path::PathBuf> {
    let raw_path = args
        .get("path")
        .and_then(|v| v.as_str())
        .map(Path::new)
        .unwrap_or(project_root);
    let resolved = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        project_root.join(raw_path)
    };

    if !resolved.exists() {
        anyhow::bail!("Export path does not exist: {}", resolved.display());
    }

    Ok(resolved)
}

fn collect_export(
    backend: &dyn CodeIntelligence,
    project_root: &Path,
    export_root: &Path,
    project_name: &str,
    memoir_name: &str,
) -> PreparedExport {
    let mut warnings = Vec::new();
    let cache = match ExportCache::load(project_root) {
        Ok(cache) => cache,
        Err(err) => {
            warnings.push(format!("Ignored unreadable export cache: {err}"));
            ExportCache::default()
        }
    };

    let mut graphs: Vec<CodeGraph> = Vec::new();
    let mut files_processed: usize = 0;
    let mut files_skipped_cached: usize = 0;
    let mut files_failed: usize = 0;
    let mut supported_files: usize = 0;
    let mut processed_paths: Vec<std::path::PathBuf> = Vec::new();
    let mut failure_samples: Vec<String> = Vec::new();

    let walker = WalkBuilder::new(export_root)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                if failure_samples.len() < 5 {
                    failure_samples.push(err.to_string());
                }
                files_failed += 1;
                continue;
            }
        };

        let file_path = entry.path();

        if !file_path.is_file() || !is_supported_extension(file_path) {
            continue;
        }

        supported_files += 1;

        if !cache.is_stale(file_path) {
            files_skipped_cached += 1;
            continue;
        }

        match backend.get_symbols(file_path) {
            Ok(symbols) => {
                let graph = build_graph(project_name, &symbols, file_path);
                graphs.push(graph);
                processed_paths.push(file_path.to_path_buf());
                files_processed += 1;
            }
            Err(err) => {
                files_failed += 1;
                if failure_samples.len() < 5 {
                    failure_samples.push(format!("{}: {err}", file_path.display()));
                }
            }
        }
    }

    let status = if files_processed > 0 {
        "exported"
    } else if files_failed > 0 {
        "failed"
    } else if supported_files == 0 {
        "empty"
    } else {
        "up_to_date"
    };

    PreparedExport {
        graphs,
        processed_paths,
        summary: ExportSummary {
            status,
            project: project_name.to_string(),
            memoir_name: memoir_name.to_string(),
            export_root: export_root.display().to_string(),
            supported_files,
            files_processed,
            files_skipped_cached,
            files_failed,
            failure_samples,
            warnings,
        },
        cache,
    }
}

fn render_summary_text(
    summary: &ExportSummary,
    concepts_created: Option<usize>,
    links_created: Option<usize>,
) -> String {
    match summary.status {
        "exported" => {
            let mut text = format!(
                "Exported to memoir \"{}\": {} concepts, {} links. Files processed: {}, skipped (cached): {}.",
                summary.memoir_name,
                concepts_created.unwrap_or(0),
                links_created.unwrap_or(0),
                summary.files_processed,
                summary.files_skipped_cached,
            );
            if summary.files_failed > 0 {
                text.push_str(&format!(" Files failed: {}.", summary.files_failed));
            }
            if !summary.warnings.is_empty() {
                text.push_str(&format!(" Warnings: {}.", summary.warnings.join(" | ")));
            }
            text
        }
        "up_to_date" => format!(
            "All {} supported files under {} are up to date — nothing to export.",
            summary.files_skipped_cached, summary.export_root
        ),
        "empty" => format!(
            "No supported source files found under {}.",
            summary.export_root
        ),
        "failed" => {
            let mut text = format!(
                "No files exported from {}. {} stale file(s) failed to analyze",
                summary.export_root, summary.files_failed
            );
            if summary.files_skipped_cached > 0 {
                text.push_str(&format!(
                    "; {} file(s) were skipped from cache",
                    summary.files_skipped_cached
                ));
            }
            text.push('.');
            if !summary.failure_samples.is_empty() {
                text.push_str(&format!(
                    " Examples: {}.",
                    summary.failure_samples.join(" | ")
                ));
            }
            if !summary.warnings.is_empty() {
                text.push_str(&format!(" Warnings: {}.", summary.warnings.join(" | ")));
            }
            text
        }
        _ => unreachable!("unexpected export status"),
    }
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
    let export_root = match resolve_export_root(args, project_root) {
        Ok(path) => path,
        Err(err) => return Ok(tool_error(&err.to_string())),
    };

    if !hyphae::is_available() {
        return Ok(tool_error(
            "Hyphae not available. Install hyphae to export code graphs.",
        ));
    }

    let identity = derive_export_identity(project_root);
    let mut prepared = collect_export(
        backend,
        project_root,
        &export_root,
        &identity.project,
        &identity.memoir_name,
    );

    if prepared.summary.files_processed == 0 {
        let text = render_summary_text(&prepared.summary, None, None);
        if prepared.summary.files_failed > 0 {
            return Ok(export_error(&text, &prepared.summary));
        }
        return Ok(export_response(&text, &prepared.summary));
    }

    let merged = merge_graphs(prepared.graphs);
    let graph_json = serde_json::to_value(&merged)?;

    match hyphae::export_graph(&graph_json, &identity) {
        Ok(result) => {
            let mut cache = prepared.cache;
            for path in &prepared.processed_paths {
                cache = cache.update(path);
            }
            if let Err(err) = cache.save(project_root) {
                prepared
                    .summary
                    .warnings
                    .push(format!("Failed to update export cache: {err}"));
            }
            prepared.summary.memoir_name = result.memoir_name.clone();
            let text = render_summary_text(
                &prepared.summary,
                Some(result.concepts_created),
                Some(result.links_created),
            );
            Ok(export_response(&text, &prepared.summary))
        }
        Err(err) => {
            let text = format!(
                "Hyphae export failed: {err}. {}",
                render_summary_text(&prepared.summary, None, None)
            );
            Ok(export_error(&text, &prepared.summary))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use rhizome_core::{
        BackendCapabilities, Diagnostic, Location, Position, Result as RhizomeResult, Symbol,
        SymbolKind,
    };

    use super::*;

    struct MockBackend {
        symbol_results: HashMap<PathBuf, RhizomeResult<Vec<Symbol>>>,
    }

    impl MockBackend {
        fn new(symbol_results: HashMap<PathBuf, RhizomeResult<Vec<Symbol>>>) -> Self {
            Self { symbol_results }
        }
    }

    impl CodeIntelligence for MockBackend {
        fn get_symbols(&self, file: &Path) -> RhizomeResult<Vec<Symbol>> {
            match self.symbol_results.get(file) {
                Some(Ok(symbols)) => Ok(symbols.clone()),
                Some(Err(err)) => Err(rhizome_core::RhizomeError::Other(err.to_string())),
                None => Ok(Vec::new()),
            }
        }

        fn get_definition(&self, _file: &Path, _name: &str) -> RhizomeResult<Option<Symbol>> {
            Ok(None)
        }

        fn find_references(
            &self,
            _file: &Path,
            _position: &Position,
        ) -> RhizomeResult<Vec<Location>> {
            Ok(Vec::new())
        }

        fn search_symbols(
            &self,
            _pattern: &str,
            _project_root: &Path,
        ) -> RhizomeResult<Vec<Symbol>> {
            Ok(Vec::new())
        }

        fn get_imports(&self, _file: &Path) -> RhizomeResult<Vec<Symbol>> {
            Ok(Vec::new())
        }

        fn get_diagnostics(&self, _file: &Path) -> RhizomeResult<Vec<Diagnostic>> {
            Ok(Vec::new())
        }

        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities {
                cross_file_references: false,
                rename: false,
                type_info: false,
                diagnostics: false,
            }
        }
    }

    fn sample_symbol(file: &Path) -> Symbol {
        Symbol {
            name: "demo".into(),
            kind: SymbolKind::Function,
            location: Location {
                file_path: file.display().to_string(),
                line_start: 1,
                line_end: 1,
                column_start: 1,
                column_end: 10,
            },
            scope_path: vec![],
            signature: Some("fn demo()".into()),
            doc_comment: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn resolve_export_root_uses_project_root_for_relative_paths() {
        let project_root = PathBuf::from("/tmp/example-project");
        let args = json!({ "path": "src" });
        let resolved = resolve_export_root(&args, &project_root).unwrap_err();
        assert!(resolved.to_string().contains("/tmp/example-project/src"));
    }

    #[test]
    fn collect_export_marks_parse_only_runs_as_failed() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("broken.rs");
        std::fs::write(&file_path, "fn broken(").unwrap();

        let backend = MockBackend::new(HashMap::from([(
            file_path.clone(),
            Err(rhizome_core::RhizomeError::Other("parse failure".into())),
        )]));

        let prepared = collect_export(&backend, dir.path(), dir.path(), "demo", "code:demo");
        assert_eq!(prepared.summary.status, "failed");
        assert_eq!(prepared.summary.supported_files, 1);
        assert_eq!(prepared.summary.files_processed, 0);
        assert_eq!(prepared.summary.files_failed, 1);
        assert!(prepared.summary.failure_samples[0].contains("broken.rs"));
    }

    #[test]
    fn collect_export_reports_cached_and_processed_files() {
        let dir = tempfile::tempdir().unwrap();
        let fresh_file = dir.path().join("fresh.rs");
        let cached_file = dir.path().join("cached.rs");
        std::fs::write(&fresh_file, "fn fresh() {}").unwrap();
        std::fs::write(&cached_file, "fn cached() {}").unwrap();

        let cache = ExportCache::new().update(&cached_file);
        cache.save(dir.path()).unwrap();

        let backend = MockBackend::new(HashMap::from([(
            fresh_file.clone(),
            Ok(vec![sample_symbol(&fresh_file)]),
        )]));

        let prepared = collect_export(&backend, dir.path(), dir.path(), "demo", "code:demo");
        assert_eq!(prepared.summary.status, "exported");
        assert_eq!(prepared.summary.supported_files, 2);
        assert_eq!(prepared.summary.files_processed, 1);
        assert_eq!(prepared.summary.files_skipped_cached, 1);
        assert_eq!(prepared.processed_paths, vec![fresh_file]);
    }

    #[test]
    fn collect_export_includes_tree_sitter_supported_languages_beyond_old_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let supported_files = [
            dir.path().join("component.tsx"),
            dir.path().join("types.pyi"),
            dir.path().join("script.zsh"),
        ];
        let unsupported_files = [dir.path().join("lock.tf"), dir.path().join("notes.md")];

        for file in supported_files.iter().chain(unsupported_files.iter()) {
            std::fs::write(file, "sample").unwrap();
        }

        let backend = MockBackend::new(HashMap::new());
        let prepared = collect_export(&backend, dir.path(), dir.path(), "demo", "code:demo");

        assert_eq!(prepared.summary.status, "exported");
        assert_eq!(prepared.summary.supported_files, supported_files.len());
        assert_eq!(prepared.summary.files_processed, supported_files.len());
        assert_eq!(prepared.summary.files_failed, 0);

        let processed_paths: Vec<PathBuf> = prepared.processed_paths;
        for file in &supported_files {
            assert!(processed_paths.contains(file));
        }
        for file in &unsupported_files {
            assert!(!processed_paths.contains(file));
        }
    }

    #[test]
    fn is_supported_extension_matches_tree_sitter_backed_languages() {
        for name in [
            "demo.rs",
            "demo.py",
            "demo.pyi",
            "demo.js",
            "demo.jsx",
            "demo.mjs",
            "demo.cjs",
            "demo.ts",
            "demo.tsx",
            "demo.mts",
            "demo.cts",
            "demo.go",
            "demo.java",
            "demo.c",
            "demo.h",
            "demo.cpp",
            "demo.cxx",
            "demo.cc",
            "demo.hpp",
            "demo.hxx",
            "demo.hh",
            "demo.rb",
            "demo.rake",
            "demo.gemspec",
            "demo.ru",
            "demo.ex",
            "demo.exs",
            "demo.zig",
            "demo.cs",
            "demo.swift",
            "demo.php",
            "demo.hs",
            "demo.lhs",
            "demo.sh",
            "demo.bash",
            "demo.zsh",
            "demo.ksh",
            "demo.lua",
        ] {
            assert!(
                is_supported_extension(Path::new(name)),
                "{name} should be exportable"
            );
        }

        for name in [
            "demo.tf",
            "demo.kt",
            "demo.dart",
            "demo.fs",
            "demo.fsi",
            "demo.fsx",
            "demo.fsscript",
            "demo.clj",
            "demo.cljs",
            "demo.cljc",
            "demo.edn",
            "demo.ml",
            "demo.mli",
            "demo.jl",
            "demo.nix",
            "demo.gleam",
            "demo.vue",
            "demo.svelte",
            "demo.astro",
            "demo.prisma",
            "demo.typ",
            "demo.yaml",
        ] {
            assert!(
                !is_supported_extension(Path::new(name)),
                "{name} should not be exportable"
            );
        }
    }

    #[test]
    fn render_summary_text_reports_empty_exports_clearly() {
        let summary = ExportSummary {
            status: "empty",
            project: "demo".into(),
            memoir_name: "code:demo".into(),
            export_root: "/tmp/demo".into(),
            supported_files: 0,
            files_processed: 0,
            files_skipped_cached: 0,
            files_failed: 0,
            failure_samples: Vec::new(),
            warnings: Vec::new(),
        };

        let text = render_summary_text(&summary, None, None);
        assert!(text.contains("No supported source files found"));
    }
}
