//! `rhizome doctor` — diagnose common issues with the rhizome installation.

use anyhow::Result;
use ignore::WalkBuilder;
use rhizome_core::{
    BackendSelector, Language, LanguageStatus, RhizomeConfig, derive_export_identity,
    manual_install_hint,
};
use spore::editors::{self, Editor};
use spore::jsonrpc::{Request, Response};
use spore::{Tool, discover};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

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
    let project_root = detect_project_root();
    let config = RhizomeConfig::load(&project_root).unwrap_or_default();
    let configured_languages = configured_languages(&config);
    let detected_languages = detect_project_languages(&project_root);
    let detected_language_set: HashSet<Language> = detected_languages
        .iter()
        .map(|(language, _count)| language.clone())
        .collect();
    let mut selector = BackendSelector::new(config);
    let statuses = selector.status();
    let relevant_statuses =
        relevant_lsp_statuses(&statuses, &detected_language_set, &configured_languages);
    let installer = selector.installer();

    println!("\x1b[1mTree-Sitter Backends\x1b[0m");
    let tree_sitter_languages = statuses.iter().filter(|status| status.tree_sitter).count();
    pass(&format!(
        "{tree_sitter_languages} languages with tree-sitter support"
    ));

    // ─────────────────────────────────────────────────────────────────────────
    // LSP Servers
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mLSP Servers\x1b[0m");
    let mut lsp_found = 0;
    let lsp_binaries = summarize_lsp_binaries(&relevant_statuses);
    let configured_lsp_servers = lsp_binaries.len();
    if relevant_statuses.is_empty() || lsp_binaries.is_empty() {
        pass("No project or explicitly configured languages require LSP checks");
    }
    for binary in &lsp_binaries {
        if binary.available {
            let path = binary
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "available".to_string());
            pass(&format!(
                "{} found ({}) at {}",
                binary.binary,
                format_languages(&binary.languages),
                path
            ));
            lsp_found += 1;
        } else {
            warn(&format!(
                "{} not found ({}) — install: {}",
                binary.binary,
                format_languages(&binary.languages),
                manual_install_hint(&binary.binary, installer.bin_dir())
            ));
            warnings += 1;
        }
    }
    pass(&format!(
        "{lsp_found}/{} relevant LSP servers installed",
        configured_lsp_servers
    ));

    // ─────────────────────────────────────────────────────────────────────────
    // Hyphae Integration
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mHyphae Integration\x1b[0m");
    match discover(Tool::Hyphae) {
        Some(info) => match probe_hyphae_export_path(&info.binary_path, &project_root) {
            Ok(()) => {
                pass("Hyphae export path healthy (serve accepts and validates export payloads)")
            }
            Err(error) => {
                warn(&format!("Hyphae export path unavailable — {error}"));
                warnings += 1;
            }
        },
        None => {
            warn("Hyphae not installed — code graph export disabled");
            warnings += 1;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Export Cache
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mExport Cache\x1b[0m");
    let cache_path = rhizome_core::ExportCache::cache_path(&project_root);
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
        if fix {
            print!("  Rebuilding export cache... ");
            match run_export_fix(&project_root) {
                Ok(()) => pass("Export completed"),
                Err(error) => {
                    if cache_path.exists() {
                        warn(&format!(
                            "Export rebuilt the cache but reported an error: {error}"
                        ));
                        warnings += 1;
                    } else {
                        fail(&format!("Export failed: {error}"));
                        errors += 1;
                    }
                }
            }
        } else {
            warn(&format!("No export cache at {}", cache_path.display()));
            warnings += 1;
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
    if detected_languages.is_empty() {
        warn("No recognized source files found");
        warnings += 1;
    } else {
        let summary: Vec<String> = detected_languages
            .iter()
            .take(5)
            .map(|(language, count)| format!("{language} ({count})"))
            .collect();
        pass(&format!("Languages detected: {}", summary.join(", ")));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // MCP Registration
    // ─────────────────────────────────────────────────────────────────────────
    println!();
    println!("\x1b[1mMCP Registration\x1b[0m");
    match discover(Tool::Rhizome) {
        Some(info) => pass(&format!("rhizome binary at {}", info.binary_path.display())),
        None => {
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
        _ => "editor",
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

fn detect_project_root() -> PathBuf {
    let root = std::env::var_os("RHIZOME_PROJECT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    canonical_project_root(root)
}

fn canonical_project_root(root: PathBuf) -> PathBuf {
    std::fs::canonicalize(&root).unwrap_or(root)
}

fn run_export_fix(project_root: &Path) -> Result<()> {
    let executable = std::env::current_exe()?;
    let (program, args) = export_fix_invocation(executable, project_root);
    let output = Command::new(program).args(args).output()?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("exit status {}", output.status)
    };

    anyhow::bail!("{details}");
}

fn export_fix_invocation(executable: PathBuf, project_root: &Path) -> (PathBuf, Vec<OsString>) {
    let args = vec![
        OsString::from("export"),
        OsString::from("--project"),
        project_root.as_os_str().to_os_string(),
    ];
    (executable, args)
}

fn configured_languages(config: &RhizomeConfig) -> HashSet<Language> {
    config
        .languages
        .keys()
        .filter_map(|name| Language::from_name(name))
        .collect()
}

fn relevant_lsp_statuses<'a>(
    statuses: &'a [LanguageStatus],
    detected_languages: &HashSet<Language>,
    configured_languages: &HashSet<Language>,
) -> Vec<&'a LanguageStatus> {
    statuses
        .iter()
        .filter(|status| {
            detected_languages.contains(&status.language)
                || configured_languages.contains(&status.language)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LspBinaryStatus {
    binary: String,
    languages: Vec<Language>,
    available: bool,
    path: Option<PathBuf>,
}

fn summarize_lsp_binaries(statuses: &[&LanguageStatus]) -> Vec<LspBinaryStatus> {
    let mut by_binary: BTreeMap<String, LspBinaryStatus> = BTreeMap::new();

    for status in statuses
        .iter()
        .filter(|status| status.lsp_binary != "(none)")
    {
        let entry = by_binary
            .entry(status.lsp_binary.clone())
            .or_insert_with(|| LspBinaryStatus {
                binary: status.lsp_binary.clone(),
                languages: Vec::new(),
                available: false,
                path: None,
            });
        if !entry.languages.contains(&status.language) {
            entry.languages.push(status.language.clone());
        }
        entry.available |= status.lsp_available;
        if entry.path.is_none() {
            entry.path = status.lsp_path.clone();
        }
    }

    for binary in by_binary.values_mut() {
        binary
            .languages
            .sort_by_key(|language| language.to_string());
    }

    by_binary.into_values().collect()
}

fn format_languages(languages: &[Language]) -> String {
    languages
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn detect_project_languages(root: &Path) -> Vec<(Language, usize)> {
    let mut counts: HashMap<Language, usize> = HashMap::new();

    let walker = WalkBuilder::new(root).hidden(true).git_ignore(true).build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        let Some(language) = Language::from_extension(extension) else {
            continue;
        };
        if !language.tree_sitter_supported() {
            continue;
        }

        *counts.entry(language).or_insert(0) += 1;
    }

    let mut detected: Vec<(Language, usize)> = counts.into_iter().collect();
    detected.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
    });
    detected
}

fn probe_hyphae_export_path(binary: &Path, project_root: &Path) -> Result<()> {
    let mut process = HyphaeProbeProcess::spawn(binary)?;

    process.send_request(&Request::new(
        "initialize",
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "rhizome-doctor",
                "version": env!("CARGO_PKG_VERSION"),
            },
        }),
    ))?;
    let initialize = process.recv_response(Duration::from_secs(5))?;
    if initialize.result.is_none() {
        anyhow::bail!("Hyphae initialize returned no result");
    }

    process.send_request(&Request::new("tools/list", serde_json::json!({})))?;
    let response = process.recv_response(Duration::from_secs(5))?;
    let result = response
        .result
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Hyphae tools/list returned no result"))?;
    let tools = result
        .get("tools")
        .and_then(|value| value.as_array())
        .ok_or_else(|| anyhow::anyhow!("Hyphae tools/list response missing tools array"))?;

    let import_tool = tools
        .iter()
        .find(|tool| {
            tool.get("name")
                .and_then(|value| value.as_str())
                .is_some_and(|name| name == "hyphae_import_code_graph")
        })
        .ok_or_else(|| anyhow::anyhow!("hyphae_import_code_graph not advertised by Hyphae"))?;
    validate_hyphae_import_schema(import_tool)?;

    process.send_request(&Request::new(
        "tools/call",
        serde_json::json!({
            "name": "hyphae_import_code_graph",
            "arguments": hyphae_probe_arguments(project_root)
        }),
    ))?;
    let response = process.recv_response(Duration::from_secs(5))?;
    let error_text = probe_error_text_from_response(&response)?;
    if hyphae_probe_error_indicates_argument_shape_accepted(error_text) {
        Ok(())
    } else {
        anyhow::bail!("hyphae_import_code_graph returned an unexpected probe error: {error_text}");
    }
}

fn validate_hyphae_import_schema(tool: &serde_json::Value) -> Result<()> {
    let properties = tool
        .get("inputSchema")
        .and_then(|value| value.get("properties"))
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            anyhow::anyhow!("hyphae_import_code_graph inputSchema missing properties")
        })?;

    validate_schema_type(properties, "project", "string")?;
    validate_schema_type(properties, "project_root", "string")?;
    validate_schema_type(properties, "worktree_id", "string")?;
    validate_schema_type(properties, "nodes", "array")?;
    validate_schema_type(properties, "edges", "array")?;

    Ok(())
}

fn hyphae_probe_arguments(project_root: &Path) -> serde_json::Value {
    let identity = derive_export_identity(project_root);
    let mut arguments = serde_json::Map::from_iter([
        (
            "project".to_string(),
            serde_json::Value::String(identity.project),
        ),
        (
            "nodes".to_string(),
            serde_json::json!([{
                "name": "doctor-probe-node",
                "labels": ["function"],
                "description": "Non-persistent validation probe"
            }]),
        ),
        (
            "edges".to_string(),
            serde_json::json!([{
                "source": "doctor-probe-node",
                "target": "missing-target",
                "relation": "calls",
                "weight": 0.3
            }]),
        ),
    ]);
    arguments.insert(
        "project_root".to_string(),
        serde_json::Value::String(identity.project_root),
    );
    if let Some(worktree_id) = identity.worktree_id {
        arguments.insert(
            "worktree_id".to_string(),
            serde_json::Value::String(worktree_id),
        );
    }

    serde_json::Value::Object(arguments)
}

fn validate_schema_type(
    properties: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    expected_type: &str,
) -> Result<()> {
    let property = properties
        .get(field)
        .ok_or_else(|| anyhow::anyhow!("hyphae_import_code_graph inputSchema missing `{field}`"))?;
    let actual_type = property
        .get("type")
        .ok_or_else(|| anyhow::anyhow!("hyphae_import_code_graph `{field}` is missing a type"))?;
    let matches = match actual_type {
        serde_json::Value::String(value) => value == expected_type,
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| value.as_str() == Some(expected_type)),
        _ => false,
    };

    if matches {
        return Ok(());
    }

    anyhow::bail!("hyphae_import_code_graph `{field}` must advertise type `{expected_type}`")
}

fn probe_error_text_from_response(response: &Response) -> Result<&str> {
    if let Some(result) = response.result.as_ref() {
        let is_error = result
            .get("isError")
            .and_then(|value| value.as_bool())
            .ok_or_else(|| {
                anyhow::anyhow!("Hyphae tools/call response missing structured error flag")
            })?;
        if !is_error {
            anyhow::bail!(
                "hyphae_import_code_graph accepted an intentionally invalid probe payload"
            );
        }

        let error_text = result
            .get("content")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|item| item.get("text"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow::anyhow!("Hyphae tools/call response missing error text"))?;
        return Ok(error_text);
    }

    if let Some(error) = response.error.as_ref() {
        if let Some(text) = error
            .data
            .as_ref()
            .and_then(|data| data.get("content"))
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|item| item.get("text"))
            .and_then(|value| value.as_str())
        {
            return Ok(text);
        }

        return Ok(error.message.as_str());
    }

    anyhow::bail!("Hyphae tools/call returned no result or JSON-RPC error")
}

