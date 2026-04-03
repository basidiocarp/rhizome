# LSP Server Guide

Reference for all language servers that Rhizome supports or can integrate with.
Covers installation, features, and configuration for each language.

## Quick Install

Most servers are npm-based. This installs the full recommended set:

```bash
# Core stack
rustup component add rust-analyzer
pipx install pyright
npm i -g typescript-language-server typescript

# Shell, config, and markup
npm i -g bash-language-server yaml-language-server vscode-langservers-extracted

# Web and frontend
npm i -g graphql-language-service-cli graphql @mdx-js/language-server

# Infrastructure
brew install hashicorp/tap/terraform-ls
npm i -g dockerfile-language-server-nodejs

# PHP (pick one)
composer global require phpactor/phpactor
# npm i -g intelephense     # alternative — faster, freemium

# Linting and formatting
npm i -g @biomejs/biome

# Optional
brew install lua-language-server
brew install taplo           # TOML schema-aware server
```

Rhizome auto-installs most of these on first use when the package manager is available. Installing upfront avoids the cold-start delay on first tool call.

---

## Language Server Reference

### Rust — rust-analyzer

| Field | Value |
|-------|-------|
| Binary | `rust-analyzer` |
| Install | `rustup component add rust-analyzer` |
| Command | `rust-analyzer` (stdio by default) |
| Features | Completions, diagnostics, hover, references, rename, go-to-definition, code actions, formatting (rustfmt), inlay hints, semantic tokens |
| Rhizome | Full tree-sitter queries + LSP auto-install |

Ships with rustup since Rust 1.64. Tree-sitter covers 80% of rhizome's tools without it, but rust-analyzer is needed for `find_references`, `rename_symbol`, and `get_hover_info`.

---

### TypeScript / JavaScript — typescript-language-server

| Field | Value |
|-------|-------|
| Binary | `typescript-language-server` |
| Install | `npm i -g typescript-language-server typescript` |
| Command | `typescript-language-server --stdio` |
| Peer dep | Requires `typescript` installed alongside |
| Features | Completions, diagnostics, hover, references, rename, go-to-definition, code actions, formatting |
| Rhizome | Full tree-sitter queries (including TSX variant) + LSP auto-install |

