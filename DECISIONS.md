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

## 9. Roles are folders with an AGENT.md, shared rules live in TEAM.md

2026-10-05

**Context:** the user wants each role described in its own file: what it does, who is on the team with it, how it works, how it searches the web.

**Decision:** `agents/<role>/AGENT.md` per role, plus one `agents/TEAM.md` with the rules all roles share (the protocol for talking to orda, web search rules, the guard, git). Each AGENT.md keeps the role-specific parts of the same topics, including when and what that role should search for. Agents in the config point at a role by folder name, so two agents can share one (builder and builder-2). The files are compiled into the binary and can be overridden from `~/.config/orda/agents/`. The vendor adapters moved from `src/agents` to `src/vendors` so the name `agents` means one thing.

**Rejected:** writing the shared rules into every AGENT.md. Eight copies of the protocol would drift, and a change to the protocol must change what orda parses at the same time.

**Consequences:** the protocol (`ASK:`, `REPORT:`, `LEARNED:`) is a contract between TEAM.md and `roles.rs`. The live roster is appended at start, so "who is on the team" in each AGENT.md describes roles, not the current names and models.

## 10. Agents talk to orda with plain prefixed lines

2026-10-05

**Context:** agents need to ask questions, report, and record what they learned, and they run as headless CLIs whose only output is their message stream.

**Decision:** three line prefixes in ordinary output, parsed by orda. Markdown bold or backticks around them are tolerated.

**Rejected:** giving each vendor a custom tool (an MCP server) for asking and reporting. More reliable to parse, but it needs a server per session and differs per vendor. Worth revisiting if agents often get the format wrong.

## 11. A scout role picks models from data, the owner approves

2026-10-05

**Context:** new models come out almost every week. The user wants an agent that finds them, with real data, and tells the boss which model should do which job.

**Decision:** a `scout` role. It scans when orda starts and the last scan is over 3 days old, or when the owner presses `s`. It only considers models the installed CLIs can run on the owner's plans (Claude Max, ChatGPT Plus, Google AI Pro). Evidence is public benchmarks and release notes plus orda's own record of every run (`stats.jsonl`). It proposes switches with `RECOMMEND:` lines; each becomes a question, and only an approval changes `config.toml`. Approvals and refusals are remembered for all projects, so a declined switch is not proposed again without new evidence.

**Rejected:** a daily scan (most days find nothing and still cost a session), a timer that runs while orda is closed (orda only needs the answer when it runs), applying switches automatically (one wrong switch slows the whole team), and API or local models (not usable without new adapters; the scout mentions them only as "worth watching").

**Consequences:** the first real start of orda runs a scan. The first live scan (2026-10-05) took 43 seconds, 10 searches, and recommended moving the builder from gpt-6-sol to gpt-6.1-sol.

## 12. The team column runs the full height

2026-10-05

**Context:** with the scout the team has seven workers. Two cards per row needs four rows, which did not fit above the bottom row; three per row made every card unreadable.

**Decision:** the default layout drops the bottom row. The team takes the whole left column, and the right grid has three rows: questions and git, web and guard, log and usage. The usage panel stacks limits above tokens when it is narrower than 80 columns. The old layout is still one config change away (`right` with four modules, `bottom = ["log", "usage"]`).

**Supersedes:** the bottom row part of the screen in DESIGN.md as first built.

## 13. Out of limits: switch at once, hand the task over, switch back by itself

2026-10-05

**Context:** a Codex plan runs out quickly on gpt-6.1-sol. The user wants the agent moved to the best Claude model for the same work (Opus 5.5) without being asked, carrying on from where it stopped, and moved back when the limit resets, with every change in the log.

