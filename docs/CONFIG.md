# Configuration Reference

Complete guide to Rhizome configuration. Settings apply to symbol extraction, LSP servers, export behavior, and project root detection.

## Configuration Files

Rhizome loads configuration from two sources, with project config overriding global:

| Scope | Path | Purpose |
|-------|------|---------|
| Global | `<platform config dir>/rhizome/config.toml` | System-wide defaults for all projects |
| Project | `<project_root>/.rhizome/config.toml` | Project-specific overrides |

Typical global config locations:
- macOS: `~/Library/Application Support/rhizome/config.toml`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/rhizome/config.toml`
- Windows: `%APPDATA%\rhizome\config.toml`

**Loading order:**
1. Load global config
2. Load project config (if exists)
3. Apply environment variables (highest priority)
4. For per-language settings, project values override global

## Configuration Sections

### [languages.*]

Per-language LSP server configuration.

```toml
[languages.rust]
server_binary = "rust-analyzer"          # LSP server binary name or path
server_args = []                         # Arguments passed to server
enabled = true                           # Enable/disable this language
initialization_options = { }             # LSP-specific init options (JSON)
```

#### Language Keys

Keys are lowercase language names (match `Language::from_name()`):

```toml
[languages.rust]        # Rust
[languages.python]      # Python
[languages.javascript]  # JavaScript
[languages.typescript]  # TypeScript
[languages.go]          # Go
[languages.java]        # Java
[languages.c]           # C
[languages.cpp]         # C++
[languages.ruby]        # Ruby
[languages.elixir]      # Elixir
[languages.zig]         # Zig
[languages.csharp]      # C#
[languages.fsharp]      # F#
[languages.swift]       # Swift
[languages.php]         # PHP
[languages.haskell]     # Haskell
[languages.bash]        # Bash
[languages.terraform]   # Terraform
[languages.kotlin]      # Kotlin
[languages.dart]        # Dart
[languages.lua]         # Lua
[languages.clojure]     # Clojure
[languages.ocaml]       # OCaml
[languages.julia]       # Julia
[languages.nix]         # Nix
[languages.gleam]       # Gleam
[languages.vue]         # Vue
[languages.svelte]      # Svelte
[languages.astro]       # Astro
[languages.prisma]      # Prisma
[languages.typst]       # Typst
[languages.yaml]        # YAML
```

#### server_binary

LSP server binary name or path.

- **Name**: System looks for the binary in `PATH` and in Rhizome's managed bin directory
  ```toml
  server_binary = "rust-analyzer"
  # Looks in: PATH, then the managed bin dir from [lsp].bin_dir
  ```

- **Path**: Absolute path to binary
  ```toml
  server_binary = "/opt/custom/rust-analyzer"
  # Uses exact path
  ```

- **Default**: Language-specific (auto-detected if not specified)

#### server_args

Arguments passed to the LSP server on startup.

```toml
[languages.rust]
server_args = []                    # No args

[languages.python]
server_args = ["--stdio"]           # Single arg

[languages.typescript]
server_args = ["--stdio", "--log=verbose"]  # Multiple args
```

- **Default**: Language-specific defaults (see [ARCHITECTURE.md](./ARCHITECTURE.md))
- **Note**: Some servers require `--stdio` for JSON-RPC over stdin/stdout

Common args by language:

| Language | Typical Args | Purpose |
|----------|--------------|---------|
| Rust | `[]` | No args needed |
| Python | `["--stdio"]` | Enable stdio mode |
| TypeScript | `["--stdio"]` | Enable stdio mode |
| Go | `[]` | No args needed |
| Bash | `["start"]` | Start server mode |
| Terraform | `["serve"]` | Serve mode |

#### enabled

Enable or disable a language entirely.

```toml
[languages.java]
enabled = false          # Rhizome skips Java entirely

