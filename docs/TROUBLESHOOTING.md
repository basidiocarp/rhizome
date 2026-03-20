# Troubleshooting Guide

Rhizome problems and fixes. This guide covers backend selection, LSP setup, tool failures, and error interpretation.

## Backend Selection Issues

### Symptom: Tool returns empty results, should have data

**Diagnosis**: Tree-sitter backend is being used, language has no query pattern.

Tree-sitter only supports 10 languages with precise query patterns (Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Ruby, PHP). Other languages get basic extraction via generic fallback.

**Fix:**

1. Check `rhizome status` — if LSP is available, use LSP-requiring tool:
   ```
   get_symbols → no detailed output
   → rename_symbol (requires LSP) → full data
   ```

2. If LSP not available, contribute a query pattern:
   - File: `crates/rhizome-treesitter/src/queries.rs`
   - Add language-specific query and compile step
   - See [ARCHITECTURE.md](./ARCHITECTURE.md) for pattern format

### Symptom: LSP tool fails but tree-sitter works

**Diagnosis**: Backend selection chose LSP, but server is unavailable or unresponsive.

**Fix:**

1. Verify server is running:
   ```bash
   rhizome status
   # Look for: LSP binary: <name>, LSP available: Yes
   ```

2. Check server logs (language-specific):
   ```bash
   # Rust
   cat /tmp/ra.log

   # Python
   PYRIGHT_PYTHONPATH=... pyright-langserver --stdio
   ```

3. Restart Rhizome:
   ```bash
   # Kill existing MCP server
   pkill rhizome

   # Start fresh
   rhizome serve
   ```

### Symptom: "Tool requires LSP but server unavailable" error

**Diagnosis**: Tool (e.g., `rename_symbol`) needs LSP, server binary not found.

**Fix:**

