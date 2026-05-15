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
