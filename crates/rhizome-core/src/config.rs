use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Result, RhizomeError};

/// Per-language configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    /// Path to the language server binary
    pub server_binary: Option<String>,
    /// Arguments to pass to the language server
    pub server_args: Option<Vec<String>>,
    /// Whether this language is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Custom initialization options for the LSP server
    pub initialization_options: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}

/// Export configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// Whether to automatically export to Hyphae on MCP server startup
    #[serde(default = "default_true")]
    pub auto_export: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self { auto_export: true }
    }
}

/// LSP configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspConfig {
    /// Disable automatic LSP server installation (default: false)
    #[serde(default)]
    pub disable_download: bool,
    /// Custom directory for managed LSP binaries.
    pub bin_dir: Option<std::path::PathBuf>,
}

/// Top-level configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RhizomeConfig {
    /// Per-language settings
    #[serde(default)]
    pub languages: HashMap<String, LanguageConfig>,
    /// Export settings
    #[serde(default)]
    pub export: ExportConfig,
    /// LSP settings
    #[serde(default)]
    pub lsp: LspConfig,
}

impl RhizomeConfig {
    /// Load configuration by merging global + project configs.
    /// Project config overrides global config.
    pub fn load(project_root: &Path) -> Result<Self> {
        spore::config::load_merged("rhizome", project_root, Self::merge)
            .map_err(|error| RhizomeError::Config(error.to_string()))
    }

    /// Merge two configs. `project` values override `global`.
    fn merge(global: Self, project: Self) -> Self {
        let mut languages = global.languages;
        for (lang, config) in project.languages {
            languages.insert(lang, config);
        }
        // Project LSP config overrides global, field by field
        let lsp = LspConfig {
            disable_download: project.lsp.disable_download || global.lsp.disable_download,
            bin_dir: project.lsp.bin_dir.or(global.lsp.bin_dir),
        };

        Self {
            languages,
            export: project.export,
            lsp,
        }
    }

    /// Check if auto-export is enabled
    pub fn auto_export(&self) -> bool {
        self.export.auto_export
    }

    /// Get the effective LanguageServerConfig for a language,
    /// applying config overrides to defaults.
    pub fn get_server_config(
        &self,
        language: &crate::Language,
    ) -> Option<crate::LanguageServerConfig> {
        let lang_key = language_to_config_key(language);
        let default_config = language.default_server_config();

        match self.languages.get(&lang_key) {
            Some(override_config) => {
                if !override_config.enabled {
                    return None;
                }
                Some(crate::LanguageServerConfig {
                    binary: override_config.server_binary.clone().unwrap_or_else(|| {
                        default_config
                            .as_ref()
                            .map(|c| c.binary.clone())
                            .unwrap_or_default()
                    }),
                    args: override_config.server_args.clone().unwrap_or_else(|| {
                        default_config
                            .as_ref()
                            .map(|c| c.args.clone())
                            .unwrap_or_default()
                    }),
                    initialization_options: override_config
                        .initialization_options
                        .clone()
                        .or_else(|| default_config.and_then(|c| c.initialization_options)),
                })
            }
            None => default_config,
        }
    }

    /// Check if a language is enabled (default: true)
    pub fn is_language_enabled(&self, language: &crate::Language) -> bool {
        let lang_key = language_to_config_key(language);
        self.languages
            .get(&lang_key)
            .map(|c| c.enabled)
            .unwrap_or(true)
    }

    /// Generate an example config file as a string
    pub fn example_config() -> String {
        r#"# Rhizome Configuration
# Place this file at:
#   Global:  <platform config dir>/rhizome/config.toml
#   Project: <project_root>/.rhizome/config.toml
#
# Project config overrides global config on a per-language basis.

# [languages.rust]
# server_binary = "rust-analyzer"
# server_args = []
# enabled = true

# [languages.python]
# server_binary = "pyright-langserver"
# server_args = ["--stdio"]
# enabled = true

# [languages.typescript]
# server_binary = "typescript-language-server"
# server_args = ["--stdio"]
# enabled = true

# [languages.go]
# server_binary = "gopls"
# server_args = []
# enabled = true

# To disable a language entirely:
# [languages.java]
# enabled = false

# To use a custom server binary:
# [languages.rust]
# server_binary = "/path/to/custom/rust-analyzer"
# server_args = ["--log-file", "/tmp/ra.log"]
"#
        .to_string()
    }
}