fn hyphae_probe_error_indicates_argument_shape_accepted(error_text: &str) -> bool {
    let normalized = error_text.trim().to_ascii_lowercase();
    let mentions_validation = normalized.contains("validation") || normalized.contains("invalid");
    let mentions_edge_target = normalized.contains("edges[0]")
        || (normalized.contains("edge") && normalized.contains("target"));
    let mentions_missing_node = normalized.contains("not found")
        || normalized.contains("missing")
        || normalized.contains("unknown")
        || normalized.contains("unresolved");

    mentions_validation && mentions_edge_target && mentions_missing_node
}

struct HyphaeProbeProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Option<ChildStdout>,
}

impl HyphaeProbeProcess {
    fn spawn(binary: &Path) -> Result<Self> {
        let mut child = Command::new(binary)
            .args(["serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Hyphae probe missing stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Hyphae probe missing stdout"))?;

        Ok(Self {
            child,
            stdin,
            stdout: Some(stdout),
        })
    }

    fn send_request(&mut self, request: &Request) -> Result<()> {
        let encoded = serde_json::to_string(request)?;
        self.stdin.write_all(encoded.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn recv_response(&mut self, timeout: Duration) -> Result<Response> {
        let stdout = self
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Hyphae probe missing stdout reader"))?;
        let (response, stdout) = recv_line_delimited_response(stdout, timeout)?;
        self.stdout = Some(stdout);
        Ok(response)
    }
}

impl Drop for HyphaeProbeProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn recv_line_delimited_response(
    stdout: ChildStdout,
    timeout: Duration,
) -> Result<(Response, ChildStdout)> {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            let read = match reader.read_line(&mut line) {
                Ok(read) => read,
                Err(error) => {
                    let _ = tx.send((
                        Err::<Response, anyhow::Error>(error.into()),
                        reader.into_inner(),
                    ));
                    return;
                }
            };

            if read == 0 {
                let _ = tx.send((
                    Err::<Response, anyhow::Error>(anyhow::anyhow!(
                        "EOF while reading Hyphae response"
                    )),
                    reader.into_inner(),
                ));
                return;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() || !trimmed.starts_with('{') {
                continue;
            }

            match serde_json::from_str::<Response>(trimmed) {
                Ok(response) => {
                    let _ = tx.send((Ok(response), reader.into_inner()));
                    return;
                }
                Err(error) => {
                    let _ = tx.send((Err(error.into()), reader.into_inner()));
                    return;
                }
            }
        }
    });

