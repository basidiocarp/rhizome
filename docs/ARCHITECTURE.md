# Rhizome Architecture

Rhizome is a code intelligence MCP server with a 5-crate workspace design. Two backends—tree-sitter and LSP—are selected per tool call. This document describes the architecture and data flow.

## Workspace Structure

```
rhizome-cli           Clap-based CLI entry point
  │
  └─ rhizome-mcp      MCP JSON-RPC server + tool dispatcher
       │
       ├─ rhizome-treesitter  Tree-sitter backend (10 languages with patterns)
       │    │
       │    └─ rhizome-core   Domain types, traits, backends
       │
       └─ rhizome-lsp        LSP backend (32 languages supported)
            │
            └─ rhizome-core  Domain types, traits, backends
```

All five crates compile into a single binary. `rhizome-core` is the foundation; the other crates implement the `CodeIntelligence` trait.

## Core Trait: CodeIntelligence

All backends implement this interface:

```rust
pub trait CodeIntelligence {
    fn get_symbols(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn get_structure(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn get_definition(&self, file: &Path, name: &str) -> Result<Option<Symbol>>;
    fn find_references(&self, file: &Path, position: Position) -> Result<Vec<Location>>;
    fn search_symbols(&self, pattern: &str, project_root: &Path) -> Result<Vec<Symbol>>;
    fn get_imports(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn get_diagnostics(&self, file: &Path) -> Result<Vec<Diagnostic>>;
}
```

Both tree-sitter and LSP backends implement this interface fully. The tool dispatcher selects which backend to use for each call.

## Tool Call Flow: Request to Response

When an MCP tool call arrives:

1. **Routing** (ToolDispatcher.call_tool)
   - Maps tool name to handler function
   - Example: `get_symbols` → `symbol_tools::get_symbols()`

2. **Backend Selection** (BackendSelector.select)
   - Checks tool requirement (RequiresLsp, PrefersLsp, TreeSitter)
   - Example: `rename_symbol` requires LSP
   - Example: `find_references` prefers LSP, falls back to tree-sitter
   - Example: `get_symbols` always uses tree-sitter

3. **Lazy LSP Initialization** (ToolDispatcher.ensure_lsp)
   - First LSP tool call initializes the backend
   - Subsequent calls reuse the cached LSP client
   - LSP servers are auto-installed if missing

4. **Execution** (Backend method)
   - Tree-sitter: parse file, run query patterns, extract symbols
   - LSP: send request to running language server, parse response

5. **Response** (JSON serialization)
   - Symbols, definitions, references, etc. → JSON
   - Sent back to MCP client

## Backend Selection Logic

File: `crates/rhizome-core/src/backend_selector.rs`

### Tool Requirements

| Requirement | Examples | Fallback Behavior |
|------------|----------|-------------------|
| `TreeSitter` | `get_symbols`, `get_structure`, `get_exports`, `get_complexity` | Always tree-sitter (fast, no deps) |
| `PrefersLsp` | `find_references`, `get_diagnostics` | LSP if available, tree-sitter if not |
| `RequiresLsp` | `rename_symbol`, `get_hover_info` | Error with install hint if unavailable |

### Selection Process

```rust
pub fn select(&mut self, tool_name: &str, language: &Language) -> ResolvedBackend {
    let requirement = tool_requirement(tool_name);  // Look up in match statement

    match requirement {
        TreeSitter => ResolvedBackend::TreeSitter,
        RequiresLsp => {
            let probe = self.probe_language(language);  // Check if binary exists
            if probe.available {
                ResolvedBackend::Lsp
            } else {
                ResolvedBackend::LspUnavailable {
                    binary: probe.binary,
                    install_hint: format!("...: install via {cmd}")
                }
            }
        }
        PrefersLsp => {
            let probe = self.probe_language(language);
            if probe.available {
                ResolvedBackend::Lsp
            } else {
                ResolvedBackend::TreeSitter  // Fallback
            }
        }
    }
}
```

**Probing** checks:
1. Is the server binary in PATH?
2. If not, is auto-download enabled? (`RHIZOME_DISABLE_LSP_DOWNLOAD=1` disables)
3. If enabled, try to install via recipe (e.g., `rustup component add rust-analyzer`)
4. Return availability status

## Tree-Sitter Backend

File: `crates/rhizome-treesitter/src/lib.rs`

