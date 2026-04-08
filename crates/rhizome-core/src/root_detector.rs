use std::path::{Path, PathBuf};

use crate::Language;

// ─────────────────────────────────────────────────────────────────────────────
// Root markers per language
// ─────────────────────────────────────────────────────────────────────────────

impl Language {
    /// Files that indicate a project root for this language.
    pub fn root_markers(&self) -> &[&str] {
        match self {
            Language::Rust => &["Cargo.toml", "Cargo.lock"],
            Language::Python => &[
                "pyproject.toml",
                "setup.py",
                "setup.cfg",
                "requirements.txt",
                "Pipfile",
                "pyrightconfig.json",
            ],
            Language::JavaScript | Language::TypeScript => {
                &["tsconfig.json", "package.json", "jsconfig.json"]
            }
            Language::Go => &["go.work", "go.mod", "go.sum"],
            Language::Java => &["pom.xml", "build.gradle", "build.gradle.kts"],
            Language::C | Language::Cpp => &["CMakeLists.txt", "compile_commands.json", "Makefile"],
            Language::Ruby => &["Gemfile"],
            Language::Elixir => &["mix.exs", "mix.lock"],
            Language::Zig => &["build.zig", "build.zig.zon"],
            Language::CSharp => &[".sln", ".slnx", ".csproj", "global.json"],
            Language::FSharp => &[".sln", ".slnx", ".fsproj", "global.json"],
            Language::Swift => &["Package.swift"],
            Language::Php => &["composer.json", "composer.lock"],
            Language::Haskell => &["cabal.project", "stack.yaml", "*.cabal"],
            Language::Bash => &[],
            Language::Terraform => &["main.tf", "terraform.tfvars"],
            Language::Kotlin => &["build.gradle.kts", "build.gradle", "pom.xml"],
            Language::Dart => &["pubspec.yaml", "pubspec.lock"],
            Language::Lua => &[".luarc.json", ".luacheckrc"],
            Language::Clojure => &["deps.edn", "project.clj", "build.clj"],
            Language::OCaml => &["dune-project", "dune", "*.opam"],
            Language::Julia => &["Project.toml", "Manifest.toml"],
            Language::Nix => &["flake.nix", "default.nix", "shell.nix"],
            Language::Gleam => &["gleam.toml"],
            Language::Vue | Language::Svelte | Language::Astro | Language::Prisma => {
                &["package.json", "tsconfig.json"]
            }
            Language::Typst => &["typst.toml"],
            Language::Yaml => &[],
            Language::Other(_) => &[],
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Root detection
// ─────────────────────────────────────────────────────────────────────────────

/// Detect the workspace root for a given file and language.
///
/// Walks up from the file's directory looking for language-specific project
/// markers. For Rust, specifically looks for `[workspace]` in Cargo.toml
/// to find the workspace root. For Go, prefers `go.work` over `go.mod`.
///
/// Falls back to the nearest `.git` directory, then the file's parent.
pub fn detect_workspace_root(file: &Path, language: &Language, stop_at: &Path) -> PathBuf {
    let start_dir = if file.is_file() {
        file.parent().unwrap_or(file)
    } else {
        file
    };

    match language {
        Language::Rust => detect_rust_root(start_dir, stop_at),
        Language::Go => detect_go_root(start_dir, stop_at),
        Language::TypeScript | Language::JavaScript => detect_js_root(start_dir, stop_at),
        _ => detect_generic_root(start_dir, language, stop_at),
    }
}

/// Rust: walk up looking for `Cargo.toml` with `[workspace]`, then fall back
/// to the nearest `Cargo.toml`.
fn detect_rust_root(start: &Path, stop_at: &Path) -> PathBuf {
    let mut nearest_cargo = None;
    let mut workspace_root = None;
    let mut dir = start;

    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if nearest_cargo.is_none() {
                nearest_cargo = Some(dir.to_path_buf());
            }
            if let Ok(content) = std::fs::read_to_string(&cargo_toml)
                && content.contains("[workspace]")
            {
                workspace_root = Some(dir.to_path_buf());
            }
        }

        if dir == stop_at || !dir.starts_with(stop_at) {
            break;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => break,
        }
    }

    workspace_root
        .or(nearest_cargo)
        .unwrap_or_else(|| fallback_root(start, stop_at))
}

/// Go: prefer `go.work` (workspace mode), then `go.mod`.
fn detect_go_root(start: &Path, stop_at: &Path) -> PathBuf {
    // First pass: look for go.work
    if let Some(root) = walk_up_for_file(start, "go.work", stop_at) {
        return root;
    }
    // Second pass: look for go.mod
    if let Some(root) = walk_up_for_file(start, "go.mod", stop_at) {
        return root;
    }
    fallback_root(start, stop_at)
}

/// JS/TS: look for tsconfig.json or package.json, but skip dirs with deno.json.
fn detect_js_root(start: &Path, stop_at: &Path) -> PathBuf {
    let mut dir = start;
    loop {
        // Skip Deno projects
        if dir.join("deno.json").exists() || dir.join("deno.jsonc").exists() {
            if dir == stop_at || !dir.starts_with(stop_at) {
                break;
            }
            match dir.parent() {
                Some(parent) if parent != dir => {
                    dir = parent;
                    continue;
                }
                _ => break,
            }
        }

        for marker in &["tsconfig.json", "jsconfig.json", "package.json"] {
            if dir.join(marker).exists() {
                return dir.to_path_buf();
            }
        }

        if dir == stop_at || !dir.starts_with(stop_at) {
            break;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => break,
        }
    }
    fallback_root(start, stop_at)
}

/// Generic: walk up looking for any of the language's root markers.
fn detect_generic_root(start: &Path, language: &Language, stop_at: &Path) -> PathBuf {
    let markers = language.root_markers();
    if markers.is_empty() {
        return fallback_root(start, stop_at);
    }

    let mut dir = start;
    loop {
        for marker in markers {
            if dir_matches_marker(dir, marker) {
                return dir.to_path_buf();
            }
        }

        if dir == stop_at || !dir.starts_with(stop_at) {
            break;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => break,
        }
    }
    fallback_root(start, stop_at)
}

fn dir_matches_marker(dir: &Path, marker: &str) -> bool {
    if let Some(suffix) = marker.strip_prefix("*.") {
        return std::fs::read_dir(dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .any(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext == suffix)
            });
    }

