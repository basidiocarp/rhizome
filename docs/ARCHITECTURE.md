# Rhizome Architecture

Rhizome is a 5-crate workspace that compiles into a single binary. It solves a
practical tension: tree-sitter is fast and local, but LSP is deeper and more
semantic, so Rhizome selects the right backend per tool call instead of forcing
one model onto every request. This document covers that split, the request
path, and the parts contributors extend most often.

---

## Design Principles

- **Backend by capability, not ideology** — use tree-sitter where it is enough,
  escalate to LSP where semantics matter.
- **Fast by default** — symbol and structure queries should work offline and
  without booting language servers whenever possible.
- **Project-aware semantics** — when LSP is required, root detection and
  language-server lifecycle matter as much as the tool implementation.
- **Lazy heavy paths** — LSP startup and installation are deferred until a tool
  actually needs them.
- **MCP-first output** — responses are shaped for tool consumers, not for humans
  reading raw editor logs.

---

## Workspace Structure

```text
rhizome-cli ──► rhizome-mcp ──► rhizome-treesitter ──► rhizome-core
                     │                                      ▲
                     └──────────► rhizome-lsp ──────────────┘
```

All five crates compile into the `rhizome` binary.

- **`rhizome-core`**: Domain types, backend selection, root detection, config,
  installer logic, and shared abstractions. No tool transport here.
- **`rhizome-treesitter`**: Fast parsing and symbol extraction for supported
  languages, plus generic fallback walkers.
- **`rhizome-lsp`**: Language-server client management, request dispatch, and
  response translation into Rhizome types.
- **`rhizome-mcp`**: Tool definitions and MCP request handling.
- **`rhizome-cli`**: Binary entry point, status surfaces, and direct CLI
  commands.

### Boundary Rules

- `rhizome-core` owns backend selection, config, root detection, and code-graph
  primitives.
- `rhizome-mcp` owns tool routing and lazy backend startup; it should not make
  ad hoc backend decisions outside the shared selector.
- `rhizome-lsp` owns live language-server processes and protocol translation,
  not domain policy.
- Export stays on the shared tree-sitter/code-graph path unless a handoff
  explicitly expands the contract.
- If a new tool needs LSP semantics, classify it in `BackendSelector` first
  and then wire the dispatcher to that classification.

---

## Core Abstraction

```rust
pub trait CodeIntelligence {
    fn get_symbols(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn get_structure(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn get_definition(&self, file: &Path, name: &str) -> Result<Option<Symbol>>;
    fn find_references(&self, file: &Path, position: Position) -> Result<Vec<Location>>;
    fn get_imports(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn get_diagnostics(&self, file: &Path) -> Result<Vec<Diagnostic>>;
}
```

Both the tree-sitter and LSP backends implement this trait. The dispatcher
consumes it through `BackendSelector`, and the invariant is that callers should
not need to care which backend produced the answer unless the tool explicitly
requires LSP.

---

## Request Flow

When an MCP tool call arrives:

1. **Route the tool** (`ToolDispatcher.call_tool`)
   Maps the tool name to a concrete handler.
   Example: `get_symbols` goes to the symbol tool path, while `rename_symbol`
   goes to an edit path.

2. **Resolve the backend** (`BackendSelector.select`)
   Looks up whether the tool is `TreeSitter`, `PrefersLsp`, or `RequiresLsp`.
   Example: `get_structure` stays on tree-sitter; `rename_symbol` requires LSP;
   `find_references` prefers LSP but can fall back.

3. **Detect the project root** (`root_detector`)
   Walks upward from the file to find the right language markers or falls back
   to `.git` or the parent directory.
   Example: Rust prefers a workspace `Cargo.toml`; Go prefers `go.work`.

4. **Initialize heavy services lazily** (`ToolDispatcher.ensure_lsp`)
   Starts or reuses an LSP client only if the selected backend needs it.
   Example: the first Rust rename request may install or start
   `rust-analyzer`; later requests reuse it.

5. **Execute the backend method**
   Tree-sitter parses the file and runs language queries or generic fallback
   walkers. LSP sends a JSON-RPC request to the server and translates the
   response.

6. **Serialize the result**
   Symbols, locations, diagnostics, or edit previews are returned in
   MCP-friendly JSON.

---

## Tree-Sitter Backend

File: `crates/rhizome-treesitter/src/lib.rs`

### How It Works

