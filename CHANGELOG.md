# Changelog

All notable changes to Rhizome are documented in this file.

## [Unreleased]

## [0.7.7] - 2026-04-08

### Changed

- **Foundation alignment**: README, config docs, troubleshooting, and
  architecture notes now describe backend and transport boundaries more
  explicitly.
- **Boundary verification**: added focused coverage for backend selection and
  repo-level architectural contracts.

### Fixed

- **Tracing depth**: non-serve CLI flows, installer boundaries, and LSP startup
  paths now enter the shared root, workflow, and subprocess spans more
  consistently.
- **Operator docs**: runtime docs now match `RHIZOME_LOG` defaults and the real
  warning-level behavior of the touched paths.

## [0.7.6] - 2026-04-08

### Fixed

- **Logging contract docs now match runtime behavior**: README and operational
  docs now document `RHIZOME_LOG` as the primary knob, `RUST_LOG` as fallback,
  and `warn` as the default level.
- **Broader tracing around fragile runtime boundaries**: installer and Hyphae
  export paths now enter shared tool, workflow, and subprocess spans so
  subprocess failures are easier to localize.
- **Consistent CLI root spans**: non-serve command paths now enter the shared
  root/workflow tracing boundary instead of only instrumenting `serve`.

## [0.7.5] - 2026-04-08

### Changed

- **Shared Spore logging rollout**: Rhizome now consumes `spore v0.4.9`,
  initializes logging with the app-aware path, and adds shared root, request,
  tool, and workflow spans around MCP serve and auto-export flows.
- **Runtime compatibility with non-exhaustive Spore editors**: Doctor now
  handles future `spore::editors::Editor` variants without failing to compile.
- **Accurate MCP version reporting**: `initialize` now reports the real MCP
  crate version instead of the old hard-coded `0.4.0` string.

### Changed

- **Docs cleanup**: Internal docs moved under `docs/`, and the README,
  architecture, and language-setup docs were refreshed to match the current
  backend model.

## [0.7.2] - 2026-04-01

### Fixed

- **Release-gating for macOS binaries**: Apple release builds now fail on
  tree-sitter smoke-test or MCP initialize regressions instead of publishing
  artifacts after advisory-only checks.
- **macOS build diagnostics**: Release workflows now emit verbose native
  compiler logs, pin the SDK and deployment target, re-sign the binary, and
  upload diagnostics when the Apple build path fails.

## [0.7.0] - 2026-03-31

### Added

- **Broader export language coverage**: Hyphae export now follows Rhizome's
  wider language map instead of truncating code-graph export to a narrow
  extension subset.

### Changed

- **Canonical export identity**: Hyphae export now derives project and worktree
  identity from canonical roots and emits the published `code-graph-v1`
  envelope.
- **Stricter contract handling**: Doctor and export paths now validate the real
  Hyphae import contract and stop relying on older compatibility shims.

### Fixed

- **Auto-export resilience**: Background export now retries with backoff and
  escalates persistent failures at warning level instead of failing silently.
- **Doctor signal quality**: Hyphae health checks now probe the actual import
  path, respect ignore-aware language scanning, and report more realistic
  export readiness.

## [0.6.2] - 2026-03-27

### Added

- **Rename preview and impact analysis**: `rename_symbol` now supports dry-run
  preview, and `analyze_impact` summarizes grouped references, callers,
  callees, tests, and backend confidence.
- **Scope-aware symbol identity**: Rhizome now emits `qualified_name` and
  `stable_id` metadata so nested symbols and methods keep container context.
- **Persistent workspace symbol snapshots**: Tree-sitter workspace search now
  persists scoped symbol indexes under `.rhizome/`.

### Changed

- **Workspace cache hardening**: Persistent workspace snapshots now use schema
  versioning and stronger file fingerprints instead of trusting timestamps
  alone.
- **Scoped project state paths**: Project-local cache and config state now flow
  through shared `.rhizome/` path helpers.

### Fixed

- **Hyphae export resilience**: Export now resolves relative roots against the
  project root, reports partial failures more clearly, and tolerates unreadable
  cache state better.
- **Wildcard root markers**: Haskell `*.cabal` and OCaml `*.opam` root
  detection now works during workspace discovery.
- **Worktree-aware cache invalidation**: Workspace snapshots now preserve
  branch and worktree scoping and refresh correctly for modified or deleted
  files.

## [0.6.1] - 2026-03-26

### Added

- **Host-aware MCP setup output**: `rhizome init` now supports `--editor
  <host>` and can print paste-ready config snippets for Claude Code, Codex CLI,
  Cursor-family editors, and other supported hosts.
- **Per-host doctor guidance**: `rhizome doctor` now gives host-specific next
  steps when MCP registration is missing.

### Changed

- **Platform-aware path resolution**: Rhizome now uses shared Spore helpers for
  config loading, managed LSP bin directories, and path reporting.
- **Windows-safe managed PATH**: LSP installer PATH augmentation now uses
  platform-safe path joining.
- **Multi-host diagnostics**: `rhizome doctor` now inspects detected editor and
  host configs instead of only checking Claude Code registration.