    dir.join(marker).exists()
}

/// Walk up from `start` looking for a specific file, stopping at `stop_at`.
fn walk_up_for_file(start: &Path, filename: &str, stop_at: &Path) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        if dir.join(filename).exists() {
            return Some(dir.to_path_buf());
        }
        if dir == stop_at || !dir.starts_with(stop_at) {
            return None;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => return None,
        }
    }
}

/// Fallback: look for `.git`, then return `stop_at`.
fn fallback_root(start: &Path, stop_at: &Path) -> PathBuf {
    walk_up_for_file(start, ".git", stop_at).unwrap_or_else(|| stop_at.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rust_finds_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create workspace structure
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate-a\"]",
        )
        .unwrap();
        fs::create_dir_all(root.join("crate-a/src")).unwrap();
        fs::write(root.join("crate-a/Cargo.toml"), "[package]\nname = \"a\"").unwrap();
        fs::write(root.join("crate-a/src/main.rs"), "fn main() {}").unwrap();

        let detected =
            detect_workspace_root(&root.join("crate-a/src/main.rs"), &Language::Rust, root);
        assert_eq!(detected, root);
    }

    #[test]
    fn rust_falls_back_to_nearest_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Single crate, no workspace
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"foo\"").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let detected = detect_workspace_root(&root.join("src/main.rs"), &Language::Rust, root);
        assert_eq!(detected, root);
    }

    #[test]
    fn go_prefers_go_work() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("go.work"), "go 1.21").unwrap();
        fs::create_dir_all(root.join("svc/cmd")).unwrap();
        fs::write(root.join("svc/go.mod"), "module svc").unwrap();
        fs::write(root.join("svc/cmd/main.go"), "package main").unwrap();

        let detected = detect_workspace_root(&root.join("svc/cmd/main.go"), &Language::Go, root);
        assert_eq!(detected, root);
    }

    #[test]
    fn go_falls_back_to_go_mod() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("cmd")).unwrap();
        fs::write(root.join("go.mod"), "module myapp").unwrap();
        fs::write(root.join("cmd/main.go"), "package main").unwrap();

        let detected = detect_workspace_root(&root.join("cmd/main.go"), &Language::Go, root);
        assert_eq!(detected, root);
    }

    #[test]
    fn js_skips_deno_projects() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Parent has package.json, subdir has deno.json
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::create_dir_all(root.join("deno-app")).unwrap();
        fs::write(root.join("deno-app/deno.json"), "{}").unwrap();
        fs::write(root.join("deno-app/main.ts"), "").unwrap();

        let detected =
            detect_workspace_root(&root.join("deno-app/main.ts"), &Language::TypeScript, root);
        // Should skip deno-app/ and find root/package.json
        assert_eq!(detected, root);
    }

    #[test]
    fn python_finds_pyproject() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src/myapp")).unwrap();
        fs::write(root.join("pyproject.toml"), "[project]").unwrap();
        fs::write(root.join("src/myapp/main.py"), "").unwrap();

        let detected =
            detect_workspace_root(&root.join("src/myapp/main.py"), &Language::Python, root);
        assert_eq!(detected, root);
    }

    #[test]
    fn fallback_to_stop_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("random")).unwrap();
        fs::write(root.join("random/file.rb"), "").unwrap();

        let detected = detect_workspace_root(&root.join("random/file.rb"), &Language::Ruby, root);
        assert_eq!(detected, root);
    }

    #[test]
    fn fallback_uses_worktree_root_when_git_is_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path().join("repo");
        let worktree_root = dir.path().join("wt1");
        let nested_dir = worktree_root.join("packages/app");

        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("script.rb"), "").unwrap();
        fs::create_dir_all(repo_root.join(".git/worktrees/wt1")).unwrap();
        fs::write(
            worktree_root.join(".git"),
            "gitdir: ../repo/.git/worktrees/wt1\n",
        )
        .unwrap();

        let detected = detect_workspace_root(
            &nested_dir.join("script.rb"),
            &Language::Ruby,
            &worktree_root,
        );
        assert_eq!(detected, worktree_root);
    }

    #[test]
    fn haskell_root_detection_supports_cabal_wildcards() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("demo.cabal"), "name: demo").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        let file = root.join("src/Main.hs");
        fs::write(&file, "main = putStrLn \"hi\"").unwrap();

        let detected = detect_workspace_root(&file, &Language::Haskell, root);
        assert_eq!(detected, root);
    }

    #[test]
    fn ocaml_root_detection_supports_opam_wildcards() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("demo.opam"), "opam-version: \"2.0\"").unwrap();
        fs::create_dir_all(root.join("lib")).unwrap();
        let file = root.join("lib/main.ml");
        fs::write(&file, "let () = print_endline \"hi\"").unwrap();

        let detected = detect_workspace_root(&file, &Language::OCaml, root);
        assert_eq!(detected, root);
    }
}
