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
- **Remembered answers in the prompt.** Feed the saved answers to every agent at start, so nobody asks the same thing twice.