    let (result, stdout) = rx
        .recv_timeout(timeout)
        .map_err(|_| anyhow::anyhow!("Hyphae probe timed out waiting for a response"))?;
    result.map(|response| (response, stdout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

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
        assert!(
            root.get(Editor::ClaudeCode.mcp_key())
                .and_then(|value| value.get("rhizome"))
                .is_some()
        );
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
        assert!(
            root.get(Editor::CodexCli.mcp_key())
                .and_then(|value: &toml::Value| value.get("rhizome"))
                .is_some()
        );
    }

    #[test]
    fn registration_repair_hint_includes_editor_specific_init_command() {
        let hint = registration_repair_hint(Editor::CodexCli);
        assert!(hint.contains("rhizome init --editor codex"));
    }

    #[test]
    fn relevant_lsp_statuses_only_include_project_or_configured_languages() {
        let statuses = vec![
            LanguageStatus {
                language: Language::Rust,
                tree_sitter: true,
                lsp_binary: "rust-analyzer".into(),
                lsp_available: true,
                lsp_path: None,
            },
            LanguageStatus {
                language: Language::Java,
                tree_sitter: true,
                lsp_binary: "jdtls".into(),
                lsp_available: false,
                lsp_path: None,
            },
            LanguageStatus {
                language: Language::Lua,
                tree_sitter: true,
                lsp_binary: "lua-language-server".into(),
                lsp_available: true,
                lsp_path: None,
            },
        ];

        let detected = HashSet::from([Language::Rust]);
        let configured = HashSet::from([Language::Java]);
        let relevant = relevant_lsp_statuses(&statuses, &detected, &configured);

        assert_eq!(relevant.len(), 2);
        assert_eq!(relevant[0].language, Language::Rust);
        assert_eq!(relevant[1].language, Language::Java);
    }

    #[test]
    fn summarize_lsp_binaries_deduplicates_shared_servers() {
        let statuses = vec![
            LanguageStatus {
                language: Language::JavaScript,
                tree_sitter: true,
                lsp_binary: "typescript-language-server".into(),
                lsp_available: true,
                lsp_path: Some(PathBuf::from("/tmp/typescript-language-server")),
            },
            LanguageStatus {
                language: Language::TypeScript,
                tree_sitter: true,
                lsp_binary: "typescript-language-server".into(),
                lsp_available: true,
                lsp_path: Some(PathBuf::from("/tmp/typescript-language-server")),
            },
            LanguageStatus {
                language: Language::C,
                tree_sitter: true,
                lsp_binary: "clangd".into(),
                lsp_available: false,
                lsp_path: None,
            },
        ];

        let refs: Vec<&LanguageStatus> = statuses.iter().collect();
        let binaries = summarize_lsp_binaries(&refs);

        assert_eq!(binaries.len(), 2);
        assert_eq!(binaries[0].binary, "clangd");
        assert_eq!(binaries[0].languages, vec![Language::C]);
        assert!(!binaries[0].available);
        assert_eq!(binaries[1].binary, "typescript-language-server");
        assert_eq!(
            binaries[1].languages,
            vec![Language::JavaScript, Language::TypeScript]
        );
        assert!(binaries[1].available);
        assert_eq!(
            binaries[1].path,
            Some(PathBuf::from("/tmp/typescript-language-server"))
        );
    }

    #[test]
    fn detect_project_languages_recognizes_nested_supported_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".gitignore"), "ignored/\n*.generated.rs\n").unwrap();
        let nested = dir.path().join("nested");
        let ignored = dir.path().join("ignored");
        let hidden = dir.path().join(".hidden");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(&ignored).unwrap();
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(nested.join("worker.py"), "def worker():\n    pass\n").unwrap();
        std::fs::write(nested.join("config.yml"), "name: example\n").unwrap();
        std::fs::write(nested.join("infra.tf"), "terraform {}\n").unwrap();
        std::fs::write(nested.join("notes.txt"), "ignore me\n").unwrap();
        std::fs::write(ignored.join("skipped.go"), "package main\n").unwrap();
        std::fs::write(hidden.join("skipped.ts"), "export {}\n").unwrap();
        std::fs::write(dir.path().join("build.generated.rs"), "fn skip() {}\n").unwrap();

        let detected = detect_project_languages(dir.path());

        assert_eq!(detected.len(), 2);
        assert!(
            detected
                .iter()
                .any(|(language, count)| *language == Language::Rust && *count == 1)
        );
        assert!(
            detected
                .iter()
                .any(|(language, count)| *language == Language::Python && *count == 1)
        );
        assert!(
            detected
                .iter()
                .all(|(language, _)| *language != Language::Go)
        );
        assert!(
            detected
                .iter()
                .all(|(language, _)| *language != Language::TypeScript)
        );
        assert!(
            detected
                .iter()
                .all(|(language, _)| *language != Language::Terraform)
        );
        assert!(
            detected
                .iter()
                .all(|(language, _)| *language != Language::Yaml)
        );
    }

    #[cfg(unix)]
    #[test]
    fn probe_hyphae_export_path_requires_valid_export_argument_shape() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().join("repo");
        let binary = dir.path().join("hyphae");
        let log_path = dir.path().join("probe.log");
        fs::create_dir_all(project_root.join(".git")).unwrap();
        fs::write(project_root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        let canonical_project_root = project_root.canonicalize().unwrap();
        let project_name = project_root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let script = r#"#!/usr/bin/env python3
import json
import sys

if len(sys.argv) > 1 and sys.argv[1] == "--version":
    print("hyphae 1.2.3")
    raise SystemExit(0)

if len(sys.argv) > 1 and sys.argv[1] == "serve":
    log_path = "__LOG_PATH__"
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        message = json.loads(line)
        method = message.get("method")
        args = message.get("params", {}).get("arguments", {})
        with open(log_path, "a", encoding="utf-8") as log:
            log.write(method + "\n")
            if method == "tools/call":
                log.write(json.dumps(args, sort_keys=True) + "\n")
        if method == "initialize":
            result = {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "hyphae", "version": "1.2.3"},
            }
        elif method == "tools/list":
            result = {
                "tools": [
                    {
                        "name": "hyphae_import_code_graph",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "project": {"type": "string"},
                                "project_root": {"type": "string"},
                                "worktree_id": {"type": "string"},
                                "nodes": {"type": "array"},
                                "edges": {"type": "array"},
                            },
                        },
                    }
                ]
            }
        elif method == "tools/call":
            if args == {
                "project": "__PROJECT_NAME__",
                "project_root": "__PROJECT_ROOT__",
                "worktree_id": "main",
                "nodes": [{
                    "name": "doctor-probe-node",
                    "labels": ["function"],
                    "description": "Non-persistent validation probe",
                }],
                "edges": [{
                    "source": "doctor-probe-node",
                    "target": "missing-target",
                    "relation": "calls",
                    "weight": 0.3,
                }],
            }:
                result = {
                    "isError": True,
                    "content": [{
                        "type": "text",
                        "text": "validation error: edges[0] target unknown to nodes",
                    }],
                }
            else:
                result = {
                    "isError": True,
                    "content": [{
                        "type": "text",
                        "text": "unexpected probe payload",
                    }],
                }
        else:
            result = {}
        sys.stdout.write(json.dumps({
            "jsonrpc": "2.0",
            "id": message.get("id"),
            "result": result,
        }) + "\n")
        sys.stdout.flush()
    raise SystemExit(0)

