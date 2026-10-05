# Ideas

Not decided, not scheduled. Delete an idea once it is built or rejected. When and how to edit this file is described in `CLAUDE.md`.

- **Settings screen.** An in-app editor on top of `config.toml`: reorder modules, pick colours, add agents, with live preview. The file stays the source of truth (DECISIONS #6).
- **Mouse.** Click a card to talk to that agent, click option chips to answer, scroll the log.
- **Routing rules.** A small config saying which vendor gets which kind of task, for example Gemini for quick lookups and reading large codebases, Codex for implementation, Claude for review.
- **Cost panel.** Money per agent and per task, plus what the same task would have cost on one model only (the concept showed "all on opus would be ~410k").
- **Git worktree per worker.** So two workers never edit the same files at once. The git panel already shows worktrees.
- **Talk to a worker directly.** Select a card and type to it, without going through the boss.
- **Session replay.** Save the event streams so a finished session can be played back on the same screen.
- **Local models.** An Ollama adapter for small jobs on the RTX 5070.
- **An orda MCP server.** `ask`, `report` and `learned` as real tools instead of prefixed lines (DECISIONS #10), if agents often get the line format wrong.
- **Lessons across projects.** Let ripple mark a lesson as general so it goes into a global file every project reads, not just this project's `LESSONS.md`.
- **`orda scan`.** Run the secret scanner on a commit range or the whole history from the command line, for repos that existed before orda.
- **keel: architecture and conventions.** Keeps `ARCHITECTURE.md` (modules, what may depend on what, which helper does which job) and checks each commit for a reinvented helper, a second way of doing something, or a broken boundary. Why: each AI change is reasonable alone and inconsistent with the rest; cross-file calls fell 35% and refactoring 70% as AI tools spread (GitClear, The Maintainability Gap 2026).
- **pruner: the full-time deleter.** In its own worktree, merges duplicates, removes dead code and flags, turns swallowed errors into real handling, flattens one-user abstractions; behaviour-preserving, gated by tester, ripple and referee, weekly within a token budget. Why: duplicated blocks +81%, error-masking code +47% (GitClear 2026), AI code 20 to 30% more verbose.
- **pulse: performance.** Keeps small benchmarks for the hot paths, runs parent and commit, holds a commit over a regression threshold with the numbers and the cause. Why: AI code that passes its tests often regresses performance through inefficient calls, loops and algorithms (Empirical Software Engineering, 2026).
- **mentor: comprehension.** After each merged task, what changed and why in plain words with the three places to look; tours of an area on request. Why: comprehension debt, code that passes every check but nobody can explain or take over.
