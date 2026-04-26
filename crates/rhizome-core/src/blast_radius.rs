use crate::symbol::SymbolKind;
use serde::{Deserialize, Serialize};

/// Represents a symbol that is potentially affected by a change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRef {
    pub name: String,
    pub file_path: String,
    pub kind: SymbolKind,
    /// BFS depth from the changed symbol (1 = direct dependent, 2+ = transitive).
    pub depth: u32,
}

/// The result of a blast-radius simulation: which symbols are affected by changing a given symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadius {
    pub symbol: String,
    pub file_path: String,
    pub direct_dependents: Vec<SymbolRef>,
    pub transitive_dependents: Vec<SymbolRef>,
    /// File paths of affected test files.
    pub affected_tests: Vec<String>,
    /// Composite risk score 0.0–1.0.
    pub risk_score: f32,
    /// Scope note for the caller (e.g., tree-sitter limitation).
    pub note: String,
}

/// Compute a composite risk score from dependent counts.
/// Clamps to 0.0–1.0.
pub fn compute_risk_score(direct_count: usize, transitive_count: usize, test_count: usize) -> f32 {
    let raw = direct_count as f32 * 2.0 + transitive_count as f32 * 0.5 + test_count as f32 * 1.0;
    (raw / 10.0).min(1.0)
}

/// Return true if a file path looks like a test file.
pub fn is_test_file(path: &str) -> bool {
    path.contains("/test") || path.contains("_test") || path.split('/').any(|p| p == "tests")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_score_zero_counts() {
        assert_eq!(compute_risk_score(0, 0, 0), 0.0);
    }

    #[test]
    fn risk_score_low_counts() {
        let score = compute_risk_score(1, 0, 0);
        assert!(score > 0.0 && score < 1.0);
    }

    #[test]
    fn risk_score_clamps_to_one() {
        let score = compute_risk_score(100, 100, 100);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn test_file_detection_slash_test() {
        assert!(is_test_file("/project/src/tests/foo.rs"));
    }

    #[test]
    fn test_file_detection_underscore_test() {
        assert!(is_test_file("/project/src/foo_test.rs"));
    }

    #[test]
    fn test_file_detection_test_in_path() {
        assert!(is_test_file("/project/src/test_utils.rs"));
    }

    #[test]
    fn non_test_file_not_detected() {
        assert!(!is_test_file("/project/src/main.rs"));
    }
}
