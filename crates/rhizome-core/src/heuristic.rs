//! Heuristic structural fallback backend.
//!
//! Produces a structural outline using indentation depth and bracket counting
//! when both tree-sitter and LSP backends are unavailable. Region IDs use a
//! content hash for stability across repeated calls on unchanged files.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::{
    BackendCapabilities, CodeIntelligence, Diagnostic, Location, Position, Result, RhizomeError,
    Symbol, SymbolKind,
};

/// A single region identified by the heuristic backend.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HeuristicRegion {
    /// Stable ID in the form `h-{content_hash}-{line_number}`.
    pub region_id: String,
    /// 1-based start line.
    pub line: u32,
    /// 1-based end line (inclusive).
    pub line_end: u32,
    /// Nesting depth (0 = top level).
    pub depth: u32,
    /// Compact label derived from the first structural line.
    pub label: String,
}

/// Internal candidate before region bounds are finalized.
#[derive(Debug, Clone)]
struct CandidateRegion {
    line_index: usize,
    line_end_index: usize,
    depth: usize,
    label: String,
    /// Raw text of the structural line, used for hashing.
    raw_line: String,
}

/// Heuristic structural fallback backend.
///
/// Reads a file line-by-line, computes indentation depth, tracks bracket depth,
/// and identifies section boundaries. Produces output compatible with the
/// existing `get_structure` response schema.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeuristicBackend;

impl HeuristicBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Produce a structural outline for a file on disk.
    pub fn outline(&self, file: &Path) -> Result<Vec<HeuristicRegion>> {
        let source = read_source(file)?;
        Ok(self.outline_from_source(&source))
    }

    /// Return the raw text for a heuristic region by its `h-*` region ID.
    pub fn get_region_text(&self, file: &Path, region_id: &str) -> Result<String> {
        let source = read_source(file)?;
        let outline = self.outline_from_source(&source);
        let region = outline
            .iter()
            .find(|r| r.region_id == region_id)
            .ok_or_else(|| RhizomeError::SymbolNotFound(region_id.to_string()))?;

        let chunks: Vec<&str> = source.split_inclusive('\n').collect();
        if chunks.is_empty() {
            return Ok(String::new());
        }

        let start = region.line.saturating_sub(1) as usize;
        let end = region.line_end.saturating_sub(1) as usize;
        if start >= chunks.len() || end >= chunks.len() || end < start {
            return Err(RhizomeError::ParseError(format!(
                "region {region_id} is out of range for {}",
                file.display()
            )));
        }

        Ok(chunks[start..=end].concat())
    }

    /// Core outline logic operating on an in-memory source string.
    fn outline_from_source(&self, source: &str) -> Vec<HeuristicRegion> {
        let lines: Vec<&str> = source.lines().collect();
        if lines.is_empty() {
            return Vec::new();
        }

        let mut regions = Vec::new();
        let mut bracket_depth = 0usize;

        for (line_index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || is_comment_line(trimmed) {
                bracket_depth = update_bracket_depth(bracket_depth, trimmed);
                continue;
            }

            let indent = indent_depth(line);
            let leading_closers = trimmed
                .chars()
                .take_while(|c| matches!(c, '}' | ')' | ']'))
                .count();
            let effective_depth =
                indent.saturating_add(bracket_depth.saturating_sub(leading_closers));

            if is_structural_line(trimmed, indent, effective_depth) {
                regions.push(CandidateRegion {
                    line_index,
                    line_end_index: line_index,
                    depth: effective_depth,
                    label: compact_label(trimmed),
                    raw_line: trimmed.to_string(),
                });
            }

            bracket_depth = update_bracket_depth(bracket_depth, trimmed);
        }

        // If no structural lines were found, create a single region from the
        // first non-empty line spanning the whole file.
        if regions.is_empty()
            && let Some((line_index, line)) =
                lines.iter().enumerate().find(|(_, l)| !l.trim().is_empty())
        {
            let trimmed = line.trim();
            regions.push(CandidateRegion {
                line_index,
                line_end_index: lines.len().saturating_sub(1),
                depth: 0,
                label: compact_label(trimmed),
                raw_line: trimmed.to_string(),
            });
        }

        finalize_region_bounds(&mut regions, lines.len().saturating_sub(1));

        regions
            .into_iter()
            .map(|r| {
                let line_number = r.line_index + 1;
                let hash = content_hash(&r.raw_line);
                HeuristicRegion {
                    region_id: format!("h-{hash:016x}-{line_number}"),
                    line: line_number as u32,
                    line_end: (r.line_end_index + 1) as u32,
                    depth: r.depth as u32,
                    label: r.label,
                }
            })
            .collect()
    }
}

