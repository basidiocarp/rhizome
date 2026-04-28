use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::RhizomeConfig;
use crate::language::Language;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// What a tool needs from a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendRequirement {
    /// Tree-sitter is sufficient.
    TreeSitter,
    /// LSP is required — error if unavailable.
    RequiresLsp,
    /// LSP is preferred — fall back to tree-sitter if unavailable.
    PrefersLsp,
}

/// The resolved decision for a specific tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolvedBackend {
    TreeSitter,
    Lsp,
    /// Legacy -- only used for `region-*` ID lookup in `get_region`, never selected for new requests.
    Parserless,
    /// Heuristic structural fallback using indentation and bracket counting.
    Heuristic,
    /// LSP was required but the server binary wasn't found.
    LspUnavailable {
        binary: String,
        /// Human-readable hint for how to make the server available.
        hint: String,
    },
}

/// Per-language availability info for `rhizome status`.
#[derive(Debug, Clone, Serialize)]
pub struct LanguageStatus {
    pub language: Language,
    pub tree_sitter: bool,
    pub lsp_binary: String,
    pub lsp_available: bool,
    pub lsp_path: Option<PathBuf>,
}

/// Cached result of a binary lookup.
#[derive(Debug, Clone)]
struct ServerProbe {
    binary: String,
    available: bool,
    path: Option<PathBuf>,
}

// ─────────────────────────────────────────────────────────────────────────────
// BackendSelector
// ─────────────────────────────────────────────────────────────────────────────

pub struct BackendSelector {
    config: RhizomeConfig,
    bin_dir: PathBuf,
    lsp_disabled: bool,
    cache: HashMap<Language, ServerProbe>,
}

impl BackendSelector {
    pub fn new(config: RhizomeConfig) -> Self {
        let env_disabled = std::env::var("RHIZOME_DISABLE_LSP_DOWNLOAD")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let lsp_disabled = config.lsp.disable_download.unwrap_or(false) || env_disabled;
        let bin_dir = config
            .lsp
            .bin_dir
            .clone()
            .unwrap_or_else(crate::paths::managed_bin_dir);
        Self {
            config,
            bin_dir,
            lsp_disabled,
            cache: HashMap::new(),
        }
    }

    /// The managed LSP binary directory.
    pub fn lsp_bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    /// Whether LSP server auto-download is currently enabled.
    pub fn lsp_download_enabled(&self) -> bool {
        !self.lsp_disabled
    }

    /// Determine which backend to use for a given tool and language.
    pub fn select(&mut self, tool_name: &str, language: &Language) -> ResolvedBackend {
        let requirement = tool_requirement(tool_name);

        match requirement {
            BackendRequirement::TreeSitter => {
                if parserless_supported(tool_name) && !language.tree_sitter_supported() {
                    self.outline_fallback(language)
                } else {
                    ResolvedBackend::TreeSitter
                }
            }
            BackendRequirement::RequiresLsp => {
                let bin_dir = self.bin_dir.clone();
                let probe = self.probe_language(language);
                if probe.available {
                    ResolvedBackend::Lsp
                } else {
                    ResolvedBackend::LspUnavailable {
                        binary: probe.binary.clone(),
                        hint: lsp_unavailable_hint(&probe.binary, &bin_dir),
                    }
                }
            }
            BackendRequirement::PrefersLsp => {
                let probe = self.probe_language(language);
                if probe.available {
                    ResolvedBackend::Lsp
                } else if parserless_supported(tool_name) && !language.tree_sitter_supported() {
                    ResolvedBackend::Heuristic
                } else {
                    ResolvedBackend::TreeSitter
                }
            }
        }
    }

    /// Get status for all known languages (find-only, no auto-download).
    pub fn status(&mut self) -> Vec<LanguageStatus> {
        all_languages()
            .iter()
            .map(|lang: &Language| {
                let probe = find_server(lang, &self.config, &self.bin_dir);
                LanguageStatus {
                    language: lang.clone(),
                    tree_sitter: lang.tree_sitter_supported()
                        && self.config.is_language_enabled(lang),
                    lsp_binary: probe.binary.clone(),
                    lsp_available: probe.available,
                    lsp_path: probe.path.clone(),
                }
            })
            .collect()
    }

    /// Probe a language for binary availability (read-only, no auto-download).
    fn probe_language(&mut self, language: &Language) -> &ServerProbe {
        let refresh_probe = self
            .cache
            .get(language)
            .is_none_or(|probe| !probe.available);
        if refresh_probe {
            let probe = find_server(language, &self.config, &self.bin_dir);
            self.cache.insert(language.clone(), probe);
        }
        &self.cache[language]
    }

