# Rhizome Roadmap

This page is the Rhizome-specific backlog. The workspace [ROADMAP.md](../../ROADMAP.md) keeps the ecosystem sequencing, and [MASTER-ROADMAP.md](../../MASTER-ROADMAP.md) keeps the cross-repo summary.

## Recently Shipped

- Host-aware `init` flows instead of one generic setup path.
- Broader multi-host `doctor` checks and repair guidance.
- Shared path handling and Windows-safe managed binary and PATH behavior.
- Project-level summarize workflow.
- Export-to-Hyphae memoir and code-graph path.
- Expanded tree-sitter query coverage beyond the original small core set.
- Existing rename and workspace-edit foundation, symbol move and copy MVP, and worktree-aware cache partitioning remain in place.

## Next

### Change-impact analysis

Add cross-file call graphs, dependency graphs, and materially better change-impact reasoning.

### Workspace index

Add a persistent workspace index or daemon for larger repos so repeated queries stop paying full parse cost.

### Refactor preview

Add dry-run and preview flows before applying rename or edit operations.

### Export reliability

Keep the `rhizome` to `hyphae` export path reliable and boring in both tests and real usage.

## Later

### Architectural summaries

Expand project summaries from symbol listings into higher-level architectural overviews.

### Contributor tooling

Add clearer contributor tooling and docs for expanding tree-sitter language and query coverage.

### Non-standard containers

Add support for Jupyter or other non-standard code containers if real demand appears.

### More query coverage

Keep expanding tree-sitter coverage where offline parsing still lags behind supported languages.

## Research

### Semantic refactoring

Go beyond symbol-level edits toward more semantic refactoring that combines tree-sitter precision with LSP-backed confidence.

### Cross-repo impact

Explore change-impact reasoning across monorepos or adjacent repositories once the single-repo path is stable.
