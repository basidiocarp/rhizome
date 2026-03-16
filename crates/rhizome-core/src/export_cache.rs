use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Mtime-based file change cache for incremental exports.
///
/// Tracks the last-exported modification time (Unix epoch seconds) for each file,
/// allowing callers to skip re-exporting files that haven't changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCache {
    /// Maps file paths to their last-exported mtime (Unix epoch seconds)
    pub files: HashMap<String, u64>,
}

impl ExportCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Loads cache from `.rhizome/cache.json` under `project_root`.
    /// Returns an empty cache if the file doesn't exist.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join(".rhizome").join("cache.json");
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let cache: Self = serde_json::from_str(&content)?;
        Ok(cache)
    }

    /// Saves cache to `.rhizome/cache.json` under `project_root`.
    /// Creates the `.rhizome/` directory if it doesn't exist.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = project_root.join(".rhizome");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("cache.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Returns true if the file should be re-exported.
    ///
    /// A file is stale if it's not in the cache, its current mtime is newer
    /// than the cached mtime, or its metadata can't be read.
    pub fn is_stale(&self, file_path: &Path) -> bool {
        let current_mtime = match Self::get_mtime(file_path) {
            Some(mtime) => mtime,
            None => return true,
        };

        let path_str = file_path.to_string_lossy();
        match self.files.get(path_str.as_ref()) {
            Some(&cached_mtime) => current_mtime > cached_mtime,
            None => true,
        }
    }

    /// Returns a new `ExportCache` with the file's current mtime recorded.
    /// If getting the mtime fails, returns a clone of self unchanged.
    pub fn update(&self, file_path: &Path) -> Self {
        let mut new_cache = self.clone();
        if let Some(mtime) = Self::get_mtime(file_path) {
            let path_str = file_path.to_string_lossy().into_owned();
            new_cache.files.insert(path_str, mtime);
        }
        new_cache
    }

    /// Gets a file's mtime as Unix epoch seconds, or None on failure.
    fn get_mtime(file_path: &Path) -> Option<u64> {
        let metadata = std::fs::metadata(file_path).ok()?;
        let modified = metadata.modified().ok()?;
        let duration = modified.duration_since(UNIX_EPOCH).ok()?;
        Some(duration.as_secs())
    }
}

impl Default for ExportCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn empty_cache_reports_all_files_as_stale() {
        let cache = ExportCache::new();
        assert!(cache.is_stale(Path::new("src/main.rs")));
        assert!(cache.is_stale(Path::new("nonexistent.rs")));
    }

    #[test]
    fn matching_mtime_reports_not_stale() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let mtime = ExportCache::get_mtime(&file_path).unwrap();
        let cache = ExportCache {
            files: HashMap::from([(file_path.to_string_lossy().into_owned(), mtime)]),
        };

        assert!(!cache.is_stale(&file_path));
    }

    #[test]
    fn save_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ExportCache {
            files: HashMap::from([
                ("src/main.rs".to_string(), 1_710_000_000),
                ("src/lib.rs".to_string(), 1_710_000_100),
            ]),
        };

        cache.save(dir.path()).unwrap();
        let loaded = ExportCache::load(dir.path()).unwrap();

        assert_eq!(loaded.files.len(), 2);
        assert_eq!(loaded.files["src/main.rs"], 1_710_000_000);
        assert_eq!(loaded.files["src/lib.rs"], 1_710_000_100);
    }

    #[test]
    fn update_returns_new_cache_with_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let cache = ExportCache::new();
        assert!(cache.files.is_empty());

        let updated = cache.update(&file_path);
        // Original cache is unchanged
        assert!(cache.files.is_empty());
        // Updated cache has the entry
        assert_eq!(updated.files.len(), 1);
        assert!(updated
            .files
            .contains_key(&file_path.to_string_lossy().into_owned()));
    }

    #[test]
    fn load_returns_empty_cache_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ExportCache::load(dir.path()).unwrap();
        assert!(cache.files.is_empty());
    }

    #[test]
    fn load_returns_error_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let rhizome_dir = dir.path().join(".rhizome");
        fs::create_dir_all(&rhizome_dir).unwrap();
        fs::write(rhizome_dir.join("cache.json"), "not valid json").unwrap();

        let result = ExportCache::load(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn update_with_nonexistent_file_returns_unchanged() {
        let cache = ExportCache {
            files: HashMap::from([("existing.rs".to_string(), 1_710_000_000)]),
        };
        let updated = cache.update(Path::new("/nonexistent/file.rs"));
        assert_eq!(updated.files.len(), 1);
        assert_eq!(updated.files["existing.rs"], 1_710_000_000);
    }
}
