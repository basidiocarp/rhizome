# Rhizome Documentation

Guides for developers building with or debugging Rhizome. These docs assume familiarity with code intelligence tools, LSP, and tree-sitter. The focus is mechanism: what happens when X.

## Documents

### [architecture.md](./architecture.md) — System Design

How Rhizome is organized and how data flows. Read this when you're new to Rhizome, need to add a language or tool, or are debugging a backend selection issue. Covers the 5-crate workspace, the `CodeIntelligence` trait, tool call flow (request → backend selection → execution → response), backend selection logic, the default tree-sitter language set plus optional grammar pack, multi-client LSP design, root detection, config loading, MCP tool categories, and Hyphae integration.

### [language-setup.md](./language-setup.md) — Getting Languages Working

How to make language X work with Rhizome. Read this when a language isn't working, you're evaluating coverage, or you need to install or override an LSP server. Covers three paths: out-of-the-box tree-sitter in the default build, an optional `lang-all` grammar pack for the heaviest niche languages, LSP auto-install (32 built-in server mappings), and custom TOML configuration. Also documents how auto-install works, how to disable it, custom server examples for Java (JDTLS) and C/C++ (Clangd), and the relevant environment variables.

### [config.md](./config.md) — Configuration Reference

Complete configuration options and defaults. Read this when you need to override a language server setting, understand config merging, set up per-project configuration, or pass LSP-specific initialization options. Covers global vs project config files, the `[languages.*]`, `[lsp]`, and `[export]` sections, per-language options, environment variables, priority order (env > project > global > built-in), and example configurations for common scenarios.

### [troubleshooting.md](./troubleshooting.md) — Common Issues and Fixes

Problems and fixes. Read this when something isn't working, you see an error message you don't understand, a tool returns empty results, an LSP server won't start, or export to Hyphae fails. Covers backend selection issues, LSP auto-install failures, tool execution failures, Hyphae export failures, configuration issues, performance on large files, an error message reference table, and when and how to report bugs.

### [tooling.md](./tooling.md) — Test and Performance Workflow

How to run the repo-local test and performance surfaces. Read this when you are choosing between `cargo nextest`, `cargo test`, Criterion, or whole-command timing. Covers the command surface Rhizome expects for day-to-day development and for performance investigation.

### [lsp-guide.md](./lsp-guide.md) — Server Reference

Reference material for supported language servers, install commands, and
provider-specific notes. Read this when you need the long-form LSP catalog
instead of the shorter setup guidance.

## Quick Navigation

**I want to...**

- **Understand how Rhizome works**: Read [architecture.md](./architecture.md)
- **Get language X working**: Read [language-setup.md](./language-setup.md)
- **Configure LSP servers**: Read [config.md](./config.md)
- **Fix a problem**: Read [troubleshooting.md](./troubleshooting.md)
- **Choose the right test or profiling tool**: Read [tooling.md](./tooling.md)
- **Browse the full LSP catalog**: Read [lsp-guide.md](./lsp-guide.md)
- **Find error message help**: See tables in [troubleshooting.md](./troubleshooting.md)
- **Find the active backlog**: Read [plans/README.md](./plans/README.md)

## Common Questions

### How do I know if my language is supported?

```bash
rhizome status
```

Shows all 32 languages with tree-sitter status (`active` or `n/a`), LSP binary name, and LSP availability. See [language-setup.md](./language-setup.md#path-1-out-of-the-box-languages-tree-sitter) for the full language list.

### What's the difference between tree-sitter and LSP?

| Aspect | Tree-Sitter | LSP |
|--------|-------------|-----|
| **Speed** | Fast (<10ms for 100 lines) | Slower (100-500ms) |
| **Setup** | Zero, built-in | Auto-install or manual |
| **Precision** | High (language-specific query patterns) | Complete (full language support) |
| **Tools supported** | Symbol extraction, structure | Everything: definitions, references, rename, hover |
| **Languages** | 14 in the default build, plus 3 opt-in grammar-pack languages | 32 (one per language) |
| **No runtime deps** | Yes | Requires language server binary |

See [architecture.md: Backend Selection Logic](./architecture.md#backend-selection-logic) for which tools use which backend.

### Why is my language slow?

Tree-sitter performance depends on file size. Expect:
- <10ms for 100 lines
- 50-100ms for 5000 lines
- 100-500ms for large files with LSP

See [troubleshooting.md: Performance Issues](./troubleshooting.md#performance-issues).

### How do I report a bug?

Include `rhizome status` output, `rhizome --version` output, relevant config files, logs from `RHIZOME_LOG=debug rhizome serve`, and reproduction steps. See [troubleshooting.md: When to Escalate](./troubleshooting.md#when-to-escalate).

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

See [architecture.md: Development Commands](./architecture.md#development-commands).

## Further Reading

- **Rhizome source**: `crates/` directory
- **LSP spec**: https://microsoft.github.io/language-server-protocol/
- **Tree-sitter**: https://tree-sitter.github.io/tree-sitter/
- **Basidiocarp workspace**: Parent project, see `../CLAUDE.md`

## Quick Index of Topics

| Topic | Document | Section |
|-------|----------|---------|
| 5-crate workspace | architecture.md | Workspace Structure |
| Tool dispatch | architecture.md | Tool Call Flow |
| Backend selection | architecture.md | Backend Selection Logic |
| Tree-sitter support | architecture.md | Tree-Sitter Backend |
| LSP support | architecture.md | LSP Backend |
| Root detection | architecture.md | Root Detection |
| Hyphae export | architecture.md | Hyphae Integration |
| Tree-sitter languages | language-setup.md | Path 1 |
| LSP auto-install | language-setup.md | Path 2 |
| Custom LSP config | language-setup.md | Path 3 |
| Config files | config.md | Configuration Files |
| Config sections | config.md | Configuration Sections |
| Config defaults | config.md | Default Values |
| Backend selection issues | troubleshooting.md | Backend Selection Issues |
| LSP auto-install issues | troubleshooting.md | LSP Server Auto-Install Issues |
| Tool failures | troubleshooting.md | Tool Execution Failures |
| Config issues | troubleshooting.md | Configuration Issues |
| Performance | troubleshooting.md | Performance Issues |
| Error messages | troubleshooting.md | Common Error Messages |
