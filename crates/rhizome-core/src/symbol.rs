use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Type,
    Constant,
    Variable,
    Module,
    Import,
    Property,
    Field,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Location {
    pub file_path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub column_start: u32,
    pub column_end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_path: Vec<String>,
    pub signature: Option<String>,
    pub doc_comment: Option<String>,
    pub children: Vec<Symbol>,
}

impl Symbol {
    #[must_use]
    pub fn qualified_name(&self) -> String {
        if self.scope_path.is_empty() {
            self.name.clone()
        } else {
            format!("{}::{}", self.scope_path.join("::"), self.name)
        }
    }

    #[must_use]
    pub fn stable_id(&self) -> String {
        format!(
            "{}::{}@{}:{}",
            self.location.file_path,
            self.qualified_name(),
            self.location.line_start,
            self.location.column_start
        )
    }
}
