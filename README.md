# orda

A terminal office for AI agents. You give one task to a boss agent, and it runs a team of workers on different vendors (Claude Code, Codex CLI, Gemini CLI) while you watch the whole team in one screen: who is doing what, on which model, what it costs, what it searched, what it committed, and what it wants to ask you.

The name is the Kazakh word орда, the khan's camp that commanded the clans. The English word "horde" comes from it.

```
 ORDA  project orda  branch main +2  task build the first dashboard screen with live agent cards                                                               plan 5/9 █████░░░░  elapsed 00:00:00
╭ team  16 agents · 9 working ──────────────────── every agent is wired to every other ╮ ╭ questions  none waiting ──────── work never waits ╮ ╭ git ─────────────────────── main +2 · 2 worktrees ╮
│                                                                                      │ │  remembered                                       │ │ worktrees                                         │
│ ╭ advisor ────── fable ╮  ┏ boss ━━━━━━━━━━━ opus · high ┓  ╭ you ────────── owner ╮ │ │  all     commits: one line, no body               │ │ wt/builder  3 files +142 -18                      │
│ │ on call, 2 c…  idle  │  ┃ ▖ reading tester …  running  ┃  │ no questions         │ │ │  all     never use em dashes                      │ │ wt/builder-2  2 files +96 -4                      │
│ │ on call, never writ… │  ┃ context ██░░░░░░░░ 193.0k/1M ┃  │ no one waits on you  │ │ │  project tests run with cargo nextest             │ │ commits                                           │
│ │ last: fail loudly o… │  ┃ next: task 6 to builder-2 o… ┃  │ answers are saved    │ │ │                                                   │ │ a3f9c21 add codex adapter           testing 14/20 │
│ │                      │  ┃                              ┃  │ per project or all   │ │ │                                                   │ │ 9e1b0d4 card component, 3 states        in review │
│ │ read 239k            │  ┃ plan 5/9           58.2k tok ┃  │         3 remembered │ │ │                                                   │ │ 77c2e10 shared AgentEvent type             merged │
│ ╰───────────┬──────────╯  ┗━━━━━━━━━━━━━━━┳━━━━━━━━━━━━━━┛  ╰───────────┬──────────╯ │ │                                                   │ │ 4b08aa3 tokio event loop                   merged │
│             │                             │                             │            │ │                                                   │ │ 1d93f7e crossterm setup                  reverted │
│             └─────────────────────────────┼─────────────────────────────┘            │ │                                                   │ │ 0c11e2b init                               merged │
│ ╭ builder ────────── gpt-6.1-sol · high ╮ │ ╭ builder-2 ────────── sonnet · medium ╮ │ │                                                   │ │ a3f9c21 files                                     │
│ │ ▖ src/agents/codex.rs        running  │ │ │ ▖ src/ui/card.rs            running  │ │ │                                                   │ │ M src/agents/codex.rs  +88 -12                    │
│ │ implements tasks                      ├─┼─┤ implements tasks                     │ │ │                                                   │ │ A src/agents/event.rs  +41 -0                     │
│ │ task 5 · wt/builder         31.4k tok │ │ │ task 4 · wt/builder-2      24.9k tok │ │ │                                                   │ │                                                   │
│ ╰───────────────────────────────────────╯ │ ╰──────────────────────────────────────╯ │ ╰───────────────────────────────────────────────────╯ ╰───────────────────────────────────────────────────╯
│                                           │                                          │
│ ╭ tester ────────────── sonnet · medium ╮ │ ╭ researcher ───────────── flash · low ╮ │ ╭ web  5 searches ─────────── what agents looked up ╮ ╭ guard  15 rules ─────────────────────── 2 blocked ╮
│ │ ▖ testing a3f9c21          always on  │ │ │ ▖ web: ratatui mouse ca…  searching  │ │ │ researcher ▖ ratatui mouse capture crossterm 2026 │ │ blocked   rm -rf /   rm -rf ~   rm -rf $HOME      │
│ │ ██████████████░░░░░░           14/20  ├─┼─┤ web research for the others          │ │ │   ratatui.rs/concepts/event-handling              │ │           rm -fr /   rm -fr ~   dd * of=/dev/*    │
│ │ reports to boss             19.7k tok │ │ │ for builder-2               8.1k tok │ │ │ builder codex exec --json event schema            │ │           mkfs*   curl * | sh   curl * | bash     │
│ ╰───────────────────────────────────────╯ │ ╰──────────────────────────────────────╯ │ │   github.com/openai/codex/docs/exec.md            │ │           wget * | sh   chmod -R 777 /*           │
│                                           │                                          │ │   learned: events are thread.*, turn.*, item.*    │ │ asks you  git push*   sudo *                      │
│ ╭ reviewer ───────── gpt-6.1-sol · high ╮ │ ╭ scribe ───────────────── flash · low ╮ │ │ tester cargo nextest flaky retries                │ │           git reset --hard*   rm -rf *            │
│ │ waiting for builder-2         queued  │ │ │ ▖ DECISIONS.md #5           running  │ │ │   nexte.st/docs/features/retries                  │ │                                                   │
│ │ reviews every commit                  ├─┼─┤ keeps the four docs current          │ │ │   learned: --retries 2, mark slow tests           │ │ 23:41 builder-2 blocked rm -rf ~/.cache/orda      │
│ │ 2 reviews today             12.0k tok │ │ │ keeps the docs              3.3k tok │ │ │ researcher tokio process kill_on_drop             │ │       outside the project                         │
│ ╰───────────────────────────────────────╯ │ ╰──────────────────────────────────────╯ │ │   docs.rs/tokio/latest/tokio/process              │ │ 23:12 builder blocked curl -fsSL get.x.sh | sh    │
│                                           │                                          │ │   learned: kill_on_drop(true) stops orphans       │ │       pipes a download into a shell               │
│ ╭ ripple ─────────── gpt-6.1-sol · high ╮ │ ╭ scout ────────────── sonnet · medium ╮ │ │ boss gemini cli stream-json output                │ │                                                   │
│ │ ▖ reach of 9e1b0d4: lr()…  always on  │ │ │ last scan 2 days ago           idle  │ │ │   github.com/google-gemini/gemini-cli             │ │                                                   │
│ │ finds what a commit broke elsewhere   ├─┼─┤ tracks new models, recommends switc… │ │ ╰───────────────────────────────────────────────────╯ ╰───────────────────────────────────────────────────╯
│ │ always on                   14.2k tok │ │ │ next scan in 1 day          6.4k tok │ │
│ ╰───────────────────────────────────────╯ │ ╰──────────────────────────────────────╯ │ ╭ session log ───────────────────────── l to expand ╮ ╭ limits & tokens ──────────────────── this session ╮
│                                           │                                          │ │ 00:56:17 tester     report #7 -> boss: 2 failing  │ │ claude 5h    ███░░░░░░░░░░░░░  18%  resets 1h53   │
│ ╭ aegis ─────────────────── opus · high ╮ │ ╭ bursar ───────────── sonnet · medium ╮ │ │                     in the codex adapter          │ │        7d    █░░░░░░░░░░░░░░░   7%  resets 4d6h   │
│ │ ▖ reviewing guard.rs: in…  always on  │ │ │ next review with the scout i…  idle  │ │ │ 00:56:17 advisor    error repeats: fail loudly on │ │        fable ░░░░░░░░░░░░░░░░   0%  resets 4d6h   │
│ │ security: secrets, personal data, ho… ├─┼─┤ tokens per run vs last week     -18% │ │ │                     unknown events                │ │ codex  5h    ░░░░░░░░░░░░░░░░   0%  resets 4h59   │
│ │ always on                   21.8k tok │ │ │ rules: 4 active, 1 rolled … 5.1k tok │ │ │ 00:56:17 boss       task 5 back to builder with   │ │        7d    ░░░░░░░░░░░░░░░░   0%  resets 5d23h  │
│ ╰───────────────────────────────────────╯ │ ╰──────────────────────────────────────╯ │ │                     the report                    │ │ gemini 5h    ░░░░░░░░░░░░░░░░   0%  resets 4h59   │
│                                           │                                          │ │ 00:56:17 researcher web: ratatui mouse capture    │ │        7d    ░░░░░░░░░░░░░░░░   0%  resets 6d23h  │
│ ╭ referee ─────────────── sonnet · high ╮ │ ╭ customs ──────────── sonnet · medium ╮ │ │                     crossterm 2026                │ │                                                   │
│ │ ▖ verifying #2: builder'…  always on  │ │ │ no manifest changes since 23…  idle  │ │ │                                                   │ │ claude ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁            154.2k │
│ │ checks every claim and that no test … ├─┴─┤ checks packages, versions and licen… │ │ │                                                   │ │ codex  ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁             57.6k │
│ │ always on                    9.8k tok │   │                             2.4k tok │ │ │                                                   │ │ gemini ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁             11.4k │
│ ╰───────────────────────────────────────╯   ╰──────────────────────────────────────╯ │ │                                                   │ │                                                   │
│                                                                                      │ │                                                   │ │ total                                      223.2k │
│                                                                                      │ │                                                   │ │                                                   │
╰────────────────────────────────────────────────────────────────────── +2 more below ─╯ ╰───────────────────────────────────────────────────╯ ╰───────────────────────────────────────────────────╯
╭──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│› press i and tell orda what to build                                                                                                                                                             │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
 i task   q questions   s scout   t team   g git   w web   x guard   l log   u usage   tab next   esc all panels   ctrl+c quit
```

