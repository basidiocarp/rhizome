# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Rhizome is the code-intelligence MCP server for the ecosystem. It is a five-crate Rust workspace that combines tree-sitter parsing, LSP-backed analysis, and a transport layer that chooses the right backend per call. The current MCP surface is 40 tools: structural read and analysis tools, structural edit tools, what-if simulation, export tools, and onboard. Rhizome owns code intelligence and export; it does not own memory, shell filtering, or install policy.

Backend choice is part of the core contract, not an implementation detail. New
tool work should first classify the capability in `rhizome-core::BackendSelector`,
then wire the MCP or CLI surface to that shared decision. `rhizome-core` should
not grow direct `rhizome-lsp` or `rhizome-mcp` dependencies, and export logic
should stay on the shared tree-sitter/code-graph path unless the handoff says
otherwise.

---

## Operating Model

- Do not execute code. Rhizome is static analysis and structural editing only.
- Do not require LSP for every feature. Tree-sitter is still the default path for a large part of the surface.
- Do not turn Rhizome into a stateful IDE clone. It provides structure-aware tools when raw reads would be weaker.
- Do not absorb Hyphae import semantics. Rhizome produces the code graph; Hyphae owns memoir import.

---

## Failure Modes

- **LSP unavailable**: falls back to tree-sitter where it can, otherwise returns an install-oriented error.
- **Tree-sitter parse failure**: returns a clear parse failure tied to the file.
- **Workspace root not detected**: uses a narrower root and may lose project-wide context.
- **Unsupported language or missing server**: returns the supported-language or install guidance instead of guessing.

---

## State Locations

| What | Path |
|------|------|
| Config file | `~/.config/rhizome/config.toml` |
| Auto-installed LSP servers | `~/.rhizome/bin/` |
| Runtime logs | stderr |

---

## Build & Test Commands

```bash
cargo build
cargo build --release

cargo test --all
cargo test -p rhizome-treesitter
cargo test -p rhizome-lsp

cargo clippy
cargo fmt --check
cargo fmt
```

---

## Architecture

```text
rhizome-cli ──► rhizome-mcp ──► rhizome-treesitter ──► rhizome-core
                     │                                       ▲
                     └────────► rhizome-lsp ────────────────┘
```

- **rhizome-core**: language, backend, config, root-detection, and graph-export primitives.
- **rhizome-treesitter**: fast offline parsing and symbol extraction.
- **rhizome-lsp**: richer cross-file analysis and refactor support.
- **rhizome-mcp**: MCP tool dispatch and backend selection.
- **rhizome-cli**: command-line entry point and operator surfaces.

---

## Core Abstraction

```rust
pub trait CodeIntelligence {
    fn get_symbols(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn find_references(&self, file: &Path, position: Position) -> Result<Vec<Location>>;
    fn get_diagnostics(&self, file: &Path) -> Result<Vec<Diagnostic>>;
}
```

Both the tree-sitter and LSP backends implement this trait. The rest of the system should add capabilities through that boundary when possible instead of inventing backend-specific side paths.

---

## Key Design Decisions

- **Backend selection per call**: keeps cheap tree-sitter work fast while still allowing LSP depth where it matters.
- **Multi-crate workspace**: isolates parsing, LSP, MCP, and CLI concerns cleanly.
- **Export path to Hyphae**: code-graph generation is part of the architecture, not an afterthought.

---

## Key Files

| File | Purpose |
|------|---------|
| `crates/rhizome-core/src/backend_selector.rs` | backend choice logic |
| `crates/rhizome-core/src/installer.rs` | LSP install recipes and setup |
| `crates/rhizome-core/src/root_detector.rs` | workspace-root detection |
| `crates/rhizome-core/src/graph.rs` | code-graph generation for export |
| `crates/rhizome-mcp/src/tools/mod.rs` | MCP dispatch into the backend layer |
| `crates/rhizome-core/tests/backend_boundary.rs` | backend-classification guard |

---

## MCP Tools

The following `mcp__rhizome__*` tools are available for Claude Code:

