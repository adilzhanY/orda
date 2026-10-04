# orda

A terminal office for AI agents. You give one task to a boss agent, and it runs a team of workers on different vendors (Claude Code, Codex CLI, Gemini CLI) while you watch the whole team in one screen: who is doing what, on which model, what it costs, what it searched, what it committed, and what it wants to ask you.

The name is the Kazakh word орда, the khan's camp that commanded the clans. The English word "horde" comes from it.

```
 ORDA  project orda  branch main +2  task build the first dashboard screen with live agent cards                                                               plan 5/9 █████░░░░  elapsed 00:00:00
╭ team  8 agents · 6 working ─────────────────────────────── agents report to the boss ╮ ╭ questions  none waiting ──────── work never waits ╮ ╭ git ─────────────────────── main +2 · 2 worktrees ╮
│                                                                                      │ │  remembered                                       │ │ worktrees                                         │
│ ╭ advisor ────── fable ╮  ┏ boss ━━━━━━━━━━━ opus · high ┓  ╭ you ────────── owner ╮ │ │  all     commits: one line, no body               │ │ wt/builder  3 files +142 -18                      │
│ │ on call, 2 c…  idle  │  ┃ ▖ reading tester …  running  ┃  │ no questions         │ │ │  all     never use em dashes                      │ │ wt/builder-2  2 files +96 -4                      │
│ │ on call, never writ… │  ┃ context ██░░░░░░░░ 193.0k/1M ┃  │ no one waits on you  │ │ │  project tests run with cargo nextest             │ │ commits                                           │
│ │ last: fail loudly o… │  ┃ next: task 6 to builder-2 o… ┃  │ answers are saved    │ │ │                                                   │ │ a3f9c21 add codex adapter           testing 14/20 │
│ │                      │  ┃                              ┃  │ per project or all   │ │ │                                                   │ │ 9e1b0d4 card component, 3 states        in review │
│ │ read 239k            │  ┃ plan 5/9           58.2k tok ┃  │         3 remembered │ │ │                                                   │ │ 77c2e10 shared AgentEvent type             merged │
│ ╰──────────────────────╯  ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛  ╰──────────────────────╯ │ │                                                   │ │ 4b08aa3 tokio event loop                   merged │
│                                           │                                          │ │                                                   │ │ 1d93f7e crossterm setup                  reverted │
│                     ┌─────────────────────┴────────────────────┐                     │ │                                                   │ │ 0c11e2b init                               merged │
│ ╭ builder ──────────── gpt-6-sol · high ╮  ╭ builder-2 ─────────── sonnet · medium ╮ │ │                                                   │ │ a3f9c21 files                                     │
│ │ ▖ src/agents/codex.rs        running  │  │ ▖ src/ui/card.rs             running  │ │ │                                                   │ │ M src/agents/codex.rs  +88 -12                    │
│ │ implements tasks                      │  │ implements tasks                      │ │ │                                                   │ │ A src/agents/event.rs  +41 -0                     │
│ │ parsing item.completed events         │  │ title in the top border, 3 states do… │ │ │                                                   │ │                                                   │
│ │                                       │  │                                       │ │ │                                                   │ │                                                   │
│ │ task 5 · wt/builder         31.4k tok │  │ task 4 · wt/builder-2       24.9k tok │ │ │                                                   │ │                                                   │
│ ╰───────────────────────────────────────╯  ╰───────────────────────────────────────╯ │ │                                                   │ │                                                   │
│                                                                                      │ │                                                   │ │                                                   │
│ ╭ tester ────────────── sonnet · medium ╮  ╭ researcher ────────────── flash · low ╮ │ ╰───────────────────────────────────────────────────╯ ╰───────────────────────────────────────────────────╯
│ │ ▖ testing a3f9c21          always on  │  │ ▖ web: ratatui mouse cap…  searching  │ │
│ │ ██████████████░░░░░░           14/20  │  │ web research for the others           │ │ ╭ web  5 searches ─────────── what agents looked up ╮ ╭ guard  13 rules ─────────────────────── 2 blocked ╮
│ │ report #7: turn.diff crashes the ada… │  │ 3 sources open, writing a note        │ │ │ researcher ▖ ratatui mouse capture crossterm 2026 │ │ blocked   rm -rf /   rm -rf ~   rm -rf $HOME      │
│ │                                       │  │                                       │ │ │   ratatui.rs/concepts/event-handling              │ │           dd * of=/dev/*   mkfs*   curl * | sh    │
│ │ reports to boss             19.7k tok │  │ for builder-2                8.1k tok │ │ │ builder codex exec --json event schema            │ │           curl * | bash   wget * | sh             │
│ ╰───────────────────────────────────────╯  ╰───────────────────────────────────────╯ │ │   github.com/openai/codex/docs/exec.md            │ │           chmod -R 777 /*                         │
│                                                                                      │ │   learned: events are thread.*, turn.*, item.*    │ │ asks you  git push*   sudo *                      │
│ ╭ reviewer ─────────── gpt-6-sol · high ╮  ╭ scribe ────────────────── flash · low ╮ │ │ tester cargo nextest flaky retries                │ │           git reset --hard*   rm -rf *            │
│ │ waiting for builder-2         queued  │  │ ▖ DECISIONS.md #5            running  │ │ │   nexte.st/docs/features/retries                  │ │                                                   │
│ │ reviews every commit                  │  │ keeps the four docs current           │ │ │   learned: --retries 2, mark slow tests           │ │ 23:41 builder-2 blocked rm -rf ~/.cache/orda      │
│ │ 9e1b0d4 approved with 1 note          │  │ recording your answer from 23:31      │ │ │ researcher tokio process kill_on_drop             │ │       outside the project                         │
│ │                                       │  │                                       │ │ │   docs.rs/tokio/latest/tokio/process              │ │ 23:12 builder blocked curl -fsSL get.x.sh | sh    │
│ │ 2 reviews today             12.0k tok │  │ keeps the docs               3.3k tok │ │ │   learned: kill_on_drop(true) stops orphans       │ │       pipes a download into a shell               │
│ ╰───────────────────────────────────────╯  ╰───────────────────────────────────────╯ │ │ boss gemini cli stream-json output                │ │                                                   │
│                                                                                      │ │   github.com/google-gemini/gemini-cli             │ │                                                   │
│                                                                                      │ │   learned: needs a recent gemini-cli              │ │                                                   │
│                                                                                      │ │                                                   │ │                                                   │
│                                                                                      │ │                                                   │ │                                                   │
│                                                                                      │ │                                                   │ │                                                   │
╰──────────────────────────────────────────────────────────────────────────────────────╯ ╰───────────────────────────────────────────────────╯ ╰───────────────────────────────────────────────────╯
╭ session log ──────────────────────────────────────────────────────────── l to expand ╮ ╭ limits & tokens ────────────────────────────────────────────────────────────────────────── this session ╮
│ 00:56:09 tester     report #7 -> boss: 2 failing in the codex adapter                │ │ claude 5h    █░░░░░░░░░░░░░░░   5%  resets 1h53        claude ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁           102.8k │
│ 00:56:09 advisor    error repeats: fail loudly on unknown events                     │ │        7d    █░░░░░░░░░░░░░░░   4%  resets 5d6h        codex  ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁            43.4k │
│ 00:56:09 boss       task 5 back to builder with the report                           │ │        fable ░░░░░░░░░░░░░░░░   0%  resets 5d6h        gemini ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁            11.4k │
│ 00:56:09 researcher web: ratatui mouse capture crossterm 2026                        │ │ codex  5h    ░░░░░░░░░░░░░░░░   0%  resets 4h21                                                         │
│                                                                                      │ │        7d    ░░░░░░░░░░░░░░░░   0%  resets 6d23h       total                                     157.6k │
│                                                                                      │ │ gemini 5h    ░░░░░░░░░░░░░░░░   0%  resets 4h58                                                         │
│                                                                                      │ │        7d    ░░░░░░░░░░░░░░░░   0%  resets 6d23h                                                        │
╰──────────────────────────────────────────────────────────────────────────────────────╯ ╰─────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│› press i and tell orda what to build                                                                                                                                                             │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
 i task   q questions   t team   g git   w web   x guard   l log   u usage   tab next   esc all panels   ctrl+c quit
```

