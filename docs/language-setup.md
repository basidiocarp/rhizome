# Language Setup Guide

Three paths to get Rhizome working with your language: out-of-the-box tree-sitter (zero setup), LSP with auto-install, or manual LSP configuration.

## Quick Start

**Your language here? Check this first:**

```bash
rhizome status
```

Shows a status table with `Tree-Sitter`, `LSP Server`, and `Status` columns. `Tree-Sitter` is `active` for languages with built-in tree-sitter support and `n/a` for LSP-only languages. `Status` is `available (...)` when the LSP binary is found and `not found` when it is missing.

## Path 1: Out-of-the-Box Languages (Tree-Sitter)

These languages work immediately with zero setup in the default build:

| Language | Works With | Example |
|----------|-----------|---------|
| Rust | `get_symbols`, `get_structure`, `find_references` | Yes, full support |
| Python | All tree-sitter tools | Yes, full support |
| JavaScript | All tree-sitter tools | Yes, full support |
| TypeScript | All tree-sitter tools | Yes, full support |
| Go | All tree-sitter tools | Yes, full support |
| Java | All tree-sitter tools | Yes, full support |
| C | All tree-sitter tools | Yes, full support |
| C++ | All tree-sitter tools | Yes, full support |
| Ruby | All tree-sitter tools | Yes, full support |
| PHP | All tree-sitter tools | Yes, full support |
| Bash | All tree-sitter tools (dedicated query) | Yes, full support |
| Elixir | All tree-sitter tools (dedicated query) | Yes, full support |
| Lua | All tree-sitter tools (dedicated query) | Yes, full support |
| Zig | All tree-sitter tools (dedicated query) | Yes, full support |

"Full support" means precise symbol extraction via dedicated query patterns or built-in tree-sitter extraction. Query patterns use language-specific grammar rules to extract exact symbol locations, kinds, and signatures. The generic fallback walks the AST and matches common node types like `function_definition` and `class_declaration`—useful, but less precise.

### optional grammar pack

The default build leaves the heaviest niche grammars out of the shipped
tree-sitter set to keep compile time and binary size down. If you need offline
tree-sitter support for these languages, build with
`rhizome-treesitter/lang-all`.

| Language | Default build | `lang-all` build |
|----------|---------------|------------------|
| C# | LSP-backed by default | Tree-sitter + LSP |
| Swift | LSP-backed by default | Tree-sitter + LSP |
| Haskell | LSP-backed by default | Tree-sitter + LSP |

## Path 2: LSP Languages (Auto-Install)

Rhizome has a built-in LSP server mapping for each of these languages. Some support auto-install on first use; others require manual setup:

| Language | LSP Server | Install Via | Auto-Install | Note |
|----------|-----------|-------------|--------------|------|
| Rust | rust-analyzer | rustup | Yes | `rustup component add rust-analyzer` |
| Python | pyright-langserver | pipx/pip | Yes | Installs to Rhizome's managed bin dir |
| JavaScript | typescript-language-server | npm | Yes | Includes TypeScript |
| TypeScript | typescript-language-server | npm | Yes | Includes JavaScript |
| Go | gopls | go | Yes | `go install golang.org/x/tools/gopls@latest` |
| Java | jdtls | Not auto-installed | No | Requires manual setup |
| C/C++ | clangd | Not auto-installed | No | Usually via system package manager |
| Ruby | ruby-lsp | gem | Yes | `gem install ruby-lsp` |
| Elixir | elixir-ls | mix | Yes | `mix archive.install hex elixir_ls` |
| PHP | phpactor | npm or composer | Yes | Via npm: `npm install -g @phpactor/language-server` |
| C# | csharp-ls | dotnet | Yes | `dotnet tool install -g csharp-ls` |
| F# | fsautocomplete | dotnet | Yes | `dotnet tool install -g fsautocomplete` |
| Swift | sourcekit-lsp | Xcode/Swift | Yes | Ships with Xcode |
| Bash | bash-language-server | npm | Yes | `npm install -g bash-language-server` |
| Terraform | terraform-ls | brew or download | Yes | `brew install terraform-ls` |
| Lua | lua-language-server | brew or download | Yes | `brew install lua-language-server` |
| Clojure | clojure-lsp | brew | Yes | `brew install clojure-lsp` |
| OCaml | ocamllsp | opam | Yes | `opam install ocaml-lsp-server` |
| Haskell | haskell-language-server-wrapper | ghcup | Yes | `ghcup install hls` |
| Nix | nixd | nix-env | Yes | `nix-env -iA nixpkgs.nixd` |
| Vue | vue-language-server | npm | Yes | `npm install -g @vue/language-server` |
| Svelte | svelteserver | npm | Yes | `npm install -g svelte-language-server` |
| Astro | astro-ls | npm | Yes | `npm install -g @astrojs/language-server` |
| Prisma | prisma-language-server | npm | Yes | `npm install -g @prisma/language-server` |
| Typst | tinymist | cargo | Yes | `cargo install tinymist` |
| YAML | yaml-language-server | npm | Yes | `npm install -g yaml-language-server` |

