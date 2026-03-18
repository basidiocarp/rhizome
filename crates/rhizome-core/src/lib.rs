pub mod backend;
pub mod backend_selector;
pub mod config;
pub mod export_cache;
pub mod graph;
pub mod hyphae;
pub mod installer;
pub mod language;
pub mod project_summary;
pub mod root_detector;
pub mod symbol;

pub use backend::{
    BackendCapabilities, CodeIntelligence, Diagnostic, DiagnosticSeverity, Position,
};
pub use backend_selector::{BackendRequirement, BackendSelector, LanguageStatus, ResolvedBackend};
pub use config::RhizomeConfig;
pub use export_cache::ExportCache;
pub use graph::{CodeGraph, ConceptEdge, ConceptNode};
pub use hyphae::ExportResult;
pub use installer::{install_recipe, LspInstaller};
pub use language::{Language, LanguageServerConfig};
pub use project_summary::{summarize_project, EntryPoint, ModuleSummary, ProjectSummary};
pub use root_detector::detect_workspace_root;
pub use symbol::{Location, Symbol, SymbolKind};
