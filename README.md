# Rhizome

Editor-agnostic code intelligence MCP server. Gives AI agents symbol-level
navigation, structure, and refactor surfaces instead of forcing them to read
raw files.

Named after fungal rhizomorphs, root-like structures that spread through an
environment and expose hidden pathways.

Part of the [Basidiocarp ecosystem](https://github.com/basidiocarp).

---

## The Problem

Most MCP-compatible clients have no built-in code intelligence. They can read
files, but they cannot navigate symbols, find references, inspect structure, or
perform project-wide refactors without expensive raw reads and brittle search.

## The Solution

Rhizome provides a standalone code-intelligence layer for any MCP client.
Tree-sitter handles fast offline structure work, LSP takes over when you need
cross-file semantics, and the MCP surface keeps the result compact enough to
use in real sessions.

---

## The Ecosystem

| Tool | Purpose |
|------|---------|
| **[rhizome](https://github.com/basidiocarp/rhizome)** | Code intelligence via tree-sitter and LSP |
| **[cap](https://github.com/basidiocarp/cap)** | Web dashboard for the ecosystem |
| **[cortina](https://github.com/basidiocarp/cortina)** | Lifecycle signal capture and session attribution |
| **[hyphae](https://github.com/basidiocarp/hyphae)** | Persistent agent memory |
| **[lamella](https://github.com/basidiocarp/lamella)** | Skills, hooks, and plugins for coding agents |
| **[mycelium](https://github.com/basidiocarp/mycelium)** | Token-optimized command output |
| **[spore](https://github.com/basidiocarp/spore)** | Shared transport and editor primitives |
| **[stipe](https://github.com/basidiocarp/stipe)** | Ecosystem installer and manager |
| **[volva](https://github.com/basidiocarp/volva)** | Execution-host runtime layer |

> **Boundary:** `rhizome` owns code intelligence and structural editing tools.
> It does not own memory, shell filtering, lifecycle capture, or installation.
> Backend selection stays centralized in `rhizome-core::BackendSelector`;
> `rhizome-mcp` routes tools through that shared policy instead of inventing
> ad hoc LSP branches, and `rhizome-lsp` stays focused on live protocol
> clients and translation.

---

## Quick Start

```bash
# Build
cargo build --release

# Recommended: full ecosystem setup
stipe init

# Alternative: rhizome-only config guidance
rhizome init
```

```bash
# Inspect status and servers
rhizome status
rhizome lsp status
rhizome lsp install python

# Use directly
rhizome symbols src/main.rs
rhizome structure src/lib.rs
```

---

## How It Works

```text
MCP client             Rhizome                      Backend
──────────             ───────                      ───────
tool call        ─►    backend selector      ─►    tree-sitter
deeper query     ─►    capability check      ─►    LSP if needed
edit or export   ─►    tool handler          ─►    project-aware result
```

1. Receive MCP requests: navigation, diagnostics, editing, and export calls arrive through the MCP server.
2. Select a backend: choose tree-sitter, generic fallback, or LSP based on the tool and language.
3. Run project-aware analysis: resolve structure, symbols, references, or diagnostics.
4. Return structured results: emit compact machine-readable data instead of raw file dumps.
5. Export knowledge: send code graph data to Hyphae when requested.

---

## Supported Languages

| Tier | Languages | How it works |
|------|-----------|-------------|
| Full query patterns | 10 languages | Language-specific tree-sitter queries |
| Generic fallback | 8 languages | Generic AST walker over common node types |
| LSP only | 14 or more languages | Language server required |

All 32 supported languages have LSP server configs, and 20 or more have
auto-install recipes.

---

## What Rhizome Owns

- Symbol navigation and structure queries
- Tree-sitter and LSP backend orchestration
- Safe structural editing tools
- Code graph export to Hyphae

## What Rhizome Does Not Own

- Long-term memory and retrieval: handled by `hyphae`
- Token filtering: handled by `mycelium`
- Lifecycle signal capture: handled by `cortina`
- Installation and host registration: handled by `stipe`

---

## Key Features

- Dual backend model: uses tree-sitter by default and upgrades to LSP when the task requires it.
- MCP-first surface: works with Claude Code, Codex, Cursor, Continue, and other MCP clients.
- Structural editing: exposes targeted edit tools instead of line-oriented shell patching.
- Project-aware export: can ship code graph data into Hyphae memoirs.
- Managed LSP support: includes status, install, and per-language config surfaces.

---

## Architecture

```text
rhizome/
├── rhizome-core        backend selection, config, installer
├── rhizome-treesitter  offline parsing and symbol queries
├── rhizome-lsp         LSP client backend
├── rhizome-mcp         MCP server and tool handlers
└── rhizome-cli         CLI entry point
```

```text
rhizome serve                     start MCP server
rhizome symbols <file>            list symbols
rhizome structure <file>          show symbol tree
rhizome status                    show backend status
rhizome lsp install <language>    install an LSP server
rhizome export                    export code graph to Hyphae
```

---

## Configuration

```toml
# <platform config dir>/rhizome/config.toml

[languages.python]
server_binary = "ruff"
server_args = ["server"]

[lsp]
disable_download = false
```

## Logging

Rhizome reads `RHIZOME_LOG` first, then falls back to `RUST_LOG`. If neither is
set, it defaults to `warn`.

```bash
# General debugging
RHIZOME_LOG=debug rhizome serve

# Narrow to a noisy module
RHIZOME_LOG=rhizome_mcp::tools=debug rhizome serve
```

`rhizome serve` keeps stdout reserved for newline-delimited MCP JSON-RPC
traffic. Logs go to stderr so they do not corrupt the transport.

---

## Documentation

- [docs/README.md](docs/README.md): docs index
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): backend and server architecture
- [docs/CONFIG.md](docs/CONFIG.md): config file reference
- [docs/LANGUAGE-SETUP.md](docs/LANGUAGE-SETUP.md): language and LSP setup details
- [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md): diagnostics and fixes
- [docs/ROADMAP.md](docs/ROADMAP.md): planned work
- [LSP-GUIDE.md](LSP-GUIDE.md): LSP-focused guidance

## Development

```bash
cargo build --release
cargo test --all
cargo clippy
cargo fmt
```

## License

See [LICENSE](LICENSE) for details.