### How Auto-Install Works

When you call a tool that requires LSP (e.g., `rename_symbol`):

1. Rhizome checks if the LSP server binary is in PATH
2. If found, use it
3. If not found, attempt auto-install:
   - Look up install recipe (e.g., `rustup component add rust-analyzer`)
   - Check if package manager is available (e.g., `rustup`)
   - If yes, run install command
   - If yes, binary is placed in the managed bin dir (added to PATH)
4. If install fails or is disabled, LSP-required tools return an install hint and LSP-preferred tools fall back to tree-sitter

**Example: First time calling rename_symbol on Rust**

```bash
rhizome_mcp <- {"jsonrpc": "2.0", "method": "tools/call_tool", ...}
  ├─ Backend selection: rename_symbol requires LSP
  ├─ Check: rust-analyzer in PATH? No
  ├─ Auto-install: run `rustup component add rust-analyzer`
  ├─ Verify: rust-analyzer now in PATH? Yes
  └─ Execute tool
```

**Second call**: Binary already in PATH, no install.

### Disable Auto-Install

Set environment variable before starting Rhizome:

```bash
# Disable all auto-install
export RHIZOME_DISABLE_LSP_DOWNLOAD=1

# Or in config
<platform config dir>/rhizome/config.toml:
[lsp]
disable_download = true
```

When disabled, missing servers fail with install hint.

### Verify Installation

After install, verify server works:

```bash
# Check if binary is in PATH
which rust-analyzer
which pyright-langserver

# Check the managed bin dir reported by `rhizome status`
ls -la "<managed bin dir>"

# Run rhizome status
rhizome status
```

**Status output** shows per-language:
- `Tree-Sitter`: `active` or `n/a`
- `LSP Server`: binary name
- `Status`: `available (<path>)` or `not found`

## Path 3: Custom LSP Configuration

When auto-install doesn't work, configure the server manually.

### Edit Config

**Global config**: `<platform config dir>/rhizome/config.toml`

```toml
[languages.java]
server_binary = "jdtls"
server_args = ["-configuration", "/path/to/config", "-data", "<workspace dir>"]
enabled = true
```

**Project config** (overrides global): `<project>/.rhizome/config.toml`

```toml
[languages.rust]
server_binary = "<absolute path to rust-analyzer>"
server_args = ["--log-file", "<log file path>"]
enabled = true

[lsp]
bin_dir = "<managed bin dir>"  # Where auto-install places servers
disable_download = true        # Disable auto-install for this project
```

### Configuration Options

| Option | Type | Default | Purpose |
|--------|------|---------|---------|
| `[languages.<lang>].server_binary` | String | Language default | Path or name of LSP server binary |
| `[languages.<lang>].server_args` | Array | Language default | Arguments passed to server (e.g., `["--stdio"]`) |
| `[languages.<lang>].enabled` | Boolean | true | Enable/disable language entirely |
| `[languages.<lang>].initialization_options` | JSON | None | Custom LSP init options (language-specific) |
| `[lsp].disable_download` | Boolean | false | Disable auto-install of missing servers |
| `[lsp].bin_dir` | Path | `<managed bin dir>` | Directory where auto-installed servers go |
| `[export].auto_export` | Boolean | true | Auto-export symbols to Hyphae on startup |

### Custom Server: Java (JDTLS)

