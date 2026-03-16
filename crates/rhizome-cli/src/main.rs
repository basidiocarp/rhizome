use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rhizome_core::{CodeIntelligence, Symbol, SymbolKind};
use rhizome_mcp::McpServer;
use rhizome_treesitter::TreeSitterBackend;
use tracing::info;
use tracing_subscriber::EnvFilter;

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
    Init,
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

fn cmd_init() {
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
        Commands::Init => {
            cmd_init();
            Ok(())
        }
    }
}
