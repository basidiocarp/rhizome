use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use rhizome_core::Language;

pub const RUST_QUERY: &str = r#"
(function_item name: (identifier) @name) @function

(struct_item name: (type_identifier) @name) @struct_def

(enum_item name: (type_identifier) @name) @enum_def

(trait_item name: (type_identifier) @name) @trait_def

(impl_item type: (type_identifier) @name) @impl_def

(use_declaration) @import

(const_item name: (identifier) @name) @const_def

(static_item name: (identifier) @name) @static_def
"#;

pub const PYTHON_QUERY: &str = r#"
(function_definition name: (identifier) @name) @function

(class_definition name: (identifier) @name) @class_def

(import_statement) @import

(import_from_statement) @import
"#;

pub const JAVASCRIPT_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @function

(class_declaration name: (identifier) @name) @class_def

(import_statement) @import

(lexical_declaration) @variable
"#;

pub const TYPESCRIPT_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @function

(class_declaration name: (identifier) @name) @class_def

(import_statement) @import

(lexical_declaration) @variable
"#;

pub const GO_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @function

(type_declaration (type_spec name: (type_identifier) @name)) @type_def

(import_declaration) @import
"#;

static RUST_COMPILED: OnceLock<Result<tree_sitter::Query, String>> = OnceLock::new();
static PYTHON_COMPILED: OnceLock<Result<tree_sitter::Query, String>> = OnceLock::new();
static JS_COMPILED: OnceLock<Result<tree_sitter::Query, String>> = OnceLock::new();
static TS_COMPILED: OnceLock<Result<tree_sitter::Query, String>> = OnceLock::new();
static GO_COMPILED: OnceLock<Result<tree_sitter::Query, String>> = OnceLock::new();

fn compile_query(lang: &tree_sitter::Language, source: &str) -> Result<tree_sitter::Query, String> {
    tree_sitter::Query::new(lang, source).map_err(|e| format!("{e}"))
}

pub fn get_query(lang: &Language) -> Result<&'static tree_sitter::Query> {
    let result = match lang {
        Language::Rust => {
            let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
            RUST_COMPILED.get_or_init(|| compile_query(&ts_lang, RUST_QUERY))
        }
        Language::Python => {
            let ts_lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
            PYTHON_COMPILED.get_or_init(|| compile_query(&ts_lang, PYTHON_QUERY))
        }
        Language::JavaScript => {
            let ts_lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
            JS_COMPILED.get_or_init(|| compile_query(&ts_lang, JAVASCRIPT_QUERY))
        }
        Language::TypeScript => {
            let ts_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
            TS_COMPILED.get_or_init(|| compile_query(&ts_lang, TYPESCRIPT_QUERY))
        }
        Language::Go => {
            let ts_lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
            GO_COMPILED.get_or_init(|| compile_query(&ts_lang, GO_QUERY))
        }
        _ => return Err(anyhow!("Unsupported language: {:?}", lang)),
    };

    result
        .as_ref()
        .map_err(|e| anyhow!("Query compilation failed: {}", e))
}