impl CodeIntelligence for HeuristicBackend {
    fn get_symbols(&self, file: &Path) -> Result<Vec<Symbol>> {
        let file_path = file.to_string_lossy().into_owned();
        let outline = self.outline(file)?;

        Ok(outline
            .into_iter()
            .map(|region| Symbol {
                name: region.label.clone(),
                kind: SymbolKind::Module,
                location: Location {
                    file_path: file_path.clone(),
                    line_start: region.line.saturating_sub(1),
                    line_end: region.line_end.saturating_sub(1),
                    column_start: 0,
                    column_end: 0,
                },
                scope_path: Vec::new(),
                signature: Some(format!("{} [heuristic]", region.region_id)),
                doc_comment: None,
                children: Vec::new(),
            })
            .collect())
    }

    fn get_definition(&self, file: &Path, name: &str) -> Result<Option<Symbol>> {
        Ok(self
            .get_symbols(file)?
            .into_iter()
            .find(|symbol| symbol.name == name))
    }

    fn find_references(&self, _file: &Path, _position: &Position) -> Result<Vec<Location>> {
        Err(RhizomeError::NotSupported(
            "heuristic backend only supports outline-style queries".into(),
        ))
    }

    fn search_symbols(&self, _pattern: &str, _project_root: &Path) -> Result<Vec<Symbol>> {
        Err(RhizomeError::NotSupported(
            "heuristic backend does not support workspace symbol search".into(),
        ))
    }

    fn get_imports(&self, _file: &Path) -> Result<Vec<Symbol>> {
        Ok(Vec::new())
    }

