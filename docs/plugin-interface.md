# Rhizome Analyzer Plugin Interface

This document describes the design and usage of the Rhizome analyzer plugin system.

## Overview

The analyzer plugin system extends Rhizome's code analysis capabilities for specialized file formats where tree-sitter or LSP may not be available. Plugins are registered at startup and are tried as a fallback tier in the backend selection chain:

**Resolution tiers (in order):**
1. Tree-sitter (fast, always available for supported languages)
2. LSP (rich cross-file analysis, if server is available)
3. **Analyzer Plugin** (specialized format handlers)
4. Heuristic (indentation-based structural fallback)

## Plugin Tier Behavior

When a tool requests analysis of a file:
1. If tree-sitter can handle it, tree-sitter is used.
2. If LSP is required or preferred and available, LSP is used.
3. If the file extension matches a registered plugin, the plugin is tried.
4. If the plugin returns an error, fallthrough to the next tier (usually heuristic).
5. If no other tier can handle it, heuristic fallback is used.

## The AnalyzerPlugin Trait

All plugins implement the `AnalyzerPlugin` trait defined in `crates/rhizome-core/src/plugin.rs`:

```rust
pub trait AnalyzerPlugin: Send + Sync {
    /// Unique identifier for this plugin (e.g., "toml-analyzer").
    fn id(&self) -> &str;

    /// File extensions this plugin claims (without dots, e.g., ["toml"]).
    fn supported_extensions(&self) -> &[&str];

    /// Extract structural symbols from a file.
    fn get_structure(&self, path: &Path) -> Result<Vec<Symbol>>;

    /// Retrieve a specific region by ID.
    fn get_region(&self, path: &Path, region_id: &str) -> Result<Option<FileRegion>>;

    /// Extract all symbols from a file (optional override).
    fn get_symbols(&self, path: &Path) -> Result<Vec<Symbol>> {
        self.get_structure(path)
    }
}
```

## Core Types

### Symbol

A `Symbol` represents a named structural element in a file (function, class, table, etc.):

```rust
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub scope_path: Vec<String>,
    pub signature: Option<String>,
    pub doc_comment: Option<String>,
    pub children: Vec<Symbol>,
}
```

### FileRegion

A `FileRegion` represents a contiguous block of text identified by a plugin:

```rust
pub struct FileRegion {
    pub id: String,
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
}
```

### SymbolKind

Plugins should choose the most appropriate kind for each symbol:

```rust
pub enum SymbolKind {
    Function, Method, Class, Struct, Enum, Interface, Trait, Type,
    Constant, Variable, Module, Import, Property, Field,
}
```

## Plugin Registration

Plugins are registered in `crates/rhizome-core/src/plugins/register_builtins()`:

```rust
pub fn register_builtins(registry: &mut PluginRegistry) {
    registry.register(Box::new(TomlAnalyzerPlugin::new()));
    // Add more built-in plugins here
}
```

This function is called during `BackendSelector::new()`, so all built-in plugins are available when Rhizome starts.

## Built-in Plugins

### TomlAnalyzerPlugin

Analyzes TOML configuration files. Extracts table headers as `Module`-kind symbols.

- **ID**: `toml-analyzer`
- **Extensions**: `["toml"]`
- **Symbols**: One per top-level table header (e.g., `[package]`, `[dependencies]`)
- **Regions**: Identified as `table:<name>` containing the content under that header

Example:

```toml
[package]
name = "myapp"

[dependencies]
serde = "1"
```

Yields:
- Symbol: "package" (Module kind)
- Symbol: "dependencies" (Module kind)
- Region "table:package": Lines 1-2
- Region "table:dependencies": Lines 4-5

## API Versioning

The plugin interface is versioned via `PLUGIN_API_VERSION` (currently `1`).

**Breaking changes:**
- Changes to method signatures
- Changes to return types
- Changes to core types like `Symbol` or `FileRegion`

**Non-breaking changes:**
- Adding new `SymbolKind` variants
- Adding optional fields to `Symbol` (via `#[serde(default)]`)
- Adding new plugin methods with default implementations

**Compatibility strategy:**
When a breaking change is necessary, `PLUGIN_API_VERSION` is incremented. Plugins are responsible for checking this constant and returning an error if incompatible with the installed Rhizome version. External plugins will need to declare their API version requirement.

## External Plugin Loading

External plugin support is planned but not yet implemented. When enabled via the `RHIZOME_PLUGIN_PATH` environment variable, Rhizome will:

1. Search the specified directory for plugin shared libraries (`.so`, `.dylib`, `.dll`)
2. Load and initialize each plugin using a standardized ABI
3. Register them with the same registry as built-in plugins

The ABI will use C-compatible function pointers and stable types to ensure compatibility across compiler versions.

Currently, setting `RHIZOME_PLUGIN_PATH` logs an informational message but does not load plugins.

## Design Decisions

1. **Object-safe trait**: Plugins are stored as trait objects (`Box<dyn AnalyzerPlugin>`), allowing dynamic dispatch without generic overhead.

2. **Fallible analysis**: `Result`-based returns allow plugins to gracefully decline or error, enabling tier fallthrough.

3. **In-memory registry**: The registry is populated at startup and kept in memory. No database or file-based configuration is used.

4. **Minimal interface**: The trait defines only essential methods. Plugins needing to store state or configuration can do so internally.

5. **Plugin tier placement**: Between LSP and heuristic ensures plugins can be used for format-specific analysis while preserving heuristic as the final fallback for any text file.

## Testing a Plugin

Unit tests should verify:

1. Plugin ID and extension claims are correct
2. Extraction produces expected symbols for minimal inputs
3. Region retrieval works correctly
4. Error cases are handled gracefully

Example test for a custom plugin:

```rust
#[test]
fn my_plugin_extracts_expected_symbols() -> Result<()> {
    let mut file = NamedTempFile::new()?;
    writeln!(file, "[section]\nkey = \"value\"")?;
    file.flush()?;

    let plugin = MyPlugin::new();
    let symbols = plugin.get_structure(file.path())?;

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "section");
    Ok(())
}
```

## Future Work

- External shared-library plugin loading with ABI stability guarantees
- Plugin lifecycle hooks (on-register, on-unregister, on-reload)
- Per-plugin configuration and options
- Plugin dependency resolution
- Performance profiling and plugin performance metrics
