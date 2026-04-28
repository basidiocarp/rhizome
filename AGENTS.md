# Rhizome Agent Notes

## Purpose

Rhizome owns code intelligence and structural editing for the ecosystem. Work here should keep backend choice explicit, tree-sitter and LSP behavior in their owning crates, and Hyphae export on the shared code-graph path. Rhizome analyzes and edits structure; it should not absorb memory or shell-filtering behavior.

---

## Source of Truth

- `crates/rhizome-core/`: backend selection, root detection, graph export, and shared types.
- `crates/rhizome-treesitter/`: offline parsing and symbol extraction.
- `crates/rhizome-lsp/`: richer cross-file analysis and refactor support.
- `crates/rhizome-mcp/`: MCP tool dispatch.
- `crates/rhizome-cli/`: command-line entry and operator surfaces.
- `../septa/`: authoritative schema and fixture for Hyphae export.
- `../ecosystem-versions.toml`: shared dependency pins.

If Rhizome's Hyphae export changes, update `../septa/` first.

---

## Before You Start

Before writing code, verify:

1. **Owning crate**: keep backend choice in core, tree-sitter work in treesitter, LSP work in lsp, and transport in mcp or cli.
2. **Backend selection**: classify new capabilities through the shared backend-selection path before adding tool-specific logic.
3. **Contracts**: if the code-graph export changes, read the matching `../septa/` files first.
4. **Validation target**: decide whether the change needs tree-sitter, LSP, export, or broader MCP coverage.

---

## Preferred Commands

Use these for most work:

```bash
cargo build
cargo test --all
```

For targeted work:

```bash
cargo build --release
cargo test -p rhizome-treesitter
cargo test -p rhizome-lsp
cargo clippy
cargo fmt --check
```

Optional live tests (require an installed language server or hyphae):

```bash
# Requires rust-analyzer in PATH
cargo test -p rhizome-lsp --test live_lsp -- --ignored

# Requires hyphae in PATH
cargo test -p rhizome-mcp -- test_export_to_hyphae_e2e --ignored
```

---

## Repo Architecture

Rhizome is healthiest when backend selection, parser work, LSP work, and transport stay in separate layers.

Key boundaries:

- `rhizome-core`: shared policy and export path, not transport-specific behavior.
- `rhizome-treesitter`: fast offline parsing.
- `rhizome-lsp`: richer language-server behavior.
- `rhizome-mcp`: MCP exposure and backend dispatch.
- `rhizome-cli`: operator-facing entry surface.

Current direction:

- Keep backend choice explicit per call.
- Keep export logic on the shared tree-sitter and code-graph path unless the design intentionally changes.
- Keep parser and LSP behavior tested with real language fixtures.

---

## Working Rules

- Do not let `rhizome-core` grow direct dependencies on transport crates.
- Do not add backend-specific side paths when the capability should go through shared selection.
- Treat Hyphae export changes as contract work and update `../septa/` in the same change.
- Prefer real parser or language-server fixtures over synthetic token streams.
- Validate septa contracts after changing any cross-project payload: `cd septa && bash validate-all.sh`

---

## Multi-Agent Patterns

For substantial Rhizome work, default to two agents:

**1. Primary implementation worker**
- Owns the touched crate or feature slice
- Keeps the write scope inside Rhizome unless a real contract update requires `../septa/`

**2. Independent validator**
- Reviews the broader shape instead of redoing the implementation
- Specifically looks for backend-selection drift, core-layer leakage, export-contract drift, and parser-vs-LSP confusion

Add a docs worker when `README.md`, `CLAUDE.md`, `AGENTS.md`, or public docs changed materially.

---

## Skills to Load

Use these for most work in this repo:

- `basidiocarp-rust-repos`: repo-local Rust workflow and validation habits
- `systematic-debugging`: before fixing unexplained parser, LSP, or MCP failures
- `writing-voice`: when touching README or docs prose

Use these when the task needs them:

- `test-writing`: when behavior changes need stronger coverage
- `basidiocarp-workspace-router`: when the change may spill into `septa` or `hyphae`
- `tool-preferences`: when exploration should stay tight

---

## Done Means

A task is not complete until:

- [ ] The change is in the right crate and layer
- [ ] The narrowest relevant validation has run, when practical
- [ ] Related schemas, fixtures, docs, or transport surfaces are updated if they should move together
- [ ] Any skipped validation or follow-up work is stated clearly in the final response

If validation was skipped, say so clearly and explain why.