### Fixed

- **Pip fallback portability**: Python package installation now retries without
  `--break-system-packages` when that flag is unsupported.

## [0.6.0] - 2026-03-23

### Added

- **Symbol copy and move workflows**: Added `copy_symbol` and `move_symbol`
  MCP tools for whole-symbol edits across files.
- **End-to-end rename application**: `rename_symbol` now completes workspace
  edits through the LSP pipeline.

### Fixed

- **Worktree-aware export cache**: Export cache keys now include git context so
  separate branches and worktrees do not reuse stale state.
- **LSP startup noise tolerance**: The LSP client now ignores noisy stdout
  preambles before the first JSON-RPC payload.
- **Hyphae export compatibility**: Export parsing now accepts current Hyphae
  response shapes and compact summaries, and graph merges drop invalid edges
  before import.

## [0.5.4] - 2026-03-22

### Fixed

- **Path traversal security**: `resolve_path` now canonicalizes parent
  directories for non-existent files so `../` sequences cannot bypass the
  project-root check.
- **Mutex poison safety**: LSP client and parse-cache locks now recover safely
  after a panic instead of crashing the server permanently.
- **LSP manager entry handling**: `HashMap::entry()` replaced an
  insert-then-unwrap path that could panic under edge conditions.
- **Probe server logging**: Install and probe failures now log warnings instead
  of disappearing silently.

## [0.5.3] - 2026-03-21

### Added

- **Parse-tree LRU cache**: Added a process-wide shared cache for tree-sitter
  parsed trees, keyed by file path and mtime, to speed up repeated file access.

### Changed

- **Shared Spore runtime**: Self-update and logging now use shared Spore
  modules.

## [0.4.3] - 2026-03-18

### Added

- **Editing MCP tools**: Added targeted file-editing tools such as
  `replace_symbol_body`, `insert_after_symbol`, `replace_lines`, `delete_lines`,
  and `create_file`.
- **Project summarization**: Added `summarize_project`, the matching CLI
  command, and `rhizome_onboard` for structured orientation.
- **Doctor command**: Added runtime health checks for grammars, LSP servers,
  Hyphae connectivity, and config.
- **Broader query coverage**: Extended language-specific tree-sitter queries to
  Java, C, C++, Ruby, and PHP.

## [0.4.0] - 2026-03-16

### Added

- **32-language support**: Added language-server configs, extension mappings,
  root markers, and graph metadata for a much broader language set.
- **Auto-install LSP servers**: `LspInstaller` can now download missing servers
  through native package managers and install them under `~/.rhizome/bin/`.
- **Backend auto-selection**: `BackendSelector` now maps each tool to
  tree-sitter, prefers-LSP, or requires-LSP behavior.
- **Smart root detection**: Rust, Go, JS, TS, and Python now resolve project
  roots through language-aware rules before falling back to `.git`.
- **Multi-client LSP management**: `LanguageServerManager` now supports
  multiple workspace roots in monorepos.
- **Status surface**: Added `rhizome status` and the `LspConfig` surface for
  install and availability reporting.

### Changed

- **Default server choices**: PHP now defaults to `phpactor`, and Ruby now
  defaults to `ruby-lsp`.
- **Lazy LSP initialization**: `ToolDispatcher` now combines backend selection
  with lazy LSP startup.
- **Config-aware install hints**: Install guidance now reflects the actual
  configured server binary.

### Fixed

- **TypeScript query compilation**: The `.ts` class query now uses the correct
  `type_identifier` field.
- **Hyphae export transport**: Export now uses Spore discovery and
  line-delimited JSON-RPC.
- **Export parameter shape**: `export_graph` params now match Hyphae's expected
  format.

## [0.3.0] - 2026-03-16

### Added

- **Code graph construction**: Rhizome can now build typed concept graphs from
  extracted symbols and merge them across files.
- **Hyphae export**: Added export to Hyphae over JSON-RPC stdio, plus the
  `export_to_hyphae` MCP tool and `rhizome export` CLI command.
- **Incremental export cache**: Added mtime-based file change tracking under
  `.rhizome/cache.json`.
- **Auto-export on startup**: MCP startup can now export the code graph in the
  background when Hyphae is available.

## [0.2.0] - 2026-02-28

### Added

- **MCP server**: Rhizome shipped with 25 code-intelligence tools across
  unified and expanded modes.
- **Dual backend model**: Tree-sitter covered structure work, and the LSP
  backend covered cross-file references, rename, and type information.
- **CLI surface**: Added `serve` and `analyze` commands.
- **Initial language set**: Shipped with Rust, Python, JavaScript, TypeScript,
  Go, Java, C, C++, and Ruby.
- **Release infrastructure**: Added the initial CI and release workflows.

## [0.1.0] - 2026-02-14

### Added

- **Workspace foundation**: Rhizome shipped as a 5-crate workspace.
- **Core domain types**: Added `Symbol`, `SymbolKind`, `Location`, and
  `Language`.
- **Code intelligence trait**: Added the initial `CodeIntelligence` abstraction
  with six core operations.
