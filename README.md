# Rhizome

Editor-agnostic code intelligence MCP server. Gives AI agents symbol-level code navigation — definitions, references, structure — instead of reading raw files, eliminating the biggest source of wasted tokens.

Part of the [Mycelium](https://github.com/williamnewton/claude-mycelium) ecosystem.

## Why Rhizome?

Some AI coding tools (like Claude Code) have built-in LSP access. Most don't. Cursor, Windsurf, Cline, Continue, Aider, custom agents built with the Agent SDK, and any other MCP-compatible client get **zero** code intelligence out of the box — they can read files, but they can't navigate symbols, find references, or understand code structure.

Rhizome fills that gap as a standalone MCP server that works with **any** MCP client. It also provides analysis tools that go beyond what LSP offers — file summaries, cyclomatic complexity, test discovery, annotation scanning, and git-aware symbol diffs — useful even for tools that already have LSP access.

Built in Rust with a dual-backend architecture:

- **Tree-sitter** (always available) — instant offline parsing, sub-millisecond, zero setup. Works immediately for any MCP client without installing language servers.
- **LSP** (auto-selected when needed) — cross-file go-to-definition, find-all-references, rename, type info. Automatically activates when a language server is detected or auto-installed.

## Quick Start

```bash
# Build
cargo build --release

# Generate MCP config for your editor
./target/release/rhizome init

# Check backend status and available LSP servers
./target/release/rhizome status

# Use directly from the CLI
./target/release/rhizome symbols src/main.rs
./target/release/rhizome structure src/lib.rs
```

### Add to Any MCP Client

Works with Claude Code, Cursor, Windsurf, Cline, Continue, OpenCode, and any MCP-compatible tool.

> **Note:** Claude Code already has built-in LSP access for core operations (go-to-definition, find-references, rename). Rhizome still adds value there through its analysis tools (`summarize_file`, `get_complexity`, `get_tests`, `get_annotations`, `get_diff_symbols`) that LSP doesn't provide. For all other MCP clients, Rhizome provides the full code intelligence stack.

```bash
rhizome init
# Paste the output into your MCP settings
```

Or manually add to your MCP configuration:

```json
{
  "mcpServers": {
    "rhizome": {
      "command": "rhizome",
      "args": ["serve"],
      "env": {}
    }
  }
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Rhizome MCP Server (JSON-RPC 2.0 over stdio)               │
│  └─ Tool Registry (26 tools)                                │
├─────────────────────────────────────────────────────────────┤
│ Backend Auto-Selection                                      │
│  ├─ Tree-sitter (default for most tools)                    │
│  ├─ LSP (auto-upgrade for refs, rename, hover, diagnostics) │
│  └─ Per-tool requirement mapping                            │
├─────────────────────────────────────────────────────────────┤
│ Smart Infrastructure                                        │
│  ├─ Auto-install LSP servers (~/.rhizome/bin/)              │
│  ├─ Per-language workspace root detection                   │
│  ├─ Multi-client LSP (per language × root pair)             │
│  └─ 32 languages, 20+ install recipes                      │
├─────────────────────────────────────────────────────────────┤
│ Hyphae Integration                                          │
│  ├─ Code graph export (symbols → concept nodes + edges)     │
│  └─ Incremental caching (mtime-based)                       │
└─────────────────────────────────────────────────────────────┘
```

## MCP Tools

Rhizome exposes 26 tools over the [Model Context Protocol](https://modelcontextprotocol.io).

### Unified vs Expanded Mode

By default, Rhizome runs in **unified mode** — exposing a single `rhizome` tool with a `command` parameter. This keeps the agent's system prompt small (~200 tokens vs ~3,000+).

```bash
# Unified mode (default) — 1 tool, minimal token overhead
rhizome serve

# Expanded mode — 26 separate tools, full discoverability
rhizome serve --expanded
```

In unified mode, the agent calls:
```json
{ "name": "rhizome", "arguments": { "command": "get_symbols", "file": "src/main.rs" } }
```

In expanded mode, each tool is a separate MCP tool:
```json
{ "name": "get_symbols", "arguments": { "file": "src/main.rs" } }
```

### Symbol Navigation (tree-sitter)

| Tool | Description |
|------|-------------|
| `get_symbols` | List all symbols (functions, structs, classes, etc.) in a file |
| `get_structure` | Hierarchical symbol tree with nesting depth control |
| `get_definition` | Full symbol definition including body (with large-body preview) |
| `get_symbol_body` | Get the source code body of a specific symbol by name and optional line |
| `search_symbols` | Find symbols by name pattern across the project |
| `find_references` | Find all references to a symbol at a given position |
| `go_to_definition` | Jump from a usage to its definition |
| `get_signature` | Get only the signature of a symbol (no body) |
| `get_imports` | List all import/use statements in a file |
| `get_call_sites` | Find all function call expressions in a file |

### Code Intelligence (tree-sitter)

| Tool | Description |
|------|-------------|
| `get_scope` | Get the enclosing scope (function, class, module) at a given line |
| `get_exports` | List only public/exported symbols in a file |
| `summarize_file` | Compact file summary — public signatures only, no bodies |
| `get_tests` | Find test functions in a file |
| `get_type_definitions` | List type definitions (structs, enums, interfaces, type aliases) |
| `get_parameters` | Extract function parameters with types |
| `get_dependencies` | Map which functions call which within a file |
| `get_enclosing_class` | Get parent class/struct and all sibling methods for a method |
| `get_complexity` | Calculate cyclomatic complexity for functions in a file |
| `get_annotations` | Find TODO, FIXME, HACK, and other annotation comments |

### Git Integration (tree-sitter + git)

| Tool | Description |
|------|-------------|
| `get_diff_symbols` | Show which symbols were modified in uncommitted changes or between commits |
| `get_changed_files` | List files with uncommitted changes and their modified symbol counts |

### Deep Intelligence (LSP — auto-selected)

| Tool | Description |
|------|-------------|
| `rename_symbol` | Refactor rename across the entire project |
| `get_diagnostics` | Compiler errors and warnings for a file |
| `get_hover_info` | Type information and documentation at a position |

LSP tools are **auto-selected** when needed. If the required language server is not installed, Rhizome will attempt to auto-install it. If auto-install fails, a clear error with manual install instructions is returned.

### Hyphae Integration

| Tool | Description |
|------|-------------|
| `export_to_hyphae` | Export code graph to Hyphae for semantic knowledge storage |

## Supported Languages

Rhizome supports **32 languages**. Tree-sitter parsing is available for the original 9; all 32 have LSP server configs with auto-install where possible.

| Language | Tree-sitter | Default LSP Server | Auto-install |
|----------|:-----------:|-------------------|:------------:|
| Rust | ✅ | `rust-analyzer` | ✅ rustup |
| Python | ✅ | `pyright-langserver` | ✅ pipx/pip |
| JavaScript | ✅ | `typescript-language-server` | ✅ npm |
| TypeScript | ✅ | `typescript-language-server` | ✅ npm |
| Go | ✅ | `gopls` | ✅ go install |
| Java | — | `jdtls` | — |
| C/C++ | — | `clangd` | — |
| Ruby | — | `ruby-lsp` | ✅ gem |
| Elixir | — | `elixir-ls` | — |
| Zig | — | `zls` | — |
| C# | — | `csharp-ls` | ✅ dotnet tool |
| F# | — | `fsautocomplete` | ✅ dotnet tool |
| Swift | — | `sourcekit-lsp` | — (Xcode) |
| PHP | — | `phpactor` | — |
| Haskell | — | `haskell-language-server-wrapper` | ✅ ghcup |
| Bash | — | `bash-language-server` | ✅ npm |
| Terraform | — | `terraform-ls` | ✅ brew |
| Kotlin | — | `kotlin-language-server` | — |
| Dart | — | `dart` (built-in) | — (SDK) |
| Lua | — | `lua-language-server` | ✅ brew |
| Clojure | — | `clojure-lsp` | ✅ brew |
| OCaml | — | `ocamllsp` | ✅ opam |
| Julia | — | `julia` (LanguageServer.jl) | — |
| Nix | — | `nixd` | ✅ nix-env |
| Gleam | — | `gleam lsp` (built-in) | — (SDK) |
| Vue | — | `vue-language-server` | ✅ npm |
| Svelte | — | `svelteserver` | ✅ npm |
| Astro | — | `astro-ls` | ✅ npm |
| Prisma | — | `prisma-language-server` | ✅ npm |
| Typst | — | `tinymist` | ✅ cargo |
| YAML | — | `yaml-language-server` | ✅ npm |

### Alternative Servers

Any server can be overridden via config. Auto-install recipes exist for these alternatives:

| Binary | Replaces | Install via |
|--------|----------|-------------|
| `pylsp` | pyright | pipx |
| `ruff` | pyright | pipx |
| `jedi-language-server` | pyright | pipx |
| `solargraph` | ruby-lsp | gem |
| `biome` | typescript-language-server | npm |
| `intelephense` | phpactor | npm |
| `omnisharp` | csharp-ls | dotnet tool |

## Backend Auto-Selection

Rhizome automatically picks the best backend for each tool call:

| Backend | Tools | Behavior |
|---------|-------|----------|
| **Tree-sitter** | `get_symbols`, `get_structure`, and 18 others | Always used — instant, no dependencies |
| **LSP preferred** | `find_references`, `get_diagnostics` | Uses LSP if available, falls back to tree-sitter |
| **LSP required** | `rename_symbol`, `get_hover_info` | Requires LSP — auto-installs or returns install instructions |

### Auto-Install

When an LSP-requiring tool is called and the server binary isn't found, Rhizome:
1. Checks `~/.rhizome/bin/` and system PATH
2. Attempts to install via the language's package manager
3. Falls back to a clear error with manual install instructions

Disable with `RHIZOME_DISABLE_LSP_DOWNLOAD=1` or in config:
```toml
[lsp]
disable_download = true
```

### Smart Root Detection

LSP servers need a workspace root. Rhizome detects it per-language:

- **Rust**: walks up looking for `Cargo.toml` with `[workspace]`
- **Go**: prefers `go.work` over `go.mod`
- **JS/TS**: finds `tsconfig.json`/`package.json`, skips Deno dirs
- **Python**: `pyproject.toml`, `setup.py`, `requirements.txt`
- Falls back to `.git` directory

This enables monorepo support — different languages get different workspace roots.

## CLI Commands

```
rhizome serve [--project <path>] [--expanded]    Start MCP server on stdio
rhizome symbols <file>                           List symbols in a file
rhizome structure <file>                         Show file structure as a tree
rhizome status [--project <path>]                Show backend status per language
rhizome export [--project <path>]                Export code graph to Hyphae
rhizome init [--config]                          Print MCP config or example config.toml
rhizome --version                                Print version
```

| Flag | Description |
|------|-------------|
| `--project <path>` | Set workspace root (default: auto-detect from `.git`) |
| `--expanded` | Expose 26 separate MCP tools instead of unified `rhizome` command |
| `--config` | Print example `config.toml` instead of MCP JSON |

### Example Output

```
$ rhizome status
Rhizome Backend Status
======================

Language       Tree-Sitter    LSP Server                     Status
--------       -----------    ----------                     ------
Rust           active         rust-analyzer                  available (/home/user/.cargo/bin/rust-analyzer)
Python         active         pyright-langserver             available (/home/user/.local/bin/pyright-langserver)
Go             active         gopls                          available (/home/user/.rhizome/bin/gopls)
TypeScript     active         typescript-language-server     not found
...

Auto-install: enabled (set RHIZOME_DISABLE_LSP_DOWNLOAD=1 to disable)
Managed bin dir: /home/user/.rhizome/bin
```

```
$ rhizome symbols src/main.rs
struct Config [2:0-5:1]
  pub struct Config
fn new [9:4-11:5]
  pub fn new(name: String, value: i32) -> Self
fn process [19:0-21:1]
  pub fn process(config: &Config) -> String
const MAX_SIZE [23:0-23:29]
  const MAX_SIZE: usize = 1024;
```

## Configuration

Rhizome loads configuration from TOML files, with project config overriding global:

- **Global**: `~/.config/rhizome/config.toml`
- **Project**: `<project_root>/.rhizome/config.toml`

```toml
# Override the default language server
[languages.python]
server_binary = "ruff"
server_args = ["server"]

# Disable a language entirely
[languages.java]
enabled = false

# Custom server binary path
[languages.rust]
server_binary = "/path/to/custom/rust-analyzer"
server_args = ["--log-file", "/tmp/ra.log"]

# LSP auto-install settings
[lsp]
disable_download = false          # Set to true to disable auto-install
# bin_dir = "/custom/path/bin"   # Override managed bin directory

# Hyphae export settings
[export]
auto_export = true                # Export code graph on MCP server startup
```

## Project Structure

```
rhizome/
├── crates/
│   ├── rhizome-core/        # Domain types, backend selection, installer, root detection, config
│   ├── rhizome-treesitter/  # Tree-sitter backend (offline parsing)
│   ├── rhizome-lsp/         # LSP client backend (multi-client, per-root)
│   ├── rhizome-mcp/         # MCP server + 26 tool handlers
│   └── rhizome-cli/         # CLI entry point (binary)
└── tests/
```

## Development

```bash
# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Release build (single ~9MB binary)
cargo build --release
```

## License

See [LICENSE](LICENSE) for details.
