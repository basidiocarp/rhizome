pub mod backend;
pub mod backend_selector;
pub mod blast_radius;
pub mod change_classifier;
pub mod config;
pub mod error;
pub mod export_cache;
pub mod fingerprint;
pub mod graph;
pub mod heuristic;
pub mod hyphae;
pub mod installer;
pub mod language;
pub mod parserless;
pub mod paths;
pub mod project_summary;
pub mod repo_understanding;
pub mod root_detector;
pub mod symbol;

pub use backend::{
    BackendCapabilities, CodeIntelligence, Diagnostic, DiagnosticSeverity, Position,
};
pub use backend_selector::{BackendRequirement, BackendSelector, LanguageStatus, ResolvedBackend};
pub use blast_radius::{BlastRadius, SymbolRef, compute_risk_score, is_test_file};
pub use change_classifier::{ChangeClass, classify_change};
pub use config::RhizomeConfig;
pub use error::{Result, RhizomeError};
pub use export_cache::{ExportCache, ExportIdentity, derive_export_identity};
pub use fingerprint::Fingerprint;
pub use graph::{CodeGraph, ConceptEdge, ConceptNode};
pub use heuristic::{HeuristicBackend, HeuristicRegion};
pub use hyphae::ExportResult;
pub use installer::{LspInstaller, install_recipe, manual_install_hint};
pub use language::{Language, LanguageServerConfig};
pub use parserless::{ParserlessBackend, ParserlessRegion};
pub use paths::{
    augmented_path, global_config_path, managed_bin_dir, project_config_path, project_state_dir,
};
pub use project_summary::{EntryPoint, ModuleSummary, ProjectSummary, summarize_project};
pub use repo_understanding::{
    RepoSurfaceKind, RepoSurfaceNode, RepoSurfaceSummary, RepoUnderstandingArtifact,
    RepoUnderstandingExportOutcome, RepoUnderstandingExportStatus, RepoUnderstandingRefreshKind,
    UnderstandingUpdateClass, classify_repo_surface,
};
pub use root_detector::detect_workspace_root;
pub use symbol::{Location, Symbol, SymbolKind, find_symbol_by_name};
