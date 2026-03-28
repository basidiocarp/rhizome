# Rhizome

Editor-agnostic code intelligence MCP server. Gives AI agents symbol-level code navigation — definitions, references, structure — instead of reading raw files, eliminating the biggest source of wasted tokens.

Part of the [Basidiocarp ecosystem](https://github.com/basidiocarp) — see the [Technical Overview](https://github.com/basidiocarp/.github/blob/main/profile/README.md#technical-overview) for how Rhizome fits with Hyphae, Mycelium, Cap, and Lamella.

## Why Rhizome?

Some AI coding tools (like Claude Code) have built-in LSP access. Most don't. Cursor, Windsurf, Cline, Continue, Aider, custom agents built with the Agent SDK, and any other MCP-compatible client get **zero** code intelligence out of the box — they can read files, but they can't navigate symbols, find references, or understand code structure.

Rhizome fills that gap as a standalone MCP server that works with **any** MCP client. It also provides analysis tools that go beyond what LSP offers — file summaries, cyclomatic complexity, test discovery, annotation scanning, and git-aware symbol diffs — useful even for tools that already have LSP access.

Built in Rust with a dual-backend architecture:

- **Tree-sitter** (always available) — instant offline parsing, sub-millisecond, zero setup. 18 languages with grammars compiled in.
- **LSP** (auto-selected when needed) — cross-file go-to-definition, find-all-references, rename, type info. Auto-installs servers for 20+ languages.

## Quick Start

```sh
# Build
cargo build --release

# Generate MCP config guidance for detected hosts
cargo run --release -- init

# Print a paste-ready MCP snippet for one host
cargo run --release -- init --editor claude-code
cargo run --release -- init --editor codex

# Check backend status and available LSP servers
cargo run --release -- status

# Manage LSP servers
cargo run --release -- lsp status
cargo run --release -- lsp install python

# Use directly from the CLI
cargo run --release -- symbols src/main.rs
cargo run --release -- structure src/lib.rs
```

### Add to Any MCP Client

Works with Claude Code, Cursor, Windsurf, Cline, Continue, OpenCode, and any MCP-compatible tool.

```sh
rhizome init
# Shows detected hosts and the right MCP snippet shape for each one

rhizome init --editor claude-code
# Prints only the Claude Code JSON snippet

rhizome init --editor codex
# Prints only the Codex TOML snippet
```

For JSON MCP clients:

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

For Codex CLI TOML config:

```toml
[mcp_servers.rhizome]
command = "rhizome"
args = ["serve"]
```

Rhizome uses platform-specific config and data directories. Use `rhizome status` to see the resolved managed bin directory on the current machine.

## Supported Languages

Rhizome supports **32 languages** with a three-tier parsing strategy. See the [Technical Overview: Tree-sitter Code Parsing](https://github.com/basidiocarp/.github/blob/main/profile/README.md#tree-sitter-code-parsing--rhizome) for details.

| Tier | Languages | How it works |
|------|-----------|-------------|
| **Full query patterns** (10) | Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Ruby, PHP | Language-specific tree-sitter S-expression queries for precise symbol extraction |
| **Generic fallback** (8) | Bash, C#, Elixir, Lua, Swift, Zig, Haskell, TOML | AST walker matching common node types (`function_definition`, `class_declaration`, etc.) |
| **LSP only** (14+) | Kotlin, Dart, Clojure, OCaml, Julia, Nix, Gleam, Vue, Svelte, Astro, Prisma, Typst, YAML, F# | Requires installed language server — auto-installed when available |

All 32 languages have LSP server configs. 20+ have auto-install recipes (npm, pip, cargo, gem, go, brew, dotnet, opam, ghcup, mix).

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Rhizome MCP Server (JSON-RPC 2.0 over stdio)               │
│  └─ Tool Registry (35 tools)                                │
├─────────────────────────────────────────────────────────────┤
│ Backend Auto-Selection                                      │
│  ├─ Tree-sitter (default, 18 languages with grammars)       │
│  ├─ Generic fallback (languages without query patterns)     │
│  ├─ LSP (auto-upgrade for refs, rename, hover, diagnostics) │
│  └─ Per-tool requirement mapping                            │
├─────────────────────────────────────────────────────────────┤
│ Smart Infrastructure                                        │
│  ├─ Auto-install LSP servers (platform data dir/bin)        │
│  ├─ Per-language workspace root detection                   │
│  ├─ Multi-client LSP (per language × root pair)             │
│  ├─ Path traversal prevention in edit tools                 │
│  └─ 32 languages, 20+ install recipes                      │
├─────────────────────────────────────────────────────────────┤
│ Hyphae Integration                                          │
│  ├─ Code graph export (symbols → concept nodes + edges)     │
│  └─ Incremental caching (mtime-based)                       │
└─────────────────────────────────────────────────────────────┘
```

## MCP Tools

### Symbol Navigation (tree-sitter)

| Tool | Description |
|------|-------------|
| `get_symbols` | List all symbols in a file |
| `get_structure` | Hierarchical symbol tree with nesting depth control |
| `get_definition` | Full symbol definition including body |
| `get_symbol_body` | Source code body of a specific symbol |
| `search_symbols` | Find symbols by name pattern across the project |
| `find_references` | Find all references to a symbol |
| `analyze_impact` | Summarize likely blast radius, callers, and callees for a symbol |
| `go_to_definition` | Jump from usage to definition |
| `get_signature` | Signature only (no body) |
| `get_imports` | All import/use statements |
| `get_call_sites` | All function call expressions |

### Code Intelligence (tree-sitter)

| Tool | Description |
|------|-------------|
| `get_scope` | Enclosing scope at a given line |
| `get_exports` | Public/exported symbols only |
| `summarize_file` | Compact summary — signatures only |
| `get_tests` | Test functions in a file |
| `get_type_definitions` | Structs, enums, interfaces, type aliases |
| `get_parameters` | Function parameters with types |
| `get_dependencies` | Intra-file call graph |
| `get_enclosing_class` | Parent class + sibling methods |
| `get_complexity` | Cyclomatic complexity per function |
| `get_annotations` | TODO, FIXME, HACK comments |

### File Editing (7 tools)

| Tool | Description |
|------|-------------|
| `replace_symbol_body` | Replace a symbol's implementation |
| `insert_after_symbol` | Insert code after a symbol |
| `insert_before_symbol` | Insert code before a symbol |
| `replace_lines` | Replace a line range |
| `insert_at_line` | Insert at a specific line |
| `delete_lines` | Delete a line range |
| `create_file` | Create a new file |

All edit tools validate paths stay within the project root (path traversal prevention).

### Git Integration

| Tool | Description |
|------|-------------|
| `get_diff_symbols` | Symbols modified in uncommitted changes |
| `get_changed_files` | Changed files with modified symbol counts |

### Deep Intelligence (LSP — auto-selected)

| Tool | Description |
|------|-------------|
| `rename_symbol` | Project-wide refactor rename with optional dry-run preview |
| `get_diagnostics` | Compiler errors and warnings |
| `get_hover_info` | Type information and docs |

### Hyphae Integration

| Tool | Description |
|------|-------------|
| `export_to_hyphae` | Export code graph as knowledge memoir with cache-aware export summaries |

## Backend Auto-Selection

See [Technical Overview: LSP Auto-Management](https://github.com/basidiocarp/.github/blob/main/profile/README.md#lsp-auto-management--rhizome) for details.

| Backend | Tools | Behavior |
|---------|-------|----------|
| **Tree-sitter** | `get_symbols`, `get_structure`, 18 others | Always used — instant, no dependencies |
| **LSP preferred** | `find_references`, `get_diagnostics` | LSP if available, tree-sitter fallback |
| **LSP required** | `rename_symbol`, `get_hover_info` | Requires LSP — auto-installs or returns instructions |

## CLI Commands

```
rhizome serve [--project <path>] [--expanded]    Start MCP server
rhizome symbols <file>                           List symbols
rhizome structure <file>                         Show structure tree
rhizome status [--project <path>]                Backend status per language
rhizome lsp status [--json]                      LSP server availability
rhizome lsp install <language>                   Install LSP server
rhizome export [--project <path>]                Export code graph to Hyphae
rhizome init [--config] [--editor <host>]       Print MCP or example config
rhizome doctor [--fix]                           Diagnose issues
rhizome self-update [--check]                    Check/install updates
rhizome summarize [--project <path>] [--json]    Project summary
```

## Configuration

```toml
# <platform config dir>/rhizome/config.toml

[languages.python]
server_binary = "ruff"
server_args = ["server"]

[languages.java]
enabled = false

[lsp]
disable_download = false

[export]
auto_export = true
```

## Project Structure

```
rhizome/
├── crates/
│   ├── rhizome-core/        # Domain types, backend selection, installer, config
│   ├── rhizome-treesitter/  # Tree-sitter backend (18 language grammars)
│   ├── rhizome-lsp/         # LSP client backend (multi-client, per-root)
│   ├── rhizome-mcp/         # MCP server + 35 tool handlers
│   └── rhizome-cli/         # CLI entry point
└── tests/
```

## License

See [LICENSE](LICENSE) for details.
