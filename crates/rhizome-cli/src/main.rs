use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rhizome_core::{CodeIntelligence, Language, Symbol, SymbolKind};
use rhizome_mcp::McpServer;
use rhizome_treesitter::TreeSitterBackend;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod doctor;
mod self_update;

#[derive(Parser)]
#[command(name = "rhizome", version, about = "Code intelligence MCP server")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
        /// Attempt to fix detected issues
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
        #[arg(long)]
        json: bool,
    },
    /// Install LSP server for a language
    Install {
        /// Language name (e.g. rust, python, typescript)
        language: String,
    },
}

fn detect_project_root(hint: Option<PathBuf>) -> PathBuf {
    if let Some(root) = hint {
        return root;
    }

    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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

fn cmd_init(config_mode: bool) {
    if config_mode {
        print!("{}", rhizome_core::RhizomeConfig::example_config());
    } else {
        let config = serde_json::json!({
            "mcpServers": {
                "rhizome": {
                    "command": "rhizome",
                    "args": ["serve"],
                    "env": {}
                }
            }
        });
        println!("{}", serde_json::to_string_pretty(&config).unwrap());
    }
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

    println!("Rhizome Backend Status");
    println!("======================\n");
    println!(
        "{:<14} {:<14} {:<30} Status",
        "Language", "Tree-Sitter", "LSP Server"
    );
    println!(
        "{:<14} {:<14} {:<30} ------",
        "--------", "-----------", "----------"
    );

    for s in &statuses {
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
            "active",
            s.lsp_binary,
            status,
        );
    }

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

fn cmd_lsp_status(json_output: bool) -> Result<()> {
    // ─────────────────────────────────────────────────────────────────────────────
    // Load configuration and get status
    // ─────────────────────────────────────────────────────────────────────────────

    let config = rhizome_core::RhizomeConfig::default();
    let mut selector = rhizome_core::BackendSelector::new(config);
    let statuses = selector.status();

    if json_output {
        let json = serde_json::to_string_pretty(&statuses)
            .context("Failed to serialize status to JSON")?;
        println!("{json}");
    } else {
        // ─────────────────────────────────────────────────────────────────────────────
        // Print table format
        // ─────────────────────────────────────────────────────────────────────────────

        println!("Rhizome LSP Status");
        println!("==================\n");
        println!(
            "{:<14} {:<14} {:<30} Status",
            "Language", "Tree-Sitter", "LSP Server"
        );
        println!(
            "{:<14} {:<14} {:<30} ------",
            "--------", "-----------", "----------"
        );

        for s in &statuses {
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
                "active",
                s.lsp_binary,
                status,
            );
        }
    }

    Ok(())
}

fn cmd_lsp_install(language_name: &str) -> Result<()> {
    // ─────────────────────────────────────────────────────────────────────────────
    // Parse language name and get configuration
    // ─────────────────────────────────────────────────────────────────────────────

    let language = Language::from_name(language_name)
        .with_context(|| format!("Unknown language: {language_name}"))?;

    let config = rhizome_core::RhizomeConfig::default();
    let selector = rhizome_core::BackendSelector::new(config);
    let installer = selector.installer();

    // ─────────────────────────────────────────────────────────────────────────────
    // Get default server config and install
    // ─────────────────────────────────────────────────────────────────────────────

    let server_config = language
        .default_server_config()
        .with_context(|| format!("No LSP server available for {}", language))?;

    println!("Installing LSP server for {}...", language);
    println!("Server binary: {}", server_config.binary);

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
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { project, expanded } => cmd_serve(project, expanded).await,
        Commands::Symbols { file } => cmd_symbols(&file),
        Commands::Structure { file } => cmd_structure(&file),
        Commands::Init { config } => {
            cmd_init(config);
            Ok(())
        }
        Commands::Export { project } => cmd_export(project),
        Commands::Status { project } => cmd_status(project),
        Commands::SelfUpdate { check } => self_update::run(check),
        Commands::Doctor { fix } => doctor::run(fix),
        Commands::Summarize { project, json } => cmd_summarize(project, json),
        Commands::Lsp { action } => match action {
            LspAction::Status { json } => cmd_lsp_status(json),
            LspAction::Install { language } => cmd_lsp_install(&language),
        },
    }
}
