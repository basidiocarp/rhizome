use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use rhizome_core::Language;

pub struct ParserPool {
    parsers: HashMap<ParserDialect, tree_sitter::Parser>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ParserDialect {
    Language(Language),
    TypeScriptTsx,
}

impl Default for ParserPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserPool {
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    pub fn get_parser(&mut self, lang: &Language) -> Result<&mut tree_sitter::Parser> {
        self.get_parser_for_dialect(ParserDialect::Language(lang.clone()))
    }

    pub fn get_parser_for_file(
        &mut self,
        lang: &Language,
        file: &Path,
    ) -> Result<&mut tree_sitter::Parser> {
        let dialect = Self::dialect_for_file(lang, file);
        self.get_parser_for_dialect(dialect)
    }

    fn get_parser_for_dialect(
        &mut self,
        dialect: ParserDialect,
    ) -> Result<&mut tree_sitter::Parser> {
        if !self.parsers.contains_key(&dialect) {
            let mut parser = tree_sitter::Parser::new();
            let ts_lang = Self::get_language(&dialect)?;
            parser
                .set_language(&ts_lang)
                .map_err(|e| anyhow!("Failed to set language: {}", e))?;
            self.parsers.insert(dialect.clone(), parser);
        }
        Ok(self.parsers.get_mut(&dialect).unwrap())
    }

    fn dialect_for_file(lang: &Language, file: &Path) -> ParserDialect {
        match (
            lang,
            file.extension().and_then(|extension| extension.to_str()),
        ) {
            (Language::TypeScript, Some("tsx")) => ParserDialect::TypeScriptTsx,
            _ => ParserDialect::Language(lang.clone()),
        }
    }

    fn get_language(dialect: &ParserDialect) -> Result<tree_sitter::Language> {
        match dialect {
            ParserDialect::Language(Language::Rust) => Ok(tree_sitter_rust::LANGUAGE.into()),
            ParserDialect::Language(Language::Python) => Ok(tree_sitter_python::LANGUAGE.into()),
            ParserDialect::Language(Language::JavaScript) => {
                Ok(tree_sitter_javascript::LANGUAGE.into())
            }
            ParserDialect::Language(Language::TypeScript) => {
                Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            }
            ParserDialect::TypeScriptTsx => Ok(tree_sitter_typescript::LANGUAGE_TSX.into()),
            ParserDialect::Language(Language::Go) => Ok(tree_sitter_go::LANGUAGE.into()),
            ParserDialect::Language(Language::Java) => Ok(tree_sitter_java::LANGUAGE.into()),
            ParserDialect::Language(Language::C) => Ok(tree_sitter_c::LANGUAGE.into()),
            ParserDialect::Language(Language::Cpp) => Ok(tree_sitter_cpp::LANGUAGE.into()),
            ParserDialect::Language(Language::Ruby) => Ok(tree_sitter_ruby::LANGUAGE.into()),
            ParserDialect::Language(Language::Php) => Ok(tree_sitter_php::LANGUAGE_PHP.into()),
            ParserDialect::Language(Language::Bash) => Ok(tree_sitter_bash::LANGUAGE.into()),
            ParserDialect::Language(Language::CSharp) => Ok(tree_sitter_c_sharp::LANGUAGE.into()),
            ParserDialect::Language(Language::Elixir) => Ok(tree_sitter_elixir::LANGUAGE.into()),
            // Kotlin: tree-sitter-kotlin uses incompatible tree-sitter version; uses generic fallback
            ParserDialect::Language(Language::Lua) => Ok(tree_sitter_lua::LANGUAGE.into()),
            ParserDialect::Language(Language::Swift) => Ok(tree_sitter_swift::LANGUAGE.into()),
            ParserDialect::Language(Language::Zig) => Ok(tree_sitter_zig::LANGUAGE.into()),
            ParserDialect::Language(Language::Haskell) => Ok(tree_sitter_haskell::LANGUAGE.into()),
            ParserDialect::Language(lang) => Err(anyhow!("No tree-sitter grammar for {:?}", lang)),
        }
    }
}
