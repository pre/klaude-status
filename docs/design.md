# klaude-status: design

## Why a binary

Claude Code's `statusLine` is a command that receives JSON on stdin and whose
stdout is drawn under the prompt. It runs **after every turn** (300 ms
debounce) and additionally every `refreshInterval` seconds. That imposes three
constraints, and they shape the whole implementation:

1. **Speed.** A shell script that forks `git status` costs hundreds of
   milliseconds in a large repo. A Rust binary with gitoxide does the same work
   in single-digit milliseconds in a normal repo, process start included.
2. **No crashing.** A panic message or an error string goes straight into the
   UI. Every input field is an `Option`, every parse is `unwrap_or_default`,
   and nothing is indexed without a bounds check.
3. **No side effects.** No network, no cache files, no state. The same input
   always produces the same output, which is what makes the output testable.

## Where the input schema came from

The schema was not taken from documentation but from two sources that check
each other:

1. **The Claude Code binary.** The builder for the status line JSON is still
   readable in the bundled source:

   ```sh
   strings -a ~/.local/share/claude/versions/<version> | grep -o "function slv(.\{0,4000\}"
   ```

   The builder shows exactly which fields are conditional (`...x && {...}`),
   which tells you which fields can be absent and when.

2. **A capture from a real session.** A temporary wrapper as the statusLine
   command:

   ```sh
   #!/bin/sh
   cat > /tmp/statusline-input.json
   exec <the real command>
   ```

   Claude Code reads `settings.json` on the fly, so the capture takes effect
   without a restart. This also confirms the actual *value shapes* (for
   instance that `used_percentage` is a number and not a string, and that
   `resets_at` is in unix seconds).

Both are worth repeating after a Claude Code upgrade: unknown fields are
ignored safely, but they will not appear in the status line until they are
added to `src/input.rs`.

## Fields as of 2.1.226

Always present: `session_id`, `transcript_path`, `cwd`, `prompt_id`, `model`
(`id`, `display_name`), `workspace` (`current_dir`, `project_dir`,
`added_dirs`), `version`, `output_style.name`, `cost` (`total_cost_usd`,
`total_duration_ms`, `total_api_duration_ms`, `total_lines_added`,
`total_lines_removed`), `context_window` (`total_input_tokens`,
`total_output_tokens`, `context_window_size`, `current_usage`,
`used_percentage`, `remaining_percentage`), `exceeds_200k_tokens`, `fast_mode`,
`thinking.enabled`.

Conditional:

| Field | Present when |
| --- | --- |
| `effort.level` | only on effort-capable models (not Opus 4.x, Sonnet 4.x, Haiku 4.5, claude-3-*) |
| `session_name` | the session has a name |
| `workspace.repo` | the remote is recognized (`host`, `owner`, `name`) |
| `workspace.git_worktree` | the session is in a worktree |
| `rate_limits` | subscription quota data has loaded (`five_hour`, `seven_day`) |
| `vim.mode` | vim mode is on |
| `agent.name` | the status line is being drawn for a subagent |
| `remote.session_id` | remote session |
| `pr` | the branch has a PR (`number`, `url`, `review_state`, `kind`) |
| `worktree` | a Claude Code managed worktree (`name`, `path`, `branch`, `original_cwd`, `original_branch`) |

**Not included:** `permission_mode`. The base builder for hook payloads
produces it, but the status line calls that builder with no arguments, so the
field stays undefined and disappears from the JSON. The permission mode
(`plan`, `acceptEdits`, `bypassPermissions`) is therefore **not observable**
from the status line. It is read anyway: if it ever gets added, `BYPASS` starts
showing up without a code change. Claude Code draws the permission mode itself
next to the prompt.

## Context fill is the number that matters

`context_window.used_percentage` predicts when the conversation gets compacted.
Compaction is where work quality typically dips, because part of the context is
replaced by a summary. That is why it gets a bar rather than just a percentage:
a bar registers in peripheral vision without being read.

