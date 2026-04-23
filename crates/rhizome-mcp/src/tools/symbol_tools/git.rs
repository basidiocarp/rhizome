#![allow(
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    unused_imports
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use rhizome_core::{CodeIntelligence, Symbol};
use serde_json::{Value, json};

use super::navigation::find_innermost_scope;
use super::{ToolSchema, tool_response};

/// Validates that a git ref argument contains only safe characters.
///
/// Accepts refs that match `[a-zA-Z0-9/_\-.~^@{} ]+`. This covers branch
/// names, tags, commit SHAs, and common revision syntax while rejecting
/// flag-like values (e.g. `--exec=cmd`).
fn validate_git_ref(r: &str) -> Result<()> {
    if r.is_empty() {
        return Err(anyhow::anyhow!("git ref must not be empty"));
    }
    if r.bytes().all(|b| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'/' | b'_' | b'-' | b'.' | b'~' | b'^' | b'@' | b'{' | b'}' | b' '
            )
    }) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "invalid git ref {:?}: contains characters not allowed in a ref",
            r
        ))
    }
}

pub fn get_diff_symbols(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file_filter = args.get("file").and_then(|v| v.as_str());
    let ref1 = args.get("ref1").and_then(|v| v.as_str());
    let ref2 = args.get("ref2").and_then(|v| v.as_str());

    if let Some(r) = ref1 {
        validate_git_ref(r)?;
    }
    if let Some(r) = ref2 {
        validate_git_ref(r)?;
    }

    let mut cmd = Command::new("git");
    cmd.current_dir(project_root);

    match (ref1, ref2) {
        (Some(r1), Some(r2)) => {
            cmd.args(["diff", "--unified=0", r1, r2]);
        }
        (Some(r1), None) => {
            cmd.args(["diff", "--unified=0", r1]);
        }
        _ => {
            cmd.args(["diff", "--unified=0", "HEAD"]);
        }
    }

    if let Some(f) = file_filter {
        cmd.arg("--").arg(f);
    }

    let output = cmd.output()?;
    if !output.status.success() && output.stdout.is_empty() {
        // git diff returns 0 even with no changes; non-zero may mean bad ref
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            return Ok(tool_response(&format!("git diff error: {stderr}")));
        }
    }

    let diff_text = String::from_utf8_lossy(&output.stdout);
    if diff_text.is_empty() {
        return Ok(tool_response("No changes found"));
    }

    let changed_files = parse_diff_hunks(&diff_text);
    let mut results = Vec::new();

    for (rel_path, changed_lines) in &changed_files {
        let abs_path = if Path::new(rel_path).is_absolute() {
            PathBuf::from(rel_path)
        } else {
            project_root.join(rel_path)
        };

        if !abs_path.exists() {
            // File was deleted
            for &line in changed_lines {
                results.push(json!({
                    "name": "(deleted)",
                    "kind": "Unknown",
                    "file": rel_path,
                    "line": line,
                    "status": "deleted",
                }));
            }
            continue;
        }

        let symbols = match backend.get_symbols(&abs_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut matched_symbols: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for &line in changed_lines {
            if let Some(sym) = find_innermost_scope(&symbols, line) {
                if matched_symbols.insert(sym.stable_id()) {
                    results.push(json!({
                        "name": sym.name,
                        "qualified_name": sym.qualified_name(),
                        "stable_id": sym.stable_id(),
                        "kind": format!("{:?}", sym.kind),
                        "file": rel_path,
                        "line_start": sym.location.line_start,
                        "line_end": sym.location.line_end,
                        "status": "modified",
                    }));
                }
            }
        }
    }

    let text = serde_json::to_string_pretty(&results)?;
    Ok(tool_response(&text))
}

