use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info, warn};

use crate::error::{Result, RhizomeError};
use crate::Language;

// ─────────────────────────────────────────────────────────────────────────────
// Install recipes keyed by binary name
// ─────────────────────────────────────────────────────────────────────────────

/// How to install a specific LSP server binary.
#[derive(Debug, Clone)]
pub struct InstallRecipe {
    /// The package manager binary (e.g. "rustup", "npm").
    pub manager: &'static str,
    /// Arguments to the package manager.
    pub args: &'static [&'static str],
    /// Environment variable to set to the managed bin dir (e.g. "GOBIN").
    pub bin_env: Option<&'static str>,
    /// Install strategy for bin dir placement.
    pub strategy: InstallStrategy,
}

/// How to place binaries in the managed bin dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStrategy {
    /// Package manager handles its own PATH (e.g. rustup).
    ManagerOwned,
    /// Use --prefix <bin_dir> (npm style).
    NpmPrefix,
    /// Use --bindir <bin_dir> (gem style).
    GemBinDir,
    /// Set a bin env var (e.g. GOBIN, CARGO_INSTALL_ROOT).
    BinEnv,
    /// Use pipx or fall back to pip.
    PipxOrPip,
    /// Use dotnet tool install --tool-path <bin_dir> (C#/F# tools).
    DotnetToolPath,
}