Wraps `tsserver` (TypeScript's compiler service) in an LSP interface. The standard choice for non-VS-Code editors. Handles both `.ts` and `.js` files.

---

### Python — pyright-langserver

| Field | Value |
|-------|-------|
| Binary | `pyright-langserver` |
| Install | `pipx install pyright` |
| Command | `pyright-langserver --stdio` |
| Features | Completions, diagnostics, hover, references, rename, go-to-definition, code actions, call hierarchy |
| Rhizome | Full tree-sitter queries + LSP auto-install |

Microsoft's Python type checker and language server. The `pyright` package includes both the CLI type checker and the LSP server binary.

Rhizome's installer also recognizes `pylsp` (community, plugin-based), `ruff` (linter/formatter), and `jedi-language-server` (lightweight) as alternatives.

---

### Bash — bash-language-server

| Field | Value |
|-------|-------|
| Binary | `bash-language-server` |
| Install | `npm i -g bash-language-server` |
| Command | `bash-language-server start` |
| Features | Completions, diagnostics (via shellcheck if installed), hover (man page excerpts), references, rename, go-to-definition (within file) |
| Rhizome | Full tree-sitter queries + LSP auto-install |

Tree-sitter queries handle function and variable extraction well. The LSP adds diagnostics (especially with shellcheck installed) and cross-reference support.

---

### PHP — phpactor / intelephense

**phpactor:**

| Field | Value |
|-------|-------|
| Binary | `phpactor` |
| Install | `composer global require phpactor/phpactor` |
| Command | `phpactor language-server` |
| Features | Completions, diagnostics, hover, references, rename, go-to-definition, code actions, formatting |
| Rhizome | Full tree-sitter queries + LSP auto-install |

**intelephense** (alternative):

| Field | Value |
|-------|-------|
| Binary | `intelephense` |
| Install | `npm i -g intelephense` |
| Command | `intelephense --stdio` |
| Features | Same as phpactor, plus faster indexing on large codebases |
| Note | Freemium — some features (rename, code actions) require a license key |
| Rhizome | Install recipe available |

phpactor is fully open source. intelephense is faster and more feature-complete but requires a license key for rename and code actions.

---

### YAML — yaml-language-server

| Field | Value |
|-------|-------|
| Binary | `yaml-language-server` |
| Install | `npm i -g yaml-language-server` |
| Command | `yaml-language-server --stdio` |
| Features | Completions (from JSON Schema), diagnostics, hover, formatting, document symbols |
| Rhizome | LSP only (no tree-sitter queries) |

Maintained by Red Hat. Supports JSON Schema Store for auto-completion of `docker-compose.yml`, GitHub Actions workflows, Kubernetes manifests, and other common YAML formats. Custom schema mapping is available via modeline comments (`# yaml-language-server: $schema=...`).

Atmos stack files (`.yaml`) are covered by this server.

---

### Terraform — terraform-ls

| Field | Value |
|-------|-------|
| Binary | `terraform-ls` |
| Install | `brew install hashicorp/tap/terraform-ls` |
| Command | `terraform-ls serve` |
| Features | Completions (providers, resources, attributes), diagnostics, hover, references, go-to-definition, formatting, semantic tokens |
| Rhizome | LSP only (no tree-sitter queries) |

HashiCorp's official language server. Provides provider-aware completions (AWS, GCP, Azure resource schemas), module references, and variable tracking. Requires the `terraform` CLI for provider schema fetching. Handles both `.tf` and `.tfvars` files.

---

### CSS — vscode-css-language-server

| Field | Value |
|-------|-------|
| Binary | `vscode-css-language-server` |
| Install | `npm i -g vscode-langservers-extracted` |
| Command | `vscode-css-language-server --stdio` |
| Features | Completions, diagnostics, hover, references, rename, color information, folding |
| Rhizome | LSP only |

Extracted from VS Code's built-in CSS extension. Handles `.css`, `.scss`, and `.less` files as a standard LSP server outside VS Code. Part of the `vscode-langservers-extracted` package (see note below).

---

### HTML — vscode-html-language-server

| Field | Value |
|-------|-------|
| Binary | `vscode-html-language-server` |
| Install | `npm i -g vscode-langservers-extracted` |
| Command | `vscode-html-language-server --stdio` |
| Features | Completions (tags, attributes), hover, formatting (via js-beautify), linked editing (rename open/close tags), folding |
| Rhizome | LSP only |

Extracted from VS Code's built-in HTML extension. Handles `.html` and `.htm` files with formatting built in via js-beautify. Part of the `vscode-langservers-extracted` package (see note below).

---

### JSON — vscode-json-language-server

| Field | Value |
|-------|-------|
| Binary | `vscode-json-language-server` |
| Install | `npm i -g vscode-langservers-extracted` |
| Command | `vscode-json-language-server --stdio` |
| Features | Completions (from JSON Schema), diagnostics (syntax + schema validation), hover (schema descriptions), formatting |
| Rhizome | LSP only |

Extracted from VS Code's built-in JSON extension. Connects to JSON Schema Store for auto-completion and validation of `package.json`, `tsconfig.json`, `.eslintrc.json`, and hundreds of other common JSON config files. Part of the `vscode-langservers-extracted` package (see note below).

---

### About `vscode-langservers-extracted`

One npm package provides four standalone LSP servers extracted from VS Code:

```
npm i -g vscode-langservers-extracted
```

| Binary | Source Extension | Handles |
|--------|----------------|---------|
| `vscode-css-language-server` | VS Code CSS | `.css`, `.scss`, `.less` |
| `vscode-html-language-server` | VS Code HTML | `.html`, `.htm` |
| `vscode-json-language-server` | VS Code JSON | `.json`, `.jsonc` |
| `vscode-eslint-language-server` | VS Code ESLint | JS/TS (linting) |

All four communicate via `--stdio` and work in any editor (Neovim, Helix, Zed, Emacs, etc.). The package is maintained by the community (`hrsh7th`), but the server code comes from the official `microsoft/vscode` repository. Older package names like `css-languageserver-bin`, `html-languageserver-bin`, and `vscode-json-languageserver` are deprecated—this consolidated package supersedes all of them.

---

### GraphQL — graphql-lsp

| Field | Value |
|-------|-------|
| Binary | `graphql-lsp` |
| Install | `npm i -g graphql-language-service-cli graphql` |
| Command | `graphql-lsp server -m stream` |
| Peer dep | Requires `graphql` package |
| Features | Completions, diagnostics, hover, go-to-definition, references (within schema) |
| Config | Requires `.graphqlrc.yml`, `.graphqlrc.json`, or `graphql.config.js` in project root |
| Rhizome | Not yet integrated |

Built by the GraphQL Foundation. Provides schema-aware intelligence for `.graphql` files and inline GraphQL in JS/TS (with config). The npm package name is `graphql-language-service-cli`—the binary name `graphql-lsp` comes from it.

---

### MDX — mdx-language-server

| Field | Value |
|-------|-------|
| Binary | `mdx-language-server` |
| Install | `npm i -g @mdx-js/language-server` |
| Command | `mdx-language-server --stdio` |
| Features | Completions (JSX components in markdown), diagnostics, hover, go-to-definition (imported components) |
| Rhizome | Not yet integrated |

Official server from the MDX team (unified collective). Handles `.mdx` files with JSX component awareness, understands imports, and provides component-level intelligence within markdown content.

---

### Markdown

No LSP recommended. Rhizome handles heading-based structure extraction natively via tree-sitter. Servers like `marksman` exist but add little value for agent use cases—agents need structure, not prose diagnostics.

---

### Makefile

No production-quality LSP server exists. Experimental projects like `makefile-language-server` are abandoned, and VS Code's Makefile Tools extension is VS-Code-only. Rhizome uses tree-sitter (`tree-sitter-make`) for basic target and variable extraction, which covers what agents need.

---

### Dockerfile — docker-langserver

| Field | Value |
|-------|-------|
| Binary | `docker-langserver` |
| Install | `npm i -g dockerfile-language-server-nodejs` |
| Command | `docker-langserver --stdio` |
| Features | Completions (instructions, flags), diagnostics (syntax errors, deprecated instructions), hover (instruction docs), formatting |
| Rhizome | Not yet integrated |

Community-maintained (`rcjsuen`) and the standard choice for Neovim, Helix, and other editors. Docker's official server is internal to Docker Desktop and not available standalone.

---

### Biome — biome lsp-proxy

| Field | Value |
|-------|-------|
| Binary | `biome` |
| Install | `npm i -g @biomejs/biome` |
| Command | `biome lsp-proxy` |
| Features | Diagnostics (lint errors), formatting, code actions (auto-fix) |
| Note | Linter/formatter only — no completions, hover, references, or rename |
| Rhizome | Install recipe exists |

Companion to typescript-language-server: Biome handles lint diagnostics and formatting while tsserver handles type intelligence. Use this when a project uses Biome instead of ESLint/Prettier (cap does).

---

### TOML — taplo

| Field | Value |
|-------|-------|
| Binary | `taplo` |
| Install | `brew install taplo` or `npm i -g @taplo/cli` or `cargo install taplo-cli --features lsp` |
| Command | `taplo lsp stdio` |
| Features | Completions (from TOML schema), diagnostics, hover, formatting, semantic tokens |
| Rhizome | Tree-sitter generic fallback exists; LSP not yet integrated |

Schema-aware TOML server. Provides completions and validation for `Cargo.toml`, `pyproject.toml`, `biome.json`, and other structured TOML files. Three install paths: brew is fastest on macOS, npm is most portable, cargo builds native but takes several minutes.

---

### Lua — lua-language-server

| Field | Value |
|-------|-------|
| Binary | `lua-language-server` |
| Install | `brew install lua-language-server` |
| Command | `lua-language-server` (stdio by default) |
| Features | Completions, diagnostics, hover, references, rename, go-to-definition, formatting, semantic tokens, type annotations (LuaCATS) |
| Rhizome | Full tree-sitter queries + LSP auto-install |

Maintained by LuaLS (sumneko). The standard Lua language server, used for Neovim configs, game scripting, and embedded Lua.

---

## Rhizome Backend Selection

Rhizome auto-selects the backend per tool call, so not every language needs an LSP:

| Requirement | Tools | What Happens |
|------------|-------|-------------|
| **Tree-sitter only** | `get_symbols`, `get_structure`, `get_definition`, `search_symbols`, 18 others | Fast local parsing, no server needed |
| **Prefers LSP** | `find_references`, `get_diagnostics`, `analyze_impact` | LSP if available, tree-sitter fallback |
| **Requires LSP** | `rename_symbol`, `get_hover_info` | LSP or error with install hint |

For languages with full tree-sitter query patterns (Rust, TypeScript, Python, Bash, PHP, and 13 others), most rhizome tools work without any LSP. Install the LSP when you need cross-file references, rename, hover, or diagnostics. For languages without tree-sitter support (YAML, Terraform, GraphQL, CSS, HTML, JSON, MDX, Dockerfile), the LSP is the only backend—install it to get any intelligence from rhizome.

---

## What's Not Yet in Rhizome

These servers work standalone but don't have Rhizome `Language` enum entries
or install recipes yet:

| Language | Server | Status |
|----------|--------|--------|
| GraphQL | `graphql-lsp` | Needs Language variant + install recipe |
| CSS | `vscode-css-language-server` | Needs Language variant + install recipe |
| HTML | `vscode-html-language-server` | Needs Language variant + install recipe |
| JSON | `vscode-json-language-server` | Needs Language variant + install recipe |
| MDX | `mdx-language-server` | Needs Language variant + install recipe |
| Dockerfile | `docker-langserver` | Needs Language variant + install recipe |
| TOML (LSP) | `taplo` | Has tree-sitter; needs LSP install recipe |

Adding these would require changes to:
- `crates/rhizome-core/src/language.rs` — new `Language` enum variants
- `crates/rhizome-core/src/installer.rs` — new `install_recipe()` entries
- `crates/rhizome-core/src/language.rs` — new `default_server_config()` entries