fn parse_diff_hunks(diff: &str) -> Vec<(String, Vec<u32>)> {
    let mut result: Vec<(String, Vec<u32>)> = Vec::new();
    let mut current_file: Option<String> = None;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            current_file = Some(rest.to_string());
        } else if line.starts_with("+++ /dev/null") {
            current_file = None;
        } else if line.starts_with("@@ ") {
            if let Some(ref file) = current_file {
                // Parse the +c,d part from "@@ -a,b +c,d @@"
                if let Some(plus_part) = line.split('+').nth(1) {
                    let range_part = plus_part.split(' ').next().unwrap_or("");
                    let parts: Vec<&str> = range_part.split(',').collect();
                    if let Ok(start) = parts[0].parse::<u32>() {
                        let count = parts
                            .get(1)
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(1);
                        // Convert 1-based to 0-based
                        let start_0 = start.saturating_sub(1);
                        let entry = result.iter_mut().find(|(f, _)| f == file);
                        let lines = if let Some((_, lines)) = entry {
                            lines
                        } else {
                            result.push((file.clone(), Vec::new()));
                            &mut result.last_mut().unwrap().1
                        };
                        for i in 0..count {
                            lines.push(start_0 + i);
                        }
                    }
                }
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tool 6: get_annotations
// ---------------------------------------------------------------------------

/// Find TODO, FIXME, HACK, and other annotation comments in a file.

pub fn get_changed_files(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let ref1 = args.get("ref1").and_then(|v| v.as_str());
    let ref2 = args.get("ref2").and_then(|v| v.as_str());

    if let Some(r) = ref1 {
        validate_git_ref(r)?;
    }
    if let Some(r) = ref2 {
        validate_git_ref(r)?;
    }

    // Get list of changed files
    let mut name_cmd = Command::new("git");
    name_cmd.current_dir(project_root);
    match (ref1, ref2) {
        (Some(r1), Some(r2)) => {
            name_cmd.args(["diff", "--name-only", r1, r2]);
        }
        (Some(r1), None) => {
            name_cmd.args(["diff", "--name-only", r1]);
        }
        _ => {
            name_cmd.args(["diff", "--name-only", "HEAD"]);
        }
    }

    let name_output = name_cmd.output()?;
    let names_text = String::from_utf8_lossy(&name_output.stdout);

    // Get stat info
    let mut stat_cmd = Command::new("git");
    stat_cmd.current_dir(project_root);
    match (ref1, ref2) {
        (Some(r1), Some(r2)) => {
            stat_cmd.args(["diff", "--stat", r1, r2]);
        }
        (Some(r1), None) => {
            stat_cmd.args(["diff", "--stat", r1]);
        }
        _ => {
            stat_cmd.args(["diff", "--stat", "HEAD"]);
        }
    }
    let stat_output = stat_cmd.output()?;
    let stat_text = String::from_utf8_lossy(&stat_output.stdout);

    // Parse stat lines into a map of file -> lines_changed
    let stat_map = parse_stat_lines(&stat_text);

    let supported_exts = [
        "rs", "py", "js", "ts", "jsx", "tsx", "mjs", "go", "java", "c", "cpp", "h", "hpp",
    ];

    let mut results = Vec::new();

    for line in names_text.lines() {
        let file = line.trim();
        if file.is_empty() {
            continue;
        }

        let ext = Path::new(file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let lines_changed = stat_map
            .get(file)
            .cloned()
            .unwrap_or_else(|| "?".to_string());

        if supported_exts.contains(&ext) {
            let abs_path = if Path::new(file).is_absolute() {
                PathBuf::from(file)
            } else {
                project_root.join(file)
            };

            let symbol_count = if abs_path.exists() {
                backend
                    .get_symbols(&abs_path)
                    .map(|syms| count_all_symbols(&syms))
                    .unwrap_or(0)
            } else {
                0
            };

            results.push(json!({
                "file": file,
                "symbols": symbol_count,
                "lines_changed": lines_changed,
            }));
        } else {
            results.push(json!({
                "file": file,
                "symbols": 0,
                "lines_changed": lines_changed,
            }));
        }
    }

    if results.is_empty() {
        return Ok(tool_response("No changed files found"));
    }

    let text = serde_json::to_string_pretty(&results)?;
    Ok(tool_response(&text))
}

fn parse_stat_lines(stat: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in stat.lines() {
        // Format: " path/to/file.rs | 10 ++++----"
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        if parts.len() == 2 {
            let file = parts[0].trim().to_string();
            let stats = parts[1].trim();
            // Extract the +N/-M pattern
            let plus_count = stats.matches('+').count();
            let minus_count = stats.matches('-').count();
            map.insert(file, format!("+{plus_count}/-{minus_count}"));
        }
    }
    map
}

fn count_all_symbols(symbols: &[Symbol]) -> usize {
    let mut count = symbols.len();
    for sym in symbols {
        count += count_all_symbols(&sym.children);
    }
    count
}
