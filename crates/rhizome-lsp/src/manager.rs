use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use rhizome_core::Language;

use crate::client::LspClient;

/// Manages multiple LSP client instances, one per language.
pub struct LanguageServerManager {
    clients: HashMap<Language, LspClient>,
    workspace_root: PathBuf,
}

impl LanguageServerManager {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            clients: HashMap::new(),
            workspace_root,
        }
    }

    /// Get or lazily spawn an LSP client for the given language.
    /// Re-spawns the server if the previous process has exited.
    pub async fn get_client(&mut self, language: &Language) -> Result<&mut LspClient> {
        // Check if existing client's process has exited
        let needs_respawn = self
            .clients
            .get_mut(language)
            .map(|c| !c.is_alive())
            .unwrap_or(false);

        if needs_respawn {
            tracing::info!(
                "LSP server for {:?} has exited, removing stale client",
                language
            );
            self.clients.remove(language);
        }

        if !self.clients.contains_key(language) {
            let config = language.default_server_config().ok_or_else(|| {
                anyhow::anyhow!("No default language server config for {:?}", language)
            })?;

            tracing::info!("Spawning LSP server: {} for {:?}", config.binary, language);
            let mut client = LspClient::spawn(&config).await?;
            client.initialize(&self.workspace_root).await?;
            self.clients.insert(language.clone(), client);
        }

        Ok(self.clients.get_mut(language).unwrap())
    }

    /// Shut down all active language server clients.
    pub async fn shutdown_all(&mut self) -> Result<()> {
        let languages: Vec<Language> = self.clients.keys().cloned().collect();
        for lang in languages {
            if let Some(mut client) = self.clients.remove(&lang) {
                if let Err(e) = client.shutdown().await {
                    tracing::warn!("Error shutting down LSP for {:?}: {}", lang, e);
                }
            }
        }
        Ok(())
    }
}
