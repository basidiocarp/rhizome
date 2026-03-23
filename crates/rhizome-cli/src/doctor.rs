//! `rhizome doctor` — diagnose common issues with the rhizome installation.

use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

/// Known LSP servers to check in PATH.
const LSP_SERVERS: &[(&str, &str, &str)] = &[
    ("rust-analyzer", "Rust", "brew install rust-analyzer"),
    ("pyright-langserver", "Python", "pip install pyright"),
    (
        "typescript-language-server",
        "TypeScript",
        "npm i -g typescript-language-server",
    ),
    ("gopls", "Go", "go install golang.org/x/tools/gopls@latest"),
    ("clangd", "C/C++", "brew install llvm"),
    ("jdtls", "Java", "brew install jdtls"),
    ("ruby-lsp", "Ruby", "gem install ruby-lsp"),
    (
        "lua-language-server",
        "Lua",
        "brew install lua-language-server",
    ),
    ("elixir-ls", "Elixir", "mix archive.install hex elixir_ls"),
    ("zls", "Zig", "brew install zls"),
];

/// Tree-sitter languages with full query support.
const TREESITTER_LANGUAGES: &[&str] = &["Rust", "Python", "JavaScript", "TypeScript", "Go"];

pub fn run(fix: bool) -> Result<()> {
    println!();
    println!("\x1b[1mRhizome Doctor\x1b[0m");
    println!("{}", "\u{2500}".repeat(45));
    println!();

    let mut errors = 0u32;
    let mut warnings = 0u32;

    // ─────────────────────────────────────────────────────────────────────────
    // Tree-Sitter Backends
    // ─────────────────────────────────────────────────────────────────────────
    println!("\x1b[1mTree-Sitter Backends\x1b[0m");
    for lang in TREESITTER_LANGUAGES {
        pass(&format!("{lang} parser available"));
    }
    pass(&format!(
        "{} languages with tree-sitter queries",
        TREESITTER_LANGUAGES.len()
    ));

    // ─────────────────────────────────────────────────────────────────────────
    // LSP Servers
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mLSP Servers\x1b[0m");
    let mut lsp_found = 0;
    for (bin, lang, install) in LSP_SERVERS {
        match which::which(bin) {
            Ok(path) => {
                pass(&format!("{bin} found ({lang}) at {}", path.display()));
                lsp_found += 1;
            }
            Err(_) => {
                warn(&format!("{bin} not found ({lang}) — install: {install}"));
                warnings += 1;
            }
        }
    }
    pass(&format!(
        "{lsp_found}/{} LSP servers installed",
        LSP_SERVERS.len()
    ));

    // ─────────────────────────────────────────────────────────────────────────
    // Hyphae Integration
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mHyphae Integration\x1b[0m");
    match which::which("hyphae") {
        Ok(_) => pass("Hyphae available (code graph export enabled)"),
        Err(_) => {
            warn("Hyphae not installed — code graph export disabled");
            warnings += 1;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Export Cache
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mExport Cache\x1b[0m");
    let project_root = detect_project_root();
    let scoped_cache_path = rhizome_core::ExportCache::cache_path(&project_root);
    let legacy_cache_path = rhizome_core::ExportCache::legacy_cache_path(&project_root);
    let cache_path = if scoped_cache_path.exists() {
        scoped_cache_path
    } else {
        legacy_cache_path
    };
    if cache_path.exists() {
        match std::fs::read_to_string(&cache_path) {
            Ok(content) => {
                let count = content.matches('"').count() / 4; // rough key count
                pass(&format!(
                    "Cache at {} (~{} files tracked)",
                    cache_path.display(),
                    count
                ));
            }
            Err(_) => {
                warn("Cache file exists but unreadable");
                warnings += 1;
            }
        }
    } else {
        warn("No export cache (run: rhizome export --project .)");
        warnings += 1;
        if fix {
            print!("  Running export... ");
            match Command::new("rhizome")
                .args(["export", "--project", &project_root.to_string_lossy()])
                .output()
            {
                Ok(output) if output.status.success() => pass("Export completed"),
                _ => fail("Export failed"),
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Configuration
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mConfiguration\x1b[0m");

    let global_config = dirs::config_dir()
        .map(|d| d.join("rhizome/config.toml"))
        .unwrap_or_else(|| PathBuf::from("~/.config/rhizome/config.toml"));
    if global_config.exists() {
        pass(&format!("Global config: {}", global_config.display()));
    } else {
        warn(&format!(
            "No global config at {} (using defaults)",
            global_config.display()
        ));
        warnings += 1;
    }

    let project_config = project_root.join(".rhizome/config.toml");
    if project_config.exists() {
        pass(&format!("Project config: {}", project_config.display()));
    } else {
        warn("No project config (.rhizome/config.toml)");
        warnings += 1;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Project Detection
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mProject Detection\x1b[0m");
    pass(&format!("Project root: {}", project_root.display()));

    // Count files by extension
    let mut lang_counts: Vec<(&str, usize)> = Vec::new();
    count_files_by_ext(&project_root, &mut lang_counts);
    lang_counts.sort_by(|a, b| b.1.cmp(&a.1));
    if lang_counts.is_empty() {
        warn("No recognized source files found");
        warnings += 1;
    } else {
        let summary: Vec<String> = lang_counts
            .iter()
            .take(5)
            .map(|(lang, count)| format!("{lang} ({count})"))
            .collect();
        pass(&format!("Languages detected: {}", summary.join(", ")));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // MCP Server
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mMCP Server\x1b[0m");
    match which::which("rhizome") {
        Ok(path) => pass(&format!("rhizome binary at {}", path.display())),
        Err(_) => {
            fail("rhizome binary not in PATH");
            errors += 1;
        }
    }
    pass(&format!("Version: {}", env!("CARGO_PKG_VERSION")));

    if which::which("claude").is_ok() {
        match Command::new("claude").args(["mcp", "list"]).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("rhizome") {
                    pass("Registered as Claude Code MCP server");
                } else {
                    warn("Not registered — run: claude mcp add --scope user rhizome -- rhizome serve --expanded");
                    warnings += 1;
                }
            }
            Err(_) => {
                warn("Could not check Claude Code MCP registration");
                warnings += 1;
            }
        }
    }

    // Summary
    println!();
    if errors == 0 && warnings == 0 {
        println!("\x1b[32m0 errors, 0 warnings\x1b[0m");
    } else if errors == 0 {
        println!("\x1b[32m0 errors\x1b[0m, \x1b[33m{warnings} warning(s)\x1b[0m");
    } else {
        println!("\x1b[31m{errors} error(s)\x1b[0m, \x1b[33m{warnings} warning(s)\x1b[0m");
    }
    println!();

    if errors > 0 {
        anyhow::bail!("{errors} error(s) detected");
    }
    Ok(())
}

fn detect_project_root() -> PathBuf {
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

fn count_files_by_ext(root: &std::path::Path, counts: &mut Vec<(&'static str, usize)>) {
    let mut map: std::collections::HashMap<&'static str, usize> = std::collections::HashMap::new();

    let classify = |ext: &str| -> Option<&'static str> {
        match ext {
            "rs" => Some("Rust"),
            "py" => Some("Python"),
            "ts" | "tsx" => Some("TypeScript"),
            "js" | "jsx" => Some("JavaScript"),
            "go" => Some("Go"),
            "java" => Some("Java"),
            "c" | "h" => Some("C"),
            "cpp" | "cc" | "hpp" => Some("C++"),
            "rb" => Some("Ruby"),
            _ => None,
        }
    };

    let mut check = |path: &std::path::Path| {
        if let Some(lang) = path.extension().and_then(|e| e.to_str()).and_then(classify) {
            *map.entry(lang).or_insert(0) += 1;
        }
    };

    // Walk top 2 levels only
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            check(&path);
            if path.is_dir() {
                if let Ok(sub) = std::fs::read_dir(&path) {
                    for sub_entry in sub.flatten() {
                        check(&sub_entry.path());
                    }
                }
            }
        }
    }

    for (lang, count) in map {
        counts.push((lang, count));
    }
}

fn pass(msg: &str) {
    println!("  \x1b[32m\u{2713}\x1b[0m {msg}");
}

fn warn(msg: &str) {
    println!("  \x1b[33m\u{26a0}\x1b[0m {msg}");
}

fn fail(msg: &str) {
    println!("  \x1b[31m\u{2717}\x1b[0m {msg}");
}
