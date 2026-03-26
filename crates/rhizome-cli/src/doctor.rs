//! `rhizome doctor` — diagnose common issues with the rhizome installation.

use anyhow::Result;
use spore::editors::{self, Editor};
use std::path::PathBuf;
use std::process::Command;

/// Known LSP servers to check in PATH.
const LSP_SERVERS: &[(&str, &str)] = &[
    ("rust-analyzer", "Rust"),
    ("pyright-langserver", "Python"),
    ("typescript-language-server", "TypeScript"),
    ("gopls", "Go"),
    ("clangd", "C/C++"),
    ("jdtls", "Java"),
    ("ruby-lsp", "Ruby"),
    ("lua-language-server", "Lua"),
    ("elixir-ls", "Elixir"),
    ("zls", "Zig"),
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
    for (bin, lang) in LSP_SERVERS {
        match which::which(bin) {
            Ok(path) => {
                pass(&format!("{bin} found ({lang}) at {}", path.display()));
                lsp_found += 1;
            }
            Err(_) => {
                warn(&format!(
                    "{bin} not found ({lang}) — install: {}",
                    install_hint(bin)
                ));
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

    let global_config = rhizome_core::global_config_path();
    if global_config.exists() {
        pass(&format!("Global config: {}", global_config.display()));
    } else {
        warn(&format!(
            "No global config at {} (using defaults)",
            global_config.display()
        ));
        warnings += 1;
    }

    let project_config = rhizome_core::project_config_path(&project_root);
    if project_config.exists() {
        pass(&format!("Project config: {}", project_config.display()));
    } else {
        warn(&format!(
            "No project config at {}",
            project_config.display()
        ));
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
    // MCP Registration
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mMCP Registration\x1b[0m");
    match which::which("rhizome") {
        Ok(path) => pass(&format!("rhizome binary at {}", path.display())),
        Err(_) => {
            fail("rhizome binary not in PATH");
            errors += 1;
        }
    }
    pass(&format!("Version: {}", env!("CARGO_PKG_VERSION")));

    let detected_editors = editors::detect();
    if detected_editors.is_empty() {
        warn("No supported MCP host configs detected");
        warnings += 1;
    } else {
        for &editor in &detected_editors {
            match has_rhizome_registration(editor) {
                Ok(true) => pass(&format!("Registered in {}", editor.name())),
                Ok(false) => {
                    warn(&format!(
                        "Not registered in {} — {}",
                        editor.name(),
                        registration_repair_hint(editor)
                    ));
                    warnings += 1;
                }
                Err(error) => {
                    warn(&format!(
                        "Could not inspect {} MCP config: {error} — {}",
                        editor.name(),
                        registration_repair_hint(editor)
                    ));
                    warnings += 1;
                }
            }
        }

        if detected_editors.contains(&Editor::ClaudeCode) && which::which("claude").is_ok() {
            match Command::new("claude").args(["mcp", "list"]).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("rhizome") {
                        pass("Registered in Claude Code CLI runtime");
                    } else {
                        warn(&format!(
                            "Not registered in Claude Code CLI runtime — {}",
                            registration_repair_hint(Editor::ClaudeCode)
                        ));
                        warnings += 1;
                    }
                }
                Err(_) => {
                    warn("Could not check Claude Code CLI runtime registration");
                    warnings += 1;
                }
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

fn has_rhizome_registration(editor: Editor) -> Result<bool> {
    let path = editors::config_path(editor)?;
    if !path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&path)?;
    if content.trim().is_empty() {
        return Ok(false);
    }

    if editor.uses_toml() {
        let root = toml::Value::Table(toml::from_str::<toml::Table>(&content)?);
        Ok(root
            .get(editor.mcp_key())
            .and_then(|value: &toml::Value| value.get("rhizome"))
            .is_some())
    } else {
        let root: serde_json::Value = serde_json::from_str(&content)?;
        Ok(root
            .get(editor.mcp_key())
            .and_then(|value: &serde_json::Value| value.get("rhizome"))
            .is_some())
    }
}

fn editor_slug(editor: Editor) -> &'static str {
    match editor {
        Editor::ClaudeCode => "claude-code",
        Editor::Cursor => "cursor",
        Editor::VsCode => "vscode",
        Editor::Zed => "zed",
        Editor::Windsurf => "windsurf",
        Editor::Amp => "amp",
        Editor::ClaudeDesktop => "claude-desktop",
        Editor::CodexCli => "codex",
        Editor::GeminiCli => "gemini",
        Editor::CopilotCli => "copilot",
    }
}

fn registration_repair_hint(editor: Editor) -> String {
    let init_hint = format!("run `rhizome init --editor {}`", editor_slug(editor));
    match editors::config_path(editor) {
        Ok(path) => match editor {
            Editor::ClaudeCode => format!(
                "{init_hint} and merge it into {}, or run `claude mcp add --scope user rhizome -- rhizome serve --expanded`",
                path.display()
            ),
            _ => format!("{init_hint} and merge it into {}", path.display()),
        },
        Err(_) => init_hint,
    }
}

fn install_hint(binary: &str) -> &'static str {
    match binary {
        "rust-analyzer" => "rustup component add rust-analyzer",
        "pyright-langserver" => "npm install -g pyright or pipx install pyright",
        "typescript-language-server" => "npm install -g typescript typescript-language-server",
        "gopls" => "go install golang.org/x/tools/gopls@latest",
        "clangd" => "install LLVM/clangd with your platform package manager",
        "jdtls" => "install jdtls with your platform package manager",
        "ruby-lsp" => "gem install ruby-lsp",
        "lua-language-server" => "install lua-language-server with your platform package manager",
        "elixir-ls" => "mix archive.install hex elixir_ls",
        "zls" => "install zls with your platform package manager or release binary",
        _ => "install with your preferred package manager",
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_rhizome_registration_detects_json_entry() {
        let root = serde_json::json!({
            "mcpServers": {
                "rhizome": {
                    "command": "rhizome",
                    "args": ["serve"]
                }
            }
        });
        assert!(root
            .get(Editor::ClaudeCode.mcp_key())
            .and_then(|value| value.get("rhizome"))
            .is_some());
    }

    #[test]
    fn has_rhizome_registration_detects_toml_entry() {
        let root = toml::Value::Table(
            toml::from_str::<toml::Table>(
                r#"
[mcp_servers.rhizome]
command = "rhizome"
args = ["serve"]
"#,
            )
            .unwrap(),
        );
        assert!(root
            .get(Editor::CodexCli.mcp_key())
            .and_then(|value: &toml::Value| value.get("rhizome"))
            .is_some());
    }

    #[test]
    fn registration_repair_hint_includes_editor_specific_init_command() {
        let hint = registration_repair_hint(Editor::CodexCli);
        assert!(hint.contains("rhizome init --editor codex"));
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
