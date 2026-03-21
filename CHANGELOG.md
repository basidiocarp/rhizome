# Changelog

## [0.5.3] - 2026-03-21

### Added

- **Parse-tree LRU cache**: Process-wide shared cache (100 entries) for tree-sitter parsed trees, keyed by `(file_path, mtime)`. Eliminates redundant re-parsing when multiple MCP tools operate on the same file. ~5-10x speedup on repeated file access.

### Changed

- **Spore v0.3.0**: Self-update and logging now use shared spore modules.

## [0.4.3] - 2026-03-18

### Added

- **7 file editing MCP tools**: `replace_symbol_body`, `insert_after_symbol`, `insert_before_symbol`, `replace_lines`, `insert_lines`, `delete_lines`, `create_file` — enabling agents to make targeted code edits through Rhizome.
- **`summarize_project` MCP tool**: Generates a high-level project summary including language breakdown, key entry points, dependency count, and architecture overview.
- **`rhizome summarize` CLI command**: CLI entrypoint for project summarization.
- **`rhizome_onboard` MCP tool**: Guided onboarding that detects the project stack, available backends, and returns a structured orientation for new agents.
- **`rhizome doctor` diagnostic command**: Health check that validates tree-sitter grammars, LSP server availability, Hyphae connectivity, and configuration.
- **Tree-sitter queries for Java, C, C++, Ruby, PHP**: Extended language-specific query patterns from 5 to 10 languages, improving symbol extraction accuracy for these languages.
- **Spore adoption for Hyphae discovery**: Replaced manual binary detection with the shared `spore` crate for consistent Hyphae tool resolution during export.

## [0.4.0] - 2026-03-16

### Added

- **32 language support**: Elixir, Zig, C#, F#, Swift, PHP, Haskell, Bash, Terraform, Kotlin, Dart, Lua, Clojure, OCaml, Julia, Nix, Gleam, Vue, Svelte, Astro, Prisma, Typst, YAML — each with LSP server config, file extension mapping, root markers, and graph metadata.
- **Auto-install LSP servers**: `LspInstaller` auto-downloads missing servers via native package managers (rustup, pipx/pip, npm, go install, gem, dotnet tool, ghcup, brew, opam, nix-env, cargo). Installs to `~/.rhizome/bin/`. Controlled by `RHIZOME_DISABLE_LSP_DOWNLOAD=1` env var or `lsp.disable_download` config.
- **Binary-name-based install recipes**: 20+ install recipes keyed by server binary name, not language. Users who configure alternative servers (e.g. `ruff` instead of `pyright`, `ruby-lsp` instead of `solargraph`) get auto-install support automatically.
- **Backend auto-selection**: `BackendSelector` maps each tool to a backend requirement (tree-sitter, prefers-lsp, requires-lsp) and resolves per call. Tree-sitter for most tools, automatic LSP upgrade for `find_references`, `get_diagnostics`, `rename_symbol`, `get_hover_info`.
- **Smart root detection**: Per-language workspace root detection — Rust walks up for `[workspace]` in `Cargo.toml`, Go prefers `go.work`, JS/TS skips Deno dirs, Python finds `pyproject.toml`. Falls back to `.git`.
- **Multi-client LSP management**: `LanguageServerManager` keyed by `(Language, PathBuf)` supports multiple LSP clients for different workspace roots in monorepos.
- **`rhizome status` CLI command**: Shows per-language backend availability, detected LSP server paths, auto-install state, and managed bin directory.
- **`LspConfig`**: New config section with `disable_download` and `bin_dir` fields.

### Changed

- PHP default server changed from `intelephense` to `phpactor`.
- Ruby default server changed from `solargraph` to `ruby-lsp`.
- `ToolDispatcher` uses `RefCell` for lazy LSP initialization with `BackendSelector` integration.
- `LspBackend` exposes root-aware methods (`*_with_root`) alongside `CodeIntelligence` trait.
- Install hints now derive from the recipe registry, showing correct commands for whatever server is configured.

### Fixed

- TypeScript tree-sitter query: use `type_identifier` for `class_declaration` name field (was `identifier`, causing query compilation failure on `.ts` files).
- Export integration tests handle environments where Hyphae is installed.
- Hyphae export uses spore for tool discovery and line-delimited JSON-RPC.
- Flattened `export_graph` params to match Hyphae's expected format.

## [0.3.0] - 2026-03-16

### Added

- **Code graph construction** (`rhizome-core::graph`): Build concept graphs from extracted symbols with typed nodes, labeled edges (contains, imports), and per-node metadata (file path, line range, language). Supports merging graphs across files with deduplication.
- **Hyphae integration** (`rhizome-core::hyphae`): Export code graphs to Hyphae's semantic knowledge store via JSON-RPC over stdio. Spawns `hyphae serve`, sends the graph, and returns concept/link counts.
- **Incremental export cache** (`rhizome-core::export_cache`): Mtime-based file change tracking that skips unchanged files on re-export. Persists to `.rhizome/cache.json`.
- **`export_to_hyphae` MCP tool**: Walks the project (respecting `.gitignore`), extracts symbols via tree-sitter, builds a merged concept graph, and sends it to Hyphae. Reports files processed vs skipped.
- **`rhizome export` CLI command**: CLI entrypoint for the Hyphae export pipeline.
- **Auto-export on MCP server startup**: When Hyphae is available and `export.auto_export` is enabled (default), the MCP server automatically exports the code graph in the background on startup.
- **Export configuration** (`RhizomeConfig.export`): New `ExportConfig` section with `auto_export` toggle.
- Graph integration tests verifying end-to-end symbol extraction → graph building for Rust and Python fixtures.
- Integration tests for the export tool (unavailable, unified mode, and E2E scenarios).

## [0.2.0] - 2026-02-28

### Added

- MCP server with 25 code intelligence tools (unified + expanded modes)
- Tree-sitter backend: symbol extraction, definitions, references, imports, diagnostics
- LSP backend: async client with JSON-RPC, cross-file references, rename, type info
- CLI with `serve` and `analyze` commands
- Support for 9 languages: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Ruby
- CI/CD workflows and release infrastructure

## [0.1.0] - 2026-02-14

### Added

- Initial workspace setup with 5 crates
- Core domain types: `Symbol`, `SymbolKind`, `Location`, `Language`
- `CodeIntelligence` trait with 6 operations
