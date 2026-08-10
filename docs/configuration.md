# Configuration reference

The config file is optional. Without it, `Config::default()` in `src/config.rs`
applies, which is the JSON shown below. A file that fails to parse falls back
to the defaults silently: a broken config must never leave the status line
empty, because an empty line is indistinguishable from a broken install.

## Location

`~/.claude/klaude-status.json`, or wherever `KLAUDE_STATUS_CONFIG` points.

## Keys

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

| Key | Default | Meaning |
| --- | --- | --- |
| `lines` | two lines, as above | One inner list per output line, in segment order. Unknown names are ignored. |
| `color` | auto | `true`/`false` to force. Unset means on unless `NO_COLOR` is set. |
| `max_width` | `0` | `0` detects the terminal width; anything else pins the line to that many columns. |
| `bar_width` | `8` | Width of the context-fill bar, in cells. |
| `git_timeout_ms` | `250` | Deadline for the dirty check. `0` removes the deadline. |

A line whose segments all return nothing is dropped entirely, so a config with
three lines can render as two.

## Segments

Every segment returns nothing when it has nothing to say, and the separator
goes with it. Segments not listed in a line are never computed; in particular
`git` is the only one that touches the disk, so leaving it out makes the whole
program pure formatting.

| Name | Content | What the color means |
| --- | --- | --- |
| `path` | project root emphasized, path inside it dimmed, `+Nd` for added directories | - |
| `git` | `⎇ branch`, `*` modified, `?` unknown, `↑n` ahead, `↓n` behind, `⧉ name` worktree | green clean, yellow modified, gray unknown, magenta detached |
| `session` | session name, or `#abcd` from the id | - |
| `model` | display name, `1M` suffix on a 1M-context model | - |
| `effort` | `low` … `max` | magenta max/xhigh, blue high, dim otherwise |
| `flags` | `⚡fast`, `no-think`, `200k+`, output style, `@subagent`, vim mode, `PR#n` | red means pay attention |
| `context` | bar + `%` + `used/size` | green < 60 %, yellow < 80 %, red above |
| `limits` | `5h n%` and `7d n%` + countdown to reset | green < 75 %, yellow < 90 %, red above |
| `cost` | `$n`, `+added/-removed`, session duration | - |
| `api` | share of the session clock spent waiting on the API | - |
| `repo` | `owner/name` | - |
| `version` | `cc2.1.226` | - |

`api`, `repo` and `version` are not on the default lines.

## Fitting

Segments are joined with ` │ `. When the result is wider than the budget, they
are dropped one at a time in `DROP_ORDER` (`render.rs`), weakest first:

```
version, api, repo, cost, session, limits, context, flags, effort, git, model, path
```

Only if dropping everything droppable still does not fit is the line truncated,
and that truncation is ANSI-aware so it can never emit half an escape sequence.

The terminal width is probed from **stderr**, because stdout is a pipe. Failing
that, `COLUMNS`. Failing that, no limit is applied at all and Claude Code
truncates the line itself.

## Environment variables

| Variable | Effect |
| --- | --- |
| `KLAUDE_STATUS_CONFIG` | Path to the config file, overriding the default location. |
| `KLAUDE_STATUS_LOG` | Append a diagnostic record of every run to this path. See the troubleshooting section in the README. |
| `NO_COLOR` | Disable colors, unless `color` is set explicitly. |
| `COLUMNS` | Fallback width when stderr is not a terminal. |