### How It Works

1. **Parser Pool**: Reuses `tree_sitter::Parser` instances per language (cached for performance)
2. **Parse File**: `parser.parse(source_bytes, None)` → tree-sitter syntax tree
3. **Run Query**: Execute language-specific query pattern (defined in `queries.rs`)
4. **Extract Symbols**: Walk matched nodes, extract name, kind, line, scope
5. **Return**: Vec of symbols with kind (function, class, import, etc.)

### Language Support: Query Patterns vs Generic Fallback

**10 languages with optimized query patterns** (fast, precise):
- Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Ruby, PHP

Example Rust query:
```
(function_item name: (identifier) @name) @function
(struct_item name: (type_identifier) @name) @struct_def
(trait_item name: (type_identifier) @name) @trait_def
(import_statement) @import
```

**7 additional languages with built-in tree-sitter extraction**:
- Bash, C#, Elixir, Lua, Swift, Zig, Haskell

Generic fallback walks the tree and matches common node types: `function_definition`, `class_declaration`, `method`, `import`, etc.

**15 languages LSP-only** (no tree-sitter parser):
- Terraform, F#, Kotlin, Dart, Clojure, OCaml, Julia, Nix, Gleam, Vue, Svelte, Astro, Prisma, Typst, YAML

For LSP-only languages, tools requiring tree-sitter (e.g., `get_symbols`) either:
- Use the generic fallback if a parser can be loaded
- Return an empty list with a note that LSP is required

### Adding Query Patterns

File: `crates/rhizome-treesitter/src/queries.rs`

To add or improve a language's query:

1. Write tree-sitter query matching the language grammar
2. Add constant (e.g., `SWIFT_QUERY`) with pattern string
3. Add compile step in `get_query()`
4. Add OnceLock cache

Example for Swift (hypothetical):
```rust
pub const SWIFT_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @function
(class_declaration name: (type_identifier) @name) @class_def
(struct_declaration name: (type_identifier) @name) @struct_def
(import_statement) @import
"#;
```

## LSP Backend

File: `crates/rhizome-lsp/src/lib.rs`

### How It Works

1. **Language Server Manager**: Keyed by `(Language, PathBuf)` for monorepo support
   - Multiple servers can run simultaneously (one per project root)
   - Each server handles its own `initialize`, lifecycle, and cache

2. **Start Server**: Spawn LSP server process via stdio
   - Example Rust: `rust-analyzer --log-file=/tmp/ra.log`
   - Communicate via JSON-RPC over stdin/stdout

3. **Send Request**: Format LSP request (e.g., `textDocument/definition`)
   - Include document URI, line, column
   - Wait for response (with timeout)

4. **Parse Response**: Extract symbols/locations/diagnostics from LSP response
   - Convert to Rhizome types (Symbol, Location, Diagnostic)

5. **Cache**: Reuse server across calls (no restart between requests)

### Multi-Client Design

Managers track `(Language, project_root)` tuples. This allows:

```rust
// Same language, different roots → different servers
get_definition("/path/a/file.rs", Lang::Rust)  // Uses server for /path/a
get_definition("/path/b/file.rs", Lang::Rust)  // Uses server for /path/b

// Different language → different servers
get_definition("/path/file.rs", Lang::Rust)    // rust-analyzer
get_definition("/path/file.py", Lang::Python)  // pyright
```

### Root Detection

File: `crates/rhizome-core/src/root_detector.rs`

Each language has markers that identify project roots:

| Language | Markers |
|----------|---------|
| Rust | `Cargo.toml` (with `[workspace]` for workspace root), `Cargo.lock` |
| Python | `pyproject.toml`, `setup.py`, `requirements.txt`, `Pipfile`, `pyrightconfig.json` |
| JavaScript/TypeScript | `package.json`, `tsconfig.json`, `jsconfig.json` |
| Go | `go.work`, `go.mod`, `go.sum` |
| Java | `pom.xml`, `build.gradle`, `build.gradle.kts` |
| C/C++ | `CMakeLists.txt`, `compile_commands.json`, `Makefile` |

Special handling:
- **Rust**: Looks for `[workspace]` in Cargo.toml to find workspace root (monorepo support)
- **Go**: Prefers `go.work` (workspace) over `go.mod`
- **JS/TS**: Skips Deno projects (detects `deno.json`)

