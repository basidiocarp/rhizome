use rhizome_core::BackendRequirement;
use rhizome_core::backend_selector::{parserless_supported, tool_requirement};

#[test]
fn tool_requirement_stays_centralized_for_the_current_dispatch_surface() {
    for tool in [
        "get_definition",
        "search_symbols",
        "go_to_definition",
        "get_signature",
        "get_imports",
        "get_call_sites",
        "get_scope",
        "get_exports",
        "summarize_file",
        "get_tests",
        "get_diff_symbols",
        "get_annotations",
        "get_complexity",
        "get_type_definitions",
        "get_dependencies",
        "get_parameters",
        "get_enclosing_class",
        "get_symbol_body",
        "get_region",
        "get_changed_files",
        "summarize_project",
        "replace_symbol_body",
        "insert_after_symbol",
        "insert_before_symbol",
        "replace_lines",
        "insert_at_line",
        "delete_lines",
        "create_file",
        "copy_symbol",
        "move_symbol",
        "export_to_hyphae",
        "rhizome_onboard",
    ] {
        assert_eq!(
            tool_requirement(tool),
            BackendRequirement::TreeSitter,
            "{tool} should stay on the shared tree-sitter path unless the contract changes"
        );
    }

    for tool in ["find_references", "get_diagnostics"] {
        assert_eq!(
            tool_requirement(tool),
            BackendRequirement::PrefersLsp,
            "{tool} should prefer LSP but still keep a shared fallback"
        );
    }

    for tool in ["rename_symbol", "get_hover_info"] {
        assert_eq!(
            tool_requirement(tool),
            BackendRequirement::RequiresLsp,
            "{tool} should remain explicitly LSP-gated"
        );
    }
}

#[test]
fn parserless_fallback_is_limited_to_outline_tools() {
    assert!(parserless_supported("get_symbols"));
    assert!(parserless_supported("get_structure"));

    for tool in [
        "get_definition",
        "find_references",
        "rename_symbol",
        "get_diagnostics",
        "export_to_hyphae",
    ] {
        assert!(
            !parserless_supported(tool),
            "{tool} should not silently expand the parserless boundary"
        );
    }
}
