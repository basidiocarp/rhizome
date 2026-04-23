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
    pub export_status: RepoUnderstandingExportStatus,
    pub summary: ProjectSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoUnderstandingExportOutcome {
    CompleteSuccess,
    PartialSuccess,
    CachedReuse,
    NoSupportedFiles,
    FullFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoUnderstandingRefreshKind {
    FullRefresh,
    PartialRefresh,
    CachedReuse,
    NoRefresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoUnderstandingExportStatus {
    pub outcome: RepoUnderstandingExportOutcome,
    pub refresh_kind: RepoUnderstandingRefreshKind,
    pub any_exports_succeeded: bool,
    pub any_exports_failed: bool,
    pub safe_to_consume: bool,
}

impl RepoUnderstandingExportStatus {
    pub fn from_export_stats(
        supported_files: usize,
        files_processed: usize,
        files_skipped_cached: usize,
        files_failed: usize,
    ) -> Self {
        let any_exports_succeeded = files_processed > 0 || files_skipped_cached > 0;
        let any_exports_failed = files_failed > 0;
        let outcome = if supported_files == 0 {
            RepoUnderstandingExportOutcome::NoSupportedFiles
        } else if any_exports_failed {
            if any_exports_succeeded {
                RepoUnderstandingExportOutcome::PartialSuccess
            } else {
                RepoUnderstandingExportOutcome::FullFailure
            }
        } else if files_processed > 0 {
            RepoUnderstandingExportOutcome::CompleteSuccess
        } else {
            RepoUnderstandingExportOutcome::CachedReuse
        };
        let refresh_kind = if files_processed > 0 {
            if files_skipped_cached > 0 {
                RepoUnderstandingRefreshKind::PartialRefresh
            } else {
                RepoUnderstandingRefreshKind::FullRefresh
            }
        } else if supported_files == 0 || any_exports_failed {
            RepoUnderstandingRefreshKind::NoRefresh
        } else {
            RepoUnderstandingRefreshKind::CachedReuse
        };

        Self {
            outcome,
            refresh_kind,
            any_exports_succeeded,
            any_exports_failed,
            safe_to_consume: supported_files > 0 && !any_exports_failed,
        }
    }
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
        supported_files: usize,
        files_processed: usize,
        files_skipped_cached: usize,
        files_failed: usize,
    ) -> Self {
        let export_status = RepoUnderstandingExportStatus::from_export_stats(
            supported_files,
            files_processed,
            files_skipped_cached,
            files_failed,
        );

        match export_status.outcome {
            RepoUnderstandingExportOutcome::FullFailure => Self::Failed,
            RepoUnderstandingExportOutcome::CachedReuse
            | RepoUnderstandingExportOutcome::NoSupportedFiles => Self::Unchanged,
            RepoUnderstandingExportOutcome::PartialSuccess => Self::Incremental,
            RepoUnderstandingExportOutcome::CompleteSuccess => {
                if files_skipped_cached > 0 {
                    Self::Incremental
                } else {
                    Self::Fresh
                }
            }
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
            UnderstandingUpdateClass::from_export_stats(2, 1, 1, 0),
            UnderstandingUpdateClass::Incremental
        );
        assert_eq!(
            UnderstandingUpdateClass::from_export_stats(0, 0, 0, 0),
            UnderstandingUpdateClass::Unchanged
        );
        assert_eq!(
            UnderstandingUpdateClass::from_export_stats(1, 0, 0, 1),
            UnderstandingUpdateClass::Failed
        );
    }

    #[test]
    fn export_status_distinguishes_complete_partial_cached_and_failed_runs() {
        let complete = RepoUnderstandingExportStatus::from_export_stats(2, 2, 0, 0);
        assert_eq!(
            complete.outcome,
            RepoUnderstandingExportOutcome::CompleteSuccess
        );
        assert_eq!(
            complete.refresh_kind,
            RepoUnderstandingRefreshKind::FullRefresh
        );
        assert!(complete.any_exports_succeeded);
        assert!(!complete.any_exports_failed);
        assert!(complete.safe_to_consume);

        let partial = RepoUnderstandingExportStatus::from_export_stats(3, 1, 1, 1);
        assert_eq!(
            partial.outcome,
            RepoUnderstandingExportOutcome::PartialSuccess
        );
        assert_eq!(
            partial.refresh_kind,
            RepoUnderstandingRefreshKind::PartialRefresh
        );
        assert!(partial.any_exports_succeeded);
        assert!(partial.any_exports_failed);
        assert!(!partial.safe_to_consume);

        let cached = RepoUnderstandingExportStatus::from_export_stats(3, 0, 3, 0);
        assert_eq!(cached.outcome, RepoUnderstandingExportOutcome::CachedReuse);
        assert_eq!(
            cached.refresh_kind,
            RepoUnderstandingRefreshKind::CachedReuse
        );
        assert!(cached.any_exports_succeeded);
        assert!(!cached.any_exports_failed);
        assert!(cached.safe_to_consume);

        let empty = RepoUnderstandingExportStatus::from_export_stats(0, 0, 0, 0);
        assert_eq!(
            empty.outcome,
            RepoUnderstandingExportOutcome::NoSupportedFiles
        );
        assert_eq!(empty.refresh_kind, RepoUnderstandingRefreshKind::NoRefresh);
        assert!(!empty.any_exports_succeeded);
        assert!(!empty.any_exports_failed);
        assert!(!empty.safe_to_consume);

        let failed = RepoUnderstandingExportStatus::from_export_stats(2, 0, 0, 2);
        assert_eq!(failed.outcome, RepoUnderstandingExportOutcome::FullFailure);
        assert_eq!(failed.refresh_kind, RepoUnderstandingRefreshKind::NoRefresh);
        assert!(!failed.any_exports_succeeded);
        assert!(failed.any_exports_failed);
        assert!(!failed.safe_to_consume);
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
