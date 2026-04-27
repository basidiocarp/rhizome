use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use spore::logging::{SpanContext, subprocess_span, tool_span};
use tracing::{debug, info, warn};

use crate::Language;
use crate::error::{Result, RhizomeError};
use crate::paths;

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

impl InstallRecipe {
    pub fn manual_install_command(&self, bin_dir: &Path) -> String {
        let mut parts = vec![self.manager.to_string()];

        match self.strategy {
            InstallStrategy::NpmPrefix => {
                parts.push("install".to_string());
                parts.push("--prefix".to_string());
                parts.push(bin_dir.display().to_string());
                parts.extend(self.args.iter().skip(1).map(|arg| arg.to_string()));
            }
            InstallStrategy::GemBinDir => {
                parts.extend(self.args.iter().map(|arg| arg.to_string()));
                parts.push("--bindir".to_string());
                parts.push(bin_dir.display().to_string());
            }
            InstallStrategy::BinEnv => {
                if let Some(env_key) = self.bin_env {
                    parts = vec![
                        format!("{env_key}={}", bin_dir.display()),
                        self.manager.to_string(),
                    ];
                }
                parts.extend(self.args.iter().map(|arg| arg.to_string()));
            }
            InstallStrategy::DotnetToolPath => {
                parts.extend(self.args.iter().map(|arg| arg.to_string()));
                parts.push(bin_dir.display().to_string());
            }
            InstallStrategy::PipxOrPip | InstallStrategy::ManagerOwned => {
                parts.extend(self.args.iter().map(|arg| arg.to_string()));
            }
        }

        parts.join(" ")
    }
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
        // pyright is an npm package; npm --prefix places pyright-langserver in
        // node_modules/.bin which rhizome's augmented PATH already covers.
        "pyright-langserver" | "pyright" => Some(InstallRecipe {
            manager: "npm",
            args: &["install", "pyright"],
            bin_env: None,
            strategy: InstallStrategy::NpmPrefix,
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
        // elixir-ls is not on hex.pm; it requires downloading a GitHub release
        // and extracting the language_server.sh script — skip auto-install

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
        // omnisharp ships as platform-specific binaries from GitHub releases,
        // not as a dotnet tool — skip auto-install

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

pub fn manual_install_hint(binary_name: &str, bin_dir: &Path) -> String {
    match install_recipe(binary_name) {
        Some(recipe) => recipe.manual_install_command(bin_dir),
        None => "install with your preferred package manager".to_string(),
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
        let bin_dir = bin_dir.unwrap_or_else(paths::managed_bin_dir);
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
    pub fn augmented_path(&self) -> std::ffi::OsString {
        paths::augmented_path(&self.bin_dir)
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

        let span_context = SpanContext::for_app("rhizome")
            .with_tool(binary_name)
            .with_workspace_root(self.bin_dir.display().to_string());
        let _tool_span = tool_span("ensure_lsp_server", &span_context).entered();
        let install_cmd = recipe.manual_install_command(&self.bin_dir);
        let _subprocess_span = subprocess_span(&install_cmd, &span_context).entered();

        info!(
            "Installing LSP server: {binary_name} via {}",
            recipe.manager
        );

        let mut command = Command::new(recipe.manager);

        match recipe.strategy {
            InstallStrategy::NpmPrefix => {
                // npm install --prefix <managed bin dir> <packages>
                command.arg("install");
                command.arg("--prefix");
                command.arg(&self.bin_dir);
                command.args(&recipe.args[1..]); // skip "install"
            }
            InstallStrategy::GemBinDir => {
                // gem install <gem> --bindir <managed bin dir>
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
                // dotnet tool install <tool> --tool-path <managed bin dir>
                command.args(recipe.args);
                command.arg(&self.bin_dir);
            }
            InstallStrategy::PipxOrPip | InstallStrategy::ManagerOwned => {
                command.args(recipe.args);
            }
        }

        // Use a bounded timeout (5 minutes) so a hung package manager cannot
        // block the server indefinitely. The child is killed and reaped on timeout.
        const TIMEOUT: Duration = Duration::from_secs(300);
        let output = run_with_timeout(command, TIMEOUT).map_err(|e| {
            RhizomeError::Other(format!(
                "Failed to run {} for {binary_name}: {}",
                recipe.manager, e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let install_cmd = recipe.manual_install_command(&self.bin_dir);
            return Err(RhizomeError::Other(format!(
                "Failed to install {binary_name} via {install_cmd}: {}",
                stderr.trim()
            )));
        }

        info!("Successfully installed {binary_name}");
        self.find_binary(binary_name)
            .ok_or_else(|| {
                RhizomeError::Other(format!(
                    "{binary_name} installed successfully but was not found in {}",
                    self.augmented_path().as_os_str().to_string_lossy()
                ))
            })
            .map(Some)
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
        let prefix_dir = self.bin_dir.parent().ok_or_else(|| {
            RhizomeError::Other(format!(
                "Cannot derive pip install prefix from managed bin dir {}",
                self.bin_dir.display()
            ))
        })?;

        std::fs::create_dir_all(&self.bin_dir).map_err(|e| {
            RhizomeError::Other(format!("Failed to create rhizome bin directory: {}", e))
        })?;

        let span_context = SpanContext::for_app("rhizome")
            .with_tool(binary_name)
            .with_workspace_root(prefix_dir.display().to_string());
        let _tool_span = tool_span("ensure_lsp_server", &span_context).entered();
        let _subprocess_span = subprocess_span(
            &format!("{pip} install --prefix {} {package}", prefix_dir.display()),
            &span_context,
        )
        .entered();

        info!("Installing LSP server: {binary_name} via {pip}");

        let output = run_pip_install(pip, package, prefix_dir).map_err(|e| {
            RhizomeError::Other(format!("Failed to run {pip} for {binary_name}: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RhizomeError::Other(format!(
                "Failed to install {binary_name} via {pip} into {}: {}",
                prefix_dir.display(),
                stderr.trim()
            )));
        }

        info!("Successfully installed {binary_name} via {pip}");
        self.find_binary(binary_name)
            .ok_or_else(|| {
                RhizomeError::Other(format!(
                    "{binary_name} installed via {pip} but was not found in {}",
                    self.augmented_path().as_os_str().to_string_lossy()
                ))
            })
            .map(Some)
    }
}

fn run_pip_install(
    pip: &str,
    package: &str,
    prefix_dir: &Path,
) -> std::io::Result<std::process::Output> {
    const TIMEOUT: Duration = Duration::from_secs(300);

    let mut first = Command::new(pip);
    first
        .arg("install")
        .arg("--prefix")
        .arg(prefix_dir)
        .arg("--break-system-packages")
        .arg(package);
    let output = run_with_timeout(first, TIMEOUT)?;

    if output.status.success() {
        return Ok(output);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("no such option: --break-system-packages")
        || stderr.contains("unrecognized arguments: --break-system-packages")
    {
        let mut second = Command::new(pip);
        second.arg("install").arg("--prefix").arg(prefix_dir).arg(package);
        return run_with_timeout(second, TIMEOUT);
    }

    Ok(output)
}

/// Spawn a command and wait for it to finish, killing and reaping it if the
/// timeout elapses.  Returns an `io::Error` with kind `TimedOut` on timeout.
fn run_with_timeout(mut cmd: Command, timeout: Duration) -> std::io::Result<std::process::Output> {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let deadline = Instant::now() + timeout;
    let poll_interval = Duration::from_millis(250);

    loop {
        match child.try_wait()? {
            Some(status) => {
                // Process finished — collect output.
                let stdout = child.stdout.take().map(|mut r| {
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut r, &mut buf).ok();
                    buf
                }).unwrap_or_default();
                let stderr = child.stderr.take().map(|mut r| {
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut r, &mut buf).ok();
                    buf
                }).unwrap_or_default();
                return Ok(std::process::Output { status, stdout, stderr });
            }
            None => {
                if Instant::now() >= deadline {
                    // Kill the child and reap it to avoid zombies.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "subprocess timed out",
                    ));
                }
                std::thread::sleep(poll_interval);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bin_dir_uses_managed_bin_dir() {
        let installer = LspInstaller::new(None, false);
        assert_eq!(installer.bin_dir(), paths::managed_bin_dir());
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
        assert_eq!(r.strategy, InstallStrategy::NpmPrefix);
        assert_eq!(r.manager, "npm");

        let r = install_recipe("rust-analyzer").unwrap();
        assert_eq!(r.strategy, InstallStrategy::ManagerOwned);
    }

    #[test]
    fn augmented_path_includes_bin_dir() {
        let installer = LspInstaller::new(Some(PathBuf::from("/test/bin")), false);
        let path = installer.augmented_path();
        let mut split = std::env::split_paths(&path);
        assert_eq!(split.next(), Some(PathBuf::from("/test/bin")));
    }

    #[cfg(unix)]
    #[test]
    fn pip_fallback_retries_without_break_system_packages_on_unsupported_flag() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-pip.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nif [ \"$2\" = \"--break-system-packages\" ]; then echo 'no such option: --break-system-packages' >&2; exit 2; fi\nexit 0\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let output = run_pip_install(script.to_str().unwrap(), "demo-package", dir.path()).unwrap();
        assert!(output.status.success());
    }
}