See [LANGUAGE-SETUP.md: Path 2](./LANGUAGE-SETUP.md#path-2-lsp-languages-auto-install) for auto-install or manual setup.

Quick check:
```bash
which rust-analyzer  # Check if binary is in PATH
ls ~/.rhizome/bin/   # Check auto-installed binaries
```

## LSP Server Auto-Install Issues

### Symptom: Auto-install fails silently, server still missing

**Diagnosis**: Package manager not found, or install recipe error.

**How auto-install works:**

1. Look up recipe by server binary name (e.g., `rust-analyzer` → `rustup component add`)
2. Check if package manager exists (`which rustup`)
3. If yes, run install command
4. If any step fails, return None and tool gets tree-sitter fallback

**Fix:**

1. Check if package manager is installed:
   ```bash
   which rustup    # Rust
   which pip3      # Python
   which npm       # JavaScript/TypeScript
   which go        # Go
   which gem       # Ruby
   ```

2. Install package manager if missing

3. Manual install using hint:
   ```bash
   rustup component add rust-analyzer
   ```

4. Verify:
   ```bash
   rhizome status
   ```

### Symptom: "LSP auto-install disabled"

**Diagnosis**: You set `RHIZOME_DISABLE_LSP_DOWNLOAD=1` or config has `disable_download = true`.

**Fix:**

```bash
# Check environment
echo $RHIZOME_DISABLE_LSP_DOWNLOAD  # Should be unset

# Check config
cat ~/.config/rhizome/config.toml
# [lsp] section should have disable_download = false (or omitted)

# Unset variable
unset RHIZOME_DISABLE_LSP_DOWNLOAD

# Or edit config to re-enable
```

### Symptom: "Package manager not found: rustup"

**Diagnosis**: Package manager not in PATH, can't proceed with auto-install.

**Example**: You have Rust installed but `rustup` isn't accessible.

**Fix:**

1. Install package manager or make it available in PATH:
   ```bash
   # Rust: curl https://sh.rustup.rs -sSf | sh
   # Python: already comes with pip
   # Node: brew install node (macOS)
   ```

2. Or manually install server:
   ```bash
   rustup component add rust-analyzer
   ```

3. Verify:
   ```bash
   which rust-analyzer
   rhizome status
   ```

## Tool Execution Failures

### Symptom: "Unknown tool: <name>"

**Diagnosis**: Misspelled tool name or tool doesn't exist.

**Fix:**

List all tools:
```bash
rhizome list-tools
```

Check tool name matches exactly (case-sensitive).

### Symptom: Tool times out or hangs

**Diagnosis**: LSP server unresponsive, network issue, or large file.

**Fix:**

1. Kill hanging process:
   ```bash
   pkill rhizome
   ```

2. Check if LSP server is responsive:
   ```bash
   # Manual LSP test
   echo '{"jsonrpc": "2.0", "method": "initialize", ...}' | rust-analyzer
   ```

3. Reduce scope:
   - Try smaller file
   - Try different tool
   - Check logs: `RUST_LOG=debug rhizome serve`

4. Restart with fresh LSP connection:
   ```bash
   pkill rhizome
   rhizome serve
   ```

### Symptom: "File not found" error

**Diagnosis**: Tool received relative path, needs absolute path.

**Fix:**

Always use absolute paths:
```bash
# Wrong
rhizome symbols src/main.rs

# Right
rhizome symbols /path/to/project/src/main.rs
```

For MCP, client must pass absolute path in request.

### Symptom: Tool works on one file, fails on another

**Diagnosis**: File-specific issue (syntax error, unsupported syntax, encoding).

**Fix:**

1. Check file is valid:
   ```bash
   # Syntax check
   rustc --crate-type lib src/file.rs  # For Rust
   python -m py_compile src/file.py    # For Python
   ```

2. Check encoding:
   ```bash
   file src/file.rs
   # Should be "ASCII text" or "UTF-8 Unicode text"
   ```

3. Check for unsupported syntax:
   - Macros in Rust (tree-sitter can't fully parse)
   - Template syntax in C++ (tree-sitter limitations)

4. File issue if valid but still fails

## Hyphae Export Failures

### Symptom: "Failed to export to Hyphae"

**Diagnosis**: Hyphae not installed, not running, or connection issue.

**Fix:**

1. Check if Hyphae is available:
   ```bash
   which hyphae
   # Or check if MCP server for Hyphae is running
   ```

2. Start Hyphae:
   ```bash
   hyphae serve  # Or however Hyphae is started
   ```

3. Check IPC connectivity:
   ```bash
   # If using socket:
   ls /tmp/hyphae.sock

   # If using stdio, verify MCP server is running
   ```

4. Check logs:
   ```bash
   RUST_LOG=debug rhizome serve
   # Look for export-related errors
   ```

### Symptom: Export incomplete (some files missing)

**Diagnosis**: Large project, export interrupted or timed out.

**Fix:**

1. Check which files were exported:
   ```bash
   # Check Hyphae database
   hyphae query "SELECT COUNT(*) FROM symbols"
   ```

2. Re-run export:
   ```bash
   rhizome export /path/to/project
   ```

3. Or export subset of project:
   ```bash
   rhizome export /path/to/project/src
   ```

## Configuration Issues

### Symptom: Config file not being read

**Diagnosis**: File in wrong location, TOML syntax error, or wrong permissions.

**Fix:**

1. Check file exists and is readable:
   ```bash
   # Global config
   cat ~/.config/rhizome/config.toml

   # Project config
   cat <project>/.rhizome/config.toml
   ```

2. Validate TOML syntax:
   ```bash
   # Python
   python3 -c "import tomllib; tomllib.loads(open('config.toml').read())"

   # Or use online validator: https://www.toml-lint.com/
   ```

3. Check permissions:
   ```bash
   ls -la ~/.config/rhizome/config.toml
   # Should be readable by your user
   ```

4. Rhizome loads in this order:
   ```
   1. Global: ~/.config/rhizome/config.toml
   2. Project: <project>/.rhizome/config.toml (overrides global)
   3. Environment: RHIZOME_* variables (override both)
   ```

   To verify what's loaded:
   ```bash
   RUST_LOG=debug rhizome serve 2>&1 | grep -i config
   ```

### Symptom: Custom server binary not being used

**Diagnosis**: Config not loaded, wrong key name, or binary path wrong.

**Fix:**

1. Verify config:
   ```toml
   [languages.rust]
   server_binary = "/opt/custom/rust-analyzer"
   ```

2. Check key names are correct:
   - `server_binary` (not `binary` or `server`)
   - `server_args` (not `args`)
   - Language key like `rust` (not `Rust`)

3. Test binary works:
   ```bash
   /opt/custom/rust-analyzer --version
   # Should print version, not error
   ```

4. Reload config:
   ```bash
   # Kill and restart Rhizome
   pkill rhizome
   rhizome serve
   ```

### Symptom: Project config not overriding global

**Diagnosis**: Config merge issue or incorrect path.

**Fix:**

1. Check project root is detected:
   ```bash
   RUST_LOG=debug rhizome serve 2>&1 | grep -i "project.root"
   ```

2. Verify project config exists:
   ```bash
   cat <project>/.rhizome/config.toml
   ```

3. Check key names are correct (must match exactly)

4. Project config path must be: `<detected_root>/.rhizome/config.toml`
   - If root detected wrong, project config won't be found
   - See [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md) for root detection

## Performance Issues

### Symptom: Slow symbol extraction on large files

**Diagnosis**: Tree-sitter or LSP processing overhead, timeout.

**Fix:**

1. Check file size:
   ```bash
   wc -l src/file.rs  # Lines of code
   du -h src/file.rs  # File size
   ```

   If >10K lines or >1MB, expect slower extraction.

2. Switch to LSP if available:
   - LSP is typically faster for large files
   - `find_references`, `rename_symbol` forces LSP use

3. Reduce scope:
   - Extract symbols from module instead of whole file
   - Use `search_symbols` instead of scanning file

4. Check resource usage:
   ```bash
   # During rhizome operation
   top -p $(pgrep rhizome)  # CPU, memory
   ```

   If high memory, file a bug.

### Symptom: MCP server connection slow

**Diagnosis**: JSON-RPC overhead, network latency.

**Fix:**

1. Use direct CLI instead:
   ```bash
   rhizome symbols /path/to/file.rs
   # Faster than MCP for single operations
   ```

2. Batch operations if possible
   - Reduce tool call overhead

3. Check network:
   - If MCP over network, latency adds up
   - Use local socket for best performance

## Debugging

### Enable debug logging

```bash
# Show all debug messages
RUST_LOG=debug rhizome serve

# Show only Rhizome logs (not dependencies)
RUST_LOG=rhizome=debug rhizome serve

# Show specific module
RUST_LOG=rhizome_mcp::tools=debug rhizome serve
```

### Check version

```bash
rhizome --version
```

### Report a bug

Include:
1. `rhizome status` output
2. `rhizome --version` output
3. Relevant config files (`~/.config/rhizome/config.toml`, project `.rhizome/config.toml`)
4. Logs with `RUST_LOG=debug rhizome serve`
5. Example file that reproduces issue
6. Expected vs actual behavior

## Common Error Messages

| Error | Cause | Fix |
|-------|-------|-----|
| "Unsupported extension .xyz" | File type not recognized | Check if Language enum supports extension |
| "No tree-sitter grammar for <lang>" | Language has no query pattern | Use LSP if available |
| "LSP server not found: rust-analyzer" | Binary not in PATH | Auto-install or install manually |
| "Auto-install disabled" | `RHIZOME_DISABLE_LSP_DOWNLOAD=1` | Unset variable or enable in config |
| "Cannot initialize LSP: connection timeout" | Server not responding | Kill and restart server |
| "Project root not detected" | Wrong markers or path | Check language root markers |
| "File not found: /path/to/file" | Relative path passed instead of absolute | Use absolute path |
| "Tool not found: <name>" | Misspelled tool name | Run `rhizome list-tools` to verify |
| "Export failed: Hyphae unavailable" | Hyphae not running | Start Hyphae, check connection |
| "Configuration parse error" | Invalid TOML syntax | Validate TOML, fix syntax |

## Performance Benchmarks

For reference, typical performance:

| Operation | File Size | Time | Backend |
|-----------|-----------|------|---------|
| `get_symbols` | 100 lines | <10ms | Tree-sitter |
| `get_symbols` | 5000 lines | 50-100ms | Tree-sitter |
| `find_references` | 5000 lines | 100-500ms | LSP |
| `rename_symbol` | Any | 100-300ms | LSP |
| `export_to_hyphae` | Project (100 files) | 1-5s | Tree-sitter |

If your times are much worse, check:
- File encoding (UTF-8 preferred)
- File syntax validity
- System resources (CPU, disk, memory)

## When to Escalate

File a bug if:
1. Same operation fails consistently on multiple files
2. Error message is unclear or contradicts documentation
3. Performance degradation after upgrade
4. Crash with stack trace
5. LSP server crashes with logs

Include reproduction steps and logs (see "Report a bug" section above).