**Decision:**
- `[fallback]` in config.toml maps each vendor to the model its agents use while that vendor is out of limits (codex to claude opus, claude to codex gpt-6.1-sol, gemini to claude sonnet). If the fallback is out too, its own fallback is tried; if every one is, the agent waits and resumes by itself at the first reset.
- A limit is detected from the stream (Codex "You've hit your usage limit ... try again at 3:05 PM", Claude's `rate_limit_event` with status `rejected` and no paid overage, or a 429 result), from the stderr of a failed run, and ahead of time from limit-watch when a 5 hour or weekly window is at 100%.
- On a hit, the vendor is blocked until its reset time (30 minutes when the message does not say), the run is stopped, and the same task is started on the fallback with a handoff: the original task, everything the previous model did (its tool calls, results and messages), and an instruction to check `git status` and `git diff` before continuing. The edits are already on disk.
- When the limit resets, an idle agent goes back to its own model at once; a busy one finishes its current run on the fallback first and goes back after it.
- The log shows each step ("codex limit hit, back at 15:05", "switched gpt-6.1-sol -> opus (claude)", "back on gpt-6.1-sol, switched from opus"), the card title says "opus · fallback", and the limits panel shows "out of limits, back at 15:05". Tokens stay booked to the vendor that spent them.

**Rejected:**
- Switching back in the middle of a run. Stopping a model mid-edit to hand the task back saves a little of the fallback's limit but risks a half-finished change, and the handoff costs a fresh context. The next run boundary comes soon enough.
- Resuming the same conversation on the new model. Vendors cannot read each other's sessions, so the handoff is a written summary plus the files on disk.
- Asking first. The user wants no interruption; the log is the record.

## 14. Agents message each other with typed cards, directly, boss copied

2026-10-06

**Context:** the user wants agents to work like a real team: the tester sends what failed to the builder, the builder fixes it and sends it back, and the owner sees each message travel between the cards. The builder should keep building rather than testing; the tester is the testing expert.

**Decision:**
- **Typed cards**, written by agents as `MSG <from> -> <to> [re #N]`, `key: value` lines, `END`. Kinds: bug, fixed, review, done, question, note, each with required fields (a bug needs title, repro, expected, actual, done when). orda numbers every card, threads replies by `re #N`, and logs missing fields.
- **Direct, boss copied.** A card goes straight to its receiver; the boss gets a digest of every card at the start of its next run instead of a run per card.
- **Delivery.** A free receiver starts at once with the whole thread as its task; a busy one gets it after its current run (an inbox per agent).
- **Builders run quick checks only** (build, lint, existing tests for the touched files) and never write new tests. The tester owns the test suite and writes a failing regression test before sending a bug.
- **Three rounds.** The third `bug` card in one thread sends an `escalation` card to the boss with the whole thread; the boss consults the advisor.
- **On screen** the card travels as a block with a fading trail along the gaps between cards and the connector row (never across a card), the team panel's title bar names it while it flies, and the receiving card's border lights up in the card's colour when it lands.

**Rejected:** free-text messages (the repro and the pass condition go missing), JSON (models break it), routing everything through the boss (a bottleneck that costs a boss run per message), builders writing their own tests (the user wants the split), a floating label beside the moving block (wider than the gap, it covered the neighbouring cards).

**Consequences:** a real Haiku tester wrote a complete, valid bug card on the first try and picked the builder from the branch name. Agents only learn whose commit it is from the branch (`wt/<name>`), so builders must commit there.

## 15. One wired network instead of a line per pair

2026-10-06

**Context:** the connector only joined the boss to the first row of workers, so most agents looked unrelated. The user wants every agent visibly connected to the boss and to every other agent, with messages travelling between any two.

**Decision:** every card plugs into one network: a drop from each card in the boss row into the connector row, a lane down each gap between worker columns, and a stub with a border junction from every worker into its lane. Messages follow the drawn wires (breadth-first search over the network), so what moves is always on a visible line.

**Rejected:** a separate line for every pair. Ten agents make 45 pairs; in a character grid they would cross and overlap into noise, and most could not be routed around the cards at all.

## 16. ripple: a role for what a commit breaks elsewhere

2026-10-06

**Context:** "fix one thing, break ten others". A change passes its tests because the tests check what the builder touched; what breaks is the code that depends on it. Even the strongest models do this. Nobody on the team owned that blast radius.

**Decision:** a `ripple` role, always on like the tester. For every commit it lists what the change touched at its edges (signatures, types, config keys, formats, output something parses) and every user of each. Leaf changes get a quick read of each user; shared ones get a deep check that builds the parent and the commit side by side and compares them on the same inputs. A break must be proven with a test that passes on the parent and fails on the commit. Then it sends a `bug` card to the builder, a `note` to the tester about dependents without tests, puts a `HOLD:` on the commit (the boss cannot merge it; the git panel shows "held by ripple"), and `RELEASE:`s it when the fix holds. Each break ends with a `LESSON:` line; orda appends it to the project's `LESSONS.md`, which every agent gets at the start of every run. ripple never edits project code. Default model: codex gpt-6.1-sol, high effort (it falls back to opus on limits like everyone).

**Rejected:** ripple fixing breaks itself (it would edit code beside the builders and become one more source of breaks, and the builder would never learn), advisory-only reports (a busy boss merges a known break), a deep check on every commit (too costly for one-file leaf changes), checking shared code only (small leaf changes hide a fair share of breaks).

## 17. aegis and a zero-token secret scanner

2026-10-06

**Context:** the user wants a security agent responsible for leaked secrets, personal and financial data, injection holes and easy ways in.

**Decision:** two layers. orda's own scanner (`secrets.rs`) reads the lines every new commit adds and looks for credentials by their known prefixes (AWS, Anthropic, OpenAI, GitHub, GitLab, Slack, Google, Stripe), private keys, passwords written as literals, committed `.env` and key files, email addresses and card numbers (Luhn checked). A hit holds the commit at once, asks the owner (a pushed secret must be rotated, not just deleted), and sends aegis a note. `aegis` (claude opus, always on) gives every commit a look, reads risky ones in depth (input handling, queries, commands, paths, auth, money, dependencies, configuration), proves each hole locally, holds the commit like ripple, asks the owner on critical findings, writes the safe pattern as a lesson, and audits the whole project weekly.

While designing it, thinking like aegis found a real hole in orda: the guard did not see commands inside `bash -c "..."`, `eval`, `$( )` or after wrappers like `env` and `nohup`. The guard now looks there too; quoted text that only mentions a risky command now asks the owner instead of passing, which costs one question at most.

**Rejected:** advisory only (a key is leaked the moment it is pushed), aegis fixing holes itself (it would edit beside the builders, who would never learn the safe pattern).

## 18. bursar: savings rules as measured experiments

2026-10-06

**Context:** the user wants an agent that watches everyone's token spending, the boss included, teaches them to spend less without making them worse, keeps learning from research, and works with the scout.

**Decision:** `bursar` (claude sonnet) runs every three days right before the scout and hands it its report. It reads orda's numbers for free: every run now records input and output tokens, tool calls and time. It adds rules with `SAVE: <role or all> | <rule>` and removes them with `DROP: <id>`. orda stores each rule with the success rate and tokens per run of its scope at that moment, gives it to every agent of that scope, and rolls it back by itself when the success rate falls more than 10 points over at least 10 runs. At most 12 rules are active. Model and effort changes stay with the scout and the owner; the bursar sends the scout its evidence in a note. The bursar's card shows tokens per run this week against last week.

**Rejected:** a report only (slow to have any effect), full automation including model and effort changes (the fastest way to quietly make the team worse).

## 19. orda watches new commits; bigger teams get shorter cards

2026-10-06

**Decision:** in real mode orda notices every new commit in the repo, runs the secret scanner on it, and (unless `[watch] commits = false`) hands it to the always-on agents (tester, ripple, aegis) as a task, now or after their current run. Commits that existed when orda started are not handed out. With ten workers the team panel shortens worker cards from 7 rows to 6 or 5 before leaving any out; the output lines go first.

## 20. referee: no claim without a run, no test bent to pass

2026-10-06

**Context:** the user picked four agents from research into AI-written code. Most completion claims from coding agents have no passing run behind them (65 to 69% in one audit of 516), and models that can edit their tests often weaken them instead of fixing the code (30% for o3 in METR's runs, 57 to 73% for one open model); telling them not to barely helps.

**Decision:** a `referee` (claude sonnet, high, always on). It gets every new commit and a copy of every `fixed` and `done` card, re-runs what a claim rests on in a clean checkout, and reads every test change for the known bends (asserts removed or loosened, skips, deleted tests, swallowed errors, special-cased test inputs, mocks of the thing under test). It holds commits and has the last word on "done". orda's free check (`integrity.rs`) flags the same patterns on every commit and sends them to the referee as a note, without holding: only the referee can tell a cleanup from a cheat.

## 21. customs: packages, versions, licenses

2026-10-06

**Context:** about a fifth of AI code samples name a package that does not exist, the same invented names recur and attackers register them; 25 to 38% of AI-written API calls are deprecated; license conflicts in commercial codebases are rising with AI code.

**Decision:** a free check (`deps.rs`) reads the dependency lists of Cargo.toml, package.json, pyproject.toml, requirements files and go.mod before and after each commit and asks each new package's registry (crates.io, npm, PyPI, the Go proxy) whether it exists; a package the registry does not know is held at once and sent to `customs`. `customs` (claude sonnet, always on, but only handed commits that touch a manifest or lockfile) checks names against lookalikes, age and maintainers, licenses, every call against the pinned version's docs, and copied code. Known vulnerabilities stay with aegis.

## 22. anchor: criteria before work, evidence after

2026-10-06

**Context:** agents silently drop requirements and drift from the spec with all tests green.

**Decision:** a task from the owner now goes to `anchor` first. It writes numbered, checkable acceptance criteria into the project's `SPEC.md`; the boss starts with them. When the boss finishes, anchor checks that every criterion has evidence and sends the boss a card for what is missing. Without an anchor on the team, tasks go straight to the boss as before.

## 23. curator and fresh contexts

2026-10-06

**Context:** coding agents degrade as a run grows: success falls after about 35 minutes, doubling a task's length roughly quadruples failures, instructions fade after 10 to 15 turns.

**Decision:** orda restarts any run past the limits in `[fresh]` (35 minutes, 80 tool calls or 160k context by default) with the same handoff it uses for limit switches. `curator` (claude haiku, low) keeps a short `MAP.md` of the project, which every agent gets at the start of every run so restarts and new runs do not have to explore, and points the owners of stale lessons and savings rules at them. It runs after every task the boss finishes.

## 24. Slim cards for quiet rows

2026-10-06

**Decision:** with fourteen workers the team panel first shortens cards, then shrinks rows where nobody is working (idle, queued, done, no message in the air) to three-line cards that stay wired. A row grows back the moment one of its agents starts working or receives a card. Positions never change.

**Deferred:** keel, pruner, pulse and mentor, with their evidence, are in IDEAS.md.

## 25. Real mode: worktrees, checks that gate merges, an enforced guard

2026-10-06

**Context:** the user wants to type a task in any folder and watch the team build it.

**Decision:**
- **Worktrees.** Builders, the tester and the commit checkers each work in their own git worktree under `~/.local/share/orda/wt/<project>-<id>/<agent>`, on a branch `wt/<agent>`. Builders work in parallel without touching each other's files or the owner's checkout; checkers get a worktree set to exactly the commit they check. The boss and the doc keepers (anchor, curator, scribe) work in the project. Only the boss merges, into the main branch, and nobody pushes.
- **Delegation through cards.** The boss hands out `task` cards; delivering a card starts the receiver. Builders answer with a `done` card.
- **Checks gate merges.** orda records who is checking each builder commit; the boss is asked to merge only when the last check is in and nothing holds the commit. `FINISHED:` marks the whole task done.
- **The guard is enforced.** Claude agents run with edits and shell allowed, and `orda hook` as a PreToolUse hook that denies guarded commands in every permission mode. Codex runs in its workspace-write sandbox. A new folder is made a git repo on the first task.
- Researcher and scribe moved from Gemini to Claude until a Gemini adapter exists; the reviewer is now always on, since merges wait for its check.

**Rejected:** one shared checkout for everyone (parallel builders would overwrite each other), merging on a builder's word (the whole point of the checks), Claude's `bypassPermissions` (the hook and acceptEdits give the same autonomy with a real guard).

## 26. What the first real run taught

2026-10-06

**Context:** the owner's first real task ("build a command line todo app in Python...") in `~/dev/hello-orda` ran 83 agent runs in ten minutes and produced tests but no app, with nothing merged.

**Findings and fixes:**
- **The Codex model did not exist for this account.** `gpt-6.1-sol` had been made the default after the scout recommended it from a blog post saying it was on the Plus plan; the API answers "not supported when using Codex with a ChatGPT account". Every Codex agent died at once. Defaults are back to `gpt-6-sol`. A model the vendor refuses now becomes `AgentEvent::ModelUnavailable`: that one model is blocked for the day (`codex:gpt-6-sol`, not the whole vendor) and the agent moves to its fallback with a handoff, like a used-up limit. An approved scout switch is applied only after `vendors::probe` gets an answer from the model.
- **A failed run went unnoticed.** The builder that should have written the app died and nobody re-did its work. A failed run now sends the boss a `failure` card.
- **Agents thanked each other forever.** Every `done` and `note` card started its receiver, which answered with another one: the referee and builder-2 exchanged about twenty. Only `task`, `bug`, `review`, `fixed`, `question`, `escalation` and `failure` cards start a run now (`mail::wakes`); `done` and `note` wait for the receiver's next run. TEAM.md forbids acknowledgement cards. The referee verifies a claim about a commit once.
- **Nobody could tell whether it was finished.** The header now shows the task's state (working, waiting for you, waiting for a limit, idle, stalled, finished) and a task clock that stops when the task is done. orda wakes the boss after 20 seconds with nothing running and the task unfinished, at most 3 times without a new commit, then calls it stalled. The task is finished when anchor's final check finds nothing missing (or at the boss's `FINISHED:` without an anchor); a desktop notification says so.
- **Tokens were overcounted** about tenfold by counting cache reads, which repeat every turn. Spent tokens now leave out cache reads (Claude) and cached input (Codex).
- **The git panel hid the work.** It listed only the main branch's commits. It now lists every branch's commits with the branch each was made on (found in the branch reflogs, since checkers' branches contain the commits they check), and each worktree with how many commits it has left to merge.
- **There was no log to read afterwards.** Every log line also goes to `~/.local/share/orda/logs/<project>.log`.
