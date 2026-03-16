use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    Ruby,
    Elixir,
    Zig,
    CSharp,
    FSharp,
    Swift,
    Php,
    Other(String),
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Language> {
        match ext {
            "rs" => Some(Language::Rust),
            "py" | "pyi" => Some(Language::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Language::JavaScript),
            "ts" | "tsx" | "mts" | "cts" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            "c" | "h" => Some(Language::C),
            "cpp" | "cxx" | "cc" | "hpp" => Some(Language::Cpp),
            "rb" | "rake" | "gemspec" => Some(Language::Ruby),
            "ex" | "exs" => Some(Language::Elixir),
            "zig" | "zon" => Some(Language::Zig),
            "cs" => Some(Language::CSharp),
            "fs" | "fsi" | "fsx" | "fsscript" => Some(Language::FSharp),
            "swift" => Some(Language::Swift),
            "php" => Some(Language::Php),
            _ => None,
        }
    }

    pub fn default_server_config(&self) -> Option<LanguageServerConfig> {
        match self {
            Language::Rust => Some(LanguageServerConfig {
                binary: "rust-analyzer".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::Python => Some(LanguageServerConfig {
                binary: "pyright-langserver".to_string(),
                args: vec!["--stdio".to_string()],
                initialization_options: None,
            }),
            Language::JavaScript | Language::TypeScript => Some(LanguageServerConfig {
                binary: "typescript-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                initialization_options: None,
            }),
            Language::Go => Some(LanguageServerConfig {
                binary: "gopls".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::Java => Some(LanguageServerConfig {
                binary: "jdtls".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::C | Language::Cpp => Some(LanguageServerConfig {
                binary: "clangd".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::Ruby => Some(LanguageServerConfig {
                binary: "solargraph".to_string(),
                args: vec!["stdio".to_string()],
                initialization_options: None,
            }),
            Language::Elixir => Some(LanguageServerConfig {
                binary: "elixir-ls".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::Zig => Some(LanguageServerConfig {
                binary: "zls".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::CSharp => Some(LanguageServerConfig {
                binary: "csharp-ls".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::FSharp => Some(LanguageServerConfig {
                binary: "fsautocomplete".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::Swift => Some(LanguageServerConfig {
                binary: "sourcekit-lsp".to_string(),
                args: vec![],
                initialization_options: None,
            }),
            Language::Php => Some(LanguageServerConfig {
                binary: "intelephense".to_string(),
                args: vec!["--stdio".to_string()],
                initialization_options: None,
            }),
            Language::Other(_) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageServerConfig {
    pub binary: String,
    pub args: Vec<String>,
    pub initialization_options: Option<serde_json::Value>,
}