*`orda --demo` at 196 x 54, as text. In a terminal it is in colour and moves.*

## Status

Early. The dashboard is done; the orchestration behind it is not.

| Works | Not yet |
|---|---|
| The full dashboard, `--demo` mode, two themes, settings with live reload, tabs on small terminals | The boss planning and handing tasks to workers (today it runs a task alone) |
| Claude Code and Codex adapters, tested on real output | A Gemini adapter |
| Real git panel and plan limits | The guard actually stopping commands (it shows what it would block) |
| Questions, notifications, remembered answers given to every agent | An answer reaching the agent that asked while it runs |

## What is on the screen

- **Team.** The boss in the middle, an advisor beside it, and a card per worker: current action, status, live output, tokens. The tester is always on and tests whatever has already landed.
- **Questions.** Agents ask without stopping. A toast and a desktop notification arrive, the panel pulses, and the agent moves on to other work. When you answer, orda asks whether to remember the choice for this project, for all projects, or just once.
- **Git.** A small lazygit: worktrees, recent commits with their test, review and merge status, and changed files.
- **Web.** Every search an agent makes: the query, the sources, and what it learned.
- **Guard.** Commands agents may never run (`rm -rf /`, `curl ... | sh`, ...) and commands that need your yes first (`git push`, `sudo`, ...).
- **Limits and tokens.** 5 hour and weekly plan limits per vendor with reset times, and tokens per vendor with sparklines.

