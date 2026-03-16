# Rhizome

Code intelligence MCP server for the [Mycelium](https://github.com/williamnewton/claude-mycelium) ecosystem. Gives AI agents symbol-level code navigation — definitions, references, structure — instead of reading raw files, eliminating the biggest source of wasted tokens.

Built in Rust with a dual-backend architecture:

- **Tree-sitter** (always available) — instant offline parsing, sub-millisecond, no setup required
- **LSP** (when a language server is installed) — cross-file go-to-definition, find-all-references, rename, type info

## Quick Start

```bash
# Build
cargo build --release

# Generate MCP config for your editor
./target/release/rhizome init

# Use directly from the CLI
./target/release/rhizome symbols src/main.rs
./target/release/rhizome structure src/lib.rs
```

### Add to Claude Code

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
┌─────────────────────────────────────────────────────────┐
│ Rhizome MCP Server (JSON-RPC 2.0 over stdio)           │
│  └─ Tool Registry (25 tools)                            │
├─────────────────────────────────────────────────────────┤
│ Symbol Layer (unified CodeIntelligence trait)            │
│  ├─ TreeSitterBackend (offline, instant, any language)   │
│  └─ LspBackend (deep, cross-file, when server available) │
├─────────────────────────────────────────────────────────┤
│ Language Detection & Server Management                   │
│  ├─ File extension → language mapping                    │
│  ├─ Language → server binary mapping                     │
│  └─ Server lifecycle (spawn, init, shutdown, restart)    │
└─────────────────────────────────────────────────────────┘
```

## MCP Tools

Rhizome exposes 25 tools over the [Model Context Protocol](https://modelcontextprotocol.io).

### Unified vs Expanded Mode

By default, Rhizome runs in **unified mode** — exposing a single `rhizome` tool with a `command` parameter. This keeps the agent's system prompt small (~200 tokens vs ~3,000+).

```bash
# Unified mode (default) — 1 tool, minimal token overhead
rhizome serve

# Expanded mode — 25 separate tools, full discoverability
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

### Deep Intelligence (LSP required)

| Tool | Description |
|------|-------------|
| `rename_symbol` | Refactor rename across the entire project |
| `get_diagnostics` | Compiler errors and warnings for a file |
| `get_hover_info` | Type information and documentation at a position |

LSP tools gracefully report when a language server is not available, suggesting the appropriate server to install.

## Supported Languages

Tree-sitter parsing works out of the box for:

| Language | Tree-sitter | Default LSP Server |
|----------|-------------|-------------------|
| Rust | ✅ | `rust-analyzer` |
| Python | ✅ | `pyright-langserver` |
| JavaScript | ✅ | `typescript-language-server` |
| TypeScript | ✅ | `typescript-language-server` |
| Go | ✅ | `gopls` |
| Java | — | `jdtls` |
| C/C++ | — | `clangd` |
| Ruby | — | `solargraph` |

## CLI Commands

```
rhizome serve [--project <path>] [--expanded]    Start MCP server on stdio
rhizome symbols <file>                           List symbols in a file
rhizome structure <file>                         Show file structure as a tree
rhizome init                                     Print MCP config JSON for editors
rhizome --version                                Print version
```

| Flag | Description |
|------|-------------|
| `--project <path>` | Set workspace root (default: auto-detect from `.git`) |
| `--expanded` | Expose 25 separate MCP tools instead of unified `rhizome` command |

### Example Output

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

$ rhizome structure src/main.rs
├── struct Config [2:0-5:1]
├── struct Config [7:0-16:1]
│   ├── method new [9:4-11:5]
│   └── method value [13:4-15:5]
├── fn process [19:0-21:1]
├── const MAX_SIZE [23:0-23:29]
├── enum Status [28:0-31:1]
└── trait Processor [33:0-35:1]
```

## Configuration

Rhizome loads configuration from TOML files, with project config overriding global:

- **Global**: `~/.config/rhizome/config.toml`
- **Project**: `<project_root>/.rhizome/config.toml`

```toml
# Override the default language server
[languages.python]
server_binary = "pylsp"
server_args = []

# Disable a language entirely
[languages.java]
enabled = false

# Custom server binary path
[languages.rust]
server_binary = "/path/to/custom/rust-analyzer"
server_args = ["--log-file", "/tmp/ra.log"]
```

## Project Structure

```
rhizome/
├── crates/
│   ├── rhizome-core/        # Symbol types, traits, language detection, config
│   ├── rhizome-treesitter/  # Tree-sitter backend (offline parsing)
│   ├── rhizome-lsp/         # LSP client backend (cross-file intelligence)
│   ├── rhizome-mcp/         # MCP server + 25 tool handlers
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