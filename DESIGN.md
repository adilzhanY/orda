# Design

How orda looks and behaves. Update this before building a component and whenever the look changes. When and how to edit this file is described in `CLAUDE.md`.

The approved concept is `.lavish/orda-dashboard.html` (2026-10-05). The terminal build follows it.

## Principles

- **Every agent is a card.** The state of the whole team is visible at a glance, without opening anything.
- **Colour means vendor, not decoration.** Each vendor has one accent, used on its card border, its name in the log and its limit bars. Everything else is neutral.
- **Calm when idle.** Only working things move. A finished card goes still. orda redraws only when something changed.
- **Numbers over adjectives.** Tokens, tests, limits and times, not "working hard".
- **Nobody waits on you.** Questions never stop the team; only a guard question pauses the one agent that asked.
- **No emoji, no Unicode icons.** Box-drawing and block characters only (`▖▘▝▗` spinner, `█░` bars, `▁..█` sparklines).

## Themes

| Key | orda | iris | Used for |
|---|---|---|---|
| bg | `#1b1d22` | `#000000` | screen background (`none` keeps the terminal's) |
| text | `#c9cdd4` | `#a6a6ac` | body text |
| bright | `#eef0f3` | `#f2f2f2` | titles, current actions, numbers |
| dim | `#6b7280` | `#6e6e73` | hints, times, labels |
| line | `#3a3f48` | `#232326` | idle borders, connector, empty bar cells |
| badge | `#262a31` | `#161618` | badge and chip backgrounds, toast, selected commit |
| claude | `#e8855a` | `#f2f2f2` | Claude agents |
| codex | `#7aa7ff` | `#f2f2f2` | Codex agents, "in review", project-scoped answers |
| gemini | `#5fd3c4` | `#f2f2f2` | Gemini agents, the web panel title, "searching" |
| fable | `#a990e0` | `#f2f2f2` | the advisor (any Fable model), answers for all projects |
| ask | `#f2c14e` | `#f2f2f2` | questions, toast, "asks you" guard rules, busy commits |
| ok | `#7ad67a` | `#f2f2f2` | running, passing, merged |
| bad | `#ef6b6b` | `#ff6961` | failed, blocked, reverted |

Iris has no accents by design, so under it vendors are told apart by the model name in each card title.

## Screen

```
 ORDA  project · branch · task ......................... plan ████░░ · elapsed
╭ team ───────────────────────────────╮ ╭ questions ─────╮ ╭ git ───────────╮
│ [advisor]   ┏ boss ┓    [you]       │ │                │ │                │
│                 │                   │ ╰────────────────╯ ╰────────────────╯
│        ┌────────┴────────┐          │ ╭ web ───────────╮ ╭ guard ─────────╮
│ [worker]          [worker]          │ │                │ │                │
│ [worker]          [worker]          │ ╰────────────────╯ ╰────────────────╯
│ [worker]          [worker]          │ ╭ session log ───╮ ╭ limits & tok ──╮
│ [worker]                            │ │                │ │                │
╰─────────────────────────────────────╯ ╰────────────────╯ ╰────────────────╯
╭ › input ──────────────────────────────────────────────────────────────────╮
 key hints
```

Workers sit two to a row, three when the team column is at least 110 columns wide. Which module goes where is set in `[layout]`: `left` (the wide column, first module takes the spare height), `right` (a two-column grid, row by row) and `bottom` (one row, side by side). The left column and the right columns split 165 : 200, like the concept.

**Compact mode.** Below `compact_width` x `compact_height` (default 170 x 44) the body becomes one module at a time with a tab bar on top and a one-line limits and tokens summary under it. Pressing a module key in full mode zooms that module the same way; esc returns.

## Components

**Panel.** Rounded border in `line`, title cut into the top border on the left (bold), a dim hint on the right. A focused panel gets a thick border.

**Agent card.** 7 rows; worker cards shrink to 6 or 5 when the team would not fit otherwise, dropping output lines first. When even that is not enough, a row where nobody is working (no run, no question, no failure, no message in flight) becomes a slim card: title border, one line with the action and badge, bottom border. Slim cards stay wired, and their row grows back as soon as one of its agents becomes active. Border: the vendor accent while running, searching or asking; `line` when idle, queued or done; `bad` when failed. The boss card has a thick border. Title: name (left, accent when active, dim otherwise), model and effort (right, effort dropped first when the border is short). Rows:
1. spinner (only while busy) + current action, status badge on the right
2. boss: context bar against 1M; tester: test progress bar with `passed/total`; bursar: tokens per run this week against last week (`-18%` in `ok`, a rise in `bad`); others: their job
3. and 4. the last two output lines
5. task or worktree on the left, tokens on the right

On a fallback model the right title reads `<model> · fallback` instead of the effort.

Badges: `running` / `always on` (ok), `searching` (gemini), `asking` (ask), `queued` / `idle` (dim), `done` (ok), `failed` (bad), all on `badge`.

**You card.** In the boss row, right of the boss. Question count with a `q` badge, who is paused on a guard answer, and how many answers are remembered. Border turns `ask` while questions wait.

**Message flight.** When an agent sends a card, a block travels along the wiring from the sender's junction to the receiver's. The head is `█` in the card's colour (bug and escalation `bad`, fixed and done `ok`, review `codex`, question `ask`), with a three-cell trail `▓▒░` in the sender's accent. It takes 1.4 seconds. While it flies, the team panel's title bar shows `sender -> receiver` and the card as a coloured chip (`bug #12`). When it lands, the receiver's border takes the card's colour for 1.6 seconds and its current action changes (`fixing #12, round 1/3`).

**Wiring.** Every card is wired into one network, so every agent is visibly connected to the boss and to every other agent. Each card in the boss row drops a line from the middle of its bottom border (`┬`, `┳` on the boss's thick border) into the connector row, which runs across under them. A lane runs down each gap between worker columns (the gap is 3 columns wide, the lane in its middle), and every worker card plugs into the lane beside it with a short stub and a junction cut into its border (`├` on the right edge, `┤` on the left), so a row reads `├─┼─┤`. Lanes hang from the connector row. Lines are `line`; junctions keep the card's border colour. A message flight follows these wires, found by a shortest-path search, so it never crosses a card.

**Questions panel.** Border pulses between `ask` and `line` (about once a second) while questions wait. Each question: asker (in its accent), task, age; the question in bright. The selected one shows numbered options as chips; once one is picked, `remember for p this project · a all projects · o just once`. Guard questions and the scout's switch-or-keep questions skip the scope step; a scout answer is remembered for all projects. Below: the remembered answers, tagged `all` (fable), `project` (codex) or `once` (dim).

**Toast.** Top right, 52 x 5, `ask` border on `badge`, for 6 seconds when a question arrives: who asked, the question, "press q to answer, the team keeps working". A desktop notification (notify-send) goes out at the same time when `[notify] desktop = true`.

**Git panel.** Worktrees with file count and `+added -removed`; the last 12 commits (hash in `ask`, newest highlighted) with the status orda gave them (`testing 14/20`, `in review`, `merged`, `reverted`, and `held by ripple` in `bad`, which wins over any other status) or their age; the newest commit's files.

**Web panel.** Newest first: who searched (accent), a spinner while live, the query, sources (dim), `learned: ...` (ok).

**Guard panel.** Rules as chips in two labelled groups, `blocked` (bad) and `asks you` (ask), then every event: time, agent, verdict, command, and why.

**Session log.** `HH:MM:SS`, agent name padded and coloured, text. Long entries wrap under the text column. Newest at the bottom. Model switches always appear here.

**Limits & tokens.** Side by side when the panel is at least 80 columns wide, otherwise limits above tokens. Limits: per vendor, one row per window (`5h`, `7d`, `fable`), a bar in the vendor accent (turns `ask` at 70%, `bad` at 90%), percent, time to reset. A vendor whose login expired says so in `bad`, and a vendor that is out of limits shows one `bad` row, `out of limits, back at 15:05`, in place of its bars. Tokens: per vendor, a sparkline of tokens per 2 seconds and the total; then the session total.

**Input.** Rounded box, `›` in claude colour, placeholder in dim. Border turns claude colour when typing.

## Keys

| Key | Where | Does |
|---|---|---|
| `i` or `/` | anywhere | type a task, enter sends it to the boss, esc leaves |
| `q` | anywhere | answer questions |
| `s` | anywhere | run the bursar's spending review and then the scout's model scan |
| `1`..`9` | questions | pick an option |
| `p` `a` `o` | questions | remember for this project, all projects, just once |
| `j` `k` | questions | move between questions |
| `t g w x l u` | anywhere | team, git, web, guard, log, usage: zoom (full) or switch tab (compact) |
| `tab` / `shift+tab` | anywhere | next / previous module |
| `esc` | anywhere | back to all panels |
| `ctrl+c` | anywhere | quit |
