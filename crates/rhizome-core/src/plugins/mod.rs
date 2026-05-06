mod toml_analyzer;

use crate::plugin::PluginRegistry;

pub use toml_analyzer::TomlAnalyzerPlugin;

/// Register all built-in plugins with the given registry.
pub fn register_builtins(registry: &mut PluginRegistry) {
    registry.register(Box::new(TomlAnalyzerPlugin::new()));
}
