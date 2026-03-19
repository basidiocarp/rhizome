use std::collections::HashMap;

use anyhow::{anyhow, Result};
use rhizome_core::Language;

pub struct ParserPool {
    parsers: HashMap<Language, tree_sitter::Parser>,
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
        if !self.parsers.contains_key(lang) {
            let mut parser = tree_sitter::Parser::new();
            let ts_lang = Self::get_language(lang)?;
            parser
                .set_language(&ts_lang)
                .map_err(|e| anyhow!("Failed to set language: {}", e))?;
            self.parsers.insert(lang.clone(), parser);
        }
        Ok(self.parsers.get_mut(lang).unwrap())
    }

    fn get_language(lang: &Language) -> Result<tree_sitter::Language> {
        match lang {
            Language::Rust => Ok(tree_sitter_rust::LANGUAGE.into()),
            Language::Python => Ok(tree_sitter_python::LANGUAGE.into()),
            Language::JavaScript => Ok(tree_sitter_javascript::LANGUAGE.into()),
            Language::TypeScript => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Language::Go => Ok(tree_sitter_go::LANGUAGE.into()),
            Language::Java => Ok(tree_sitter_java::LANGUAGE.into()),
            Language::C => Ok(tree_sitter_c::LANGUAGE.into()),
            Language::Cpp => Ok(tree_sitter_cpp::LANGUAGE.into()),
            Language::Ruby => Ok(tree_sitter_ruby::LANGUAGE.into()),
            Language::Php => Ok(tree_sitter_php::LANGUAGE_PHP.into()),
            Language::Bash => Ok(tree_sitter_bash::LANGUAGE.into()),
            Language::CSharp => Ok(tree_sitter_c_sharp::LANGUAGE.into()),
            Language::Elixir => Ok(tree_sitter_elixir::LANGUAGE.into()),
            // Kotlin: tree-sitter-kotlin uses incompatible tree-sitter version; uses generic fallback
            Language::Lua => Ok(tree_sitter_lua::LANGUAGE.into()),
            Language::Swift => Ok(tree_sitter_swift::LANGUAGE.into()),
            Language::Zig => Ok(tree_sitter_zig::LANGUAGE.into()),
            Language::Haskell => Ok(tree_sitter_haskell::LANGUAGE.into()),
            _ => Err(anyhow!("No tree-sitter grammar for {:?}", lang)),
        }
    }
}