1. Reuse a cached parser for the target language.
2. Parse the source file into a syntax tree.
3. Run language-specific queries when available.
4. Fall back to a generic walker for languages without custom queries.
5. Translate matches into Rhizome symbols and locations.

### Capability Matrix

| Tier | Examples | Behavior |
|------|----------|----------|
| Query-backed | Rust, Python, TypeScript, Go, Java, C, C++, Ruby, PHP | Precise extraction with language-specific queries |
| Generic fallback | Bash, C#, Elixir, Lua, Swift, Zig, Haskell | Walk common node types and infer structure |
| LSP-only | Terraform, Kotlin, Dart, Vue, Svelte, Astro, Typst, YAML | Tree-sitter is insufficient or unavailable, so deeper tools rely on LSP |

### Adding a Query Pattern

File: `crates/rhizome-treesitter/src/queries.rs`

1. Write the query for the target grammar.
2. Add the constant and compile path in `get_query()`.
3. Cache it with the existing `OnceLock` pattern.
4. Add fixture coverage before relying on it in tool behavior.

---

## LSP Backend

File: `crates/rhizome-lsp/src/lib.rs`

### How It Works

1. Resolve `(language, project_root)` so monorepos can keep distinct servers.
2. Start the language server over stdio if no client exists yet.
3. Send the request with document URI and position data.
4. Translate the server response back into Rhizome types.
5. Cache the live client for the next request.

### Configuration Matrix

| Setting | Example | Behavior |
|---------|---------|----------|
| `server_binary` | `rust-analyzer` | Override the default server binary |
| `server_args` | `["--stdio"]` | Pass custom startup args |
| `enabled = false` | Java disabled | Turns a language off entirely |
| `lsp.disable_download` | `true` | Disables managed auto-install |

### Adding a Language Server

File: `crates/rhizome-core/src/config.rs`

1. Add the language to the core language enum and default server config.
2. Teach root detection which files mark a project boundary.
3. Add or update install logic if managed download is supported.
4. Add integration coverage for status or tool behavior.

---

## Configuration

Config file: `~/.config/rhizome/config.toml`
Project override: `<project_root>/.rhizome/config.toml`

```toml
[languages.rust]
server_binary = "rust-analyzer"
server_args = ["--log-file", "/tmp/ra.log"]

[languages.python]
server_binary = "pyright-langserver"
server_args = ["--stdio"]

[lsp]
disable_download = false
bin_dir = "/opt/rhizome/bin"

[export]
auto_export = true
```

Environment variables override config:

- `RHIZOME_DISABLE_LSP_DOWNLOAD=1` — disable managed LSP installs

---

## Error Handling

| Error | Cause | User Action |
|-------|-------|-------------|
| `"Unsupported extension"` | File type is not mapped to a supported language | Check the language enum and file extension mapping |
| `"No tree-sitter grammar"` | The language lacks a usable parser or query path | Use an LSP-backed tool if available or add grammar support |
| `"LSP server not found: rust-analyzer"` | Binary missing and auto-install failed | Install manually with `rustup component add rust-analyzer` |
| `"LSP auto-install disabled"` | `RHIZOME_DISABLE_LSP_DOWNLOAD=1` is set | Unset the variable or install the server yourself |
| `"Tool not found"` | Unknown MCP tool name | Check `rhizome --help` or the MCP tool list |

---

## Testing

```bash
cargo test --all
cargo test -p rhizome-treesitter
cargo test -p rhizome-lsp
```

| Category | Count | What's Tested |
|----------|-------|---------------|
| Unit | 150+ | Backend selection, config merging, root detection, parser behavior |
| Fixture-based backend tests | 50+ | Symbol extraction and language-specific behavior against sample files |
| Integration | 30+ | MCP dispatch, CLI flows, LSP fallback behavior, edit tools |
| Error and install paths | 20+ | Missing binaries, disabled downloads, unsupported languages |

Fixtures live in crate-local test directories. For tree-sitter work, update or
add fixtures before adjusting query behavior so the expected symbol shape stays
reviewable.

---

## Key Dependencies

- **`tree-sitter`** — fast local structure extraction and query-backed symbol
  discovery.
- **`lsp-types`** — protocol types for talking to language servers.
- **`tokio`** — async runtime for LSP client management while keeping a simple
  external interface.
- **`spore`** — shared IPC primitives, especially for Hyphae export and config
  helpers.