Fallback chain:
1. Walk up from file directory looking for language markers
2. If none found, look for `.git` directory
3. If none found, return file's parent directory

## Configuration

File: `crates/rhizome-core/src/config.rs`

Configuration merges from two sources, with project config overriding global:

1. **Global**: `~/.config/rhizome/config.toml`
2. **Project**: `<project_root>/.rhizome/config.toml`

Example:
```toml
[languages.rust]
server_binary = "/opt/custom/rust-analyzer"
server_args = ["--log-file", "/tmp/ra.log"]
enabled = true

[languages.python]
server_binary = "pyright-langserver"
enabled = true

[languages.java]
enabled = false  # Disable Java

[lsp]
disable_download = false  # Allow auto-install
bin_dir = "/opt/rhizome/bin"  # Custom LSP install directory

[export]
auto_export = true  # Export to Hyphae on startup
```

Environment variables override config:
- `RHIZOME_DISABLE_LSP_DOWNLOAD=1`: Disable auto-install

## MCP Tools: 26 Total

File: `crates/rhizome-mcp/src/tools/mod.rs`

Tools are grouped by category:

### Symbol Tools (18 tools)
`get_symbols`, `get_structure`, `get_definition`, `find_references`, `search_symbols`, `go_to_definition`, `get_signature`, `get_imports`, `get_call_sites`, `get_scope`, `get_exports`, `summarize_file`, `get_tests`, `get_diff_symbols`, `get_annotations`, `get_complexity`, `get_type_definitions`, `get_dependencies`, `get_parameters`, `get_enclosing_class`, `get_symbol_body`, `get_changed_files`, `summarize_project`

### File Tools (4 tools)
`get_diagnostics`, `rename_symbol`, `get_hover_info`

### Edit Tools (7 tools)
`replace_symbol_body`, `insert_after_symbol`, `insert_before_symbol`, `replace_lines`, `insert_at_line`, `delete_lines`, `create_file`

### Export Tools (1 tool)
`export_to_hyphae` — Extract symbols and build code graph for Hyphae integration

### Onboarding (1 tool)
`rhizome_onboard` — Initialize a new project

## Hyphae Integration

File: `crates/rhizome-core/src/hyphae.rs`

When a file changes, Rhizome can export symbol data to Hyphae. The flow: extract symbols via tree-sitter, link them into a graph (definitions, references, imports), send the graph to Hyphae via spore IPC, and cache checksums to skip unchanged files on the next export. This lets Hyphae index code across a project and provide cross-file symbol search, refactoring, and memory.

## Error Handling

All backends and tools return `Result<T>` with context:

| Error | Cause | User Action |
|-------|-------|-------------|
| "Unsupported extension" | File type not recognized | Check Language enum for supported extensions |
| "No tree-sitter grammar" | Language has no query pattern | Use LSP if available, or file an issue |
| "LSP server not found: rust-analyzer" | Binary not in PATH, auto-install failed | Install manually: `rustup component add rust-analyzer` |
| "LSP auto-install disabled" | `RHIZOME_DISABLE_LSP_DOWNLOAD=1` set | Unset variable or install manually |
| "Tool not found" | Unknown MCP tool name | Check `rhizome --help` for valid tools |

## Key Dependencies

- **tree-sitter**: Parsing (tree-sitter-rust, tree-sitter-python, etc.)
- **lsp-types**: LSP protocol definitions
- **tokio**: Async runtime for LSP clients
- **serde_json**: JSON serialization
- **spore**: IPC to Hyphae
- **anyhow**: Error handling (app-level)
- **tracing**: Logging

## Development Commands

```bash
# Build all crates
cargo build --release

# Run tests
cargo test --all

# Run CLI
cargo run -- serve  # Start MCP server
cargo run -- symbols <file>  # Extract symbols from file
cargo run -- status  # Show language + LSP availability

# Run specific backend tests
cargo test -p rhizome-treesitter  # Tree-sitter tests
cargo test -p rhizome-lsp          # LSP tests

# Format and lint
cargo fmt
cargo clippy
```

## Testing Strategy

- **Unit tests**: Backend implementation tests in each crate
- **Tree-sitter fixtures**: Rust, Python, TypeScript test files in `tests/fixtures/`
- **LSP tests**: Mock LSP server tests (requires language servers installed)
- **Integration**: End-to-end CLI tests

Run all: `cargo test --all`
