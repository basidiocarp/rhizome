use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub fn global_config_path() -> PathBuf {
    spore::paths::config_path("rhizome")
}

pub fn project_config_path(project_root: &Path) -> PathBuf {
    project_root.join(".rhizome").join("config.toml")
}

pub fn managed_bin_dir() -> PathBuf {
    spore::paths::data_dir("rhizome").join("bin")
}

pub fn augmented_path(bin_dir: &Path) -> OsString {
    let mut paths = vec![bin_dir.to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));

    std::env::join_paths(paths).unwrap_or_else(|_| bin_dir.as_os_str().to_os_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_config_path_ends_with_rhizome_config() {
        assert!(global_config_path().ends_with("rhizome/config.toml"));
    }

    #[test]
    fn project_config_path_uses_dot_rhizome_dir() {
        let root = Path::new("/tmp/demo-project");
        assert_eq!(
            project_config_path(root),
            PathBuf::from("/tmp/demo-project/.rhizome/config.toml")
        );
    }

    #[test]
    fn managed_bin_dir_ends_with_rhizome_bin() {
        assert!(managed_bin_dir().ends_with("rhizome/bin"));
    }

    #[test]
    fn augmented_path_starts_with_bin_dir() {
        let bin_dir = Path::new("/tmp/rhizome-bin");
        let path = augmented_path(bin_dir);
        let mut split = std::env::split_paths(&path);
        assert_eq!(split.next(), Some(bin_dir.to_path_buf()));
    }
}