    /// Pick the best outline fallback once tree-sitter cannot serve the request.
    ///
    /// Preference order: LSP > Heuristic.
    pub fn outline_fallback(&mut self, language: &Language) -> ResolvedBackend {
        let probe = self.probe_language(language);
        if probe.available {
            ResolvedBackend::Lsp
        } else {
            ResolvedBackend::Heuristic
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool → requirement mapping
// ─────────────────────────────────────────────────────────────────────────────

pub fn tool_requirement(tool_name: &str) -> BackendRequirement {
    match tool_name {
        "rename_symbol" => BackendRequirement::RequiresLsp,
        "get_diagnostics" | "find_references" => BackendRequirement::PrefersLsp,
        _ => BackendRequirement::TreeSitter,
    }
}

#[must_use]
pub fn parserless_supported(tool_name: &str) -> bool {
    matches!(tool_name, "get_symbols" | "get_structure")
}

// ─────────────────────────────────────────────────────────────────────────────
// All known languages
// ─────────────────────────────────────────────────────────────────────────────

fn all_languages() -> &'static [Language] {
    &[
        Language::Rust,
        Language::Python,
        Language::JavaScript,
        Language::TypeScript,
        Language::Go,
        Language::Java,
        Language::C,
        Language::Cpp,
        Language::Ruby,
        Language::Elixir,
        Language::Zig,
        Language::CSharp,
        Language::FSharp,
        Language::Swift,
        Language::Php,
        Language::Haskell,
        Language::Bash,
        Language::Terraform,
        Language::Kotlin,
        Language::Dart,
        Language::Lua,
        Language::Clojure,
        Language::OCaml,
        Language::Julia,
        Language::Nix,
        Language::Gleam,
        Language::Vue,
        Language::Svelte,
        Language::Astro,
        Language::Prisma,
        Language::Typst,
        Language::Yaml,
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Server binary detection
// ─────────────────────────────────────────────────────────────────────────────

/// Find-only: check if binary exists in PATH (including the managed rhizome bin dir).
/// Does NOT attempt to download or configure missing servers.
fn find_server(language: &Language, config: &RhizomeConfig, bin_dir: &Path) -> ServerProbe {
    let server_config = config.get_server_config(language);

    let binary = match &server_config {
        Some(cfg) => cfg.binary.clone(),
        None => {
            return ServerProbe {
                binary: "(none)".into(),
                available: false,
                path: None,
            };
        }
    };

    match find_binary_in_path(&binary, bin_dir) {
        Some(path) => ServerProbe {
            binary,
            available: true,
            path: Some(path),
        },
        None => ServerProbe {
            binary,
            available: false,
            path: None,
        },
    }
}

/// Check if a binary exists in PATH augmented with the managed bin dir.
fn find_binary_in_path(name: &str, bin_dir: &Path) -> Option<PathBuf> {
    let augmented = crate::paths::augmented_path(bin_dir);
    which::which_in(name, Some(augmented), ".").ok()
}

fn lsp_unavailable_hint(binary: &str, _bin_dir: &Path) -> String {
    format!(
        "{binary} not found. \
         Unset RHIZOME_DISABLE_LSP_DOWNLOAD to enable automatic server download."
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Display for Language
// ─────────────────────────────────────────────────────────────────────────────

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "Rust"),
            Language::Python => write!(f, "Python"),
            Language::JavaScript => write!(f, "JavaScript"),
            Language::TypeScript => write!(f, "TypeScript"),
            Language::Go => write!(f, "Go"),
            Language::Java => write!(f, "Java"),
            Language::C => write!(f, "C"),
            Language::Cpp => write!(f, "C++"),
            Language::Ruby => write!(f, "Ruby"),
            Language::Elixir => write!(f, "Elixir"),
            Language::Zig => write!(f, "Zig"),
            Language::CSharp => write!(f, "C#"),
            Language::FSharp => write!(f, "F#"),
            Language::Swift => write!(f, "Swift"),
            Language::Php => write!(f, "PHP"),
            Language::Haskell => write!(f, "Haskell"),
            Language::Bash => write!(f, "Bash"),
            Language::Terraform => write!(f, "Terraform"),
            Language::Kotlin => write!(f, "Kotlin"),
            Language::Dart => write!(f, "Dart"),
            Language::Lua => write!(f, "Lua"),
            Language::Clojure => write!(f, "Clojure"),
            Language::OCaml => write!(f, "OCaml"),
            Language::Julia => write!(f, "Julia"),
            Language::Nix => write!(f, "Nix"),
            Language::Gleam => write!(f, "Gleam"),
            Language::Vue => write!(f, "Vue"),
            Language::Svelte => write!(f, "Svelte"),
            Language::Astro => write!(f, "Astro"),
            Language::Prisma => write!(f, "Prisma"),
            Language::Typst => write!(f, "Typst"),
            Language::Yaml => write!(f, "YAML"),
            Language::Other(name) => write!(f, "{name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_requirements_are_correct() {
        assert_eq!(
            tool_requirement("get_symbols"),
            BackendRequirement::TreeSitter
        );
        assert_eq!(
            tool_requirement("get_structure"),
            BackendRequirement::TreeSitter
        );
        assert_eq!(
            tool_requirement("rename_symbol"),
            BackendRequirement::RequiresLsp
        );
        assert_eq!(
            tool_requirement("get_diagnostics"),
            BackendRequirement::PrefersLsp
        );
        assert_eq!(
            tool_requirement("find_references"),
            BackendRequirement::PrefersLsp
        );
    }

    #[test]
    fn select_tree_sitter_for_basic_tools() {
        let config = RhizomeConfig::default();
        let mut selector = BackendSelector::new(config);
        let result = selector.select("get_symbols", &Language::Rust);
        assert_eq!(result, ResolvedBackend::TreeSitter);
    }

    #[test]
    fn select_heuristic_for_unknown_outline_tools() {
        let config = RhizomeConfig::default();
        let mut selector = BackendSelector::new(config);
        let result = selector.select("get_structure", &Language::Other("text".into()));
        assert_eq!(result, ResolvedBackend::Heuristic);
    }

    #[test]
    fn select_lsp_unavailable_for_missing_server() {
        let config = RhizomeConfig::default();
        let mut selector = BackendSelector::new(config);
        // Use a language unlikely to have a server in test env
        let result = selector.select("rename_symbol", &Language::Java);
        match result {
            ResolvedBackend::LspUnavailable { binary, .. } => {
                assert_eq!(binary, "jdtls");
            }
            ResolvedBackend::Lsp => {
                // jdtls happens to be available — that's fine
            }
            ResolvedBackend::Parserless => {
                panic!("rename_symbol should not resolve to parserless");
            }
            ResolvedBackend::Heuristic => {
                panic!("rename_symbol should not resolve to heuristic");
            }
            ResolvedBackend::TreeSitter => {
                panic!("rename_symbol should not resolve to tree-sitter");
            }
        }
    }

    #[test]
    fn prefers_lsp_falls_back_to_tree_sitter() {
        let config = RhizomeConfig::default();
        let mut selector = BackendSelector::new(config);
        // Java unlikely to have LSP in test env
        let result = selector.select("get_diagnostics", &Language::Java);
        match result {
            ResolvedBackend::TreeSitter => {} // expected fallback
            ResolvedBackend::Lsp => {}        // jdtls available — also ok
            _ => panic!("PrefersLsp should not produce LspUnavailable"),
        }
    }

    #[test]
    fn outline_fallback_prefers_lsp_then_heuristic() {
        let config = RhizomeConfig::default();
        let mut selector = BackendSelector::new(config);
        let result = selector.outline_fallback(&Language::Other("text".into()));
        assert_eq!(result, ResolvedBackend::Heuristic);
    }

    #[test]
    fn status_returns_all_languages() {
        let config = RhizomeConfig::default();
        let mut selector = BackendSelector::new(config);
        let statuses = selector.status();
        assert_eq!(statuses.len(), 32);

        let names: Vec<String> = statuses.iter().map(|s| s.language.to_string()).collect();
        assert!(names.contains(&"Rust".to_string()));
        assert!(names.contains(&"Python".to_string()));
        assert!(names.contains(&"C++".to_string()));
        assert!(names.contains(&"Elixir".to_string()));
        assert!(names.contains(&"PHP".to_string()));
        assert!(names.contains(&"C#".to_string()));
        let yaml = statuses
            .iter()
            .find(|status| status.language == Language::Yaml)
            .unwrap();
        assert!(!yaml.tree_sitter);
        let terraform = statuses
            .iter()
            .find(|status| status.language == Language::Terraform)
            .unwrap();
        assert!(!terraform.tree_sitter);
    }

    #[test]
    fn disabled_language_does_not_fall_back_to_default_server() {
        let config = RhizomeConfig {
            languages: std::collections::HashMap::from([(
                "java".to_string(),
                crate::config::LanguageConfig {
                    server_binary: None,
                    server_args: None,
                    enabled: Some(false),
                    initialization_options: None,
                },
            )]),
            ..Default::default()
        };
        let mut selector = BackendSelector::new(config);
        let statuses = selector.status();
        let java = statuses
            .iter()
            .find(|status| status.language == Language::Java)
            .unwrap();
        assert_eq!(java.lsp_binary, "(none)");
        assert!(!java.tree_sitter);
    }

    #[test]
    fn cache_is_populated_after_probe() {
        let config = RhizomeConfig::default();
        let mut selector = BackendSelector::new(config);
        assert!(selector.cache.is_empty());

        selector.select("get_symbols", &Language::Rust);
        // TreeSitter tools don't probe — cache stays empty
        assert!(selector.cache.is_empty());

        selector.select("rename_symbol", &Language::Rust);
        // RequiresLsp probes the server
        assert!(selector.cache.contains_key(&Language::Rust));
    }

    #[test]
    fn language_display() {
        assert_eq!(Language::Rust.to_string(), "Rust");
        assert_eq!(Language::Cpp.to_string(), "C++");
        assert_eq!(Language::Other("Zig".into()).to_string(), "Zig");
    }
}
