# Tooling Adoption

Rhizome now has a repo-local test and profiling workflow instead of a generic
"just run cargo test" story.

## nextest

Use `cargo nextest` for the normal test loop when you want parallel execution,
cleaner failure output, or a faster pass over the workspace:

```bash
cargo nextest run --workspace
cargo nextest run -p rhizome-treesitter
cargo nextest run -p rhizome-cli
cargo nextest list --workspace
```

Use plain `cargo test` only when you need a specific test harness behavior that
`nextest` does not reproduce for a debugging session.

If the subcommand is missing, install `cargo-nextest` in your environment and
rerun the same commands.

## Criterion

Use Criterion when you want repeatable numbers for a specific code path inside
Rhizome.

The repo-local benchmark is:

```bash
cargo bench -p rhizome-treesitter --bench parse_symbols
```

It measures the real `TreeSitterBackend::get_symbols` path against a large Rust
fixture. That makes it useful for checking changes to parsing, query extraction,
or symbol shaping without involving the full CLI or MCP stack.

## Whole-command investigation

Use command timing plus targeted logging when you need to answer "where is the
time going?" across the full command path.

```bash
time cargo run -p rhizome-cli --bin rhizome -- symbols /absolute/path/to/file.rs
time cargo run -p rhizome-cli --bin rhizome -- serve
```

Prefer whole-command timing when:

- the slowdown spans parse, cache, and CLI layers
- you are investigating a production-like command path, not a narrow hot loop

Prefer Criterion when:

- you are comparing before/after changes on a stable code path
- you want a regression guard for symbol extraction or parsing work
- you need small, repeatable measurements that can be tracked over time
