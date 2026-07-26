# CodeGraph reference: commit hook and native CLAUDE.md integration

Load this when the user asks to install the post-commit hook or wire CodeGraph into a project's CLAUDE.md.

## For git commit hook

Install a post-commit hook that auto-rebuilds the graph after every commit. No background process needed - triggers once per commit, works with any editor.

```bash
./scripts/codegraph hook install    # install
./scripts/codegraph hook uninstall  # remove
./scripts/codegraph hook status     # check
```

After every `git commit`, the hook detects which code files changed (via `git diff HEAD~1`), re-runs AST extraction on those files, and rebuilds `graph.json` and `GRAPH_REPORT.md`. Doc/image changes are ignored by the hook - run `/codegraph --update` manually for those.

If a post-commit hook already exists, CodeGraph appends to it rather than replacing it.

## PreToolUse hook-check (already wired in this repo)

`.codex/hooks.json` runs `./scripts/codegraph hook-check` before every Bash tool call. The runtime defines `hook-check` as an always-exit-0 no-op (graph guidance reaches the agent via AGENTS.md instead), and the wrapper preserves that no-op even when the `graphify` runtime is not installed — a missing optional tool must never break Bash calls. Do not add logic to `hook-check` that can exit non-zero.

---

## For native CLAUDE.md integration

Run once per project to make CodeGraph always-on in Claude Code sessions:

```bash
./scripts/codegraph claude install
```

This writes a `## CodeGraph` section to the local `CLAUDE.md` that instructs Claude to check the graph before answering codebase questions and rebuild it after code changes. No manual `/codegraph` needed in future sessions.

```bash
./scripts/codegraph claude uninstall  # remove the section
```