*`orda --demo` at 196 x 54, as text. In a terminal it is in colour and moves.*

## Status

Early. The dashboard is done; the orchestration behind it is not.

| Works | Not yet |
|---|---|
| The full dashboard, `--demo` mode, two themes, settings with live reload, tabs on small terminals | The boss planning and handing tasks to workers (today it runs a task alone) |
| Claude Code and Codex adapters, tested on real output | A Gemini adapter |
| Real git panel and plan limits | The guard actually stopping commands (it shows what it would block) |
| Questions, notifications and remembered answers | Answers going back to the agent that asked |

## What is on the screen

- **Team.** The boss in the middle, an advisor beside it, and a card per worker: current action, status, live output, tokens. The tester is always on and tests whatever has already landed.
- **Questions.** Agents ask without stopping. A toast and a desktop notification arrive, the panel pulses, and the agent moves on to other work. When you answer, orda asks whether to remember the choice for this project, for all projects, or just once.
- **Git.** A small lazygit: worktrees, recent commits with their test, review and merge status, and changed files.
- **Web.** Every search an agent makes: the query, the sources, and what it learned.
- **Guard.** Commands agents may never run (`rm -rf /`, `curl ... | sh`, ...) and commands that need your yes first (`git push`, `sudo`, ...).
- **Limits and tokens.** 5 hour and weekly plan limits per vendor with reset times, and tokens per vendor with sparklines.

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
| `t` `g` `w` `x` `l` `u` | zoom team, git, web, guard, log, usage (or switch tab on a small terminal) |
| `tab` / `shift+tab` | next / previous module |
| `esc` | back to all panels |
| `ctrl+c` | quit |

## Settings

`~/.config/orda/config.toml`, reloaded while orda runs. `orda config` writes a commented copy of the defaults. You can change:

- `theme`: `orda` (coral, blue and teal on dark grey) or `iris` (black and white)
- `[colors]`: any single colour, or `bg = "none"` to keep your terminal's background
- `[layout]`: which panel goes where, in what order, and below which terminal size the panels become tabs
- `[guard]`: the `deny` and `ask` command lists
- `[[agents]]`: the team itself, with name, role, vendor, model, effort and job

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