[languages.python]
enabled = true           # Use Python (default)
```

- **Default**: `true`
- **Effect**: Disabled languages don't initialize server, don't respond to tools
- **Use case**: Skip problematic languages, reduce startup time

#### initialization_options

Custom LSP server initialization options (JSON object).

```toml
[languages.rust]
initialization_options = {
    "checkOnSave" = { "command" = "clippy" },
    "assist" = { "importMergeBehavior" = "crate" }
}
```

- **Type**: JSON object (TOML table)
- **Default**: Language-specific (if any)
- **Scope**: Sent to server in LSP `initialize` request
- **Use case**: Configure language-server behavior (linting, formatting, etc.)

Example: Rust-analyzer with clippy checks:

```toml
[languages.rust]
initialization_options = {
    "checkOnSave" = {
        "command" = "clippy",
        "extraArgs" = ["--all-targets", "--all-features"]
    }
}
```

### [lsp]

LSP-wide configuration.

```toml
[lsp]
disable_download = false             # Allow auto-install of servers
bin_dir = "<managed bin dir>"        # Custom directory for managed binaries
```

#### disable_download

Disable automatic LSP server installation.

- **Default**: `false` (auto-install enabled)
- **Values**: `true` or `false`
- **Override**: Environment variable `RHIZOME_DISABLE_LSP_DOWNLOAD=1` (takes precedence)

```toml
[lsp]
disable_download = true              # Manual install only
```

When disabled:
- Auto-install recipes are skipped
- Missing servers fail with install hint
- Speeds up startup (no install attempts)

#### bin_dir

Directory where auto-installed LSP servers are placed.

- **Default**: Rhizome's platform data dir, under `rhizome/bin`
- **Type**: Path (can be absolute or relative to home)
- **Must exist**: Rhizome creates it if missing
- **Added to PATH**: Auto-installed binaries are in this dir
- **Tip**: Run `rhizome status` to see the resolved managed bin dir on the current machine

```toml
[lsp]
bin_dir = "/opt/rhizome/bin"         # Custom location
```

### [export]

Hyphae export configuration.

```toml
[export]
auto_export = true                   # Auto-export symbols on startup
```

#### auto_export

Automatically export code symbols to Hyphae when MCP server starts.

- **Default**: `true`
- **Values**: `true` or `false`
- **Effect on startup**: If `true`, Rhizome sends all extracted symbols to Hyphae
- **Effect on file changes**: Incremental updates (if Hyphae notifies of changes)

```toml
[export]
auto_export = false                  # Manual export only (via export_to_hyphae tool)
```

## Environment Variables

Environment variables override config file settings (highest priority).

Examples below use POSIX shell syntax. On PowerShell, set environment variables with `$env:NAME = "value"` and clear them with `Remove-Item Env:NAME`.

| Variable | Type | Purpose | Example |
|----------|------|---------|---------|
| `RHIZOME_DISABLE_LSP_DOWNLOAD` | String | Disable auto-install (`1` or `true`) | `RHIZOME_DISABLE_LSP_DOWNLOAD=1` |
| `RHIZOME_PROJECT` | Path | Override project root detection | `RHIZOME_PROJECT=/path/to/project` |
| `RUST_LOG` | String | Logging level (debug, info, warn, error) | `RUST_LOG=rhizome=debug` |

### RHIZOME_DISABLE_LSP_DOWNLOAD

Disable automatic LSP server installation.

```sh
# Disable auto-install
export RHIZOME_DISABLE_LSP_DOWNLOAD=1
rhizome serve

# Or inline
RHIZOME_DISABLE_LSP_DOWNLOAD=1 rhizome serve
```

- **Values**: `1`, `true` (case-insensitive), or any other value = enabled
- **Override**: Overrides config `[lsp].disable_download`
- **Use case**: Development, CI/CD, or when package manager unavailable

### RHIZOME_PROJECT

Override project root detection.

```sh
# Use custom project root
export RHIZOME_PROJECT=/opt/my-project
rhizome serve

# Or inline
RHIZOME_PROJECT=/opt/my-project rhizome serve
```

- **Type**: Absolute path
- **Use case**: Monorepo with non-standard structure, testing root detection
- **Effect**: All file operations use this as project root

### RUST_LOG

Set logging verbosity.

```sh
# All Rhizome logs at debug level
RUST_LOG=rhizome=debug rhizome serve

# Specific module
RUST_LOG=rhizome_mcp::tools=debug rhizome serve

# All dependencies at info level
RUST_LOG=info rhizome serve
```

- **Levels**: `error`, `warn`, `info`, `debug`, `trace`
- **Default**: `info`
- **Use case**: Debugging, performance analysis

## Default Values

Per-language defaults (used if not in config):

### Rust
```toml
[languages.rust]
server_binary = "rust-analyzer"
server_args = []
enabled = true
```

### Python
```toml
[languages.python]
server_binary = "pyright-langserver"
server_args = ["--stdio"]
enabled = true
```

### JavaScript / TypeScript
```toml
[languages.javascript]
server_binary = "typescript-language-server"
server_args = ["--stdio"]
enabled = true