## The team

Each role is a folder in [`agents/`](agents) with an `AGENT.md` that says what the role owns, who it works with, how it works, when it asks you, and how it searches the web. The rules they all share are in [`agents/TEAM.md`](agents/TEAM.md).

| Role | Does |
|---|---|
| [boss](agents/boss/AGENT.md) | plans the task, hands out work, merges what passed, never writes code |
| [advisor](agents/advisor/AGENT.md) | a second opinion before a plan, when an error repeats, and before "done" |
| [builder](agents/builder/AGENT.md) | implements one task at a time in its own worktree |
| [designer](agents/designer/AGENT.md) | specs the look before it is built and checks it after |
| [tester](agents/tester/AGENT.md) | always on, tests every commit that landed, reports to the boss |
| [reviewer](agents/reviewer/AGENT.md) | reads every commit for correctness, simplicity and safety |
| [researcher](agents/researcher/AGENT.md) | web research for the others, with sources |
| [scribe](agents/scribe/AGENT.md) | keeps the docs true after work lands |
| [ripple](agents/ripple/AGENT.md) | always on, finds what each commit broke outside itself, proves it with a test, holds the commit, writes the lesson |
| [aegis](agents/aegis/AGENT.md) | always on, security: leaked secrets, personal and financial data, injection holes, weak dependencies; holds commits, weekly audit |
| [bursar](agents/bursar/AGENT.md) | every three days, finds where tokens go and adds savings rules that orda measures and rolls back if they hurt |
| [referee](agents/referee/AGENT.md) | always on, re-runs every claim itself and catches tests bent to pass |
| [customs](agents/customs/AGENT.md) | checks new packages exist and are the real ones, calls exist in the pinned version, licenses fit |
| [anchor](agents/anchor/AGENT.md) | writes your task as checkable criteria before work starts, checks every one has evidence after |
| [curator](agents/curator/AGENT.md) | keeps a short project map every agent reads, so fresh starts do not re-explore |
| [scout](agents/scout/AGENT.md) | every few days, finds new models and benchmarks and proposes which model each role should use |

