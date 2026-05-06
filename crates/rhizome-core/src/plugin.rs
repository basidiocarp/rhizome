use std::path::Path;

use crate::error::Result;
use crate::symbol::Symbol;

/// API version for plugins. Increment on breaking changes.
pub const PLUGIN_API_VERSION: u32 = 1;

/// A region of code identified by a plugin.
#[derive(Debug, Clone)]
pub struct FileRegion {
    pub id: String,
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// Trait for analyzing code in specific file formats.
///
/// Plugins are registered at startup and tried as a fallback tier
/// after LSP but before the heuristic structural parser.
/// If a plugin returns an error, the backend selector proceeds to the next tier.
pub trait AnalyzerPlugin: Send + Sync {
    /// Unique identifier for this plugin (e.g., "toml-analyzer").
    fn id(&self) -> &str;

    /// File extensions this plugin claims (without dots, e.g., ["toml"]).
    fn supported_extensions(&self) -> &[&str];

    /// Extract structural symbols from a file.
    fn get_structure(&self, path: &Path) -> Result<Vec<Symbol>>;

    /// Retrieve a specific region by ID.
    fn get_region(&self, path: &Path, region_id: &str) -> Result<Option<FileRegion>>;

    /// Extract all symbols from a file (optional override of get_structure).
    fn get_symbols(&self, path: &Path) -> Result<Vec<Symbol>> {
        self.get_structure(path)
    }
}

/// Registry for analyzer plugins.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn AnalyzerPlugin>>,
}

impl PluginRegistry {
    /// Create an empty plugin registry.
    pub fn new() -> Self {
        Self { plugins: vec![] }
    }

    /// Register a new plugin.
    pub fn register(&mut self, plugin: Box<dyn AnalyzerPlugin>) {
        self.plugins.push(plugin);
    }

    /// Find a plugin that supports the given file extension.
    pub fn find_for_extension(&self, ext: &str) -> Option<&dyn AnalyzerPlugin> {
        self.plugins
            .iter()
            .find(|p| p.supported_extensions().contains(&ext))
            .map(|b| b.as_ref())
    }

    /// Find a plugin by its unique ID.
    pub fn find_for_id(&self, id: &str) -> Option<&dyn AnalyzerPlugin> {
        self.plugins
            .iter()
            .find(|p| p.id() == id)
            .map(|b| b.as_ref())
    }

    /// List all registered plugins.
    pub fn list(&self) -> &[Box<dyn AnalyzerPlugin>] {
        &self.plugins
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPlugin {
        id: String,
        exts: Vec<&'static str>,
    }

    impl AnalyzerPlugin for MockPlugin {
        fn id(&self) -> &str {
            &self.id
        }

        fn supported_extensions(&self) -> &[&str] {
            &self.exts
        }

        fn get_structure(&self, _path: &Path) -> Result<Vec<Symbol>> {
            Ok(vec![])
        }

        fn get_region(&self, _path: &Path, _region_id: &str) -> Result<Option<FileRegion>> {
            Ok(None)
        }
    }

    #[test]
    fn registry_starts_empty() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.list().len(), 0);
    }

    #[test]
    fn registry_can_register_plugins() {
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(MockPlugin {
            id: "test".to_string(),
            exts: vec!["toml"],
        });
        registry.register(plugin);
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn find_for_extension_returns_matching_plugin() {
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(MockPlugin {
            id: "toml-analyzer".to_string(),
            exts: vec!["toml"],
        });
        registry.register(plugin);

        let found = registry.find_for_extension("toml");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), "toml-analyzer");
    }

    #[test]
    fn find_for_extension_returns_none_for_unsupported() {
        let registry = PluginRegistry::new();
        let found = registry.find_for_extension("xyz");
        assert!(found.is_none());
    }
}
