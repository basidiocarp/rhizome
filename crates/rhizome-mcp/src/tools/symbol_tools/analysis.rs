use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

use anyhow::Result;
use rhizome_core::{CodeIntelligence, Position, Symbol, SymbolKind};
use serde_json::{Value, json};

use super::navigation::{extract_identifier_at, is_ident_char};
use super::{ToolSchema, required_str, required_u32, resolve_project_path, tool_response};

pub fn find_references(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let line = required_u32(args, "line")?;
    let column = required_u32(args, "column")?;

    let path = resolve_project_path(file, project_root)?;
    let pos = Position { line, column };
    let locations = backend.find_references(&path, &pos)?;

    let formatted: Vec<Value> = locations
        .iter()
        .map(|loc| {
            json!({
                "file": &loc.file_path,
                "line_start": loc.line_start,
                "line_end": loc.line_end,
                "column_start": loc.column_start,
                "column_end": loc.column_end,
            })
        })
        .collect();

    let text = serde_json::to_string_pretty(&formatted)?;
    Ok(tool_response(&text))
}

/// Estimate the local/project impact of changing the symbol at a given position.

pub fn analyze_impact(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let line = required_u32(args, "line")?;
    let column = required_u32(args, "column")?;

    let path = resolve_project_path(file, project_root)?;
    let pos = Position { line, column };
    let source = std::fs::read_to_string(&path)?;
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = line as usize;

    if line_idx >= lines.len() {
        return Ok(tool_response("Position is beyond end of file"));
    }

    let symbol_name = extract_identifier_at(lines[line_idx], column as usize);
    if symbol_name.is_empty() {
        return Ok(tool_response("No identifier at the given position"));
    }

    let definition = backend.get_definition(&path, &symbol_name)?;
    let definition_location = definition.as_ref().map(|symbol| &symbol.location);
    let definition_qualified_name = definition.as_ref().map(Symbol::qualified_name);
    let references = backend
        .find_references(&path, &pos)?
        .into_iter()
        .filter(|loc| !is_definition_location(definition_location, loc))
        .collect::<Vec<_>>();
    let capabilities = backend.capabilities();
    let symbols = backend.get_symbols(&path)?;
    let dependency_map = build_dependency_map(&symbols, &lines);
    let related_symbols = backend
        .search_symbols(&symbol_name, project_root)?
        .into_iter()
        .filter(|symbol| {
            symbol.name == symbol_name
                && definition.as_ref().is_none_or(|definition_symbol| {
                    symbol.stable_id() != definition_symbol.stable_id()
                })
        })
        .collect::<Vec<_>>();
    let exact_scope_matches = definition_qualified_name
        .as_ref()
        .map(|qualified_name| {
            related_symbols
                .iter()
                .filter(|symbol| symbol.qualified_name() == *qualified_name)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ambiguous_same_name_symbols = related_symbols
        .iter()
        .filter(|symbol| {
            definition_qualified_name
                .as_ref()
                .is_none_or(|qualified_name| symbol.qualified_name() != *qualified_name)
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut references_by_file: Vec<(String, usize)> = Vec::new();
    for location in &references {
        if let Some((_, count)) = references_by_file
            .iter_mut()
            .find(|(file_path, _)| file_path == &location.file_path)
        {
            *count += 1;
        } else {
            references_by_file.push((location.file_path.clone(), 1));
        }
    }
    references_by_file.sort_by(|a, b| a.0.cmp(&b.0));

    let affected_files = references_by_file.len();
    let total_references = references.len();
    let local_callees = dependency_map
        .get(&symbol_name)
        .cloned()
        .unwrap_or_default();
    let mut local_callers = dependency_map
        .iter()
        .filter_map(|(caller, callees)| {
            if callees.iter().any(|callee| callee == &symbol_name) {
                Some(caller.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    local_callers.sort();
    let has_test_touchpoints = references_by_file
        .iter()
        .any(|(file_path, _)| file_path.contains("test"))
        || local_callers.iter().any(|caller| caller.contains("test"));
    let mut risk_factors = Vec::new();
    if affected_files > 1 {
        risk_factors.push("cross-file references".to_string());
    }
    if total_references > 5 {
        risk_factors.push("many references".to_string());
    }
    if !local_callers.is_empty() {
        risk_factors.push("has local callers".to_string());
    }
    if !local_callees.is_empty() {
        risk_factors.push("has downstream local callees".to_string());
    }
    if has_test_touchpoints {
        risk_factors.push("touches tests".to_string());
    }
    if !exact_scope_matches.is_empty() {
        risk_factors.push("same scoped symbol elsewhere in project".to_string());
    }
    if !capabilities.cross_file_references {
        risk_factors.push("cross-file references unavailable in backend".to_string());
    }
    if !ambiguous_same_name_symbols.is_empty() {
        risk_factors.push("same-name symbols elsewhere in project".to_string());
        risk_factors.push("related symbol matching is name-based".to_string());
    }
    let risk = if affected_files == 0 || (affected_files <= 1 && total_references <= 2) {
        "low"
    } else if affected_files <= 3 && total_references <= 6 {
        "medium"
    } else {
        "high"
    };
    let risk_scope = if capabilities.cross_file_references {
        "project"
    } else {
        "file"
    };
    let confidence = if !capabilities.cross_file_references || !related_symbols.is_empty() {
        "heuristic"
    } else {
        "strong"
    };

    let summary = if total_references == 0 && local_callers.is_empty() {
        format!(
            "Changing {symbol_name} has no additional references in the current {risk_scope}-level analysis scope."
        )
    } else {
        format!(
            "Changing {symbol_name} affects {total_references} reference(s) across {affected_files} file(s), with {} local caller(s) and {} local callee(s), based on {risk_scope}-level analysis.",
            local_callers.len(),
            local_callees.len()
        )
    };

    let result = json!({
        "symbol": symbol_name,
        "definition": definition.as_ref().map(|symbol| {
            json!({
                "name": symbol.name,
                "qualified_name": symbol.qualified_name(),
                "stable_id": symbol.stable_id(),
                "kind": format!("{:?}", symbol.kind),
                "file": symbol.location.file_path,
                "line_start": symbol.location.line_start,
                "line_end": symbol.location.line_end,
                "column_start": symbol.location.column_start,
                "column_end": symbol.location.column_end,
                "signature": symbol.signature,
            })
        }),
        "summary": summary,
        "risk": risk,
        "risk_scope": risk_scope,
        "confidence": confidence,
        "risk_factors": risk_factors,
        "backend_capabilities": {
            "cross_file_references": capabilities.cross_file_references,
            "rename": capabilities.rename,
            "type_info": capabilities.type_info,
            "diagnostics": capabilities.diagnostics,
        },
        "affected_files": affected_files,
        "total_references": total_references,
        "local_callers": local_callers,
        "local_callees": local_callees,
        "has_test_touchpoints": has_test_touchpoints,
        "exact_scope_matches": exact_scope_matches
            .iter()
            .map(|symbol| {
                json!({
                    "name": symbol.name,
                    "qualified_name": symbol.qualified_name(),
                    "stable_id": symbol.stable_id(),
                    "kind": format!("{:?}", symbol.kind),
                    "file": symbol.location.file_path,
                    "line_start": symbol.location.line_start,
                    "line_end": symbol.location.line_end,
                })
            })
            .collect::<Vec<_>>(),
        "related_symbols": related_symbols
            .iter()
            .map(|symbol| {
                json!({
                    "name": symbol.name,
                    "qualified_name": symbol.qualified_name(),
                    "stable_id": symbol.stable_id(),
                    "kind": format!("{:?}", symbol.kind),
                    "file": symbol.location.file_path,
                    "line_start": symbol.location.line_start,
                    "line_end": symbol.location.line_end,
                })
            })
            .collect::<Vec<_>>(),
        "references_by_file": references_by_file
            .into_iter()
            .map(|(file_path, count)| {
                json!({
                    "file": file_path,
                    "count": count,
                })
            })
            .collect::<Vec<_>>(),
    });

    Ok(tool_response(&serde_json::to_string_pretty(&result)?))
}

fn is_definition_location(
    definition: Option<&rhizome_core::Location>,
    candidate: &rhizome_core::Location,
) -> bool {
    match definition {
        Some(location) => {
            location.file_path == candidate.file_path
                && location.line_start == candidate.line_start
                && location.line_end == candidate.line_end
                && location.column_start == candidate.column_start
                && location.column_end == candidate.column_end
        }
        None => false,
    }
}

fn build_dependency_map(
    symbols: &[Symbol],
    source_lines: &[&str],
) -> BTreeMap<String, Vec<String>> {
    let mut functions: Vec<(&str, usize, usize)> = Vec::new();
    collect_function_ranges(symbols, &mut functions);
    let function_names: Vec<&str> = functions.iter().map(|(name, _, _)| *name).collect();
    let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for &(name, start, end) in &functions {
        let mut calls = Vec::new();
        let end = end.min(source_lines.len().saturating_sub(1));
        for line_idx in start..=end {
            if line_idx >= source_lines.len() {
                break;
            }
            let line = source_lines[line_idx];
            for &target in &function_names {
                if target == name {
                    continue;
                }
                if let Some(pos) = line.find(target) {
                    let after = pos + target.len();
                    let rest = line[after..].trim_start();
                    if rest.starts_with('(') {
                        let before_ok = pos == 0 || !is_ident_char(line.as_bytes()[pos - 1]);
                        let after_ok = after >= line.len()
                            || !line.as_bytes()[after].is_ascii_alphanumeric()
                                && line.as_bytes()[after] != b'_';
                        if before_ok && after_ok && !calls.iter().any(|call| call == target) {
                            calls.push(target.to_string());
                        }
                    }
                }
            }
        }
        deps.insert(name.to_string(), calls);
    }

    deps
}

/// Find the definition of the symbol at a given position.
/// Uses tree-sitter to identify the symbol name at the position, then calls get_definition.

pub fn get_complexity(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let function_filter = args.get("function").and_then(|v| v.as_str());

    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;
    let source = std::fs::read_to_string(&path)?;
    let source_lines: Vec<&str> = source.lines().collect();

    let mut results = Vec::new();
    collect_complexity(&symbols, &source_lines, function_filter, &mut results);

    let text = serde_json::to_string_pretty(&results)?;
    Ok(tool_response(&text))
}

fn collect_complexity(
    symbols: &[Symbol],
    source_lines: &[&str],
    function_filter: Option<&str>,
    out: &mut Vec<Value>,
) {
    for sym in symbols {
        if matches!(sym.kind, SymbolKind::Function | SymbolKind::Method) {
            if let Some(filter) = function_filter {
                if sym.name != filter {
                    collect_complexity(&sym.children, source_lines, function_filter, out);
                    continue;
                }
            }

            let start = sym.location.line_start as usize;
            let end = (sym.location.line_end as usize).min(source_lines.len().saturating_sub(1));

            let mut complexity: u32 = 1; // base complexity
            let branch_keywords = [
                "if ", "if(", "else if", "elif ", "match ", "for ", "for(", "while ", "while(",
                "loop ", "loop{", "&&", "||", "catch ", "catch(", "case ",
            ];

            for line_idx in start..=end {
                if line_idx >= source_lines.len() {
                    break;
                }
                let line = source_lines[line_idx].trim();
                // Skip comments
                if line.starts_with("//") || line.starts_with('#') || line.starts_with("/*") {
                    continue;
                }
                for kw in &branch_keywords {
                    // Count each occurrence of the keyword in this line
                    let mut search_from = 0;
                    while let Some(pos) = line[search_from..].find(kw) {
                        complexity += 1;
                        search_from += pos + kw.len();
                    }
                }
            }

            let rating = match complexity {
                1..=5 => "simple",
                6..=10 => "moderate",
                11..=20 => "complex",
                _ => "very complex",
            };

            out.push(json!({
                "name": sym.name,
                "complexity": complexity,
                "rating": rating,
                "line_start": sym.location.line_start,
                "line_end": sym.location.line_end,
            }));
        }

        collect_complexity(&sym.children, source_lines, function_filter, out);
    }
}

// ---------------------------------------------------------------------------
// Tool 8: get_type_definitions
// ---------------------------------------------------------------------------

/// List type definitions (structs, enums, interfaces, type aliases) in a file.

pub fn get_dependencies(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;
    let source = std::fs::read_to_string(&path)?;
    let source_lines: Vec<&str> = source.lines().collect();
    let deps = build_dependency_map(&symbols, &source_lines)
        .into_iter()
        .map(|(name, calls)| (name, json!(calls)))
        .collect::<serde_json::Map<String, Value>>();

    let text = serde_json::to_string_pretty(&Value::Object(deps))?;
    Ok(tool_response(&text))
}

fn collect_function_ranges<'a>(symbols: &'a [Symbol], out: &mut Vec<(&'a str, usize, usize)>) {
    for sym in symbols {
        if matches!(sym.kind, SymbolKind::Function | SymbolKind::Method) {
            out.push((
                &sym.name,
                sym.location.line_start as usize,
                sym.location.line_end as usize,
            ));
        }
        collect_function_ranges(&sym.children, out);
    }
}

// ---------------------------------------------------------------------------
// Tool 10: get_parameters
// ---------------------------------------------------------------------------

/// Extract function parameters with types.

pub fn get_parameters(
    backend: &dyn CodeIntelligence,
    args: &Value,
    project_root: &Path,
) -> Result<Value> {
    let file = required_str(args, "file")?;
    let function_filter = args.get("function").and_then(|v| v.as_str());

    let path = resolve_project_path(file, project_root)?;
    let symbols = backend.get_symbols(&path)?;

    let mut results = Vec::new();
    collect_parameters(&symbols, function_filter, &mut results);

    let text = serde_json::to_string_pretty(&results)?;
    Ok(tool_response(&text))
}

fn collect_parameters(symbols: &[Symbol], filter: Option<&str>, out: &mut Vec<Value>) {
    for sym in symbols {
        if matches!(sym.kind, SymbolKind::Function | SymbolKind::Method) {
            if let Some(f) = filter {
                if sym.name != f {
                    collect_parameters(&sym.children, Some(f), out);
                    continue;
                }
            }

            let params = parse_params_from_signature(sym.signature.as_deref());
            out.push(json!({
                "function": sym.name,
                "parameters": params,
            }));
        }
        collect_parameters(&sym.children, filter, out);
    }
}

fn parse_params_from_signature(sig: Option<&str>) -> Vec<Value> {
    let sig = match sig {
        Some(s) => s,
        None => return Vec::new(),
    };

    // Find the parameter list between the first '(' and its matching ')'
    let open = match sig.find('(') {
        Some(i) => i,
        None => return Vec::new(),
    };

    let mut depth = 0;
    let mut close = None;
    for (i, c) in sig[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }

    let close = match close {
        Some(c) => c,
        None => return Vec::new(),
    };

    let params_str = &sig[open + 1..close];
    if params_str.trim().is_empty() {
        return Vec::new();
    }

    // Split by commas (respecting nested generics)
    let mut params = Vec::new();
    let mut current = String::new();
    let mut angle_depth = 0;
    let mut paren_depth = 0;

    for c in params_str.chars() {
        match c {
            '<' => {
                angle_depth += 1;
                current.push(c);
            }
            '>' => {
                angle_depth -= 1;
                current.push(c);
            }
            '(' => {
                paren_depth += 1;
                current.push(c);
            }
            ')' => {
                paren_depth -= 1;
                current.push(c);
            }
            ',' if angle_depth == 0 && paren_depth == 0 => {
                params.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        params.push(current);
    }

    params
        .iter()
        .filter_map(|p| {
            let p = p.trim();
            if p.is_empty() {
                return None;
            }

            // Skip self/&self/&mut self
            if p == "self" || p == "&self" || p == "&mut self" {
                return None;
            }

            // Rust: name: Type
            if let Some(colon_pos) = p.find(':') {
                let name = p[..colon_pos].trim();
                let ty = p[colon_pos + 1..].trim();
                Some(json!({ "name": name, "type": ty }))
            } else {
                // Python/JS: just the name
                Some(json!({ "name": p, "type": null }))
            }
        })
        .collect()
}