When orda starts an agent it gives it TEAM.md, its role's AGENT.md, the current roster, and the answers you told orda to remember. Agents talk back with lines orda reads: `ASK:` becomes a question in the questions panel, `REPORT:` goes to the log, `LEARNED:` shows up under the agent's web search, and the scout's `RECOMMEND:` becomes a switch-or-keep question that edits the team when you approve.

The scout runs when orda starts and its last scan is more than 3 days old (or when you press `s`). It reads release notes, changelogs and independent benchmarks, plus orda's own record of how each model did in your projects, and only considers models your installed CLIs can run on your plans. To change how a role works without rebuilding, copy its file to `~/.config/orda/agents/<role>/AGENT.md` and edit it.

## Install

Needs Rust 1.85 or newer, plus the agent CLIs you want to use: [Claude Code](https://code.claude.com), [Codex CLI](https://github.com/openai/codex), [Gemini CLI](https://github.com/google-gemini/gemini-cli).

```bash
git clone https://github.com/adilzhanY/orda
cd orda
cargo install --path .
```

## Use

```bash
orda --demo    # a scripted team: nothing runs, no tokens spent
orda           # real mode, in your project directory
orda config    # write the settings file and print its path
```

In real mode press `i`, type a task and press enter.

| Key | Does |
|---|---|
| `i` or `/` | type a task |
| `q` | answer questions: `1`..`9` picks, then `p` this project, `a` all projects, `o` just once |
| `s` | run the bursar's spending review, then the scout's model scan |
| `t` `g` `w` `x` `l` `u` | zoom team, git, web, guard, log, usage (or switch tab on a small terminal) |
| `tab` / `shift+tab` | next / previous module |
| `esc` | back to all panels |
| `ctrl+c` | quit |

## Agents talk to each other

Agents send each other typed cards. The tester finds a failure, writes a failing test, and sends a `bug` card (title, repro, expected, actual, done when) straight to the builder who made the commit. The builder fixes it and answers `fixed` in the same thread, and the tester checks it and answers `done`, or sends the next round. The reviewer works the same way with `review` cards. Builders only run quick checks; writing tests is the tester's job. The boss is copied on every card, and a bug that comes back a third time goes to the boss and the advisor. On screen, every agent card is wired into one network (each card plugs into a lane that leads to the boss and to everyone else), each message travels as a small block along those wires from one card to the other, the title bar says what it is, and the receiver lights up when it lands. `orda --demo` plays a few of these threads, including one that gets escalated.

## Fix one thing, break ten others

The tester checks that a commit does what it says; ripple checks what else it touched. For every commit it finds every caller, type, config key and format that depends on the changed code. Small changes get a quick read; changes to shared code get built next to their parent and compared on the same inputs. A break has to be proven with a test that passes before the commit and fails after it. Then ripple sends the builder a bug card, holds the commit so the boss cannot merge it (the git panel says `held by ripple`), releases it when the fix holds, and writes a one-line lesson into the project's `LESSONS.md`. Every agent reads those lessons at the start of every run, so the team stops repeating the same kind of mistake.

## Trust nothing

The team is built around what research found AI agents get wrong. Most "done" claims from coding agents have no passing run behind them, so the referee runs every claim again itself, and orda flags every commit that removes an assert, skips a test or swallows an error. About a fifth of AI code names packages that do not exist, so orda asks each package's registry before customs even looks. Requirements get dropped silently, so anchor writes your task as checkable criteria before work starts and checks them after. Long runs get worse, so orda restarts any run past 35 minutes, 80 tool calls or 160k context with a handoff, and the curator keeps a project map so the fresh run does not start from zero.

## Security

orda scans every new commit itself, without a model, for API keys and tokens, private keys, passwords in code, committed `.env` files, email addresses and card numbers. A hit holds the commit and asks you, because a pushed secret has to be rotated. aegis, the security agent, then looks at what a pattern cannot see: input that reaches a query, a shell command or a file path, missing access checks, unsafe configuration, vulnerable dependencies. It proves each hole locally, holds the commit until it is fixed, teaches the safe pattern as a lesson, and audits the whole project once a week.

## Spending

Every run is recorded with its input and output tokens, tool calls, time and result. Every three days the bursar reviews the numbers, studies what is new about spending tokens well, and adds short savings rules for a role or for everyone, the boss included. Each rule is an experiment: orda remembers how successful that role was before the rule and rolls the rule back by itself if success drops. Questions about cheaper models or lower effort go to the scout, which runs right after the bursar with its report.

## When a limit runs out

Plans run out, Codex especially. When a vendor says an agent is out of limits, orda moves that agent to its fallback model at once (by default Codex goes to Claude Opus, Claude goes to Codex gpt-6.1-sol) and starts the same task there. The new model gets the original task, a list of everything the previous model did, and the files it already changed, so it carries on instead of starting over. When the limit resets, the agent goes back to its own model, after its current run if it is in the middle of one. If every model is out, the agent waits and resumes by itself. Each step shows in the session log, the card says `fallback`, and the limits panel shows when the vendor is back.

## Settings

`~/.config/orda/config.toml`, reloaded while orda runs. `orda config` writes a commented copy of the defaults. You can change:

- `theme`: `orda` (coral, blue and teal on dark grey) or `iris` (black and white)
- `[colors]`: any single colour, or `bg = "none"` to keep your terminal's background
- `[layout]`: which panel goes where, in what order, and below which terminal size the panels become tabs
- `[guard]`: the `deny` and `ask` command lists
- `[fallback]`: which model each vendor's agents move to when that vendor is out of limits
- `[watch]`: whether every new commit goes to the always-on agents (tester, ripple, aegis)
- `[[agents]]`: the team itself, with name, role (a folder in `agents/`), vendor, model, effort and job

Plan limits are read from `~/.local/state/limit-watch/state.json`, written by limit-watch, a small daemon of mine (not published yet) that polls each vendor's usage endpoint. The format is in `src/limits.rs`. Without it the limits panel stays empty; Claude's limits still appear while a Claude agent runs.

## How it works

Every agent runs headless and prints a JSON event stream (`claude -p --output-format stream-json`, `codex exec --json`). One adapter per vendor turns that stream into one shared event type, and orda draws its own cards from those events. It never embeds another tool's interface. The reasons behind this and every other choice are in [DECISIONS.md](DECISIONS.md); the look is specified in [DESIGN.md](DESIGN.md), and what might come next is in [IDEAS.md](IDEAS.md).

## Development

```bash
cargo test                                      # unit, adapter and render tests
ORDA_PRINT=1 cargo test render -- --nocapture   # print the demo screen at 4 sizes
cargo test live -- --ignored                    # one real Claude call (costs cents)
cargo clippy --all-targets -- -D warnings
```
