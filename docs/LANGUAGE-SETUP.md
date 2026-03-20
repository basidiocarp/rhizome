# Language Setup Guide

How to get Rhizome working with your language. Three paths: out-of-the-box (tree-sitter), LSP with auto-install, or manual LSP configuration.

## Quick Start

**Your language here? Check this first:**

```bash
rhizome status
```

Shows language support + LSP server status. Green = ready. Yellow = need to install server.

## Path 1: Out-of-the-Box Languages (Tree-Sitter)

These languages work immediately with zero setup:

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
| Bash | Basic symbol extraction (generic fallback) | Limited |
| C# | Basic symbol extraction (generic fallback) | Limited |
| Elixir | Basic symbol extraction (generic fallback) | Limited |
| Lua | Basic symbol extraction (generic fallback) | Limited |
| Swift | Basic symbol extraction (generic fallback) | Limited |
| Zig | Basic symbol extraction (generic fallback) | Limited |
| Haskell | Basic symbol extraction (generic fallback) | Limited |
| TOML | Basic symbol extraction (generic fallback) | Limited |

"Full support" = precise symbol extraction via query patterns. "Limited" = generic fallback extracts functions, classes, imports.

**What's the difference?**

- **Query patterns** (full): Language-specific grammar rules extract exact symbol locations, kinds, signatures
- **Generic fallback** (limited): Walks AST, matches common node types (function_definition, class_declaration)

## Path 2: LSP Languages (Auto-Install)

For these languages, Rhizome auto-installs the LSP server on first use:

| Language | LSP Server | Install Via | Auto-Install | Note |
|----------|-----------|-------------|--------------|------|
| Rust | rust-analyzer | rustup | Yes | `rustup component add rust-analyzer` |
| Python | pyright-langserver | pipx/pip | Yes | Installs to `~/.rhizome/bin/` |
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
   - If yes, binary placed in `~/.rhizome/bin/` (added to PATH)
4. If install fails or disabled, error with install hint

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
~/.config/rhizome/config.toml:
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

# Check ~/.rhizome/bin/ for auto-installed binaries
ls -la ~/.rhizome/bin/

# Run rhizome status
rhizome status
```

**Status output** shows per-language:
- ✓ Tree-sitter available: Yes/No
- ✓ LSP binary: Name (e.g., rust-analyzer)
- ✓ LSP available: Yes/No / Path to binary

## Path 3: Custom LSP Configuration

For languages where auto-install doesn't work, manually configure the server.

### Edit Config

**Global config**: `~/.config/rhizome/config.toml`

```toml
[languages.java]
server_binary = "jdtls"
server_args = ["-configuration", "/path/to/config", "-data", "/tmp/jdtls"]
enabled = true
```

**Project config** (overrides global): `<project>/.rhizome/config.toml`

```toml
[languages.rust]
server_binary = "/opt/custom/rust-analyzer"
server_args = ["--log-file", "/tmp/ra.log"]
enabled = true

[lsp]
bin_dir = "/opt/rhizome/bin"  # Where auto-install places servers
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
| `[lsp].bin_dir` | Path | `~/.rhizome/bin/` | Directory where auto-installed servers go |
| `[export].auto_export` | Boolean | true | Auto-export symbols to Hyphae on startup |

### Custom Server: Java (JDTLS)

JDTLS requires manual setup:

1. Download Eclipse Adoptium JDK 17+
2. Install JDTLS:

```bash
mkdir -p ~/jdtls
cd ~/jdtls
git clone https://github.com/eclipse/eclipse.jdt.ls.git
cd eclipse.jdt.ls
./mvnw clean package -DskipTests=true
# Output: org.eclipse.jdt.ls.product/target/repository/
```

3. Create config:

```toml
# ~/.config/rhizome/config.toml
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
    "-configuration", "/path/to/jdtls/org.eclipse.jdt.ls.product/target/repository/config_linux",
    "-data", "/tmp/eclipse-workspace"
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
# ~/.config/rhizome/config.toml
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
# ~/.config/rhizome/config.toml
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
RUST_LOG=debug rhizome serve

# Check LSP server logs (server-specific)
# Example Rust: /tmp/ra.log
cat /tmp/ra.log
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
| `RUST_LOG` | Logging level (debug, info, warn) | Unset (info) |

## Next Steps

1. **Check status**: `rhizome status`
2. **For LSP-only languages**: Set up custom config (see Path 3)
3. **Test a tool**: `rhizome symbols <file>`
4. **Export to Hyphae**: `rhizome export <project>`

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) for common issues and fixes.
