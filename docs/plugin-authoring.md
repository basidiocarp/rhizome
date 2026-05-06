# Plugin Authoring Guide

This guide covers how to write and register an analyzer plugin for Rhizome.

## Quick Start: The TOML Plugin Example

The built-in TOML plugin is a good starting point. It's located in:
- Implementation: `crates/rhizome-core/src/plugins/toml_analyzer.rs`
- Registration: `crates/rhizome-core/src/plugins/mod.rs`

## Step-by-Step: Building a Plugin

### 1. Define Your Plugin Struct

Create a new file in `crates/rhizome-core/src/plugins/`:

```rust
// crates/rhizome-core/src/plugins/my_format_analyzer.rs

use std::path::Path;
use crate::error::Result;
use crate::plugin::{AnalyzerPlugin, FileRegion};
use crate::symbol::{Location, Symbol, SymbolKind};

pub struct MyFormatAnalyzerPlugin;

impl MyFormatAnalyzerPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MyFormatAnalyzerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzerPlugin for MyFormatAnalyzerPlugin {
    fn id(&self) -> &str {
        "my-format-analyzer"
    }

    fn supported_extensions(&self) -> &[&str] {
        &["myformat", "myfmt"]
    }

    fn get_structure(&self, path: &Path) -> Result<Vec<Symbol>> {
        let content = std::fs::read_to_string(path)?;
        // Parse and extract symbols
        todo!()
    }

    fn get_region(&self, path: &Path, region_id: &str) -> Result<Option<FileRegion>> {
        let content = std::fs::read_to_string(path)?;
        // Find and return region by ID
        todo!()
    }
}
```

### 2. Implement the AnalyzerPlugin Trait

#### id()

Return a unique identifier. Use kebab-case (e.g., "my-format-analyzer", "json-schema-analyzer").

#### supported_extensions()

Return a slice of file extensions (without dots) that your plugin handles.

```rust
fn supported_extensions(&self) -> &[&str] {
    &["toml", "ini"]  // Handle TOML and INI files
}
```

#### get_structure()

Extract the top-level structural elements. Return a `Vec<Symbol>` where each symbol represents a major section, definition, or element.

**Good practices:**
- Use `SymbolKind` values that match the semantic meaning (Module for sections, Type for class-like constructs, Constant for config keys)
- Include accurate line and column information in `Location`
- Handle parse errors gracefully (return `Err`, not panic)
- Return an empty `Vec` rather than `None` if no symbols are found

```rust
fn get_structure(&self, path: &Path) -> Result<Vec<Symbol>> {
    let content = std::fs::read_to_string(path)?;
    let file_path = path.display().to_string();

    let mut symbols = vec![];
    for (idx, line) in content.lines().enumerate() {
        if let Some(name) = parse_section_header(line) {
            symbols.push(Symbol {
                name,
                kind: SymbolKind::Module,
                location: Location {
                    file_path: file_path.clone(),
                    line_start: (idx + 1) as u32,
                    line_end: (idx + 1) as u32,
                    column_start: 0,
                    column_end: line.len() as u32,
                },
                scope_path: vec![],
                signature: None,
                doc_comment: None,
                children: vec![],
            });
        }
    }
    Ok(symbols)
}
```

#### get_region()

Return a specific region of code identified by `region_id`. The ID format is up to your plugin (e.g., "section:name", "class:MyClass").

Return `Ok(None)` if the region doesn't exist. Return an error only if the lookup itself fails.

```rust
fn get_region(&self, path: &Path, region_id: &str) -> Result<Option<FileRegion>> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    // Parse region_id according to your scheme
    if let Some((section, _)) = region_id.split_once(':') {
        for (idx, line) in lines.iter().enumerate() {
            if line.contains(section) {
                // Find the extent of this section
                let end_idx = idx + 1; // or calculate based on your format
                return Ok(Some(FileRegion {
                    id: region_id.to_string(),
                    content: lines[idx..=end_idx].join("\n"),
                    start_line: (idx + 1) as u32,
                    end_line: (end_idx + 1) as u32,
                }));
            }
        }
    }
    Ok(None)
}
```

#### get_symbols() (optional)

If your plugin's symbol extraction differs from structure extraction, override this method:

```rust
fn get_symbols(&self, path: &Path) -> Result<Vec<Symbol>> {
    // Default implementation calls get_structure()
    self.get_structure(path)
}
```

### 3. Register the Plugin

Add your plugin to `crates/rhizome-core/src/plugins/mod.rs`:

```rust
mod my_format_analyzer;
mod toml_analyzer;

use crate::plugin::PluginRegistry;
pub use my_format_analyzer::MyFormatAnalyzerPlugin;
pub use toml_analyzer::TomlAnalyzerPlugin;

pub fn register_builtins(registry: &mut PluginRegistry) {
    registry.register(Box::new(TomlAnalyzerPlugin::new()));
    registry.register(Box::new(MyFormatAnalyzerPlugin::new()));
}
```

### 4. Write Tests

