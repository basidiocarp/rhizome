# Changelog

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
