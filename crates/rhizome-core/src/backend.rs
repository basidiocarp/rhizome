use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::symbol::{Location, Symbol};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub location: Location,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCapabilities {
    pub cross_file_references: bool,
    pub rename: bool,
    pub type_info: bool,
    pub diagnostics: bool,
}

pub trait CodeIntelligence {
    fn get_symbols(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn get_definition(&self, file: &Path, name: &str) -> Result<Option<Symbol>>;
    fn find_references(&self, file: &Path, position: &Position) -> Result<Vec<Location>>;
    fn search_symbols(&self, pattern: &str, project_root: &Path) -> Result<Vec<Symbol>>;
    fn get_imports(&self, file: &Path) -> Result<Vec<Symbol>>;
    fn get_diagnostics(&self, file: &Path) -> Result<Vec<Diagnostic>>;
    fn capabilities(&self) -> BackendCapabilities;
}
