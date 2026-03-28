# Changelog

## [Unreleased]

## [0.6.2] - 2026-03-27

### Added

- **Rename preview and impact analysis**: `rename_symbol` now supports dry-run preview, and `analyze_impact` summarizes grouped references, callers, callees, tests, and disclosed backend confidence.
- **Scope-aware symbol identity**: Rhizome now emits `qualified_name` and `stable_id` metadata so nested symbols and methods keep container context across tree-sitter and LSP paths.
- **Persistent workspace symbol snapshots**: Tree-sitter workspace search now persists scoped on-disk symbol indexes under `.rhizome/`, improving repeat query performance across process restarts.

### Changed

- **Workspace cache hardening**: Persistent workspace snapshots now use schema versioning plus stronger file fingerprints instead of trusting timestamps alone.
- **Scoped project state paths**: Project-local cache/config state now flows through shared `.rhizome/` path helpers instead of ad hoc joins.
- **Roadmap and README refresh**: Rhizome docs now describe the current impact-analysis and workspace-index direction more accurately.

### Fixed

- **Hyphae export resilience**: Export now resolves relative roots against the project root, reports partial failures more clearly, and tolerates unreadable cache state better.
- **Wildcard root markers**: Haskell `*.cabal` and OCaml `*.opam` root detection now actually works during workspace discovery.
- **Worktree-aware symbol cache invalidation**: Workspace snapshots now preserve worktree/branch scoping and refresh correctly for modified or deleted files.

## [0.6.1] - 2026-03-26

### Added

- **Host-aware MCP setup output**: `rhizome init` now supports `--editor <host>` and can print paste-ready JSON or TOML snippets for Claude Code, Codex CLI, Cursor-family editors, and other supported MCP hosts.
- **Per-host doctor repair guidance**: `rhizome doctor` now gives host-specific next steps when MCP registration is missing, including the right `rhizome init --editor ...` command for each detected host.

### Changed

- **Platform-aware path resolution**: Rhizome now uses shared `spore` path/config helpers for global config loading, managed LSP bin directories, and path reporting instead of hand-rolled Unix-shaped logic.
- **Windows-safe managed PATH**: LSP installer PATH augmentation now uses platform-safe path joining instead of hardcoded `:` separators.
- **Multi-host MCP diagnostics**: `rhizome doctor` now inspects detected editor/host configs, including Codex TOML config, rather than only checking Claude Code CLI registration.
- **Spore v0.4.3**: Rhizome now targets the current released `spore` tag.

### Fixed

- **Pip fallback portability**: Rhizome now retries Python package installation without `--break-system-packages` when that flag is unsupported, avoiding a Linux-specific failure mode on other platforms.

## [0.6.0] - 2026-03-23

### Added

- **Symbol copy and move workflows**: Added `copy_symbol` and `move_symbol` MCP tools for whole-symbol edits across files, and completed end-to-end `rename_symbol` workspace-edit application through the LSP pipeline.

### Fixed

- **Worktree-aware export cache**: Export cache keys now include git context so separate branches and worktrees do not reuse stale export state.
- **LSP startup noise tolerance**: The LSP client now ignores noisy stdout preambles before the first JSON-RPC payload instead of failing startup.
- **Hyphae export compatibility**: Export parsing now accepts current Hyphae response shapes and compact summaries, and graph merges drop invalid edges before import.
- **Released Spore dependency**: Rhizome now depends on the released `spore` `v0.4.2` tag instead of a raw git revision.

## [0.5.4] - 2026-03-22

### Fixed

- **Path traversal security**: `resolve_path` now canonicalizes parent directories for non-existent files. Previously, `../` sequences in new file paths bypassed the project root check.
- **Mutex poison safety**: Replaced 5 `lock().unwrap()` calls with poison-safe alternatives in LSP client and parse cache. A panic in one request no longer permanently crashes the server.
- **LSP manager entry API**: Replaced insert-then-unwrap pattern with `HashMap::entry()`, eliminating a potential panic if the key disappeared between operations.
- **Probe server logging**: Installation and probing failures are now logged at `warn!` level instead of silently swallowed.
- **Deprecated annotations**: Added `reason` attributes to 3 `#[allow(deprecated)]` sites.

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
