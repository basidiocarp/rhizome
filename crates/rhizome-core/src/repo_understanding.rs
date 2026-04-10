use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project_summary::ProjectSummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoSurfaceKind {
    Documentation,
    Configuration,
    Build,
}

impl fmt::Display for RepoSurfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Documentation => "documentation",
            Self::Configuration => "configuration",
            Self::Build => "build",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSurfaceNode {
    pub path: String,
    pub kind: RepoSurfaceKind,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoSurfaceSummary {
    pub documentation_files: usize,
    pub configuration_files: usize,
    pub build_files: usize,
    pub samples: Vec<RepoSurfaceNode>,
}

impl RepoSurfaceSummary {
    pub fn record(&mut self, path: &Path, kind: RepoSurfaceKind, note: &str) {
        match kind {
            RepoSurfaceKind::Documentation => self.documentation_files += 1,
            RepoSurfaceKind::Configuration => self.configuration_files += 1,
            RepoSurfaceKind::Build => self.build_files += 1,
        }

        if self.samples.len() < 12 {
            self.samples.push(RepoSurfaceNode {
                path: path.display().to_string(),
                kind,
                note: note.to_string(),
            });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.documentation_files == 0 && self.configuration_files == 0 && self.build_files == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoUnderstandingArtifact {
    pub project: String,
    pub root: PathBuf,
    pub update_class: UnderstandingUpdateClass,
    pub summary: ProjectSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnderstandingUpdateClass {
    Fresh,
    Incremental,
    Unchanged,
    Failed,
}

impl fmt::Display for UnderstandingUpdateClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Fresh => "fresh",
            Self::Incremental => "incremental",
            Self::Unchanged => "unchanged",
            Self::Failed => "failed",
        };
        f.write_str(label)
    }
}

impl UnderstandingUpdateClass {
    pub fn from_export_stats(
        files_processed: usize,
        files_skipped_cached: usize,
        files_failed: usize,
    ) -> Self {
        if files_failed > 0 && files_processed == 0 {
            Self::Failed
        } else if files_processed > 0 && files_skipped_cached > 0 {
            Self::Incremental
        } else if files_processed > 0 {
            Self::Fresh
        } else {
            Self::Unchanged
        }
    }
}

pub fn classify_repo_surface(path: &Path) -> Option<(RepoSurfaceKind, &'static str)> {
    let file_name = path.file_name()?.to_string_lossy().to_lowercase();
    let path_text = path.to_string_lossy().to_lowercase();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if matches!(
        file_name.as_str(),
        "makefile"
            | "dockerfile"
            | "justfile"
            | "taskfile.yml"
            | "taskfile.yaml"
            | "build.gradle"
            | "build.gradle.kts"
            | "cmakelists.txt"
            | "bazel.build"
    ) {
        return Some((RepoSurfaceKind::Build, "build orchestration surface"));
    }

    if path_text.contains("/docs/")
        || path_text.contains("/doc/")
        || matches!(
            file_name.as_str(),
            "readme"
                | "readme.md"
                | "readme.mdx"
                | "readme.txt"
                | "changelog"
                | "changelog.md"
                | "contributing"
                | "contributing.md"
                | "code_of_conduct.md"
                | "security.md"
                | "license"
                | "license.md"
        )
        || matches!(ext.as_str(), "md" | "mdx" | "rst" | "txt" | "adoc" | "org")
    {
        return Some((RepoSurfaceKind::Documentation, "documentation surface"));
    }

    if matches!(
        file_name.as_str(),
        "cargo.toml"
            | "cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lockb"
            | "pyproject.toml"
            | "poetry.lock"
            | "go.mod"
            | "go.sum"
            | "composer.json"
            | "composer.lock"
            | "rust-toolchain.toml"
            | "rust-toolchain"
            | "tsconfig.json"
            | "vite.config.ts"
            | "vite.config.js"
            | "jest.config.ts"
            | "jest.config.js"
            | ".gitignore"
            | ".editorconfig"
            | ".env"
            | ".env.example"
            | "dockerfile"
            | "docker-compose.yml"
            | "docker-compose.yaml"
            | "makefile"
            | "justfile"
            | "taskfile.yml"
            | "taskfile.yaml"
    ) || matches!(
        ext.as_str(),
        "toml" | "json" | "yaml" | "yml" | "ini" | "conf"
    ) || path_text.contains("/config/")
        || path_text.contains("/configs/")
        || path_text.contains("/manifest")
    {
        let note = if matches!(
            file_name.as_str(),
            "cargo.toml"
                | "cargo.lock"
                | "package.json"
                | "package-lock.json"
                | "pnpm-lock.yaml"
                | "yarn.lock"
                | "bun.lockb"
                | "pyproject.toml"
                | "poetry.lock"
                | "go.mod"
                | "go.sum"
                | "composer.json"
                | "composer.lock"
                | "rust-toolchain.toml"
                | "rust-toolchain"
                | "tsconfig.json"
                | "vite.config.ts"
                | "vite.config.js"
                | "jest.config.ts"
                | "jest.config.js"
                | ".gitignore"
                | ".editorconfig"
                | ".env"
                | ".env.example"
                | "dockerfile"
                | "docker-compose.yml"
                | "docker-compose.yaml"
                | "makefile"
                | "justfile"
                | "taskfile.yml"
                | "taskfile.yaml"
        ) {
            "configuration or build manifest"
        } else {
            "configuration surface"
        };
        return Some((RepoSurfaceKind::Configuration, note));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_docs_and_build_surfaces() {
        assert_eq!(
            classify_repo_surface(Path::new("docs/README.md"))
                .unwrap()
                .0,
            RepoSurfaceKind::Documentation
        );
        assert_eq!(
            classify_repo_surface(Path::new("Cargo.toml")).unwrap().0,
            RepoSurfaceKind::Configuration
        );
        assert_eq!(
            classify_repo_surface(Path::new("Makefile")).unwrap().0,
            RepoSurfaceKind::Build
        );
    }

    #[test]
    fn update_class_tracks_incremental_and_failed_exports() {
        assert_eq!(
            UnderstandingUpdateClass::from_export_stats(1, 1, 0),
            UnderstandingUpdateClass::Incremental
        );
        assert_eq!(
            UnderstandingUpdateClass::from_export_stats(0, 0, 0),
            UnderstandingUpdateClass::Unchanged
        );
        assert_eq!(
            UnderstandingUpdateClass::from_export_stats(0, 0, 1),
            UnderstandingUpdateClass::Failed
        );
    }

    #[test]
    fn repo_surface_summary_records_samples() {
        let mut summary = RepoSurfaceSummary::default();
        summary.record(
            Path::new("docs/README.md"),
            RepoSurfaceKind::Documentation,
            "docs",
        );
        summary.record(
            Path::new("Cargo.toml"),
            RepoSurfaceKind::Configuration,
            "config",
        );

        assert_eq!(summary.documentation_files, 1);
        assert_eq!(summary.configuration_files, 1);
        assert_eq!(summary.samples.len(), 2);
    }
}
