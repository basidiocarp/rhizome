# Rhizome Roadmap

This page is the Rhizome-specific backlog. The workspace [ROADMAP.md](../../docs/ROADMAP.md) keeps the ecosystem sequencing and cross-repo priorities.

## Recently Shipped

- Rhizome now has host-aware `init` flows and broader multi-host `doctor` checks. Setup and repair guidance is much less dependent on one default local environment.
- Shared path handling and Windows-safe managed binary behavior are in place. That makes the code-intelligence path more portable across operating systems.
- Project summarization and export-to-Hyphae code-graph support now exist as first-class workflows. Rhizome is no longer just a bag of structural queries.
- Rename preview, stronger impact summaries, and capability-aware confidence reporting make edit tooling safer than the original direct-apply model.
- Scoped workspace caching, persistent index files, and better invalidation mean repeated queries no longer start from zero every time.
- Tree-sitter coverage and symbol identity are broader than the initial narrow core. Nested symbols and container context now survive much more of the query surface.

## Next

### Change-impact analysis

Rhizome should expand impact analysis beyond grouped references into cross-file call graphs, dependency graphs, and materially better reasoning about what a change will touch. This item should stay aligned with the ecosystem roadmap because Hyphae, Cap, and Canopy all depend on stronger impact evidence.

### Workspace index

The current scoped cache files are a useful bridge, but larger repos need a stronger persistent index or daemon. That is how repeated queries stop paying full scan cost and how richer symbol identity becomes practical.

### Refactor preview

Rename is not the only risky edit. Dry-run and preview flows should extend to more edit operations so users can inspect what Rhizome plans to change before it writes to disk.

## Later

### Architectural summaries

Project summaries should grow from symbol listings into higher-level architecture views. That work becomes more valuable once the index and impact layers are stronger.

### Utility-backed LSP support

Rhizome should support non-language-specific LSP servers such as Biome when they provide useful repo-wide diagnostics, formatting, or edit intelligence that tree-sitter alone cannot cover well.

### Contributor tooling

Expanding language and query coverage will keep getting easier if the project has better contributor tooling and clearer docs for adding tree-sitter support.

### Non-standard containers

Jupyter and other non-standard code containers are a sensible future expansion, but only after the common source-file path is solid and in regular use.

### More query coverage

Offline parsing still lags behind the full set of supported languages in places. Expanding query coverage remains the right direction once the larger index and preview work lands.

## Research

### Semantic refactoring

The interesting next step is moving beyond symbol-level edits toward more semantic refactoring that combines tree-sitter precision with LSP-backed confidence. The question is how far that can go without becoming fragile across language backends.

### Cross-repo impact

Single-repo impact analysis comes first. After that, Rhizome can explore monorepo-wide and adjacent-repository reasoning where changes cross package or repo boundaries.