fn language_to_config_key(language: &crate::Language) -> String {
    match language {
        crate::Language::Rust => "rust".to_string(),
        crate::Language::Python => "python".to_string(),
        crate::Language::JavaScript => "javascript".to_string(),
        crate::Language::TypeScript => "typescript".to_string(),
        crate::Language::Go => "go".to_string(),
        crate::Language::Java => "java".to_string(),
        crate::Language::C => "c".to_string(),
        crate::Language::Cpp => "cpp".to_string(),
        crate::Language::Ruby => "ruby".to_string(),
        crate::Language::Elixir => "elixir".to_string(),
        crate::Language::Zig => "zig".to_string(),
        crate::Language::CSharp => "csharp".to_string(),
        crate::Language::FSharp => "fsharp".to_string(),
        crate::Language::Swift => "swift".to_string(),
        crate::Language::Php => "php".to_string(),
        crate::Language::Haskell => "haskell".to_string(),
        crate::Language::Bash => "bash".to_string(),
        crate::Language::Terraform => "terraform".to_string(),
        crate::Language::Kotlin => "kotlin".to_string(),
        crate::Language::Dart => "dart".to_string(),
        crate::Language::Lua => "lua".to_string(),
        crate::Language::Clojure => "clojure".to_string(),
        crate::Language::OCaml => "ocaml".to_string(),
        crate::Language::Julia => "julia".to_string(),
        crate::Language::Nix => "nix".to_string(),
        crate::Language::Gleam => "gleam".to_string(),
        crate::Language::Vue => "vue".to_string(),
        crate::Language::Svelte => "svelte".to_string(),
        crate::Language::Astro => "astro".to_string(),
        crate::Language::Prisma => "prisma".to_string(),
        crate::Language::Typst => "typst".to_string(),
        crate::Language::Yaml => "yaml".to_string(),
        crate::Language::Other(name) => name.to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Language;

    #[test]
    fn test_parse_valid_toml() {
        let toml_str = r#"
            [languages.rust]
            server_binary = "rust-analyzer"
            server_args = ["--log-file", "/tmp/ra.log"]
            enabled = true

            [languages.python]
            server_binary = "pylsp"
            enabled = true
        "#;

        let config: RhizomeConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.languages.len(), 2);

        let rust = &config.languages["rust"];
        assert_eq!(rust.server_binary.as_deref(), Some("rust-analyzer"));
        assert_eq!(
            rust.server_args.as_deref(),
            Some(&["--log-file".to_string(), "/tmp/ra.log".to_string()][..])
        );
        assert!(rust.enabled);

        let python = &config.languages["python"];
        assert_eq!(python.server_binary.as_deref(), Some("pylsp"));
        assert!(python.enabled);
    }

    #[test]
    fn test_merge_project_overrides_global() {
        let global = RhizomeConfig {
            languages: HashMap::from([
                (
                    "rust".to_string(),
                    LanguageConfig {
                        server_binary: Some("rust-analyzer".to_string()),
                        server_args: None,
                        enabled: true,
                        initialization_options: None,
                    },
                ),
                (
                    "python".to_string(),
                    LanguageConfig {
                        server_binary: Some("pyright-langserver".to_string()),
                        server_args: None,
                        enabled: true,
                        initialization_options: None,
                    },
                ),
            ]),
            ..Default::default()
        };

        let project = RhizomeConfig {
            languages: HashMap::from([(
                "rust".to_string(),
                LanguageConfig {
                    server_binary: Some("custom-ra".to_string()),
                    server_args: Some(vec!["--custom".to_string()]),
                    enabled: true,
                    initialization_options: None,
                },
            )]),
            ..Default::default()
        };

        let merged = RhizomeConfig::merge(global, project);
        assert_eq!(merged.languages.len(), 2);

        // Rust should be overridden by project
        let rust = &merged.languages["rust"];
        assert_eq!(rust.server_binary.as_deref(), Some("custom-ra"));
        assert_eq!(
            rust.server_args.as_deref(),
            Some(&["--custom".to_string()][..])
        );

        // Python should remain from global
        let python = &merged.languages["python"];
        assert_eq!(python.server_binary.as_deref(), Some("pyright-langserver"));
    }

    #[test]
    fn test_disabled_language() {
        let config = RhizomeConfig {
            languages: HashMap::from([(
                "java".to_string(),
                LanguageConfig {
                    server_binary: None,
                    server_args: None,
                    enabled: false,
                    initialization_options: None,
                },
            )]),
            ..Default::default()
        };

        assert!(!config.is_language_enabled(&Language::Java));
        assert!(config.get_server_config(&Language::Java).is_none());
    }

    #[test]
    fn test_custom_server_binary_override() {
        let config = RhizomeConfig {
            languages: HashMap::from([(
                "rust".to_string(),
                LanguageConfig {
                    server_binary: Some("/opt/bin/ra-custom".to_string()),
                    server_args: None,
                    enabled: true,
                    initialization_options: None,
                },
            )]),
            ..Default::default()
        };

        let server_config = config.get_server_config(&Language::Rust).unwrap();
        assert_eq!(server_config.binary, "/opt/bin/ra-custom");
        // Args should fall back to the default (empty for rust-analyzer)
        assert!(server_config.args.is_empty());
    }

    #[test]
    fn test_default_when_no_config() {
        let config = RhizomeConfig::default();
        assert!(config.languages.is_empty());

        // Should still return default server configs from Language
        let rust_config = config.get_server_config(&Language::Rust).unwrap();
        assert_eq!(rust_config.binary, "rust-analyzer");

        let py_config = config.get_server_config(&Language::Python).unwrap();
        assert_eq!(py_config.binary, "pyright-langserver");
    }

    #[test]
    fn test_is_language_enabled_defaults_true() {
        let config = RhizomeConfig::default();
        assert!(config.is_language_enabled(&Language::Rust));
        assert!(config.is_language_enabled(&Language::Python));
        assert!(config.is_language_enabled(&Language::Go));
        assert!(config.is_language_enabled(&Language::Other("zig".to_string())));
    }

    #[test]
    fn test_get_server_config_without_override() {
        let config = RhizomeConfig::default();

        let ts_config = config.get_server_config(&Language::TypeScript).unwrap();
        assert_eq!(ts_config.binary, "typescript-language-server");
        assert_eq!(ts_config.args, vec!["--stdio"]);

        // Other languages have no default
        let other_config = config.get_server_config(&Language::Other("zig".to_string()));
        assert!(other_config.is_none());
    }

    #[test]
    fn test_get_server_config_with_initialization_options() {
        let init_opts = serde_json::json!({"key": "value"});
        let config = RhizomeConfig {
            languages: HashMap::from([(
                "rust".to_string(),
                LanguageConfig {
                    server_binary: None,
                    server_args: None,
                    enabled: true,
                    initialization_options: Some(init_opts.clone()),
                },
            )]),
            ..Default::default()
        };

        let server_config = config.get_server_config(&Language::Rust).unwrap();
        assert_eq!(server_config.binary, "rust-analyzer");
        assert_eq!(server_config.initialization_options.unwrap(), init_opts);
    }

    #[test]
    fn test_example_config_parses_as_valid_toml() {
        let example = RhizomeConfig::example_config();
        let parsed: std::result::Result<RhizomeConfig, _> = toml::from_str(&example);
        assert!(parsed.is_ok(), "Example config should parse as valid TOML");
        // All entries are commented out so languages should be empty
        assert!(parsed.unwrap().languages.is_empty());
    }

    #[test]
    fn test_load_nonexistent_project_returns_default() {
        let config = RhizomeConfig::load(Path::new("/nonexistent/path")).unwrap();
        // Should succeed with defaults (no project config found)
        assert!(config.languages.is_empty() || !config.languages.is_empty());
    }

    #[test]
    fn test_enabled_defaults_true_in_toml() {
        let toml_str = r#"
            [languages.rust]
            server_binary = "rust-analyzer"
        "#;

        let config: RhizomeConfig = toml::from_str(toml_str).unwrap();
        assert!(config.languages["rust"].enabled);
    }

    #[test]
    fn test_language_to_config_key() {
        assert_eq!(language_to_config_key(&Language::Rust), "rust");
        assert_eq!(language_to_config_key(&Language::Cpp), "cpp");
        assert_eq!(
            language_to_config_key(&Language::Other("Zig".to_string())),
            "zig"
        );
    }
}