JDTLS requires manual setup:

1. Download Eclipse Adoptium JDK 17+
2. Install JDTLS:

```bash
mkdir -p <jdtls workspace dir>
cd <jdtls workspace dir>
git clone https://github.com/eclipse/eclipse.jdt.ls.git
cd eclipse.jdt.ls
./mvnw clean package -DskipTests=true
# Output: org.eclipse.jdt.ls.product/target/repository/
```

3. Create config:

```toml
# <platform config dir>/rhizome/config.toml
[languages.java]
server_binary = "java"
server_args = [
    "-agentlib:jdwp=transport=dt_socket,server=y,suspend=n,address=1044",
    "-Declipse.application=org.eclipse.jdt.ls.core.id1",
    "-Dosgi.bundles.defaultStartLevel=4",
    "-Declipse.product=org.eclipse.jdt.ls.core.product",
    "-Dlog.protocol=false",
    "-Dlog.level=WARNING",
    "-noverify",
    "-Xmx1G",
    "-jar", "/path/to/jdtls/org.eclipse.jdt.ls.product/target/repository/plugins/org.eclipse.jdt.ls.core_VERSION.jar",
    "-configuration", "<platform-specific jdtls config dir>",
    "-data", "<workspace dir>"
]
enabled = true
```

4. Test:

```bash
rhizome symbols <java-file>
rhizome status
```

### Custom Server: C/C++ (Clangd)

Clangd usually comes from system package manager:

```bash
# macOS
brew install llvm

# Linux
apt-get install clangd  # or similar for your distro

# Add to config if not auto-detected
# <platform config dir>/rhizome/config.toml
[languages.c]
server_binary = "clangd"
enabled = true

[languages.cpp]
server_binary = "clangd"
enabled = true
```

## Disable a Language

If a language is causing issues, disable it entirely:

```toml
# <platform config dir>/rhizome/config.toml
[languages.java]
enabled = false
```

Rhizome will skip initialization for that language.

## Troubleshooting

### "Unsupported extension"

File type not recognized. Check Language enum:

```bash
rhizome status  # Lists all 32 languages
```

Example: `.ts` → TypeScript. If you have `.ts` files but see "unsupported," file an issue.

### "No tree-sitter grammar"

Language has no query pattern. Either:
1. Use LSP (if available)
2. Use generic fallback (basic symbol extraction)
3. Contribute a query pattern

### "LSP server not found: rust-analyzer"

Server binary missing. Auto-install failed because:
- Package manager not in PATH (e.g., `rustup` not found)
- Auto-install disabled: `RHIZOME_DISABLE_LSP_DOWNLOAD=1`
- Install failed silently (check logs)

**Fix**: Install manually using hint shown by Rhizome.

### "Auto-install disabled"

You set `RHIZOME_DISABLE_LSP_DOWNLOAD=1`. Unset to allow:

```bash
unset RHIZOME_DISABLE_LSP_DOWNLOAD
```

Or edit config to set `[lsp].disable_download = false`.

### LSP server crashes

Check logs and increase verbosity:

```bash
# With logging
RHIZOME_LOG=debug rhizome serve

# Check LSP server logs (server-specific)
# Example Rust: inspect the configured log file path
cat "<configured log file path>"
```

### Tool works with tree-sitter but not LSP

Example: `get_symbols` works, but `rename_symbol` fails. Check:
1. LSP server is running: `rhizome status`
2. Server was initialized: Check LSP logs
3. File path is absolute and correct

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `RHIZOME_DISABLE_LSP_DOWNLOAD` | Disable auto-install (`1` to disable) | Unset (auto-install enabled) |
| `RHIZOME_PROJECT` | Override project root detection | Unset (auto-detect) |
| `RHIZOME_LOG` | Primary logging level or filter override | Unset (warn) |
| `RUST_LOG` | Fallback logging level or filter override | Unset |

## Next Steps

1. **Check status**: `rhizome status`
2. **For LSP-only languages**: Set up custom config (see Path 3)
3. **Test a tool**: `rhizome symbols <file>`
4. **Export to Hyphae**: `rhizome export --project <project>`

See [troubleshooting.md](./troubleshooting.md) for common issues and fixes.
