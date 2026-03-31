use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use rhizome_core::{CodeIntelligence, Language, Symbol, SymbolKind};
use rhizome_mcp::McpServer;
use rhizome_treesitter::TreeSitterBackend;
use spore::editors;
use tracing::info;

mod doctor;
mod self_update;

#[derive(Parser)]
#[command(name = "rhizome", version, about = "Code intelligence MCP server")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum InitEditor {
    #[value(name = "claude-code")]
    ClaudeCode,
    #[value(name = "cursor")]
    Cursor,
    #[value(name = "vscode")]
    VsCode,
    #[value(name = "zed")]
    Zed,
    #[value(name = "windsurf")]
    Windsurf,
    #[value(name = "amp")]
    Amp,
    #[value(name = "claude-desktop")]
    ClaudeDesktop,
    #[value(name = "codex")]
    CodexCli,
    #[value(name = "gemini")]
    GeminiCli,
    #[value(name = "copilot")]
    CopilotCli,
}

impl InitEditor {
    fn into_editor(self) -> editors::Editor {
        match self {
            Self::ClaudeCode => editors::Editor::ClaudeCode,
            Self::Cursor => editors::Editor::Cursor,
            Self::VsCode => editors::Editor::VsCode,
            Self::Zed => editors::Editor::Zed,
            Self::Windsurf => editors::Editor::Windsurf,
            Self::Amp => editors::Editor::Amp,
            Self::ClaudeDesktop => editors::Editor::ClaudeDesktop,
            Self::CodexCli => editors::Editor::CodexCli,
            Self::GeminiCli => editors::Editor::GeminiCli,
            Self::CopilotCli => editors::Editor::CopilotCli,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Start MCP server on stdio
    Serve {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
        /// Expose tools as separate MCP tools instead of unified rhizome command
        #[arg(long)]
        expanded: bool,
    },
    /// List symbols in a file
    Symbols {
        /// Path to the source file
        file: PathBuf,
    },
    /// Show file structure outline
    Structure {
        /// Path to the source file
        file: PathBuf,
    },
    /// Print MCP configuration for editors
    Init {
        /// Print an example rhizome config.toml instead of MCP config
        #[arg(long)]
        config: bool,
        /// Print a paste-ready MCP snippet for a specific editor/host
        #[arg(long, value_enum)]
        editor: Option<InitEditor>,
    },
    /// Export code symbols to Hyphae as a knowledge graph
    Export {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
    },
    /// Show backend status per language
    Status {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
    },
    /// Check for and install updates
    SelfUpdate {
        /// Only check for updates, don't download
        #[arg(long)]
        check: bool,
    },
    /// Diagnose common issues with the rhizome installation
    Doctor {
        /// Attempt to rebuild the export cache if it is missing
        #[arg(long)]
        fix: bool,
    },
    /// Summarize project structure: entry points, key types, modules, tests
    Summarize {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Manage LSP server installations
    Lsp {
        #[command(subcommand)]
        action: LspAction,
    },
}

#[derive(Subcommand)]
enum LspAction {
    /// Show LSP server status for all languages
    Status {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Install LSP server for a language
    Install {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
        /// Language name (e.g. rust, python, typescript)
        language: String,
    },
}

fn detect_project_root(hint: Option<PathBuf>) -> PathBuf {
    let root = hint
        .or_else(|| std::env::var_os("RHIZOME_PROJECT").map(PathBuf::from))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    std::fs::canonicalize(&root).unwrap_or(root)
}

fn tree_sitter_status_label(active: bool) -> &'static str {
    if active {
        "active"
    } else {
        "n/a"
    }
}

fn print_status_table(title: &str, statuses: &[rhizome_core::LanguageStatus]) {
    println!("{title}");
    println!("{}\n", "=".repeat(title.len()));
    println!(
        "{:<14} {:<14} {:<30} Status",
        "Language", "Tree-Sitter", "LSP Server"
    );
    println!(
        "{:<14} {:<14} {:<30} ------",
        "--------", "-----------", "----------"
    );

    for s in statuses {
        let status = if s.lsp_available {
            match &s.lsp_path {
                Some(p) => format!("available ({})", p.display()),
                None => "available".into(),
            }
        } else {
            "not found".into()
        };

        println!(
            "{:<14} {:<14} {:<30} {}",
            s.language.to_string(),
            tree_sitter_status_label(s.tree_sitter),
            s.lsp_binary,
            status,
        );
    }
}

fn symbol_kind_label(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "fn",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Interface => "interface",
        SymbolKind::Trait => "trait",
        SymbolKind::Type => "type",
        SymbolKind::Constant => "const",
        SymbolKind::Variable => "var",
        SymbolKind::Module => "mod",
        SymbolKind::Import => "use",
        SymbolKind::Property => "prop",
        SymbolKind::Field => "field",
    }
}

fn print_symbol_flat(sym: &Symbol) {
    let loc = &sym.location;
    println!(
        "{} {} [{}:{}-{}:{}]",
        symbol_kind_label(&sym.kind),
        sym.name,
        loc.line_start,
        loc.column_start,
        loc.line_end,
        loc.column_end,
    );
    if let Some(sig) = &sym.signature {
        println!("  {sig}");
    }
}

fn print_symbols_flat(symbols: &[Symbol]) {
    for sym in symbols {
        print_symbol_flat(sym);
        for child in &sym.children {
            print_symbol_flat(child);
        }
    }
}

fn print_tree(symbols: &[Symbol], prefix: &str, is_last_set: &[bool]) {
    let len = symbols.len();
    for (i, sym) in symbols.iter().enumerate() {
        let is_last = i == len - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let loc = &sym.location;
        println!(
            "{prefix}{connector}{} {} [{}:{}-{}:{}]",
            symbol_kind_label(&sym.kind),
            sym.name,
            loc.line_start,
            loc.column_start,
            loc.line_end,
            loc.column_end,
        );

        if !sym.children.is_empty() {
            let child_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}│   ")
            };
            let mut next_set = is_last_set.to_vec();
            next_set.push(is_last);
            print_tree(&sym.children, &child_prefix, &next_set);
        }
    }
}

