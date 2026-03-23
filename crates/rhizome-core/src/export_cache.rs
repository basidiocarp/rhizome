use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::error::Result;

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

    /// Returns the cache path for the current project/worktree context.
    ///
    /// The cache is scoped by project path plus git metadata so separate
    /// worktrees or branch checkouts do not share the same export state.
    pub fn cache_path(project_root: &Path) -> std::path::PathBuf {
        let dir = project_root.join(".rhizome");
        let scope = cache_scope(project_root);
        dir.join(format!("cache-{scope}.json"))
    }

    /// Legacy cache path used before context-aware partitioning.
    pub fn legacy_cache_path(project_root: &Path) -> std::path::PathBuf {
        project_root.join(".rhizome").join("cache.json")
    }

    /// Loads cache from the scoped path under `project_root`.
    /// Falls back to the legacy cache path for backward compatibility.
    /// Returns an empty cache if neither file exists.
    pub fn load(project_root: &Path) -> Result<Self> {
        let scoped = Self::cache_path(project_root);
        if scoped.exists() {
            let content = std::fs::read_to_string(&scoped)?;
            let cache: Self = serde_json::from_str(&content)?;
            return Ok(cache);
        }

        let legacy = Self::legacy_cache_path(project_root);
        if legacy.exists() {
            let content = std::fs::read_to_string(&legacy)?;
            let cache: Self = serde_json::from_str(&content)?;
            return Ok(cache);
        }

        Ok(Self::new())
    }

    /// Saves cache to the scoped path under `project_root`.
    /// Creates the `.rhizome/` directory if it doesn't exist.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = project_root.join(".rhizome");
        std::fs::create_dir_all(&dir)?;
        let path = Self::cache_path(project_root);
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

fn cache_scope(project_root: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    project_root.to_string_lossy().hash(&mut hasher);

    if let Some(git_marker) = find_git_marker(project_root) {
        if let Some((git_dir, head)) = git_context(&git_marker) {
            git_dir.hash(&mut hasher);
            head.hash(&mut hasher);
        }
    }

    format!("{:016x}", hasher.finish())
}

fn find_git_marker(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()
    } else {
        Some(start)
    };

    while let Some(current) = dir {
        let git_marker = current.join(".git");
        if git_marker.exists() {
            return Some(git_marker);
        }

        dir = match current.parent() {
            Some(parent) if parent != current => Some(parent),
            _ => None,
        };
    }

    None
}

fn git_context(git_marker: &Path) -> Option<(String, String)> {
    if git_marker.is_dir() {
        let git_dir = git_marker
            .canonicalize()
            .ok()
            .unwrap_or_else(|| git_marker.to_path_buf());
        let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
        return Some((git_dir.to_string_lossy().into_owned(), head));
    }

    let git_file = std::fs::read_to_string(git_marker).ok()?;
    let git_dir = git_file.strip_prefix("gitdir:")?.trim();
    let git_dir_path = if Path::new(git_dir).is_absolute() {
        Path::new(git_dir).to_path_buf()
    } else {
        git_marker.parent()?.join(git_dir)
    };
    let git_dir = git_dir_path.canonicalize().ok().unwrap_or(git_dir_path);
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    Some((git_dir.to_string_lossy().into_owned(), head))
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
        assert!(ExportCache::cache_path(dir.path()).exists());
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
        fs::write(ExportCache::cache_path(dir.path()), "not valid json").unwrap();

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

    #[test]
    fn load_falls_back_to_legacy_cache_path() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ExportCache {
            files: HashMap::from([("src/main.rs".to_string(), 42)]),
        };

        let legacy = ExportCache::legacy_cache_path(dir.path());
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, serde_json::to_string_pretty(&cache).unwrap()).unwrap();

        let loaded = ExportCache::load(dir.path()).unwrap();
        assert_eq!(loaded.files["src/main.rs"], 42);
    }

    #[test]
    fn cache_path_changes_when_branch_context_changes() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

        let main_path = ExportCache::cache_path(dir.path());

        fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature\n").unwrap();
        let feature_path = ExportCache::cache_path(dir.path());

        assert_ne!(main_path, feature_path);
    }

    #[test]
    fn cache_path_changes_for_worktree_gitdirs() {
        let dir = tempfile::tempdir().unwrap();
        let wt1 = dir.path().join("wt1");
        let wt2 = dir.path().join("wt2");
        fs::create_dir_all(&wt1).unwrap();
        fs::create_dir_all(&wt2).unwrap();

        let git_dir1 = dir.path().join("repo/.git/worktrees/wt1");
        let git_dir2 = dir.path().join("repo/.git/worktrees/wt2");
        fs::create_dir_all(&git_dir1).unwrap();
        fs::create_dir_all(&git_dir2).unwrap();
        fs::write(git_dir1.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(git_dir2.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            wt1.join(".git"),
            format!("gitdir: {}\n", git_dir1.display()),
        )
        .unwrap();
        fs::write(
            wt2.join(".git"),
            format!("gitdir: {}\n", git_dir2.display()),
        )
        .unwrap();

        let path1 = ExportCache::cache_path(&wt1);
        let path2 = ExportCache::cache_path(&wt2);

        assert_ne!(path1, path2);
    }

    #[test]
    fn cache_path_uses_git_context_from_parent_of_nested_project_root() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path().join("repo");
        let project_root = repo_root.join("packages/app");
        fs::create_dir_all(&project_root).unwrap();

        let git_dir = repo_root.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

        let main_path = ExportCache::cache_path(&project_root);

        fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature\n").unwrap();
        let feature_path = ExportCache::cache_path(&project_root);

        assert_ne!(main_path, feature_path);
    }

    #[test]
    fn cache_path_changes_for_nested_projects_in_relative_git_worktrees() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path().join("repo");
        let worktree_root1 = dir.path().join("wt1");
        let worktree_root2 = dir.path().join("wt2");
        let project_root1 = worktree_root1.join("packages/app");
        let project_root2 = worktree_root2.join("packages/app");
        fs::create_dir_all(&project_root1).unwrap();
        fs::create_dir_all(&project_root2).unwrap();

        let git_dir1 = repo_root.join(".git/worktrees/wt1");
        let git_dir2 = repo_root.join(".git/worktrees/wt2");
        fs::create_dir_all(&git_dir1).unwrap();
        fs::create_dir_all(&git_dir2).unwrap();
        fs::write(git_dir1.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(git_dir2.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            worktree_root1.join(".git"),
            "gitdir: ../repo/.git/worktrees/wt1\n",
        )
        .unwrap();
        fs::write(
            worktree_root2.join(".git"),
            "gitdir: ../repo/.git/worktrees/wt2\n",
        )
        .unwrap();

        let path1 = ExportCache::cache_path(&project_root1);
        let path2 = ExportCache::cache_path(&project_root2);

        assert_ne!(path1, path2);
    }
}
