use std::path::Path;

use crate::error::Result;
use crate::plugin::{AnalyzerPlugin, FileRegion};
use crate::symbol::{Location, Symbol, SymbolKind};

/// Analyzer for TOML files that extracts table headers as symbols.
pub struct TomlAnalyzerPlugin;

impl TomlAnalyzerPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TomlAnalyzerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzerPlugin for TomlAnalyzerPlugin {
    fn id(&self) -> &str {
        "toml-analyzer"
    }

    fn supported_extensions(&self) -> &[&str] {
        &["toml"]
    }

    fn get_structure(&self, path: &Path) -> Result<Vec<Symbol>> {
        let content = std::fs::read_to_string(path)?;
        extract_toml_tables(&content, path)
    }

    fn get_region(&self, path: &Path, region_id: &str) -> Result<Option<FileRegion>> {
        let content = std::fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();

        let tables = extract_table_regions(&lines)?;
        Ok(tables.into_iter().find(|t| t.id == region_id))
    }
}

/// Extract TOML table headers as symbols.
fn extract_toml_tables(content: &str, path: &Path) -> Result<Vec<Symbol>> {
    let lines: Vec<&str> = content.lines().collect();
    let mut symbols = vec![];

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.starts_with("[[") {
            let table_name = trimmed[1..trimmed.len() - 1].to_string();
            let location = Location {
                file_path: path.display().to_string(),
                line_start: (line_idx + 1) as u32,
                line_end: (line_idx + 1) as u32,
                column_start: 0,
                column_end: trimmed.len() as u32,
            };

            symbols.push(Symbol {
                name: table_name,
                kind: SymbolKind::Module,
                location,
                scope_path: vec![],
                signature: None,
                doc_comment: None,
                children: vec![],
            });
        }
    }

    Ok(symbols)
}

/// Extract TOML table regions as identified blocks of text.
///
/// Note: uses line-based parsing; inline comments on table headers
/// (e.g. `[package] # comment`) and values inside triple-quoted strings
/// that look like table headers are not handled.
fn extract_table_regions(lines: &[&str]) -> Result<Vec<FileRegion>> {
    let mut regions = vec![];
    let mut current_table: Option<(usize, String)> = None;

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.starts_with("[[") {
            if let Some((start_idx, table_name)) = current_table.take() {
                let table_content = lines[start_idx..line_idx].join("\n");
                regions.push(FileRegion {
                    id: format!("table:{}", table_name),
                    content: table_content,
                    start_line: start_idx as u32 + 1,
                    end_line: (line_idx - 1) as u32 + 1,
                });
            }

            let table_name = trimmed[1..trimmed.len() - 1].to_string();
            current_table = Some((line_idx, table_name));
        }
    }

    if let Some((start_idx, table_name)) = current_table {
        let table_content = lines[start_idx..].join("\n");
        regions.push(FileRegion {
            id: format!("table:{}", table_name),
            content: table_content,
            start_line: start_idx as u32 + 1,
            end_line: lines.len() as u32,
        });
    }

    Ok(regions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn toml_plugin_extracts_tables() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "[package]")?;
        writeln!(file, "name = \"foo\"")?;
        writeln!(file, "version = \"1.0.0\"")?;
        writeln!(file, "[dependencies]")?;
        writeln!(file, "serde = \"1\"")?;
        file.flush()?;

        let plugin = TomlAnalyzerPlugin::new();
        let symbols = plugin.get_structure(file.path())?;

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "package");
        assert_eq!(symbols[1].name, "dependencies");
        Ok(())
    }

    #[test]
    fn toml_plugin_id_is_correct() {
        let plugin = TomlAnalyzerPlugin::new();
        assert_eq!(plugin.id(), "toml-analyzer");
    }

    #[test]
    fn toml_plugin_supports_toml_extension() {
        let plugin = TomlAnalyzerPlugin::new();
        assert!(plugin.supported_extensions().contains(&"toml"));
    }

    #[test]
    fn toml_plugin_ignores_array_of_tables() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "[[bin]]")?;
        writeln!(file, "name = \"tool\"")?;
        writeln!(file, "[package]")?;
        writeln!(file, "name = \"foo\"")?;
        file.flush()?;

        let plugin = TomlAnalyzerPlugin::new();
        let symbols = plugin.get_structure(file.path())?;

        // [[bin]] must not appear; only [package] should be extracted
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "package");
        Ok(())
    }

    #[test]
    fn toml_plugin_get_region_returns_table_content() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "[package]")?;
        writeln!(file, "name = \"foo\"")?;
        writeln!(file, "[dependencies]")?;
        writeln!(file, "serde = \"1\"")?;
        file.flush()?;

        let plugin = TomlAnalyzerPlugin::new();
        let region = plugin.get_region(file.path(), "table:dependencies")?;

        assert!(region.is_some());
        let r = region.unwrap();
        assert_eq!(r.id, "table:dependencies");
        assert!(r.content.contains("serde"));
        Ok(())
    }
}