/// Look up the install recipe for a given server binary name.
/// Returns `None` for binaries we don't know how to install.
pub fn install_recipe(binary_name: &str) -> Option<InstallRecipe> {
    match binary_name {
        // ── Rust ────────────────────────────────────────────────────────
        "rust-analyzer" => Some(InstallRecipe {
            manager: "rustup",
            args: &["component", "add", "rust-analyzer"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── Python ─────────────────────────────────────────────────────
        "pyright-langserver" | "pyright" => Some(InstallRecipe {
            manager: "pipx",
            args: &["install", "pyright"],
            bin_env: None,
            strategy: InstallStrategy::PipxOrPip,
        }),
        "pylsp" => Some(InstallRecipe {
            manager: "pipx",
            args: &["install", "python-lsp-server"],
            bin_env: None,
            strategy: InstallStrategy::PipxOrPip,
        }),
        "ruff" => Some(InstallRecipe {
            manager: "pipx",
            args: &["install", "ruff"],
            bin_env: None,
            strategy: InstallStrategy::PipxOrPip,
        }),
        "jedi-language-server" => Some(InstallRecipe {
            manager: "pipx",
            args: &["install", "jedi-language-server"],
            bin_env: None,
            strategy: InstallStrategy::PipxOrPip,
        }),

        // ── JavaScript / TypeScript ────────────────────────────────────
        "typescript-language-server" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "typescript-language-server", "typescript"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),
        "biome" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "@biomejs/biome"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),

        // ── Go ─────────────────────────────────────────────────────────
        "gopls" => Some(InstallRecipe {
            manager: "go",
            args: &["install", "golang.org/x/tools/gopls@latest"],
            bin_env: Some("GOBIN"),
            strategy: InstallStrategy::BinEnv,
        }),

        // ── Ruby ───────────────────────────────────────────────────────
        "solargraph" => Some(InstallRecipe {
            manager: "gem",
            args: &["install", "solargraph"],
            bin_env: None,
            strategy: InstallStrategy::GemBinDir,
        }),
        "ruby-lsp" => Some(InstallRecipe {
            manager: "gem",
            args: &["install", "ruby-lsp"],
            bin_env: None,
            strategy: InstallStrategy::GemBinDir,
        }),

        // ── C / C++ ───────────────────────────────────────────────────
        // clangd and ccls require system package managers — skip auto-install

        // ── Elixir ─────────────────────────────────────────────────────
        "elixir-ls" | "elixir_ls" => Some(InstallRecipe {
            manager: "mix",
            args: &["archive.install", "hex", "elixir_ls"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── Zig ────────────────────────────────────────────────────────
        // zls requires downloading from GitHub releases — skip auto-install
        // (too platform-specific, like OpenCode does with binary downloads)

        // ── C# ────────────────────────────────────────────────────────
        "csharp-ls" => Some(InstallRecipe {
            manager: "dotnet",
            args: &["tool", "install", "csharp-ls", "--tool-path"],
            bin_env: None,
            strategy: InstallStrategy::DotnetToolPath,
        }),
        "omnisharp" => Some(InstallRecipe {
            manager: "dotnet",
            args: &["tool", "install", "omnisharp", "--tool-path"],
            bin_env: None,
            strategy: InstallStrategy::DotnetToolPath,
        }),

        // ── F# ────────────────────────────────────────────────────────
        "fsautocomplete" => Some(InstallRecipe {
            manager: "dotnet",
            args: &["tool", "install", "fsautocomplete", "--tool-path"],
            bin_env: None,
            strategy: InstallStrategy::DotnetToolPath,
        }),

        // ── PHP ───────────────────────────────────────────────────────
        "intelephense" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "intelephense"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),
        "phpactor" => Some(InstallRecipe {
            manager: "composer",
            args: &["global", "require", "phpactor/phpactor"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── Swift ─────────────────────────────────────────────────────
        // sourcekit-lsp ships with Xcode/Swift toolchain — no auto-install

        // ── Haskell ───────────────────────────────────────────────────
        "haskell-language-server-wrapper" => Some(InstallRecipe {
            manager: "ghcup",
            args: &["install", "hls"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── Bash ──────────────────────────────────────────────────────
        "bash-language-server" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "bash-language-server"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),

        // ── Terraform ─────────────────────────────────────────────────
        "terraform-ls" => Some(InstallRecipe {
            manager: "brew",
            args: &["install", "hashicorp/tap/terraform-ls"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── Kotlin ────────────────────────────────────────────────────
        // kotlin-language-server requires manual install from GitHub releases

        // ── Dart ──────────────────────────────────────────────────────
        // dart language server ships with the Dart/Flutter SDK

        // ── Lua ───────────────────────────────────────────────────────
        "lua-language-server" => Some(InstallRecipe {
            manager: "brew",
            args: &["install", "lua-language-server"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── Clojure ──────────────────────────────────────────────────
        "clojure-lsp" => Some(InstallRecipe {
            manager: "brew",
            args: &["install", "clojure-lsp/brew/clojure-lsp-native"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── OCaml ────────────────────────────────────────────────────
        "ocamllsp" => Some(InstallRecipe {
            manager: "opam",
            args: &["install", "ocaml-lsp-server"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── Nix ──────────────────────────────────────────────────────
        "nixd" => Some(InstallRecipe {
            manager: "nix-env",
            args: &["-iA", "nixpkgs.nixd"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── Vue / Svelte / Astro / Prisma ────────────────────────────
        "vue-language-server" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "@vue/language-server"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),
        "svelteserver" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "svelte-language-server"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),
        "astro-ls" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "@astrojs/language-server"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),
        "prisma-language-server" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "@prisma/language-server"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),

        // ── Typst ────────────────────────────────────────────────────
        "tinymist" => Some(InstallRecipe {
            manager: "cargo",
            args: &["install", "tinymist"],
            bin_env: None,
            strategy: InstallStrategy::ManagerOwned,
        }),

        // ── YAML ─────────────────────────────────────────────────────
        "yaml-language-server" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "yaml-language-server"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
        }),

        _ => None,
    }
}

impl Language {
    /// Returns the install recipe for this language's *default* LSP server.
    /// Prefer `install_recipe(binary_name)` for config-driven lookups.
    pub fn install_command(&self) -> Option<InstallRecipe> {
        let binary = self.default_server_config()?.binary;
        install_recipe(&binary)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LspInstaller
// ─────────────────────────────────────────────────────────────────────────────

pub struct LspInstaller {
    bin_dir: PathBuf,
    disabled: bool,
}

impl LspInstaller {
    pub fn new(bin_dir: Option<PathBuf>, disabled: bool) -> Self {
        let bin_dir = bin_dir.unwrap_or_else(default_bin_dir);
        Self { bin_dir, disabled }
    }

    /// Create from environment and config.
    pub fn from_env(config_disabled: bool, config_bin_dir: Option<PathBuf>) -> Self {
        let env_disabled = std::env::var("RHIZOME_DISABLE_LSP_DOWNLOAD")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Self::new(config_bin_dir, config_disabled || env_disabled)
    }

    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Augmented PATH including the managed bin directory.
    pub fn augmented_path(&self) -> String {
        let system_path = std::env::var("PATH").unwrap_or_default();
        format!("{}:{system_path}", self.bin_dir.display())
    }

    /// Try to find a binary in PATH (including managed bin dir).
    pub fn find_binary(&self, name: &str) -> Option<PathBuf> {
        which::which_in(name, Some(self.augmented_path()), ".").ok()
    }

    /// Ensure the LSP server binary is installed.
    /// Looks up the install recipe by binary name, not by language.
    /// Returns the binary path if found or successfully installed, None if skipped.
    pub fn ensure_server(
        &self,
        _language: &Language,
        binary_name: &str,
    ) -> Result<Option<PathBuf>> {
        // Check if already available
        if let Some(path) = self.find_binary(binary_name) {
            return Ok(Some(path));
        }

        if self.disabled {
            debug!("LSP auto-install disabled, skipping {binary_name}");
            return Ok(None);
        }

        let recipe = match install_recipe(binary_name) {
            Some(r) => r,
            None => {
                debug!("No auto-install recipe for {binary_name}");
                return Ok(None);
            }
        };

        // Check if the package manager is available
        if which::which(recipe.manager).is_err() {
            if recipe.strategy == InstallStrategy::PipxOrPip {
                return self.install_python_fallback(binary_name, &recipe);
            }
            warn!(
                "Cannot auto-install {binary_name}: {} not found in PATH",
                recipe.manager
            );
            return Ok(None);
        }

        self.run_install(&recipe, binary_name)
    }

    fn run_install(&self, recipe: &InstallRecipe, binary_name: &str) -> Result<Option<PathBuf>> {
        std::fs::create_dir_all(&self.bin_dir).map_err(|e| {
            RhizomeError::Other(format!("Failed to create rhizome bin directory: {}", e))
        })?;

        info!(
            "Installing LSP server: {binary_name} via {}",
            recipe.manager
        );

        let mut command = Command::new(recipe.manager);

        match recipe.strategy {
            InstallStrategy::NpmPrefix => {
                // npm install --prefix ~/.rhizome <packages>
                command.arg("install");
                command.arg("--prefix");
                command.arg(&self.bin_dir);
                command.args(&recipe.args[1..]); // skip "install"
            }
            InstallStrategy::GemBinDir => {
                // gem install <gem> --bindir ~/.rhizome/bin
                command.args(recipe.args);
                command.arg("--bindir");
                command.arg(&self.bin_dir);
            }
            InstallStrategy::BinEnv => {
                command.args(recipe.args);
                if let Some(env_key) = recipe.bin_env {
                    command.env(env_key, &self.bin_dir);
                }
            }
            InstallStrategy::DotnetToolPath => {
                // dotnet tool install <tool> --tool-path ~/.rhizome/bin
                command.args(recipe.args);
                command.arg(&self.bin_dir);
            }
            InstallStrategy::PipxOrPip | InstallStrategy::ManagerOwned => {
                command.args(recipe.args);
            }
        }

        let output = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| {
                RhizomeError::Other(format!(
                    "Failed to run {} for {binary_name}: {}",
                    recipe.manager, e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to install {binary_name}: {stderr}");
            return Ok(None);
        }

        info!("Successfully installed {binary_name}");
        Ok(self.find_binary(binary_name))
    }

    /// Python fallback: try pip if pipx is not available.
    fn install_python_fallback(
        &self,
        binary_name: &str,
        recipe: &InstallRecipe,
    ) -> Result<Option<PathBuf>> {
        if which::which("pip").is_err() && which::which("pip3").is_err() {
            warn!("Cannot auto-install {binary_name}: neither pipx nor pip found");
            return Ok(None);
        }

        let pip = if which::which("pip3").is_ok() {
            "pip3"
        } else {
            "pip"
        };

        // Extract the package name from the pipx args (e.g. ["install", "pyright"] → "pyright")
        let package = recipe.args.last().unwrap_or(&binary_name);

        std::fs::create_dir_all(&self.bin_dir).map_err(|e| {
            RhizomeError::Other(format!("Failed to create rhizome bin directory: {}", e))
        })?;

        info!("Installing LSP server: {binary_name} via {pip}");

        let output = Command::new(pip)
            .args(["install", "--break-system-packages", package])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| {
                RhizomeError::Other(format!("Failed to run {pip} for {binary_name}: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to install {binary_name} via {pip}: {stderr}");
            return Ok(None);
        }

        info!("Successfully installed {binary_name} via {pip}");
        Ok(self.find_binary(binary_name))
    }
}

fn default_bin_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".rhizome").join("bin"))
        .unwrap_or_else(|| PathBuf::from(".rhizome/bin"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bin_dir_under_home() {
        let dir = default_bin_dir();
        assert!(dir.to_string_lossy().contains(".rhizome/bin"));
    }

    #[test]
    fn installer_respects_disabled_flag() {
        let installer = LspInstaller::new(None, true);
        assert!(installer.is_disabled());

        let result = installer
            .ensure_server(&Language::Rust, "rust-analyzer")
            .unwrap();
        if which::which("rust-analyzer").is_ok() {
            assert!(result.is_some());
        }
    }

    #[test]
    fn installer_finds_existing_binary() {
        let installer = LspInstaller::new(None, true);
        assert!(installer.find_binary("sh").is_some());
    }

    #[test]
    fn recipes_for_default_servers() {
        assert!(Language::Rust.install_command().is_some());
        assert!(Language::Python.install_command().is_some());
        assert!(Language::TypeScript.install_command().is_some());
        assert!(Language::Go.install_command().is_some());
        assert!(Language::Ruby.install_command().is_some());
        assert!(Language::Java.install_command().is_none());
        assert!(Language::C.install_command().is_none());
        assert!(Language::Cpp.install_command().is_none());
    }

    #[test]
    fn recipes_for_alternative_servers() {
        // Python alternatives
        assert!(install_recipe("pylsp").is_some());
        assert!(install_recipe("ruff").is_some());
        assert!(install_recipe("jedi-language-server").is_some());

        // Ruby alternative
        assert!(install_recipe("ruby-lsp").is_some());

        // JS/TS alternative
        assert!(install_recipe("biome").is_some());

        // Unknown binary
        assert!(install_recipe("nonexistent-lsp-xyz").is_none());
    }

    #[test]
    fn recipe_strategy_matches_manager() {
        let r = install_recipe("gopls").unwrap();
        assert_eq!(r.strategy, InstallStrategy::BinEnv);
        assert_eq!(r.bin_env, Some("GOBIN"));

        let r = install_recipe("typescript-language-server").unwrap();
        assert_eq!(r.strategy, InstallStrategy::NpmPrefix);

        let r = install_recipe("solargraph").unwrap();
        assert_eq!(r.strategy, InstallStrategy::GemBinDir);

        let r = install_recipe("pyright-langserver").unwrap();
        assert_eq!(r.strategy, InstallStrategy::PipxOrPip);

        let r = install_recipe("rust-analyzer").unwrap();
        assert_eq!(r.strategy, InstallStrategy::ManagerOwned);
    }

    #[test]
    fn augmented_path_includes_bin_dir() {
        let installer = LspInstaller::new(Some(PathBuf::from("/test/bin")), false);
        let path = installer.augmented_path();
        assert!(path.starts_with("/test/bin:"));
    }
}