fn cmd_symbols(file: &Path) -> Result<()> {
    let backend = TreeSitterBackend::new();
    let symbols = backend
        .get_symbols(file)
        .with_context(|| format!("Failed to get symbols from {}", file.display()))?;
    print_symbols_flat(&symbols);
    Ok(())
}

fn cmd_structure(file: &Path) -> Result<()> {
    let backend = TreeSitterBackend::new();
    let symbols = backend
        .get_symbols(file)
        .with_context(|| format!("Failed to get symbols from {}", file.display()))?;
    print_tree(&symbols, "", &[]);
    Ok(())
}

fn render_editor_snippet(editor: editors::Editor) -> Result<String> {
    if editor.uses_toml() {
        let mut root = toml::Table::new();
        let mut servers = toml::Table::new();
        let mut rhizome = toml::Table::new();
        rhizome.insert(
            "command".to_string(),
            toml::Value::String("rhizome".to_string()),
        );
        rhizome.insert(
            "args".to_string(),
            toml::Value::Array(vec![toml::Value::String("serve".to_string())]),
        );
        servers.insert("rhizome".to_string(), toml::Value::Table(rhizome));
        root.insert(editor.mcp_key().to_string(), toml::Value::Table(servers));
        toml::to_string_pretty(&root).context("Failed to serialize MCP TOML snippet")
    } else {
        let snippet = serde_json::json!({
            editor.mcp_key(): {
                "rhizome": editors::mcp_entry(editor, "rhizome", &["serve"])
            }
        });
        serde_json::to_string_pretty(&snippet).context("Failed to serialize MCP JSON snippet")
    }
}

fn print_editor_block(editor: editors::Editor) -> Result<()> {
    println!("{} MCP config", editor.name());
    match editors::config_path(editor) {
        Ok(path) => println!("Path: {}", path.display()),
        Err(error) => println!("Path: unavailable ({error})"),
    }
    println!();
    println!("{}", render_editor_snippet(editor)?);
    Ok(())
}

