//! Self-update command that checks GitHub releases and downloads the latest binary.
//!
//! Thin wrapper around spore::self_update::run() with rhizome-specific parameters.

use anyhow::Result;

/// Check for updates and optionally download the latest Rhizome release from GitHub.
pub fn run(check_only: bool) -> Result<()> {
    spore::self_update::run(
        "rhizome",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY"),
        check_only,
    )
    .map_err(|e| anyhow::anyhow!(e))
}
