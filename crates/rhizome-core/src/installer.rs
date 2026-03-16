use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::Language;

// ─────────────────────────────────────────────────────────────────────────────
// Install commands per language
// ─────────────────────────────────────────────────────────────────────────────

pub struct InstallCommand {
    /// The package manager binary (e.g. "rustup", "npm").
    pub manager: &'static str,
    /// Arguments to the package manager.
    pub args: &'static [&'static str],
    /// Environment variables to set during install.
    pub env: &'static [(&'static str, &'static str)],
    /// Whether to use the managed bin dir via env var (e.g. GOBIN).
    pub bin_env: Option<&'static str>,
}

impl Language {
    /// Returns the install command for this language's LSP server, or `None`
    /// if auto-install isn't supported (e.g. Java, C/C++).
    pub fn install_command(&self) -> Option<InstallCommand> {
        match self {
            Language::Rust => Some(InstallCommand {
                manager: "rustup",
                args: &["component", "add", "rust-analyzer"],
                env: &[],
                bin_env: None, // rustup manages its own bin path
            }),
            Language::Python => Some(InstallCommand {
                manager: "pipx", // will fall back to pip
                args: &["install", "pyright"],
                env: &[],
                bin_env: None,
            }),
            Language::JavaScript | Language::TypeScript => Some(InstallCommand {
                manager: "npm",
                args: &["install", "typescript-language-server", "typescript"],
                env: &[],
                bin_env: None, // uses --prefix
            }),
            Language::Go => Some(InstallCommand {
                manager: "go",
                args: &["install", "golang.org/x/tools/gopls@latest"],
                env: &[],
                bin_env: Some("GOBIN"),
            }),
            Language::Ruby => Some(InstallCommand {
                manager: "gem",
                args: &["install", "solargraph"],
                env: &[],
                bin_env: None, // uses --bindir
            }),
            Language::Java | Language::C | Language::Cpp | Language::Other(_) => None,
        }
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

    /// Ensure the LSP server for a language is installed.
    /// Returns the binary path if found or successfully installed, None if skipped.
    pub fn ensure_server(&self, language: &Language, binary_name: &str) -> Result<Option<PathBuf>> {
        // Check if already available
        if let Some(path) = self.find_binary(binary_name) {
            return Ok(Some(path));
        }

        if self.disabled {
            debug!("LSP auto-install disabled, skipping {binary_name}");
            return Ok(None);
        }

        let install_cmd = match language.install_command() {
            Some(cmd) => cmd,
            None => {
                debug!("No auto-install available for {}", language);
                return Ok(None);
            }
        };

        // Check if the package manager is available
        if which::which(install_cmd.manager).is_err() {
            // For Python, fall back from pipx to pip
            if *language == Language::Python {
                return self.install_python_fallback(binary_name);
            }
            warn!(
                "Cannot auto-install {binary_name}: {} not found in PATH",
                install_cmd.manager
            );
            return Ok(None);
        }

        self.run_install(language, &install_cmd, binary_name)
    }

    fn run_install(
        &self,
        language: &Language,
        cmd: &InstallCommand,
        binary_name: &str,
    ) -> Result<Option<PathBuf>> {
        std::fs::create_dir_all(&self.bin_dir)
            .context("Failed to create rhizome bin directory")?;

        info!("Installing LSP server: {binary_name} via {}", cmd.manager);

        let mut command = Command::new(cmd.manager);

        // Language-specific argument customization
        match language {
            Language::JavaScript | Language::TypeScript => {
                // npm install --prefix ~/.rhizome typescript-language-server typescript
                command.arg("install");
                command.arg("--prefix");
                command.arg(&self.bin_dir);
                command.args(&cmd.args[1..]); // skip "install"
            }
            Language::Ruby => {
                // gem install solargraph --bindir ~/.rhizome/bin
                command.args(cmd.args);
                command.arg("--bindir");
                command.arg(&self.bin_dir);
            }
            _ => {
                command.args(cmd.args);
            }
        }

        // Set environment variables
        for (key, val) in cmd.env {
            command.env(key, val);
        }
        if let Some(bin_env) = cmd.bin_env {
            command.env(bin_env, &self.bin_dir);
        }

        let output = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .with_context(|| format!("Failed to run {} for {binary_name}", cmd.manager))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to install {binary_name}: {stderr}");
            return Ok(None);
        }

        info!("Successfully installed {binary_name}");

        // Check if the binary is now available
        Ok(self.find_binary(binary_name))
    }

    /// Python fallback: try pip if pipx is not available.
    fn install_python_fallback(&self, binary_name: &str) -> Result<Option<PathBuf>> {
        if which::which("pip").is_err() && which::which("pip3").is_err() {
            warn!("Cannot auto-install {binary_name}: neither pipx nor pip found");
            return Ok(None);
        }

        let pip = if which::which("pip3").is_ok() {
            "pip3"
        } else {
            "pip"
        };

        std::fs::create_dir_all(&self.bin_dir)
            .context("Failed to create rhizome bin directory")?;

        info!("Installing LSP server: {binary_name} via {pip}");

        let output = Command::new(pip)
            .args(["install", "--break-system-packages", "pyright"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .with_context(|| format!("Failed to run {pip} for {binary_name}"))?;

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
        // When disabled, should still find existing binaries
        // (rust-analyzer may or may not be installed)
        // But it should NOT attempt to install
        if which::which("rust-analyzer").is_ok() {
            assert!(result.is_some());
        }
    }

    #[test]
    fn installer_finds_existing_binary() {
        let installer = LspInstaller::new(None, true);
        // "sh" should exist on any Unix system
        assert!(installer.find_binary("sh").is_some());
    }

    #[test]
    fn install_commands_are_defined() {
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
    fn augmented_path_includes_bin_dir() {
        let installer = LspInstaller::new(Some(PathBuf::from("/test/bin")), false);
        let path = installer.augmented_path();
        assert!(path.starts_with("/test/bin:"));
    }
}