Add unit tests in the same file as your plugin:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn plugin_id_is_correct() {
        let plugin = MyFormatAnalyzerPlugin::new();
        assert_eq!(plugin.id(), "my-format-analyzer");
    }

    #[test]
    fn plugin_supports_extensions() {
        let plugin = MyFormatAnalyzerPlugin::new();
        assert!(plugin.supported_extensions().contains(&"myformat"));
    }

    #[test]
    fn get_structure_extracts_sections() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "[section1]\nkey1 = \"value1\"")?;
        writeln!(file, "[section2]\nkey2 = \"value2\"")?;
        file.flush()?;

        let plugin = MyFormatAnalyzerPlugin::new();
        let symbols = plugin.get_structure(file.path())?;

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "section1");
        assert_eq!(symbols[1].name, "section2");
        Ok(())
    }

    #[test]
    fn get_region_returns_section_content() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "[section]\nkey = \"value\"")?;
        file.flush()?;

        let plugin = MyFormatAnalyzerPlugin::new();
        let region = plugin.get_region(file.path(), "section:section")?;

        assert!(region.is_some());
        let r = region.unwrap();
        assert!(r.content.contains("key"));
        Ok(())
    }

    #[test]
    fn get_region_returns_none_for_missing_section() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "[existing]\nkey = \"value\"")?;
        file.flush()?;

        let plugin = MyFormatAnalyzerPlugin::new();
        let region = plugin.get_region(file.path(), "section:missing")?;

        assert!(region.is_none());
        Ok(())
    }
}
```

Run tests with:

```bash
cargo test --lib plugins::my_format_analyzer
```

### 5. Verify with `rhizome plugin list`

After registration, list your plugin:

```bash
rhizome plugin list
```

Output should include:

```
built-in: my-format-analyzer  [myformat, myfmt]
```

## The TOML Plugin Walkthrough

The built-in TOML plugin serves as a reference implementation. Here's how it works:

1. **Parsing**: It reads the file line-by-line looking for lines like `[section]`.
2. **Symbol Extraction**: Each section header becomes a `Module`-kind symbol.
3. **Region Extraction**: Regions are identified as `"table:<name>"` and contain all lines under that header.
4. **Error Handling**: File read errors are propagated; parse errors are gracefully skipped.

## External Plugins (Future Work)

When external plugin support is implemented, the workflow will be:

1. Create a Rust crate with your plugin implementation.
2. Expose a C-compatible initialization function via `#[no_mangle]`.
3. Place your compiled `.so`, `.dylib`, or `.dll` in `$RHIZOME_PLUGIN_PATH`.
4. Set `RHIZOME_PLUGIN_PATH` when running Rhizome.

The ABI will be stable and versioned, allowing plugins to be compiled separately from Rhizome.

Currently, external plugins are not supported. This guide covers only built-in plugins.

## Common Patterns

### Stateful Parsing

If your plugin needs to maintain state during parsing (e.g., tracking nesting levels):

```rust
pub struct MyFormatAnalyzerPlugin {
    // No internal state needed for simple cases
}

fn parse_with_state(content: &str) -> Result<Vec<Symbol>> {
    let mut symbols = vec![];
    let mut nesting_level = 0;

    for (idx, line) in content.lines().enumerate() {
        // Track state and build symbols
    }

    Ok(symbols)
}
```

### Reusing Existing Parsers

If a TOML or similar crate already parses your format, use it:

```rust
fn get_structure(&self, path: &Path) -> Result<Vec<Symbol>> {
    let content = std::fs::read_to_string(path)?;
    let parsed: toml::Table = toml::from_str(&content)?;

    let mut symbols = vec![];
    for (key, _) in parsed.iter() {
        symbols.push(Symbol {
            name: key.to_string(),
            kind: SymbolKind::Module,
            // ... location and other fields
        });
    }
    Ok(symbols)
}
```

### Hierarchical Symbols

If your format has nested structures, use the `children` field:

```rust
let mut root = Symbol {
    name: "root".to_string(),
    kind: SymbolKind::Module,
    children: vec![
        Symbol {
            name: "child1".to_string(),
            kind: SymbolKind::Field,
            children: vec![],
            // ...
        },
    ],
    // ...
};
```

## Debugging

1. **Enable logging**: Set `RUST_LOG=debug` when testing your plugin.
2. **Print symbols**: Add `eprintln!("{:#?}", symbol)` in your test to inspect the structure.
3. **Check coverage**: Write tests for edge cases (empty files, malformed input, very large files).

## Performance Considerations

- **File I/O**: Read the file once if possible; avoid multiple reads.
- **Parsing**: Use simple line-based parsing for text formats; consider using established parser crates for complex formats.
- **Memory**: Avoid building large intermediate data structures; stream or batch-process if needed.
- **Symbols**: Return only the most important structural elements. Deep hierarchies are fine, but avoid returning every single token.

## API Reference

### AnalyzerPlugin Trait

```rust
pub trait AnalyzerPlugin: Send + Sync {
    fn id(&self) -> &str;
    fn supported_extensions(&self) -> &[&str];
    fn get_structure(&self, path: &Path) -> Result<Vec<Symbol>>;
    fn get_region(&self, path: &Path, region_id: &str) -> Result<Option<FileRegion>>;
    fn get_symbols(&self, path: &Path) -> Result<Vec<Symbol>> {
        self.get_structure(path)
    }
}
```

### Key Types

- `Symbol`: A named structural element with location, kind, scope path, and optional children.
- `Location`: File path and line/column range.
- `SymbolKind`: Enum of structural element types (Function, Module, Type, etc.).
- `FileRegion`: A named block of text with start/end lines and content.
- `Result<T>`: `std::result::Result<T, RhizomeError>` for error handling.

### Constants

- `PLUGIN_API_VERSION: u32 = 1`: Current plugin interface version.

## Support and Contribution

To contribute a new built-in plugin:

1. Open an issue describing the format and use case.
2. Implement the plugin following this guide.
3. Add tests and documentation.
4. Submit a pull request against the `rhizome` repository.

For questions or issues, refer to the `plugin-interface.md` documentation or the Rhizome repository issues.
