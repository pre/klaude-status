# klaude-status

- Rust binary used as Claude Code's `statusLine` command: reads the statusline JSON on stdin, prints 2 lines with ANSI colors.
- Runs after every turn (300 ms debounce) + `refreshInterval`. Three hard rules: **no panicking, no subprocesses, no network.** Partial or malformed input produces a partial line, never an error.
- The input schema (`src/input.rs`) was read out of the Claude Code 2.1.226 binary, not from documentation. Unknown fields are ignored; conditional fields are `Option`. Field list and how it was extracted: `docs/design.md`.
- Segments (`src/segments.rs`): `path` `git` `session` `model` `effort` `flags` `context` `limits` `cost` `api` `repo` `version`. Each returns `None` when it has nothing to say, and the separator goes with it.
- Lines are defined in `~/.claude/klaude-status.json` (`lines`, `color`, `max_width`, `bar_width`, `git_timeout_ms`); without the file, `Config::default()` applies. A broken config falls back to the default silently. Reference: `docs/configuration.md`.
- In a cramped terminal, segments are dropped in `render.rs:DROP_ORDER` order before anything gets truncated. `path` and `model` survive longest.
- Git is read with gitoxide (`gix`), never a `git` process. Divergence is capped at `999+`. **Untracked files do not set the dirty flag** (it would require walking the whole working tree).
- **The dirty check is the only cost that scales with repo size** (~5 ms at 15 files, ~180 ms warm and ~1.2 s cold at 51k). It runs against a deadline: past `git_timeout_ms` the segment shows `?` rather than guessing. `0` disables the deadline. Do not replace this with a cache; the no-state rule stands.
- Quotas (`rate_limits`) and context fill come straight from the input. Do not add network calls here.
- Measure after changing anything: `cargo test` + `--demo`. Budget is single-digit milliseconds in a normal repo.
- **Diagnosing a missing line:** `KLAUDE_STATUS_LOG=/tmp/klaude-status.log` (e.g. in the `env` block of `settings.json`) records every run: timestamp, pid, input and output sizes, cwd, rendered line. It separates three distinct faults: the command is **never run** (no lines in the log), it runs but **produces nothing** (`out=0B`), or it works and the fault is in rendering. Remove the variable afterwards, it writes on every run.
- Install: `./install.sh` (`~/.local/bin`, override with `PREFIX`). The script turns `statusLine` on with an **absolute path**.
- **`statusLine.command` is always an absolute path.** Claude Code runs the command without a shell profile, so `~/.local/bin` is not on `PATH`: a bare name works in a terminal session but not in the desktop app, and the only symptom is an empty line. Verify with: `env -i sh -c '/absolute/path/klaude-status < input.json'`.
- More detail: `docs/design.md`.