On a 1M model, `exceeds_200k_tokens` is a separate signal. It says nothing
about how full the window is; it says the portion past 200k is priced
differently.

## Quotas without the network

`rate_limits` arrives in the input, so the 5-hour and 7-day quotas are free and
current. The one thing the input does **not** carry is per-model budgets.

## Git without a git process

Two of the three git facts are cheap and bounded.

**Branch and divergence.** Divergence is computed with two rev-walks where the
other side is `hidden`, capped at 999, so a branch that is thousands of commits
behind cannot slow the line down.

**Dirty state** is the expensive one, and the interesting part of the design.
Comparing the index against the working tree has to `stat` every tracked file.
The cost therefore scales with the size of the *working tree*, not the size of
your change, and a **clean** repo is the worst case, because nothing lets the
scan stop early. Measured end to end on Apple silicon, warm cache:

| Tracked files | Time |
| --- | --- |
| ~15 | ~5 ms |
| ~1k | ~18 ms |
| ~3k | ~58 ms |
| 51k | ~180 ms warm, ~1.2 s cold |

Untracked files are deliberately excluded: finding them requires a full
directory walk, which is a second traversal on top of the one above. So `*`
means "tracked files have changed".

Even so, the tail is unacceptable for something that runs after every turn, so
the check runs against a deadline (`git_timeout_ms`, default 250 ms). A
detached watchdog thread flips an interrupt flag that gitoxide's status
iterator honors mid-scan. Three outcomes instead of two:

* difference found → `*`
* scan completed, nothing found → clean
* deadline hit first → `?`

The third state is the point. Collapsing it into "clean" would mean silently
lying in exactly the repos where the check is least likely to finish, and
collapsing it into "dirty" would mean crying wolf. `git_timeout_ms: 0` removes
the deadline for anyone who would rather always wait for the true answer.

Two things bound how well the deadline works. The staged-changes comparison
runs first because it needs no `stat` at all and staged work is the likeliest
way to be dirty, but it cannot be interrupted mid-flight, so the floor is
roughly the cost of reading the index once (microseconds normally, ~60 ms at
51k files). And in a repo with clean/smudge filters configured, gitoxide may
run the filter command while comparing content, which is the one place a
subprocess can appear despite the no-subprocess rule; it comes from the repo's
own configuration rather than from this program.

## Fitting

A line is assembled from segments joined with ` │ `. If it does not fit, the
`path` segment is first swapped for a shorter form of itself (see below), then
segments are dropped in `DROP_ORDER` (weakest first) until it does; only if no
amount of dropping is enough is the line truncated, ANSI-safely. The order is
chosen so that `path` and `model` disappear last.

The path shortens rather than disappears, because losing it entirely costs more
than losing its head. The forms, widest first:

```
~/dev/work/klaude-status/.worktrees/fix-truncate
…klaude-status/.worktrees/fix-truncate
…klaude-status/…/fix-truncate
```

The emphasized part is the repository's **main** working tree, taken from
`common_dir`, so a session in a linked worktree still shows which project it
belongs to rather than the worktree's own name. Without git, `project_dir` is
used instead.

The terminal width is probed from **stderr**, because stdout is a pipe. Failing
that, `COLUMNS`. As a last resort no limit is applied at all, and Claude Code
truncates the line itself.

## Testing

`cargo test` covers the formatters (bar, tokens, durations, path collapsing,
truncation), the fact that ANSI escapes do not affect width computation, the
deadline behavior of the dirty check, and the whole pipeline against sample
inputs: an ordinary session, a filling context, a worktree with a PR and a
subagent, and partial input. `--demo` prints the same scenarios in color for a
visual check, resolved against the current directory so the git segment shows
real state.

Git state itself cannot be unit tested without a repo, so it is verified
against a throwaway repo: clean, modified, ahead, behind, diverged, detached,
and with no upstream.
