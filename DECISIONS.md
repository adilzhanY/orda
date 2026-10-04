# Decisions

Newest at the bottom. Never edit or delete an old entry. If a decision changes, add a new one and mark the old one "Superseded by #N". When and how to add entries is described in `CLAUDE.md`.

---

## 1. Build our own tool instead of using an existing orchestrator

2026-10-05

**Context:** the user wants one boss agent that runs Claude Code, Codex and Gemini workers with different roles.

**Decision:** build orda from scratch.

**Rejected:**
- PAL MCP `clink`: it handles all three vendors, but its workers are one-shot background calls with no view of their own, and the look is not ours.
- multiagents, OpenRig, Cyclops, maestro: young projects, and the UI is fixed by someone else.
- Conduit: polished, but it only shows sessions side by side. There is no boss agent.
- Claude Code agent teams + advisor: Claude models only.

**Consequences:** we own the look completely, and we also own every bug in the orchestration.

## 2. Name: orda

2026-10-05

**Context:** the user names projects with Kazakh words respelled for English readers.

**Decision:** `orda` (орда, the khan's camp that commanded the clans; English "horde" comes from it).

**Rejected:** ordai (an "AI" pun that would date quickly), boorkit, kenes, shanirak.

**Consequences:** orda.dev and orda.com are taken, so the project will need a different domain if it ever gets a site.

## 3. Rust + Ratatui, not Tauri or Python

2026-10-05

**Context:** the user wants the app fast and optimized, and wants to design every component.

**Decision:** Rust with Ratatui, crossterm and Tokio.

**Rejected:**
- Tauri: it builds desktop apps with a web view, not terminal apps.
- Python + Textual: the most ready-made widgets, but about 300 ms startup and 60 to 100 MB of memory, and it can stutter with many agents streaming at once. We draw our own components anyway, so its widget library is not worth the cost.
- Go + Bubble Tea: a good middle ground, but no faster to build in than Ratatui for us, and slower at runtime.
- Ink / OpenTUI (TypeScript): Ink slows down under many live streams, and OpenTUI is still young.

**Consequences:** startup in a few ms, one binary, small memory use. More code to write by hand, since buttons, loaders and cards are all ours. Codex CLI uses the same stack, which shows it holds up for this kind of app.

## 4. Drive agents headless through JSON streams

2026-10-05

**Context:** orda needs to show what each agent is doing.

**Decision:** run each CLI in headless mode with JSON output and draw our own card from its events. One adapter per vendor maps its events to one shared event type.

**Rejected:** embedding each vendor's own TUI in a pseudo-terminal pane. Three cramped foreign UIs would make orda look like a tmux layout, and we could not read structured state (tokens, tool calls, status) from them.

**Consequences:** orda depends on each CLI's JSON format, which can change between versions. Each adapter must fail loudly on an unknown event instead of silently dropping it.

## 5. Two themes: "orda" by default, "iris" as a switch

2026-10-05

**Context:** the dashboard concept used coral, blue, teal and violet on dark grey, taken from the Claude Code advisor animation. The user liked it and also wants Iris, their shell's design system, available.

**Decision:** two built-in themes. `orda` (the concept palette) is the default; `iris` is pure black with greys and white. Every colour can be overridden in the config.

**Rejected:** Iris only. Iris has no accent colours, so vendors could not be told apart at a glance.

**Consequences:** under `iris` the vendors are told apart by their labels, not by colour, which is what Iris asks for. Anyone who wants vendor colours on black can set them in `[colors]`.

## 6. Settings live in a TOML file, like herdr

2026-10-05

**Context:** the user wants to change anything: module order, colours, sizes.

**Decision:** `~/.config/orda/config.toml`. `orda config` writes a commented default file and prints its path. orda reloads it while running when the file changes.

**Rejected:** an in-app settings screen first. A file is diffable, can be backed up with the rest of the dotfiles, and is what herdr does. A settings screen can be added on top later (see IDEAS.md).

**Consequences:** every visual choice must be a config key with a default, never a constant buried in a draw function.

## 7. Small terminals collapse the panels into tabs

2026-10-05

**Decision:** below a configurable width or height (default 170 x 44 cells), orda shows one module at a time with a tab bar. The limits line stays visible in every tab.

**Rejected:** squeezing every panel smaller. Cards with three characters of content are worse than one readable tab.

## 8. Risky commands ask the user first

2026-10-05

**Decision:** the guard has two lists. `deny` commands are blocked outright (rm -rf on / or ~, dd onto a device, curl piped into a shell). `ask` commands (git push, anything with sudo, writes outside the project) pause that one agent and put a question in the questions panel; every other agent keeps working.

**Consequences:** the guard needs the same non-blocking question flow as normal questions, so both share one implementation.