raise SystemExit(1)
        "#;
        fs::write(
            &binary,
            script
                .replace("__LOG_PATH__", &log_path.display().to_string())
                .replace(
                    "__PROJECT_ROOT__",
                    &canonical_project_root.display().to_string(),
                )
                .replace("__PROJECT_NAME__", &project_name),
        )
        .unwrap();
        #[cfg(unix)]
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        #[cfg(unix)]
        permissions.set_mode(0o755);
        #[cfg(unix)]
        fs::set_permissions(&binary, permissions).unwrap();

        probe_hyphae_export_path(&binary, &project_root).unwrap();

        let log = fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains("tools/call"),
            "probe should call tools/call: {log}"
        );
        assert!(
            log.contains(&format!("\"project\": \"{project_name}\"")),
            "probe should send the derived project name: {log}"
        );
        assert!(
            log.contains(&format!(
                "\"project_root\": \"{}\"",
                canonical_project_root.display()
            )),
            "probe should include the derived project_root in the export payload: {log}"
        );
        assert!(
            log.contains("\"worktree_id\": \"main\""),
            "probe should include worktree_id in the export payload: {log}"
        );
    }

    #[test]
    fn validate_hyphae_import_schema_requires_identity_fields() {
        let tool = serde_json::json!({
            "name": "hyphae_import_code_graph",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "nodes": { "type": "array" },
                    "edges": { "type": "array" }
                }
            }
        });

        let err = validate_hyphae_import_schema(&tool).unwrap_err();
        assert!(err.to_string().contains("project_root"));
    }

    #[test]
    fn hyphae_probe_arguments_include_project_root_without_worktree_id() {
        let dir = tempfile::tempdir().unwrap();
        let args = hyphae_probe_arguments(dir.path());

        assert!(args.get("project").is_some());
        assert!(args.get("project_root").is_some());
        assert!(args.get("worktree_id").is_none());
    }

    #[test]
    fn hyphae_probe_error_shape_accepts_non_shallow_validation_errors() {
        assert!(hyphae_probe_error_indicates_argument_shape_accepted(
            "edges[0]: target missing during validation"
        ));
        assert!(hyphae_probe_error_indicates_argument_shape_accepted(
            "validation error: edges[0] target unknown to nodes"
        ));
        assert!(hyphae_probe_error_indicates_argument_shape_accepted(
            "invalid edge target: unresolved node reference"
        ));
        assert!(!hyphae_probe_error_indicates_argument_shape_accepted(
            "missing required field: project"
        ));
        assert!(!hyphae_probe_error_indicates_argument_shape_accepted(
            "unexpected probe payload"
        ));
        assert!(!hyphae_probe_error_indicates_argument_shape_accepted(
            "failed to create memoir: database is locked"
        ));
    }

    #[test]
    fn probe_error_text_from_response_accepts_top_level_jsonrpc_error() {
        let response = Response {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: None,
            error: Some(spore::jsonrpc::RpcError {
                code: -32602,
                message: "validation error: edges[0] target unknown to nodes".to_string(),
                data: None,
            }),
        };

        let error_text = probe_error_text_from_response(&response).unwrap();
        assert_eq!(
            error_text,
            "validation error: edges[0] target unknown to nodes"
        );
    }

    #[test]
    fn export_fix_invocation_uses_current_binary_and_project_flag() {
        let executable = PathBuf::from("/tmp/rhizome");
        let project_root = Path::new("/tmp/project");
        let (program, args) = export_fix_invocation(executable.clone(), project_root);

        assert_eq!(program, executable);
        assert_eq!(
            args,
            vec![
                OsString::from("export"),
                OsString::from("--project"),
                OsString::from("/tmp/project"),
            ]
        );
    }

    #[test]
    fn canonical_project_root_keeps_nested_project_root() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path().join("repo");
        let nested_root = repo_root.join("packages/app");

        std::fs::create_dir_all(&nested_root).unwrap();
        std::fs::create_dir_all(repo_root.join(".git")).unwrap();

        let canonical = canonical_project_root(nested_root.clone());

        assert_eq!(canonical, nested_root.canonicalize().unwrap());
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
