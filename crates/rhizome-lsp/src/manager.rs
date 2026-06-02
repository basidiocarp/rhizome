use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use dashmap::DashMap;
use rhizome_core::Language;
use spore::logging::{SpanContext, workflow_span};

use crate::client::LspClient;

/// Per-project LSP initialization warning gate. Ensures the warning is logged
/// at most once per project per process lifetime.
static LSP_INIT_WARNED: std::sync::OnceLock<DashMap<String, AtomicBool>> =
    std::sync::OnceLock::new();

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
        use std::collections::hash_map::Entry;

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

        // ─────────────────────────────────────────────────────────────────
        // Use Entry API to safely get or insert client
        // ─────────────────────────────────────────────────────────────────
        let client = match self.clients.entry(key.clone()) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                let config = language.default_server_config().ok_or_else(|| {
                    anyhow::anyhow!("No default language server config for {:?}", language)
                })?;

                tracing::info!(
                    "Spawning LSP server: {} for {:?} at {}",
                    config.binary,
                    language,
                    workspace_root.display()
                );
                let span_context = SpanContext::for_app("rhizome")
                    .with_tool(config.binary.clone())
                    .with_workspace_root(workspace_root.display().to_string());
                let _workflow_span = workflow_span("lsp_startup", &span_context).entered();
                let mut client = LspClient::spawn(&config, Some(workspace_root)).await?;
                match client.initialize(workspace_root).await {
                    Ok(()) => e.insert(client),
                    Err(init_error) => {
                        // Emit warning once per project
                        let project_id = workspace_root.display().to_string();
                        let warned_map = LSP_INIT_WARNED.get_or_init(DashMap::new);
                        let warned = warned_map
                            .entry(project_id.clone())
                            .or_insert_with(|| AtomicBool::new(false));
                        if !warned.swap(true, Ordering::Relaxed) {
                            tracing::warn!(
                                project = %project_id,
                                "LSP init failed; degraded mode for this project: {}",
                                init_error
                            );
                        }
                        return Err(init_error);
                    }
                }
            }
        };

        Ok(client)
    }

    /// Force-restart one or all LSP clients.
    ///
    /// When `target` is `Some((language, workspace_root))`, restarts only that client.
    /// When `target` is `None`, restarts all clients currently in the manager.
    ///
    /// This differs from `get_client` — it does not check if the client is alive.
    /// It force-drops the existing client (killing hung-but-alive processes) and
    /// respawns it via the normal `get_client` path.
    ///
    /// Returns a `Vec` of `(key, result)` tuples — one per restarted client.
    /// A failure in one client does not abort the restart of others.
    pub async fn restart_client(
        &mut self,
        target: Option<(Language, std::path::PathBuf)>,
    ) -> Vec<(ClientKey, Result<()>)> {
        let keys_to_restart = match target {
            Some(key) => vec![key],
            None => self.clients.keys().cloned().collect(),
        };

        let mut results = Vec::new();

        for key in keys_to_restart {
            let result = async {
                // Force-drop the existing client if present
                if self.clients.remove(&key).is_some() {
                    tracing::info!(
                        "Force-dropping existing LSP client for {:?} at {}",
                        key.0,
                        key.1.display()
                    );
                }

                // Respawn via the normal get_client path
                let _client = self.get_client(&key.0, &key.1).await?;
                Ok(())
            }
            .await;

            results.push((key, result));
        }

        results
    }

    /// Shut down all active language server clients.
    pub async fn shutdown_all(&mut self) -> Result<()> {
        let keys: Vec<ClientKey> = self.clients.keys().cloned().collect();
        for key in keys {
            if let Some(mut client) = self.clients.remove(&key)
                && let Err(e) = client.shutdown().await
            {
                tracing::warn!(
                    "Error shutting down LSP for {:?} at {}: {}",
                    key.0,
                    key.1.display(),
                    e
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that restart_client returns results for each targeted key.
    /// When target is None, the restart list should be empty (no clients to restart).
    /// When target is Some(key), it should attempt to restart that key.
    #[tokio::test]
    async fn restart_client_returns_result_per_key() {
        let mut manager = LanguageServerManager::new();

        // With no clients, restart of None should return empty results.
        let results = manager.restart_client(None).await;
        assert!(
            results.is_empty(),
            "restart_client with no clients should return empty results"
        );

        // With no clients, restart of a specific key should attempt to spawn
        // and return a result for that key (which will likely fail since the
        // language server may not be available).
        let target_key = (Language::Rust, std::path::PathBuf::from("/tmp"));
        let results = manager.restart_client(Some(target_key.clone())).await;
        assert_eq!(
            results.len(),
            1,
            "restart_client with a specific target should return one result"
        );
        assert_eq!(
            results[0].0, target_key,
            "result key should match the target"
        );
        // The result may be Ok or Err depending on whether rust-analyzer is installed.
        // We just verify it returns a result, not a panic.
    }
}