**Structural reads**:
- `mcp__rhizome__search_symbols`: find definitions and all uses of a symbol
- `mcp__rhizome__find_references`: locate all places a symbol is referenced
- `mcp__rhizome__get_definition`: jump to a symbol's definition
- `mcp__rhizome__go_to_definition`: navigate to definition (alias)
- `mcp__rhizome__get_structure`: understand a file's class and function tree
- `mcp__rhizome__summarize_file`: get a file overview without reading every line
- `mcp__rhizome__summarize_project`: get a project-level overview
- `mcp__rhizome__get_symbols`: extract all symbols from a file
- `mcp__rhizome__get_call_sites`: find where a function is called
- `mcp__rhizome__get_signature`: retrieve a function's signature
- `mcp__rhizome__get_parameters`: extract function parameters
- `mcp__rhizome__get_symbol_body`: get a symbol's implementation
- `mcp__rhizome__get_region`: extract code between positions
- `mcp__rhizome__get_scope`: analyze variable and symbol scope
- `mcp__rhizome__get_enclosing_class`: find the class containing a symbol
- `mcp__rhizome__get_imports`: list imports in a file
- `mcp__rhizome__get_exports`: list exports from a file
- `mcp__rhizome__get_dependencies`: trace module dependencies
- `mcp__rhizome__get_type_definitions`: extract type definitions
- `mcp__rhizome__get_tests`: find test files and test functions
- `mcp__rhizome__get_complexity`: measure code complexity
- `mcp__rhizome__get_annotations`: extract decorators and annotations
- `mcp__rhizome__get_diagnostics`: check for syntax and type errors

**Impact analysis**:
- `mcp__rhizome__analyze_impact`: predict blast radius of a change
- `mcp__rhizome__get_diff_symbols`: identify symbols changed in a diff
- `mcp__rhizome__get_changed_files`: list files affected by a change

**Structural edits**:
- `mcp__rhizome__rename_symbol`: rename a symbol across its scope
- `mcp__rhizome__move_symbol`: move a symbol to another file
- `mcp__rhizome__copy_symbol`: duplicate a symbol
- `mcp__rhizome__replace_symbol_body`: rewrite a symbol's implementation
- `mcp__rhizome__insert_before_symbol`: add code before a symbol
- `mcp__rhizome__insert_after_symbol`: add code after a symbol
- `mcp__rhizome__insert_at_line`: insert code at a specific line
- `mcp__rhizome__replace_lines`: replace a range of lines
- `mcp__rhizome__delete_lines`: remove lines from a file
- `mcp__rhizome__create_file`: create a new file

**What-if**:
- `mcp__rhizome__rhizome_simulate_change`: predict the effect of a change without committing

**Export**:
- `mcp__rhizome__export_repo_understanding`: generate a code-graph summary
- `mcp__rhizome__export_to_hyphae`: send code structure to Hyphae for memoir import

**Utilities**:
- `mcp__rhizome__rhizome_onboard`: initialize Rhizome with your workspace

---

## Communication Contracts

### Outbound (this project sends)

| Contract | Target | Protocol | Schema |
|----------|--------|----------|--------|
| `code-graph-v1` | Hyphae | MCP `hyphae_import_code_graph` | `septa/code-graph-v1.schema.json` |

**Source files:**
- `crates/rhizome-core/src/graph.rs`
- `crates/rhizome-core/src/hyphae.rs`

Breaking change impact: Hyphae memoir import fails or silently degrades.

### Inbound (this project receives)

Rhizome does not consume a sibling tool's runtime payload format as a first-party contract. Cap and other clients consume Rhizome's own tool surface instead.

### Shared Dependencies

- **spore**: check `../ecosystem-versions.toml` before upgrading.
- **JSON-RPC framing**: line-delimited, not Content-Length.

### Contract Validation

When changing output shapes that cross a project boundary, validate against septa:

```bash
cd ../septa && bash validate-all.sh
```

Check that this tool's schemas still pass before closing the change.

---

## Testing Strategy

- Tree-sitter tests are the primary coverage for structure and symbol extraction.
- LSP tests should focus on the features tree-sitter cannot stand in for.
- Export changes should be checked against the Hyphae contract, not only local structs.
- Use real language fixtures for parser work instead of synthetic token streams.
