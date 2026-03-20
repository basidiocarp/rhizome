# Rhizome Documentation

Comprehensive guides for developers building with or debugging Rhizome. These docs assume you're familiar with code intelligence tools, LSP, and tree-sitter. They focus on mechanism: what happens when X, not marketing abstractions.

## Documents

### [ARCHITECTURE.md](./ARCHITECTURE.md) — System Design

How Rhizome is organized and how data flows.

**Read this when:**
- You're new to Rhizome and want to understand how it works
- You need to add a new language or tool
- You're debugging a backend selection issue
- You want to understand the 5-crate workspace

**Key sections:**
- 5-crate workspace structure (rhizome-core, rhizome-treesitter, rhizome-lsp, rhizome-mcp, rhizome-cli)
- Core trait: CodeIntelligence
- Tool call flow: request → backend selection → execution → response
- Backend selection logic: RequiresLsp, PrefersLsp, TreeSitter
- Tree-sitter backend: query patterns, 10 languages with precision, 8 with fallback
- LSP backend: multi-client design, per-project-root servers
- Root detection: language-specific markers (Cargo.toml, package.json, go.mod, etc.)
- Configuration loading and merging
- MCP tools: 26 tools across 4 categories
- Hyphae integration

### [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md) — Getting Languages Working

Practical guide: "How do I make language X work with Rhizome?"

**Read this when:**
- A language isn't working and you need setup help
- You're evaluating if Rhizome supports your language
- You need to install an LSP server manually
- You want to override default server configuration

**Key sections:**
- Path 1: Out-of-the-box languages (18 with tree-sitter, zero setup)
- Path 2: LSP languages (32, auto-install on first use)
- Path 3: Custom LSP configuration (TOML overrides)
- How auto-install works (recipe lookup → package manager check → install)
- Disable auto-install: RHIZOME_DISABLE_LSP_DOWNLOAD env var or config
- Custom server examples: Java (JDTLS), C/C++ (Clangd)
- Environment variables: RHIZOME_DISABLE_LSP_DOWNLOAD, RHIZOME_PROJECT, RUST_LOG

### [CONFIG.md](./CONFIG.md) — Configuration Reference

Complete configuration options and defaults.

**Read this when:**
- You need to override a language server setting
- You want to understand how config merging works
- You're setting up per-project configuration
- You need LSP-specific initialization options

**Key sections:**
- Configuration files: global (`~/.config/rhizome/config.toml`) vs project (`<project>/.rhizome/config.toml`)
- Configuration sections: [languages.*], [lsp], [export]
- Per-language options: server_binary, server_args, enabled, initialization_options
- LSP-wide options: disable_download, bin_dir
- Export options: auto_export
- Default server configs for all 32 languages
- Environment variables: RHIZOME_DISABLE_LSP_DOWNLOAD, RHIZOME_PROJECT, RUST_LOG
- Priority order: env > project > global > built-in defaults
- Example configurations (minimal, comprehensive, project override, CI/CD, performance-tuned)

### [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) — Common Issues and Fixes

Problems you might encounter and how to solve them.

**Read this when:**
- Something isn't working and you need to debug it
- You see an error message you don't understand
- A tool returns empty results or wrong data
- LSP server won't start or crashes
- Export to Hyphae fails

**Key sections:**
- Backend selection issues: tree-sitter vs LSP fallback, why results are empty
- LSP auto-install failures: package manager not found, install disabled
- Tool execution failures: timeouts, file not found, syntax errors
- Hyphae export failures: connection issues, incomplete exports
- Configuration issues: file not found, TOML syntax errors, merge order
- Performance: large files, slow extraction, network latency
- Error message reference table
- When to escalate and how to report bugs

## Quick Navigation

**I want to...**

- **Understand how Rhizome works**: Read [ARCHITECTURE.md](./ARCHITECTURE.md)
- **Get language X working**: Read [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md)
- **Configure LSP servers**: Read [CONFIG.md](./CONFIG.md)
- **Fix a problem**: Read [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)
- **Find error message help**: See tables in [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)