[languages.typescript]
server_binary = "typescript-language-server"
server_args = ["--stdio"]
enabled = true
```

### Go
```toml
[languages.go]
server_binary = "gopls"
server_args = []
enabled = true
```

### Java
```toml
[languages.java]
server_binary = "jdtls"
server_args = []
enabled = true
```

### C / C++
```toml
[languages.c]
server_binary = "clangd"
server_args = []
enabled = true

[languages.cpp]
server_binary = "clangd"
server_args = []
enabled = true
```

### Ruby
```toml
[languages.ruby]
server_binary = "ruby-lsp"
server_args = []
enabled = true
```

### Elixir
```toml
[languages.elixir]
server_binary = "elixir-ls"
server_args = []
enabled = true
```

See [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md) for complete language server defaults.

## Example Configurations

### Minimal (Global)

Single global config for Rust + Python:

```toml
# <platform config dir>/rhizome/config.toml
[languages.python]
server_binary = "pyright-langserver"
```

Rest defaults apply. Python uses custom binary, others use defaults.

### Comprehensive (Global)

Full customization:

```toml
# <platform config dir>/rhizome/config.toml

[languages.rust]
server_binary = "rust-analyzer"
server_args = []
enabled = true
initialization_options = {
    "checkOnSave" = { "command" = "clippy" }
}

[languages.python]
server_binary = "pyright-langserver"
server_args = ["--stdio"]
enabled = true

[languages.typescript]
server_binary = "typescript-language-server"
server_args = ["--stdio"]
enabled = true

[languages.go]
server_binary = "gopls"
server_args = []
enabled = true

[languages.java]
enabled = false  # Skip Java

[lsp]
disable_download = false
bin_dir = "<managed bin dir>"

[export]
auto_export = true
```

### Project Override

Project-specific config (overrides global):

```toml
# <project>/.rhizome/config.toml

# Override global Rust config with custom binary
[languages.rust]
server_binary = "/opt/custom/rust-analyzer"
server_args = ["--log-file", "<path-to-log-file>"]

# This project has no Python support
[languages.python]
enabled = false

[lsp]
bin_dir = "/opt/rhizome-bin"
```

Global Python config still applies (not overridden), but Rust uses custom binary and Python is disabled.

### Development (CI/CD)

Disable auto-install, fast startup:

```toml
# ci-config.toml
[lsp]
disable_download = true
```

Or via environment:
```sh
RHIZOME_DISABLE_LSP_DOWNLOAD=1 rhizome serve
```

### Performance-Tuned

Disable problematic languages, custom args:

```toml
[languages.java]
enabled = false  # jdtls slow to start

[languages.kotlin]
enabled = false  # No tree-sitter, LSP-only

[languages.rust]
server_args = ["--no-default-features"]  # Reduce memory
```

## Troubleshooting Configuration

### Config not being loaded

1. Check file exists and is readable:
   ```sh
   cat "<platform config dir>/rhizome/config.toml"
   ```

2. Validate TOML syntax:
   ```sh
   python3 -c "import tomllib; tomllib.loads(open('config.toml').read())"
   ```

3. Check Rhizome is reading it:
   ```sh
   RUST_LOG=debug rhizome serve 2>&1 | grep config
   ```

### Project config not overriding global

1. Verify project root is detected:
   ```sh
   RUST_LOG=debug rhizome serve 2>&1 | grep "project.root"
   ```

2. Verify project config exists:
   ```sh
   cat <project>/.rhizome/config.toml
   ```

3. Check language keys are lowercase (e.g., `rust`, not `Rust`)

### Environment variable not taking effect

1. Verify variable is set:
   ```sh
   printenv RHIZOME_DISABLE_LSP_DOWNLOAD
   ```

2. Verify it's exported (for subprocesses):
   ```sh
   export RHIZOME_DISABLE_LSP_DOWNLOAD=1
   ```

3. Restart Rhizome after changing:
   ```sh
   RHIZOME_DISABLE_LSP_DOWNLOAD=1 rhizome serve
   ```

## Priority Order (Highest to Lowest)

1. **Environment variables** (e.g., `RHIZOME_DISABLE_LSP_DOWNLOAD=1`)
2. **Project config** (`<project>/.rhizome/config.toml`)
3. **Global config** (`<platform config dir>/rhizome/config.toml`)
4. **Built-in defaults** (Language enum defaults)

Example: If `server_binary = "custom-ra"` in project config, but `server_binary = "rust-analyzer"` in global config, the project value wins.
