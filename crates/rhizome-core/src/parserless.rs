use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::{
    BackendCapabilities, CodeIntelligence, Diagnostic, Location, Position, Result, RhizomeError,
    Symbol, SymbolKind,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ParserlessRegion {
    pub region_id: String,
    pub line: u32,
    pub line_end: u32,
    pub depth: u32,
    pub label: String,
}

#[derive(Debug, Clone)]
struct CandidateRegion {
    line_index: usize,
    line_end_index: usize,
    depth: usize,
    label: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ParserlessBackend;

impl ParserlessBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub fn outline(&self, file: &Path) -> Result<Vec<ParserlessRegion>> {
        let source = std::fs::read_to_string(file)?;
        Ok(self.outline_from_source(&source))
    }

    pub fn get_region_text(&self, file: &Path, region_id: &str) -> Result<String> {
        let source = std::fs::read_to_string(file)?;
        let outline = self.outline_from_source(&source);
        let region = outline
            .iter()
            .find(|region| region.region_id == region_id)
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

    fn outline_from_source(&self, source: &str) -> Vec<ParserlessRegion> {
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

            let indent_depth = indent_depth(line);
            let leading_closers = trimmed
                .chars()
                .take_while(|c| matches!(c, '}' | ')' | ']'))
                .count();
            let effective_depth =
                indent_depth.saturating_add(bracket_depth.saturating_sub(leading_closers));

            if is_structural_line(trimmed, indent_depth, effective_depth) {
                regions.push(CandidateRegion {
                    line_index,
                    line_end_index: line_index,
                    depth: effective_depth,
                    label: compact_label(trimmed),
                });
            }

            bracket_depth = update_bracket_depth(bracket_depth, trimmed);
        }

        if regions.is_empty()
            && let Some((line_index, line)) = lines
                .iter()
                .enumerate()
                .find(|(_, line)| !line.trim().is_empty())
        {
            regions.push(CandidateRegion {
                line_index,
                line_end_index: lines.len().saturating_sub(1),
                depth: 0,
                label: compact_label(line.trim()),
            });
        }

        finalize_region_bounds(&mut regions, lines.len().saturating_sub(1));

        regions
            .into_iter()
            .map(|region| ParserlessRegion {
                region_id: format!("region-{}", region.line_index + 1),
                line: (region.line_index + 1) as u32,
                line_end: (region.line_end_index + 1) as u32,
                depth: region.depth as u32,
                label: region.label,
            })
            .collect()
    }
}

impl CodeIntelligence for ParserlessBackend {
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
                signature: Some(format!("{} [parserless]", region.region_id)),
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
            "parserless backend only supports outline-style queries".into(),
        ))
    }

    fn search_symbols(&self, _pattern: &str, _project_root: &Path) -> Result<Vec<Symbol>> {
        Err(RhizomeError::NotSupported(
            "parserless backend does not support workspace symbol search".into(),
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

fn finalize_region_bounds(regions: &mut [CandidateRegion], last_line_index: usize) {
    let mut open: Vec<usize> = Vec::new();

    for current_index in 0..regions.len() {
        while let Some(previous_index) = open.last().copied() {
            if regions[previous_index].depth >= regions[current_index].depth {
                let previous_index = open.pop().unwrap();
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
    fn parserless_outline_is_stable_for_rust_like_source() {
        let dir = TempDir::new().unwrap();
        let file = write_file(
            &dir,
            "sample.rs",
            "pub struct Config {\n    value: String,\n}\n\nimpl Config {\n    pub fn new() -> Self {\n        Self { value: String::new() }\n    }\n}\n\npub fn process(config: &Config) {\n    println!(\"{}\", config.value);\n}\n",
        );

        let backend = ParserlessBackend::new();
        let first = backend.outline(&file).unwrap();
        let second = backend.outline(&file).unwrap();

        assert_eq!(first, second);
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
                .all(|region| region.region_id.starts_with("region-"))
        );
    }

    #[test]
    fn parserless_region_reads_full_block() {
        let dir = TempDir::new().unwrap();
        let file = write_file(
            &dir,
            "sample.txt",
            "section:\n  child: true\nnext:\n  value: 1\n",
        );

        let backend = ParserlessBackend::new();
        let text = backend.get_region_text(&file, "region-1").unwrap();
        assert!(text.contains("section:"));
        assert!(text.contains("child: true"));
        assert!(!text.contains("next:"));
    }

    #[test]
    fn empty_file_has_no_outline() {
        let dir = TempDir::new().unwrap();
        let file = write_file(&dir, "empty.txt", "");
        let backend = ParserlessBackend::new();

        assert!(backend.outline(&file).unwrap().is_empty());
    }
}