## Common Questions

### How do I know if my language is supported?

```bash
rhizome status
```

Shows all 32 languages with:
- ✓ Tree-sitter available (Yes/No)
- ✓ LSP binary name
- ✓ LSP available (Yes/No / path)

See [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md#path-1-out-of-the-box-languages-tree-sitter) for full language list.

### What's the difference between tree-sitter and LSP?

| Aspect | Tree-Sitter | LSP |
|--------|-------------|-----|
| **Speed** | Fast (<10ms for 100 lines) | Slower (100-500ms) |
| **Setup** | Zero, built-in | Auto-install or manual |
| **Precision** | High (language-specific query patterns) | Complete (full language support) |
| **Tools supported** | Symbol extraction, structure | Everything: definitions, references, rename, hover |
| **Languages** | 10 with patterns, 8 with fallback | 32 (one per language) |
| **No runtime deps** | Yes | Requires language server binary |

See [ARCHITECTURE.md: Backend Selection Logic](./ARCHITECTURE.md#backend-selection-logic) for which tools use which backend.

### Why is my language slow?

Tree-sitter performance depends on file size. Expect:
- <10ms for 100 lines
- 50-100ms for 5000 lines
- 100-500ms for large files with LSP

See [TROUBLESHOOTING.md: Performance Issues](./TROUBLESHOOTING.md#performance-issues).

### How do I report a bug?

Include:
1. `rhizome status` output
2. `rhizome --version` output
3. Relevant config files
4. Logs: `RUST_LOG=debug rhizome serve`
5. Reproduction steps

See [TROUBLESHOOTING.md: When to Escalate](./TROUBLESHOOTING.md#when-to-escalate).

## Development

Build and test Rhizome:

```bash
# Build
cargo build --release

# Test
cargo test --all

# Run CLI
cargo run -- serve        # Start MCP server
cargo run -- symbols <file>  # Extract symbols
cargo run -- status       # Show language/LSP status

# Lint and format
cargo clippy
cargo fmt
```

See [ARCHITECTURE.md: Development Commands](./ARCHITECTURE.md#development-commands).

## Further Reading

- **Rhizome source**: `crates/` directory
- **LSP spec**: https://microsoft.github.io/language-server-protocol/
- **Tree-sitter**: https://tree-sitter.github.io/tree-sitter/
- **Claude Mycelium ecosystem**: Parent project, see claude-mycelium/CLAUDE.md

## Quick Index of Topics

| Topic | Document | Section |
|-------|----------|---------|
| 5-crate workspace | ARCHITECTURE.md | Workspace Structure |
| Tool dispatch | ARCHITECTURE.md | Tool Call Flow |
| Backend selection | ARCHITECTURE.md | Backend Selection Logic |
| Tree-sitter support | ARCHITECTURE.md | Tree-Sitter Backend |
| LSP support | ARCHITECTURE.md | LSP Backend |
| Root detection | ARCHITECTURE.md | Root Detection |
| Hyphae export | ARCHITECTURE.md | Hyphae Integration |
| Tree-sitter languages | LANGUAGE-SETUP.md | Path 1 |
| LSP auto-install | LANGUAGE-SETUP.md | Path 2 |
| Custom LSP config | LANGUAGE-SETUP.md | Path 3 |
| Config files | CONFIG.md | Configuration Files |
| Config sections | CONFIG.md | Configuration Sections |
| Config defaults | CONFIG.md | Default Values |
| Backend selection issues | TROUBLESHOOTING.md | Backend Selection Issues |
| LSP auto-install issues | TROUBLESHOOTING.md | LSP Server Auto-Install Issues |
| Tool failures | TROUBLESHOOTING.md | Tool Execution Failures |
| Config issues | TROUBLESHOOTING.md | Configuration Issues |
| Performance | TROUBLESHOOTING.md | Performance Issues |
| Error messages | TROUBLESHOOTING.md | Common Error Messages |
