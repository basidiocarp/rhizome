use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rhizome_core::Language;

use crate::client::LspClient;

/// Key for multi-client management: (language, workspace_root).
type ClientKey = (Language, PathBuf);

/// Manages multiple LSP client instances, keyed by (language, workspace_root).
///
/// This supports monorepos where different subdirectories have different
/// project roots (e.g. a Rust workspace root vs a TypeScript package root).
#[derive(Default)]
pub struct LanguageServerManager {
    clients: HashMap<ClientKey, LspClient>,
}

impl LanguageServerManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or lazily spawn an LSP client for the given language and workspace root.
    /// Re-spawns the server if the previous process has exited.
    pub async fn get_client(
        &mut self,
        language: &Language,
        workspace_root: &Path,
    ) -> Result<&mut LspClient> {
        let key = (language.clone(), workspace_root.to_path_buf());

        // Check if existing client's process has exited
        let needs_respawn = self
            .clients
            .get_mut(&key)
            .map(|c| !c.is_alive())
            .unwrap_or(false);

        if needs_respawn {
            tracing::info!(
                "LSP server for {:?} at {} has exited, removing stale client",
                language,
                workspace_root.display()
            );
            self.clients.remove(&key);
        }

        if !self.clients.contains_key(&key) {
            let config = language.default_server_config().ok_or_else(|| {
                anyhow::anyhow!("No default language server config for {:?}", language)
            })?;

            tracing::info!(
                "Spawning LSP server: {} for {:?} at {}",
                config.binary,
                language,
                workspace_root.display()
            );
            let mut client = LspClient::spawn(&config).await?;
            client.initialize(workspace_root).await?;
            self.clients.insert(key.clone(), client);
        }

        Ok(self.clients.get_mut(&key).unwrap())
    }

    /// Shut down all active language server clients.
    pub async fn shutdown_all(&mut self) -> Result<()> {
        let keys: Vec<ClientKey> = self.clients.keys().cloned().collect();
        for key in keys {
            if let Some(mut client) = self.clients.remove(&key) {
                if let Err(e) = client.shutdown().await {
                    tracing::warn!(
                        "Error shutting down LSP for {:?} at {}: {}",
                        key.0,
                        key.1.display(),
                        e
                    );
                }
            }
        }
        Ok(())
    }
}