    fn get_diagnostics(&self, _file: &Path) -> Result<Vec<Diagnostic>> {
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

// ─────────────────────────────────────────────────────────────────────────────
// File reading
// ─────────────────────────────────────────────────────────────────────────────

fn read_source(path: &Path) -> std::result::Result<String, std::io::Error> {
    std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidData {
            std::io::Error::new(
                e.kind(),
                format!("file appears to be binary or non-UTF-8: {}", path.display()),
            )
        } else {
            std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e))
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Hashing
// ─────────────────────────────────────────────────────────────────────────────

/// Produce a deterministic 64-bit hash of a line's content.
///
/// Uses FNV-1a which is algorithm-stable across Rust toolchain versions.
/// Region IDs are stored by MCP callers and must not change after a compiler
/// update.
fn content_hash(text: &str) -> u64 {
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    let mut hash = FNV_OFFSET;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ─────────────────────────────────────────────────────────────────────────────
// Region bound finalization
// ─────────────────────────────────────────────────────────────────────────────

fn finalize_region_bounds(regions: &mut [CandidateRegion], last_line_index: usize) {
    let mut open: Vec<usize> = Vec::new();

    for current_index in 0..regions.len() {
        while let Some(previous_index) = open.last().copied() {
            if regions[previous_index].depth >= regions[current_index].depth {
                let previous_index = open.pop().expect("non-empty: guarded by last().copied()");
                regions[previous_index].line_end_index = regions[current_index]
                    .line_index
                    .saturating_sub(1)
                    .max(regions[previous_index].line_index);
            } else {
                break;
            }
        }
        open.push(current_index);
    }

    for remaining_index in open {
        regions[remaining_index].line_end_index =
            last_line_index.max(regions[remaining_index].line_index);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Line analysis helpers
// ─────────────────────────────────────────────────────────────────────────────

fn indent_depth(line: &str) -> usize {
    let spaces = line
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum::<usize>();
    if spaces == 0 { 0 } else { spaces.div_ceil(4) }
}

fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("--")
        || trimmed.starts_with(';')
        || (trimmed.starts_with('#') && !looks_like_heading(trimmed))
}

fn looks_like_heading(trimmed: &str) -> bool {
    trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ")
}

fn is_structural_line(trimmed: &str, indent_depth: usize, effective_depth: usize) -> bool {
    let lower = trimmed.to_ascii_lowercase();
    let declaration_like = [
        "fn ",
        "pub fn ",
        "async fn ",
        "def ",
        "class ",
        "struct ",
        "enum ",
        "trait ",
        "impl ",
        "interface ",
        "type ",
        "module ",
        "mod ",
        "func ",
        "function ",
        "export ",
        "resource ",
        "variable ",
        "locals ",
        "data ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));

    if declaration_like || looks_like_heading(trimmed) {
        return true;
    }

    if (trimmed.ends_with('{') || trimmed.ends_with(':')) && has_identifier(trimmed) {
        return true;
    }

    let entropy = line_entropy(trimmed);
    (indent_depth == 0 || effective_depth <= 1)
        && has_identifier(trimmed)
        && trimmed.len() >= 4
        && entropy >= 0.45
}

fn has_identifier(trimmed: &str) -> bool {
    trimmed.chars().any(|ch| ch.is_alphabetic() || ch == '_')
}

fn line_entropy(trimmed: &str) -> f32 {
    let compact: Vec<char> = trimmed.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.is_empty() {
        return 0.0;
    }

    let unique = compact.iter().copied().collect::<BTreeSet<_>>().len();
    unique as f32 / compact.len() as f32
}

fn compact_label(trimmed: &str) -> String {
    const MAX_LABEL_CHARS: usize = 80;
    let label: String = trimmed.chars().take(MAX_LABEL_CHARS).collect();
    if trimmed.chars().count() > MAX_LABEL_CHARS {
        format!("{label}...")
    } else {
        label
    }
}

fn update_bracket_depth(current_depth: usize, trimmed: &str) -> usize {
    let opens = trimmed
        .chars()
        .filter(|ch| matches!(ch, '{' | '[' | '('))
        .count();
    let closes = trimmed
        .chars()
        .filter(|ch| matches!(ch, '}' | ']' | ')'))
        .count();

    current_depth + opens - closes.min(current_depth + opens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_file(dir: &TempDir, name: &str, contents: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn heuristic_outline_is_stable_for_rust_like_source() {
        let dir = TempDir::new().unwrap();
        let file = write_file(
            &dir,
            "sample.rs",
            "pub struct Config {\n    value: String,\n}\n\nimpl Config {\n    pub fn new() -> Self {\n        Self { value: String::new() }\n    }\n}\n\npub fn process(config: &Config) {\n    println!(\"{}\", config.value);\n}\n",
        );

        let backend = HeuristicBackend::new();
        let first = backend.outline(&file).unwrap();
        let second = backend.outline(&file).unwrap();

        assert_eq!(first, second, "outline must be stable across calls");
        assert!(
            first
                .iter()
                .any(|region| region.label.contains("struct Config"))
        );
        assert!(
            first
                .iter()
                .any(|region| region.label.contains("fn process"))
        );
        assert!(
            first
                .iter()
                .all(|region| region.region_id.starts_with("h-")),
            "region IDs must use the h-{{hash}}-{{line}} format"
        );
    }

    #[test]
    fn heuristic_region_ids_contain_hash_and_line() {
        let dir = TempDir::new().unwrap();
        let file = write_file(&dir, "simple.py", "def hello():\n    pass\n");

        let backend = HeuristicBackend::new();
        let regions = backend.outline(&file).unwrap();

        assert!(!regions.is_empty());
        for region in &regions {
            // Format: h-{16-hex-chars}-{line_number}
            assert!(region.region_id.starts_with("h-"));
            let parts: Vec<&str> = region.region_id.splitn(3, '-').collect();
            assert_eq!(
                parts.len(),
                3,
                "region_id should have 3 parts: {}",
                region.region_id
            );
            assert_eq!(parts[0], "h");
            assert_eq!(parts[1].len(), 16, "hash should be 16 hex digits");
            assert!(
                parts[2].parse::<u32>().is_ok(),
                "third part should be a line number"
            );
        }
    }

    #[test]
    fn heuristic_region_reads_full_block() {
        let dir = TempDir::new().unwrap();
        let file = write_file(
            &dir,
            "sample.txt",
            "section:\n  child: true\nnext:\n  value: 1\n",
        );

        let backend = HeuristicBackend::new();
        let regions = backend.outline(&file).unwrap();
        let first_id = &regions[0].region_id;

        let text = backend.get_region_text(&file, first_id).unwrap();
        assert!(text.contains("section:"));
        assert!(text.contains("child: true"));
        assert!(!text.contains("next:"));
    }

    #[test]
    fn empty_file_has_no_outline() {
        let dir = TempDir::new().unwrap();
        let file = write_file(&dir, "empty.txt", "");
        let backend = HeuristicBackend::new();

        assert!(backend.outline(&file).unwrap().is_empty());
    }

    #[test]
    fn get_region_text_returns_error_for_unknown_id() {
        let dir = TempDir::new().unwrap();
        let file = write_file(&dir, "sample.txt", "hello:\n  world\n");
        let backend = HeuristicBackend::new();

        let result = backend.get_region_text(&file, "h-0000000000000000-99");
        assert!(result.is_err());
    }

    #[test]
    fn code_intelligence_get_symbols_works() {
        let dir = TempDir::new().unwrap();
        let file = write_file(&dir, "demo.rs", "fn main() {\n    println!(\"hi\");\n}\n");

        let backend = HeuristicBackend::new();
        let symbols = backend.get_symbols(&file).unwrap();
        assert!(!symbols.is_empty());
        assert!(
            symbols[0]
                .signature
                .as_ref()
                .unwrap()
                .contains("[heuristic]")
        );
    }
}
