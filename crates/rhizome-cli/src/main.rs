use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use rhizome_core::{
    CodeIntelligence, HeuristicBackend, HeuristicRegion, Language, Position, Symbol, SymbolKind,
};
use rhizome_mcp::McpServer;
use rhizome_treesitter::TreeSitterBackend;
use spore::editors;
use spore::logging::{SpanContext, root_span, workflow_span};
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
    /// Start MCP server on unix socket (singleton, shared across Claude Code windows)
    #[cfg(unix)]
    ServeSocket {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
        /// Expose tools as separate MCP tools instead of unified rhizome command
        #[arg(long)]
        expanded: bool,
    },
    /// Connect to socket server (stdio to unix socket bridge)
    #[cfg(unix)]
    Proxy,
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
    /// Export a typed repo-understanding artifact with machine-facing export status
    ExportUnderstanding {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
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
    /// Compile and store permanent environment artifact in Hyphae
    CompileEnv {
        /// Workspace/project root path
        #[arg(long, short)]
        project: Option<PathBuf>,
        /// Project name for Hyphae memoir (defaults to directory name)
        #[arg(long)]
        name: Option<String>,
        /// Mark the current artifact as stale without re-running analysis
        #[arg(long)]
        invalidate: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Manage LSP server installations
    Lsp {
        #[command(subcommand)]
        action: LspAction,
    },
    /// Manage analyzer plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Search for symbols matching a name pattern across the project
    Search {
        /// Pattern to match symbol names (case-insensitive substring)
        pattern: String,
        /// Directory to search (defaults to current directory)
        #[arg(long, short)]
        path: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Find all references to the symbol at a file position
    Refs {
        /// Path to the source file
        file: PathBuf,
        /// Line number (0-based)
        line: u32,
        /// Column number (0-based)
        col: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
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

#[derive(Subcommand)]
enum PluginAction {
    /// List all registered analyzer plugins
    List,
}

fn detect_project_root(hint: Option<PathBuf>) -> PathBuf {
    let root = hint
        .or_else(|| std::env::var_os("RHIZOME_PROJECT").map(PathBuf::from))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    std::fs::canonicalize(&root).unwrap_or(root)
}

fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Serve { .. } => "serve",
        #[cfg(unix)]
        Commands::ServeSocket { .. } => "serve_socket",
        #[cfg(unix)]
        Commands::Proxy => "proxy",
        Commands::Symbols { .. } => "symbols",
        Commands::Structure { .. } => "structure",
        Commands::Init { .. } => "init",
        Commands::Export { .. } => "export",
        Commands::ExportUnderstanding { .. } => "export_understanding",
        Commands::Status { .. } => "status",
        Commands::SelfUpdate { .. } => "self_update",
        Commands::Doctor { .. } => "doctor",
        Commands::Summarize { .. } => "summarize",
        Commands::CompileEnv { .. } => "compile_env",
        Commands::Lsp { action } => match action {
            LspAction::Status { .. } => "lsp_status",
            LspAction::Install { .. } => "lsp_install",
        },
        Commands::Plugin { action } => match action {
            PluginAction::List => "plugin_list",
        },
        Commands::Search { .. } => "search",
        Commands::Refs { .. } => "refs",
    }
}

fn current_workspace_root() -> Option<String> {
    std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
}

fn command_span_context(command: &Commands) -> SpanContext {
    let workspace_root = match command {
        Commands::Serve { project, .. } => Some(detect_project_root(project.clone())),
        #[cfg(unix)]
        Commands::ServeSocket { project, .. } => Some(detect_project_root(project.clone())),
        Commands::Export { project } => Some(detect_project_root(project.clone())),
        Commands::ExportUnderstanding { project, .. } => Some(detect_project_root(project.clone())),
        Commands::Status { project } => Some(detect_project_root(project.clone())),
        Commands::Summarize { project, .. } => Some(detect_project_root(project.clone())),
        Commands::CompileEnv { project, .. } => Some(detect_project_root(project.clone())),
        #[cfg(unix)]
        Commands::Proxy => None,
        Commands::Lsp { action } => match action {
            LspAction::Status { project, .. } | LspAction::Install { project, .. } => {
                Some(detect_project_root(project.clone()))
            }
        },
        Commands::Symbols { file } | Commands::Structure { file } | Commands::Refs { file, .. } => {
            file.parent()
                .map(|path| path.to_path_buf())
                .or_else(|| std::env::current_dir().ok())
        }
        Commands::Search { path, .. } => path.clone().or_else(|| std::env::current_dir().ok()),
        Commands::Init { .. }
        | Commands::SelfUpdate { .. }
        | Commands::Doctor { .. }
        | Commands::Plugin { .. } => None,
    };

    let context = SpanContext::for_app("rhizome");
    match workspace_root
        .map(|path| path.display().to_string())
        .or_else(current_workspace_root)
    {
        Some(workspace_root) => context.with_workspace_root(workspace_root),
        None => context,
    }
}

fn tree_sitter_status_label(active: bool) -> &'static str {
    if active { "active" } else { "n/a" }
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

fn print_heuristic_regions(regions: &[HeuristicRegion], tree: bool) {
    for region in regions {
        let indent = "  ".repeat(region.depth as usize);
        if tree {
            println!(
                "{indent}{} [{}-{}] ({})",
                region.label, region.line, region.line_end, region.region_id
            );
        } else {
            println!(
                "{} [{}-{}] ({})",
                region.label, region.line, region.line_end, region.region_id
            );
        }
    }
}

fn print_heuristic_notice(file: &Path) {
    eprintln!(
        "Heuristic structural fallback for {} (outline only, not semantic analysis).",
        file.display()
    );
}

fn cmd_symbols(file: &Path) -> Result<()> {
    let backend = TreeSitterBackend::new();
    match backend.get_symbols(file) {
        Ok(symbols) => print_symbols_flat(&symbols),
        Err(_) => {
            let heuristic = HeuristicBackend::new();
            let regions = heuristic
                .outline(file)
                .with_context(|| format!("Failed to get symbols from {}", file.display()))?;
            print_heuristic_notice(file);
            print_heuristic_regions(&regions, false);
        }
    }
    Ok(())
}

fn cmd_structure(file: &Path) -> Result<()> {
    let backend = TreeSitterBackend::new();
    match backend.get_symbols(file) {
        Ok(symbols) => print_tree(&symbols, "", &[]),
        Err(_) => {
            let heuristic = HeuristicBackend::new();
            let regions = heuristic
                .outline(file)
                .with_context(|| format!("Failed to get structure from {}", file.display()))?;
            print_heuristic_notice(file);
            print_heuristic_regions(&regions, true);
        }
    }
    Ok(())
}

fn cmd_search(pattern: &str, path: Option<PathBuf>, json: bool) -> Result<()> {
    let root =
        path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let backend = TreeSitterBackend::new();
    let syms = backend
        .search_symbols(pattern, &root)
        .with_context(|| format!("symbol search failed for pattern '{pattern}'"))?;

    if json {
        let out: Vec<_> = syms
            .iter()
            .map(|s| {
                serde_json::json!({
                    "file": s.location.file_path,
                    "line": s.location.line_start,
                    "col": s.location.column_start,
                    "kind": symbol_kind_label(&s.kind),
                    "name": s.name,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for s in &syms {
            println!(
                "{}:{}:{} {} {}",
                s.location.file_path,
                s.location.line_start,
                s.location.column_start,
                symbol_kind_label(&s.kind),
                s.name
            );
        }
    }
    Ok(())
}

fn cmd_refs(file: &Path, line: u32, col: u32, json: bool) -> Result<()> {
    let backend = TreeSitterBackend::new();
    let position = Position { line, column: col };
    let refs = match backend.find_references(file, &position) {
        Ok(r) => r,
        Err(e) => {
            return Err(e).with_context(|| {
                format!(
                    "find_references failed for {}:{}:{}",
                    file.display(),
                    line,
                    col
                )
            });
        }
    };

    if refs.is_empty() {
        eprintln!(
            "No references found (tree-sitter backend; run `rhizome lsp install <lang>` for cross-file results)"
        );
        if json {
            println!("[]");
        }
        return Ok(());
    }

    if json {
        let out: Vec<_> = refs
            .iter()
            .map(|r| {
                serde_json::json!({
                    "file": r.file_path,
                    "line": r.line_start,
                    "col": r.column_start,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for r in &refs {
            println!("{}:{}:{}", r.file_path, r.line_start, r.column_start);
        }
    }
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

fn cmd_export_understanding(project: Option<PathBuf>, json_output: bool) -> Result<()> {
    let project_root = detect_project_root(project);
    let backend = TreeSitterBackend::new();
    let args = serde_json::json!({});

    match rhizome_mcp::tools::export_tools::export_repo_understanding(
        &backend,
        &args,
        &project_root,
    ) {
        Ok(result) => {
            let output = format_understanding_output(&result, json_output)?;
            if !output.is_empty() {
                println!("{output}");
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
            eprintln!("Understanding export failed: {e}");
            std::process::exit(1);
        }
    }
}

fn format_understanding_output(result: &serde_json::Value, json_output: bool) -> Result<String> {
    if json_output {
        serde_json::to_string_pretty(result)
            .context("Failed to serialize understanding export to JSON")
    } else {
        Ok(result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|o| o.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string())
    }
}

fn cmd_status(project: Option<PathBuf>) -> Result<()> {
    let project_root = detect_project_root(project);
    let config = rhizome_core::RhizomeConfig::load(&project_root).unwrap_or_default();
    let mut selector = rhizome_core::BackendSelector::new(config);
    let statuses = selector.status();
    print_status_table("Rhizome Backend Status", &statuses);

    println!("\nBackend selection: tree-sitter (default) -> auto-upgrade to LSP when needed");
    println!("LSP-required tools: rename_symbol");
    println!("LSP-preferred tools: find_references, get_diagnostics");
    println!(
        "\nAuto-install: {}",
        if selector.lsp_download_enabled() {
            "enabled (set RHIZOME_DISABLE_LSP_DOWNLOAD=1 to disable)"
        } else {
            "disabled"
        }
    );
    println!("Managed bin dir: {}", selector.lsp_bin_dir().display());

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
    let span_context =
        SpanContext::for_app("rhizome").with_workspace_root(project_root.display().to_string());
    let _workflow_span = workflow_span("lsp_install", &span_context).entered();
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
                 Unset RHIZOME_DISABLE_LSP_DOWNLOAD and ensure package manager is available."
            );
        }
        Err(e) => {
            anyhow::bail!("Failed to install LSP server: {e}");
        }
    }
}

fn cmd_plugin_list() -> Result<()> {
    // ─────────────────────────────────────────────────────────────────────────────
    // Load plugin registry and display list
    // ─────────────────────────────────────────────────────────────────────────────

    let config = rhizome_core::RhizomeConfig::default();
    let selector = rhizome_core::BackendSelector::new(config);
    let registry = selector.plugin_registry();

    if registry.list().is_empty() {
        println!("No plugins registered.");
        return Ok(());
    }

    println!("Registered analyzer plugins:\n");
    for plugin in registry.list() {
        let exts = plugin.supported_extensions();
        let exts_str = exts.join(", ");
        println!("built-in: {}  [{}]", plugin.id(), exts_str);
    }

    Ok(())
}

fn cmd_compile_env(
    project: Option<PathBuf>,
    name: Option<String>,
    invalidate: bool,
    json_output: bool,
) -> Result<()> {
    let project_root = detect_project_root(project);
    let project_name = name.unwrap_or_else(|| {
        project_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    });

    if invalidate {
        // Mark stale: call `hyphae memoir set-meta` to set invalidated_at
        let status = std::process::Command::new("hyphae")
            .args([
                "memoir",
                "set-meta",
                &format!("compiled-env:{project_name}"),
                "--invalidate",
            ])
            .status();
        match status {
            Ok(s) if s.success() => {
                println!("Compiled environment artifact for '{project_name}' marked stale.");
            }
            _ => {
                eprintln!("Warning: hyphae not available — could not mark artifact stale");
            }
        }
        return Ok(());
    }

    println!("Compiling environment artifact for '{project_name}'...");

    // Run the existing ExportUnderstanding analysis
    let backend = TreeSitterBackend::new();
    let args = serde_json::json!({});
    let understanding = rhizome_mcp::tools::export_tools::export_repo_understanding(
        &backend,
        &args,
        &project_root,
    )?;

    // Extract text from understanding
    let understanding_text = understanding
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|o| o.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("No understanding available")
        .to_string();

    // Write to Hyphae as a permanent memoir via CLI
    // Step 1: create or update memoir
    let memoir_id = format!("compiled-env:{project_name}");
    let _create = std::process::Command::new("hyphae")
        .args([
            "memoir",
            "create",
            "--name",
            &memoir_id,
            "--description",
            &format!("Compiled environment artifact for {project_name}"),
        ])
        .status();

    // Step 2: add concept with the understanding text
    let _concept = std::process::Command::new("hyphae")
        .args([
            "memoir",
            "add-concept",
            "--memoir",
            &memoir_id,
            "--name",
            "repo_structure",
            "--definition",
            &understanding_text[..understanding_text.len().min(2000)],
        ])
        .status();

    // Step 3: mark as compiled artifact with decay=never
    let _meta = std::process::Command::new("hyphae")
        .args([
            "memoir",
            "set-meta",
            &memoir_id,
            "--decay",
            "never",
            "--authority",
            "primary",
            "--source",
            "compiled_artifact",
        ])
        .status();

    let result = serde_json::json!({
        "memoir_id": memoir_id,
        "project": project_name,
        "project_root": project_root.display().to_string(),
        "status": "compiled",
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "Environment artifact compiled and stored: {}",
            result["memoir_id"]
        );
        println!("Hyphae memoir: {}", result["memoir_id"]);
        println!("Decay: never | Authority: primary | Source: compiled_artifact");
    }

    Ok(())
}

fn resolve_lsp_install_server_config(
    config: &rhizome_core::RhizomeConfig,
    language: &Language,
) -> Option<rhizome_core::LanguageServerConfig> {
    config
        .get_server_config(language)
        .or_else(|| language.default_server_config())
}

#[cfg(unix)]
async fn cmd_serve_socket(project: Option<PathBuf>, expanded: bool) -> Result<()> {
    let project_root = detect_project_root(project);
    info!(
        "Starting MCP socket server with project root: {}",
        project_root.display()
    );

    tokio::select! {
        result = rhizome_mcp::run_socket_server(project_root, !expanded) => {
            result.context("MCP socket server error")?;
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    Ok(())
}

#[cfg(unix)]
async fn cmd_proxy() -> Result<()> {
    rhizome_mcp::run_proxy().await.context("MCP proxy error")
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
    spore::logging::init_app("rhizome", tracing::Level::WARN);
    spore::logging::install_panic_hook("rhizome");

    let cli = Cli::parse();
    let span_context = command_span_context(&cli.command);
    let _runtime_span = root_span(&span_context).entered();
    let _command_span = workflow_span(command_name(&cli.command), &span_context).entered();

    match cli.command {
        Commands::Serve { project, expanded } => cmd_serve(project, expanded).await,
        #[cfg(unix)]
        Commands::ServeSocket { project, expanded } => cmd_serve_socket(project, expanded).await,
        #[cfg(unix)]
        Commands::Proxy => cmd_proxy().await,
        Commands::Symbols { file } => cmd_symbols(&file),
        Commands::Structure { file } => cmd_structure(&file),
        Commands::Init { config, editor } => cmd_init(config, editor),
        Commands::Export { project } => cmd_export(project),
        Commands::ExportUnderstanding { project, json } => cmd_export_understanding(project, json),
        Commands::Status { project } => cmd_status(project),
        Commands::SelfUpdate { check } => self_update::run(check),
        Commands::Doctor { fix } => doctor::run(fix),
        Commands::Summarize { project, json } => cmd_summarize(project, json),
        Commands::CompileEnv {
            project,
            name,
            invalidate,
            json,
        } => cmd_compile_env(project, name, invalidate, json),
        Commands::Lsp { action } => match action {
            LspAction::Status { project, json } => cmd_lsp_status(project, json),
            LspAction::Install { project, language } => cmd_lsp_install(project, &language),
        },
        Commands::Plugin { action } => match action {
            PluginAction::List => cmd_plugin_list(),
        },
        Commands::Search {
            pattern,
            path,
            json,
        } => cmd_search(&pattern, path, json),
        Commands::Refs {
            file,
            line,
            col,
            json,
        } => cmd_refs(&file, line, col, json),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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

    #[test]
    fn command_span_context_uses_project_aware_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().join("workspace");
        std::fs::create_dir_all(&project_root).unwrap();
        let expected_root = detect_project_root(Some(project_root.clone()));

        let context = command_span_context(&Commands::Status {
            project: Some(project_root.clone()),
        });

        assert_eq!(context.service.as_deref(), Some("rhizome"));
        assert_eq!(
            context.workspace_root.as_deref(),
            Some(expected_root.display().to_string().as_str())
        );
    }

    #[test]
    fn command_name_labels_nested_lsp_commands() {
        assert_eq!(
            command_name(&Commands::Lsp {
                action: LspAction::Install {
                    project: None,
                    language: "rust".into(),
                },
            }),
            "lsp_install"
        );
    }

    #[test]
    fn format_understanding_output_preserves_machine_status_in_json_mode() {
        let result = json!({
            "content": [{ "type": "text", "text": "Repo understanding is up to date." }],
            "understanding": {
                "export_status": {
                    "outcome": "cached_reuse",
                    "refresh_kind": "cached_reuse",
                    "any_exports_succeeded": true,
                    "any_exports_failed": false,
                    "safe_to_consume": true
                }
            }
        });

        let output = format_understanding_output(&result, true).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(
            parsed["understanding"]["export_status"]["outcome"],
            "cached_reuse"
        );
        assert_eq!(
            parsed["understanding"]["export_status"]["safe_to_consume"],
            true
        );
    }
}
