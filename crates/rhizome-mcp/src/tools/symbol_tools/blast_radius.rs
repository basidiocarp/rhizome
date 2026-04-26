use std::path::Path;

use anyhow::Result;
use rhizome_core::{BlastRadius, CodeIntelligence, SymbolRef, compute_risk_score, is_test_file};
use serde_json::{Value, json};

use super::params::required_str;
use super::{ToolAnnotations, ToolSchema, resolve_project_path, tool_response};

pub fn simulate_change(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let symbol_name = required_str(args, "symbol")?;
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;

    // Verify the symbol exists in the given file.
    let file_symbols = backend.get_symbols(&path)?;
    let symbol_exists = file_symbols
        .iter()
        .any(|s| s.name == symbol_name || check_children(&s.children, symbol_name));

    if !symbol_exists {
        return Ok(tool_response(&format!(
            "Symbol '{}' not found in {}",
            symbol_name,
            path.display()
        )));
    }

    // Search for the symbol across the project workspace.
    let all_matches = backend.search_symbols(symbol_name, project_root)?;

    let definition_path = path.to_string_lossy().to_string();

    // Direct dependents: files other than the definition file that reference this symbol.
    let mut direct_dependents: Vec<SymbolRef> = all_matches
        .iter()
        .filter(|sym| sym.location.file_path != definition_path)
        .map(|sym| SymbolRef {
            name: sym.name.clone(),
            file_path: sym.location.file_path.clone(),
            kind: sym.kind.clone(),
            depth: 1,
        })
        .collect();

    // Deduplicate by file path (keep first occurrence per file).
    let mut seen_files = std::collections::HashSet::new();
    direct_dependents.retain(|s| seen_files.insert(s.file_path.clone()));

    // Sort by file_path ASC.
    direct_dependents.sort_by(|a, b| a.file_path.cmp(&b.file_path));

    // Test files: direct dependents whose paths match test patterns.
    let affected_tests: Vec<String> = direct_dependents
        .iter()
        .filter(|s| is_test_file(&s.file_path))
        .map(|s| s.file_path.clone())
        .collect();

    // Transitive: not computable with tree-sitter (no cross-file graph). Leave empty.
    let transitive_dependents: Vec<SymbolRef> = Vec::new();

    let risk_score = compute_risk_score(
        direct_dependents.len(),
        transitive_dependents.len(),
        affected_tests.len(),
    );

    let result = BlastRadius {
        symbol: symbol_name.to_string(),
        file_path: definition_path,
        direct_dependents,
        transitive_dependents,
        affected_tests,
        risk_score,
        note: "Transitive dependents require cross-file reference support (LSP). \
               Direct dependents are approximate: based on symbol name matches in the workspace index."
            .to_string(),
    };

    let text = serde_json::to_string_pretty(&result)?;
    Ok(tool_response(&text))
}

fn check_children(children: &[rhizome_core::Symbol], name: &str) -> bool {
    children
        .iter()
        .any(|s| s.name == name || check_children(&s.children, name))
}

pub fn simulate_change_schema() -> ToolSchema {
    ToolSchema {
        name: "rhizome_simulate_change".into(),
        description: "Simulate the blast radius of changing a symbol: find direct dependents, \
            affected test files, and compute a risk score (0.0–1.0). \
            Transitive dependents require an LSP server; direct dependents are workspace-wide."
            .into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "symbol": { "type": "string", "description": "Name of the symbol to simulate changing" },
                "file": { "type": "string", "description": "File path where the symbol is defined" },
                "root": { "type": "string", "description": "Optional project root" }
            },
            "required": ["symbol", "file"]
        }),
        annotations: ToolAnnotations {
            read_only_hint: true,
            destructive_hint: false,
            idempotent_hint: true,
        },
    }
}
