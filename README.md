# klaude-status

A status line for [Claude Code](https://claude.com/claude-code). It reads the
statusLine JSON on stdin and prints two lines: where you are, and what the
session is spending.

```text
klaude-status/src │ ⎇ main* │ Status line for Claude Code
Opus 5 1M │ max │ ▉░░░░░░░ 17% 168k/1M │ 5h 29% 2h14m · 7d 66% 3d11h │ $6.26 +587/-26 23m
```

Left to right on the second line: model and context size, reasoning effort, a
context-fill bar, the 5-hour and 7-day quota with a countdown to reset, then
cost, lines changed and session length.

## Why another one

The status line runs after **every turn**, debounced at 300 ms, plus once per
`refreshInterval`. That budget rules out the usual shell-script approach: a
script that forks `git status` costs hundreds of milliseconds in a large repo,
every single turn. So this is a single static binary that reads git state
directly with [gitoxide](https://github.com/GitoxideLabs/gitoxide), makes no
network calls, spawns no subprocesses of its own, and keeps no state.

It also never fails loudly. A panic message or an error string would be
rendered straight into the UI, so malformed or partial input produces a partial
line instead of an error.

## Install

Needs [Rust](https://rustup.rs).

```sh
git clone https://github.com/matti/klaude-status
cd klaude-status
./install.sh
```

That builds the release binary, installs it into `~/.local/bin` (override with
`PREFIX=/usr/local ./install.sh`), and points `statusLine` at it in
`~/.claude/settings.json`. If a status line is already configured and does not
mention `klaude-status`, the script leaves it alone and tells you.

To wire it up by hand instead:

```json
{
  "statusLine": {
    "type": "command",
    "command": "/absolute/path/to/klaude-status",
    "refreshInterval": 10
  }
}
```

**Use an absolute path.** Claude Code runs the command without your shell
profile, so `~/.local/bin` is not on `PATH`. A bare `klaude-status` works in a
terminal session but silently produces nothing in the desktop app, and the only
symptom is an empty status line.

Preview the output without Claude Code:

```sh
klaude-status --demo
```

## Configuration

Optional, at `~/.claude/klaude-status.json`. Without it the defaults below
apply. A malformed file falls back to the defaults silently rather than
breaking the line.

```json
{
  "lines": [
    ["path", "git", "session"],
    ["model", "effort", "flags", "context", "limits", "cost"]
  ],
  "color": true,
  "max_width": 0,
  "bar_width": 8,
  "git_timeout_ms": 250
}
```

Each inner list is one line, so the number of lines and the order of segments
are yours to choose. Full reference: [docs/configuration.md](docs/configuration.md).

### Segments

| Name | Shows |
| --- | --- |
| `path` | project root, then the path inside it dimmed, `+Nd` for `/add-dir` directories |
| `git` | `⎇ branch`, `*` modified, `?` check timed out, `↑n`/`↓n` ahead/behind, `⧉name` worktree |
| `session` | session name, or `#abcd` from the id |
| `model` | display name, plus `1M` on a 1M-context model |
| `effort` | `low` … `max` |
| `flags` | `⚡fast`, `no-think`, `200k+`, output style, `@subagent`, vim mode, `PR#n` |
| `context` | fill bar, percentage, tokens used against the window |
| `limits` | 5-hour and 7-day quota with a countdown to reset |
| `cost` | dollars, `+added/-removed` lines, session duration |
| `api` | share of the session clock spent waiting on the API |
| `repo` | `owner/name` |
| `version` | Claude Code version |

`api`, `repo` and `version` are off by default; add them to a line to use them.

Colors come from the 8/16 basic palette rather than 256 colors, so they follow
the terminal theme and work on light and dark backgrounds. `NO_COLOR` or
`"color": false` turns them off.

When a line does not fit, segments are dropped least-important first rather
than the line being cut mid-word; `path` and `model` are the last to go.

## Performance

Everything except git is pure formatting of data already in the input. Git is
the only thing that touches the disk, and the dirty check dominates: it has to
`stat` every tracked file, so the cost scales with the size of the working
tree, not with the size of your change.

Measured end to end, process start included (Apple silicon, warm cache):

| Repo | Tracked files | Time |
| --- | --- | --- |
| this one | ~15 | ~5 ms |
| a small app | ~1k | ~18 ms |
| a mid-size one | ~3k | ~58 ms |
| a monorepo | 51k | ~180 ms, or over a second cold |

That tail is why `git_timeout_ms` exists. Past the deadline the dirty check is
interrupted and the segment shows `?` instead of a stale-but-confident `*` or
nothing at all: in a big repo "clean" and "not checked" look identical from the
outside and mean very different things. Set it to `0` for no deadline, which is
always correct and occasionally slow.

## Troubleshooting

If the status line is blank, the first question is whether the command runs at
all. Set `KLAUDE_STATUS_LOG` in the `env` block of `settings.json`:

```json
{ "env": { "KLAUDE_STATUS_LOG": "/tmp/klaude-status.log" } }
```

Every run appends a timestamp, pid, input and output sizes, cwd and the
rendered line. That separates the three failure modes: no lines at all (the
command is never invoked, usually a wrong path), `out=0B` (it runs but produces
nothing), or a sensible line (it works and the problem is elsewhere). Remove
the variable afterwards, it writes on every run.

To reproduce what Claude Code does, without your shell profile:

```sh
env -i sh -c '/absolute/path/to/klaude-status < sample.json'
```

## Development

```sh
cargo test
cargo run -- --demo
```

`--demo` renders four scenarios (ordinary session, filling context, worktree
with a PR and a subagent, partial input) against the current directory, so the
git segment shows real state.

The input schema in `src/input.rs` was read out of the Claude Code binary
rather than from documentation; [docs/design.md](docs/design.md) explains how,
and what is deliberately not shown.

## License

MIT