fn cmd_init(config_mode: bool, editor: Option<InitEditor>) -> Result<()> {
    if config_mode {
        print!("{}", rhizome_core::RhizomeConfig::example_config());
        return Ok(());
    }

    if let Some(editor) = editor {
        print!("{}", render_editor_snippet(editor.into_editor())?);
        return Ok(());
    }

    let detected = editors::detect();
    let editors_to_show = if detected.is_empty() {
        vec![
            editors::Editor::ClaudeCode,
            editors::Editor::CodexCli,
            editors::Editor::Cursor,
            editors::Editor::ClaudeDesktop,
        ]
    } else {
        detected
    };

    println!("Rhizome MCP setup");
    println!("=================\n");
    println!(
        "Detected hosts: {}",
        editors_to_show
            .iter()
            .map(|editor| editor.name())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Use `rhizome init --editor <host>` for a paste-ready single-host snippet.\n");

    for (idx, editor) in editors_to_show.iter().enumerate() {
        print_editor_block(*editor)?;
        if idx + 1 != editors_to_show.len() {
            println!("\n---\n");
        }
    }

    Ok(())
}

fn cmd_export(project: Option<PathBuf>) -> Result<()> {
    let project_root = detect_project_root(project);
    let backend = TreeSitterBackend::new();
    let args = serde_json::json!({});

    match rhizome_mcp::tools::export_tools::export_to_hyphae(&backend, &args, &project_root) {
        Ok(result) => {
            if let Some(text) = result
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|o| o.get("text"))
                .and_then(|t| t.as_str())
            {
                println!("{text}");
            }
            if result
                .get("isError")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                std::process::exit(1);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Export failed: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_status(project: Option<PathBuf>) -> Result<()> {
    let project_root = detect_project_root(project);
    let config = rhizome_core::RhizomeConfig::load(&project_root).unwrap_or_default();
    let mut selector = rhizome_core::BackendSelector::new(config);
    let statuses = selector.status();
    print_status_table("Rhizome Backend Status", &statuses);

    let installer = selector.installer();
    println!("\nBackend selection: tree-sitter (default) -> auto-upgrade to LSP when needed");
    println!("LSP-required tools: rename_symbol, get_hover_info");
    println!("LSP-preferred tools: find_references, get_diagnostics");
    println!(
        "\nAuto-install: {}",
        if installer.is_disabled() {
            "disabled"
        } else {
            "enabled (set RHIZOME_DISABLE_LSP_DOWNLOAD=1 to disable)"
        }
    );
    println!("Managed bin dir: {}", installer.bin_dir().display());

    Ok(())
}

fn cmd_summarize(project: Option<PathBuf>, json_output: bool) -> Result<()> {
    let project_root = detect_project_root(project);
    let backend = TreeSitterBackend::new();
    let summary = rhizome_core::summarize_project(&project_root, &backend)
        .with_context(|| format!("Failed to summarize project at {}", project_root.display()))?;

    if json_output {
        let json = serde_json::to_string_pretty(&summary)
            .context("Failed to serialize summary to JSON")?;
        println!("{json}");
    } else {
        print!("{}", summary.format_display());
    }
    Ok(())
}

fn cmd_lsp_status(project: Option<PathBuf>, json_output: bool) -> Result<()> {
    // ─────────────────────────────────────────────────────────────────────────────
    // Load configuration and get status
    // ─────────────────────────────────────────────────────────────────────────────

    let project_root = detect_project_root(project);
    let config = rhizome_core::RhizomeConfig::load(&project_root).unwrap_or_default();
    let mut selector = rhizome_core::BackendSelector::new(config);
    let statuses = selector.status();

    if json_output {
        let json = serde_json::to_string_pretty(&statuses)
            .context("Failed to serialize status to JSON")?;
        println!("{json}");
    } else {
        print_status_table("Rhizome LSP Status", &statuses);
    }

    Ok(())
}

fn cmd_lsp_install(project: Option<PathBuf>, language_name: &str) -> Result<()> {
    // ─────────────────────────────────────────────────────────────────────────────
    // Parse language name and get configuration
    // ─────────────────────────────────────────────────────────────────────────────

    let language = Language::from_name(language_name)
        .with_context(|| format!("Unknown language: {language_name}"))?;
    let project_root = detect_project_root(project);
    let config = rhizome_core::RhizomeConfig::load(&project_root).unwrap_or_default();

    let server_config = resolve_lsp_install_server_config(&config, &language)
        .with_context(|| format!("No LSP server available for {}", language))?;

    println!("Installing LSP server for {}...", language);
    println!("Server binary: {}", server_config.binary);

    // Explicit install command should never be disabled
    let installer = rhizome_core::LspInstaller::new(config.lsp.bin_dir.clone(), false);

    match installer.ensure_server(&language, &server_config.binary) {
        Ok(Some(path)) => {
            println!("Successfully installed: {}", path.display());
            Ok(())
        }
        Ok(None) => {
            anyhow::bail!(
                "LSP server installation skipped. Auto-install may be disabled. \
                 Set RHIZOME_DISABLE_LSP_DOWNLOAD=0 and ensure package manager is available."
            );
        }
        Err(e) => {
            anyhow::bail!("Failed to install LSP server: {e}");
        }
    }
}

fn resolve_lsp_install_server_config(
    config: &rhizome_core::RhizomeConfig,
    language: &Language,
) -> Option<rhizome_core::LanguageServerConfig> {
    config
        .get_server_config(language)
        .or_else(|| language.default_server_config())
}

async fn cmd_serve(project: Option<PathBuf>, expanded: bool) -> Result<()> {
    let project_root = detect_project_root(project);
    info!(
        "Starting MCP server with project root: {}",
        project_root.display()
    );

    let mut server = McpServer::new(project_root, !expanded);

    tokio::select! {
        result = server.run() => {
            result.context("MCP server error")?;
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    spore::logging::init(tracing::Level::WARN);

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { project, expanded } => cmd_serve(project, expanded).await,
        Commands::Symbols { file } => cmd_symbols(&file),
        Commands::Structure { file } => cmd_structure(&file),
        Commands::Init { config, editor } => cmd_init(config, editor),
        Commands::Export { project } => cmd_export(project),
        Commands::Status { project } => cmd_status(project),
        Commands::SelfUpdate { check } => self_update::run(check),
        Commands::Doctor { fix } => doctor::run(fix),
        Commands::Summarize { project, json } => cmd_summarize(project, json),
        Commands::Lsp { action } => match action {
            LspAction::Status { project, json } => cmd_lsp_status(project, json),
            LspAction::Install { project, language } => cmd_lsp_install(project, &language),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn render_editor_snippet_uses_json_key_for_claude_code() {
        let snippet = render_editor_snippet(editors::Editor::ClaudeCode).unwrap();
        let value: serde_json::Value = serde_json::from_str(&snippet).unwrap();
        assert!(value["mcpServers"]["rhizome"].is_object());
    }

    #[test]
    fn render_editor_snippet_uses_toml_key_for_codex() {
        let snippet = render_editor_snippet(editors::Editor::CodexCli).unwrap();
        let value = toml::Value::Table(toml::from_str::<toml::Table>(&snippet).unwrap());
        assert!(value["mcp_servers"]["rhizome"].is_table());
    }

    #[test]
    fn detect_project_root_keeps_nested_project_root() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path().join("repo");
        let nested_root = repo_root.join("packages/app");

        fs::create_dir_all(&nested_root).unwrap();
        fs::create_dir_all(repo_root.join(".git")).unwrap();

        let detected = detect_project_root(Some(nested_root.clone()));

        assert_eq!(detected, nested_root.canonicalize().unwrap());
    }

    #[test]
    fn resolve_lsp_install_server_config_ignores_disabled_runtime_gate() {
        let config = rhizome_core::RhizomeConfig {
            languages: std::collections::HashMap::from([(
                "rust".to_string(),
                rhizome_core::config::LanguageConfig {
                    server_binary: None,
                    server_args: None,
                    enabled: Some(false),
                    initialization_options: None,
                },
            )]),
            ..Default::default()
        };

        let config = resolve_lsp_install_server_config(&config, &Language::Rust)
            .expect("explicit install should still resolve a server");

        assert_eq!(config.binary, "rust-analyzer");
    }
}
