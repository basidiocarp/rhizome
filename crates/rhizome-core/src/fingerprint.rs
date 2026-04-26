use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A signature-level fingerprint for a source file.
/// Tracks exports, imports, and top-level names — not bodies.
/// A matching signature_hash means the file's public interface is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// The file path this fingerprint covers.
    pub path: String,
    /// Canonical representation of the signature (sorted top-level symbol names + import paths).
    /// A change here means the public interface changed.
    pub signature_hash: String,
    /// Proxy content hash: "mtime_nanos,size_bytes". Change means body may have changed.
    pub content_hash: String,
    /// Top-level exported symbol names.
    pub exports: BTreeSet<String>,
    /// Imported module paths.
    pub imports: BTreeSet<String>,
}

impl Fingerprint {
    /// True if the public interface (exports + imports) is unchanged.
    pub fn signature_matches(&self, other: &Fingerprint) -> bool {
        self.signature_hash == other.signature_hash
    }

    /// True if the content proxy is unchanged (same mtime+size).
    pub fn content_matches(&self, other: &Fingerprint) -> bool {
        self.content_hash == other.content_hash
    }
}
