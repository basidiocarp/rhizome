# Rhizome Documentation

Guides for developers building with or debugging Rhizome. These docs assume familiarity with code intelligence tools, LSP, and tree-sitter. The focus is mechanism: what happens when X.

## Documents

### [ARCHITECTURE.md](./ARCHITECTURE.md) — System Design

How Rhizome is organized and how data flows. Read this when you're new to Rhizome, need to add a language or tool, or are debugging a backend selection issue. Covers the 5-crate workspace, the `CodeIntelligence` trait, tool call flow (request → backend selection → execution → response), backend selection logic, tree-sitter built-in support for 17 languages, multi-client LSP design, root detection, config loading, MCP tool categories, and Hyphae integration.

### [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md) — Getting Languages Working

How to make language X work with Rhizome. Read this when a language isn't working, you're evaluating coverage, or you need to install or override an LSP server. Covers three paths: out-of-the-box tree-sitter (17 languages, zero setup), LSP auto-install (32 built-in server mappings), and custom TOML configuration. Also documents how auto-install works, how to disable it, custom server examples for Java (JDTLS) and C/C++ (Clangd), and the relevant environment variables.

### [CONFIG.md](./CONFIG.md) — Configuration Reference

Complete configuration options and defaults. Read this when you need to override a language server setting, understand config merging, set up per-project configuration, or pass LSP-specific initialization options. Covers global vs project config files, the `[languages.*]`, `[lsp]`, and `[export]` sections, per-language options, environment variables, priority order (env > project > global > built-in), and example configurations for common scenarios.

### [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) — Common Issues and Fixes

Problems and fixes. Read this when something isn't working, you see an error message you don't understand, a tool returns empty results, an LSP server won't start, or export to Hyphae fails. Covers backend selection issues, LSP auto-install failures, tool execution failures, Hyphae export failures, configuration issues, performance on large files, an error message reference table, and when and how to report bugs.

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

Shows all 32 languages with tree-sitter status (`active` or `n/a`), LSP binary name, and LSP availability. See [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md#path-1-out-of-the-box-languages-tree-sitter) for the full language list.

### What's the difference between tree-sitter and LSP?

| Aspect | Tree-Sitter | LSP |
|--------|-------------|-----|
| **Speed** | Fast (<10ms for 100 lines) | Slower (100-500ms) |
| **Setup** | Zero, built-in | Auto-install or manual |
| **Precision** | High (language-specific query patterns) | Complete (full language support) |
| **Tools supported** | Symbol extraction, structure | Everything: definitions, references, rename, hover |
| **Languages** | 17 with built-in tree-sitter support | 32 (one per language) |
| **No runtime deps** | Yes | Requires language server binary |

See [ARCHITECTURE.md: Backend Selection Logic](./ARCHITECTURE.md#backend-selection-logic) for which tools use which backend.

### Why is my language slow?

Tree-sitter performance depends on file size. Expect:
- <10ms for 100 lines
- 50-100ms for 5000 lines
- 100-500ms for large files with LSP

See [TROUBLESHOOTING.md: Performance Issues](./TROUBLESHOOTING.md#performance-issues).

### How do I report a bug?

Include `rhizome status` output, `rhizome --version` output, relevant config files, logs from `RHIZOME_LOG=debug rhizome serve`, and reproduction steps. See [TROUBLESHOOTING.md: When to Escalate](./TROUBLESHOOTING.md#when-to-escalate).

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
