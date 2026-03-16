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
    Haskell,
    Bash,
    Terraform,
    Kotlin,
    Dart,
    Lua,
    Clojure,
    OCaml,
    Julia,
    Nix,
    Gleam,
    Vue,
    Svelte,
    Astro,
    Prisma,
    Typst,
    Yaml,
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
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" => Some(Language::Cpp),
            "rb" | "rake" | "gemspec" | "ru" => Some(Language::Ruby),
            "ex" | "exs" => Some(Language::Elixir),
            "zig" | "zon" => Some(Language::Zig),
            "cs" => Some(Language::CSharp),
            "fs" | "fsi" | "fsx" | "fsscript" => Some(Language::FSharp),
            "swift" => Some(Language::Swift),
            "php" => Some(Language::Php),
            "hs" | "lhs" => Some(Language::Haskell),
            "sh" | "bash" | "zsh" | "ksh" => Some(Language::Bash),
            "tf" | "tfvars" => Some(Language::Terraform),
            "kt" | "kts" => Some(Language::Kotlin),
            "dart" => Some(Language::Dart),
            "lua" => Some(Language::Lua),
            "clj" | "cljs" | "cljc" | "edn" => Some(Language::Clojure),
            "ml" | "mli" => Some(Language::OCaml),
            "jl" => Some(Language::Julia),
            "nix" => Some(Language::Nix),
            "gleam" => Some(Language::Gleam),
            "vue" => Some(Language::Vue),
            "svelte" => Some(Language::Svelte),
            "astro" => Some(Language::Astro),
            "prisma" => Some(Language::Prisma),
            "typ" | "typc" => Some(Language::Typst),
            "yaml" | "yml" => Some(Language::Yaml),
            _ => None,
        }
    }

    pub fn default_server_config(&self) -> Option<LanguageServerConfig> {
        let (binary, args) = match self {
            Language::Rust => ("rust-analyzer", vec![]),
            Language::Python => ("pyright-langserver", vec!["--stdio".into()]),
            Language::JavaScript | Language::TypeScript => {
                ("typescript-language-server", vec!["--stdio".into()])
            }
            Language::Go => ("gopls", vec![]),
            Language::Java => ("jdtls", vec![]),
            Language::C | Language::Cpp => ("clangd", vec![]),
            Language::Ruby => ("ruby-lsp", vec![]),
            Language::Elixir => ("elixir-ls", vec![]),
            Language::Zig => ("zls", vec![]),
            Language::CSharp => ("csharp-ls", vec![]),
            Language::FSharp => ("fsautocomplete", vec![]),
            Language::Swift => ("sourcekit-lsp", vec![]),
            Language::Php => ("phpactor", vec!["language-server".into()]),
            Language::Haskell => ("haskell-language-server-wrapper", vec!["--lsp".into()]),
            Language::Bash => ("bash-language-server", vec!["start".into()]),
            Language::Terraform => ("terraform-ls", vec!["serve".into()]),
            Language::Kotlin => ("kotlin-language-server", vec![]),
            Language::Dart => ("dart", vec!["language-server".into(), "--protocol=lsp".into()]),
            Language::Lua => ("lua-language-server", vec![]),
            Language::Clojure => ("clojure-lsp", vec![]),
            Language::OCaml => ("ocamllsp", vec![]),
            Language::Julia => ("julia", vec!["--startup-file=no".into(), "-e".into(), "using LanguageServer; runserver()".into()]),
            Language::Nix => ("nixd", vec![]),
            Language::Gleam => ("gleam", vec!["lsp".into()]),
            Language::Vue => ("vue-language-server", vec!["--stdio".into()]),
            Language::Svelte => ("svelteserver", vec!["--stdio".into()]),
            Language::Astro => ("astro-ls", vec!["--stdio".into()]),
            Language::Prisma => ("prisma-language-server", vec!["--stdio".into()]),
            Language::Typst => ("tinymist", vec![]),
            Language::Yaml => ("yaml-language-server", vec!["--stdio".into()]),
            Language::Other(_) => return None,
        };

        Some(LanguageServerConfig {
            binary: binary.to_string(),
            args,
            initialization_options: None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageServerConfig {
    pub binary: String,
    pub args: Vec<String>,
    pub initialization_options: Option<serde_json::Value>,
}
