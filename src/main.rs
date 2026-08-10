//! klaude-status: a status line for Claude Code.
//!
//! Reads Claude Code's statusLine JSON from stdin and prints 1 - 3 lines.
//! Design rules:
//!
//! * **Never crash.** The status line runs after every turn; a panic message or
//!   an error string would be rendered straight into the UI. Partial input
//!   produces a partial line, not an error.
//! * **No subprocesses, no network.** Everything comes from the input JSON or
//!   is read straight out of `.git` with gitoxide.
//! * **No state.** The same input always produces the same output.

mod config;
mod git;
mod input;
mod render;
mod segments;
mod style;

use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use config::Config;
use input::Input;
use segments::Ctx;
use style::Painter;

fn main() {
    if std::env::args().any(|a| a == "--demo") {
        demo();
        return;
    }

    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    let input: Input = serde_json::from_str(&raw).unwrap_or_default();

    let cfg = Config::load();
    let line = build(&input, &cfg, cfg.width_limit(terminal_width()));
    log_run(&raw, &line);
    println!("{line}");
}

/// Diagnostic log: `KLAUDE_STATUS_LOG=/path` records every run.
///
/// This is the only way to tell whether the command runs *at all*: if Claude
/// Code skips the status line, nothing is printed and nothing explains why. The
/// log separates "never ran" from "ran, produced nothing".
fn log_run(raw: &str, line: &str) {
    let Some(path) = std::env::var_os("KLAUDE_STATUS_LOG") else {
        return;
    };
    use std::io::Write;
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(
        file,
        "{} pid={} in={}B out={}B cwd={} line={:?}",
        unix_now(),
        std::process::id(),
        raw.len(),
        line.len(),
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        line
    );
}

fn build(input: &Input, cfg: &Config, width: Option<usize>) -> String {
    // Git is read only if some line asks for it: it is the only operation in
    // the whole program that touches the disk.
    let wants_git = cfg.lines.iter().flatten().any(|s| s == "git");
    let git = if wants_git {
        let dir = input.current_dir();
        if dir.is_empty() {
            None
        } else {
            git::collect(Path::new(dir), cfg.git_timeout())
        }
    } else {
        None
    };

    let ctx = Ctx {
        input,
        git,
        p: Painter::new(cfg.color_enabled()),
        cfg,
        now: unix_now(),
    };
    let line = render::render(&ctx, width);
    if !line.is_empty() {
        return line;
    }
    // A completely empty line is the worst possible output: it looks exactly
    // like a broken status line (wrong path, missing binary). If the input said
    // nothing, at least show the working directory.
    std::env::current_dir()
        .map(|p| ctx.p.bold(&p.display().to_string()))
        .unwrap_or_default()
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Our stdout is a pipe, so the width is probed from stderr and finally from
/// the environment. `None` means no limit.
fn terminal_width() -> Option<usize> {
    use std::io::stderr;
    use terminal_size::{Width, terminal_size_of};

    if let Some((Width(w), _)) = terminal_size_of(stderr()) {
        // Claude Code draws the status line inside its own box; a couple of
        // columns of margin keep it from wrapping.
        return Some((w as usize).saturating_sub(4).max(20));
    }
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .map(|w| w.saturating_sub(4).max(20))
}

/// Sample input with `{CWD}` replaced by the current directory, so the demo
/// renders the `git` segment against a real repository instead of a path that
/// exists only in this file.
fn demo_input(json: &str) -> Input {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    // Escape it as JSON would, then drop the quotes serde adds around it.
    let quoted = serde_json::to_string(&cwd).unwrap_or_default();
    let escaped = quoted.trim_matches('"');
    serde_json::from_str(&json.replace("{CWD}", escaped)).unwrap_or_default()
}

/// `--demo` renders the output from sample data without Claude Code. Used to
/// check layout and colors while developing.
///
/// The demo `resets_at` values are seconds *from now*, not unix timestamps, so
/// the countdowns look sensible whenever the demo happens to run.
fn demo() {
    let cfg = Config::load();
    let now = unix_now();
    for (title, json) in DEMOS {
        let mut input = demo_input(json);
        if let Some(rl) = input.rate_limits.as_mut() {
            for window in [rl.five_hour.as_mut(), rl.seven_day.as_mut()]
                .into_iter()
                .flatten()
            {
                window.resets_at += now;
            }
        }
        println!("\x1b[1m{title}\x1b[0m");
        println!("{}", build(&input, &cfg, cfg.width_limit(None)));
        println!();
    }
}

const DEMOS: &[(&str, &str)] = &[
    (
        "Ordinary session",
        r#"{
          "cwd": "{CWD}/src",
          "session_name": "Status line for Claude Code",
          "effort": {"level": "max"},
          "model": {"id": "claude-opus-5[1m]", "display_name": "Opus 5 (1M context)"},
          "workspace": {"current_dir": "{CWD}/src",
                        "project_dir": "{CWD}",
                        "added_dirs": []},
          "version": "2.1.226",
          "output_style": {"name": "default"},
          "cost": {"total_cost_usd": 6.2587, "total_duration_ms": 1380365,
                   "total_api_duration_ms": 1039875,
                   "total_lines_added": 587, "total_lines_removed": 26},
          "context_window": {"total_input_tokens": 168570, "context_window_size": 1000000,
                             "used_percentage": 17},
          "exceeds_200k_tokens": false, "fast_mode": false,
          "thinking": {"enabled": true},
          "rate_limits": {"five_hour": {"used_percentage": 29, "resets_at": 8040},
                          "seven_day": {"used_percentage": 66, "resets_at": 298800}}
        }"#,
    ),
    (
        "Context filling up, quota burning, thinking off",
        r#"{
          "cwd": "{CWD}",
          "effort": {"level": "low"},
          "model": {"id": "claude-sonnet-5", "display_name": "Sonnet 5"},
          "workspace": {"current_dir": "{CWD}",
                        "project_dir": "{CWD}",
                        "added_dirs": ["/tmp/extra"]},
          "cost": {"total_cost_usd": 42.5, "total_duration_ms": 9000000,
                   "total_lines_added": 12, "total_lines_removed": 340},
          "context_window": {"total_input_tokens": 173000, "context_window_size": 200000,
                             "used_percentage": 87},
          "exceeds_200k_tokens": false, "fast_mode": true,
          "thinking": {"enabled": false},
          "output_style": {"name": "Explanatory"},
          "rate_limits": {"five_hour": {"used_percentage": 93, "resets_at": 1500},
                          "seven_day": {"used_percentage": 78, "resets_at": 21600}}
        }"#,
    ),
    (
        "Worktree, PR and subagent",
        r#"{
          "cwd": "{CWD}",
          "session_name": "Fix truncation in a narrow terminal",
          "effort": {"level": "high"},
          "model": {"id": "claude-fable-5", "display_name": "Fable 5"},
          "workspace": {"current_dir": "{CWD}",
                        "project_dir": "{CWD}",
                        "added_dirs": []},
          "worktree": {"name": "fix-truncate", "branch": "fix-truncate"},
          "pr": {"number": 128, "review_state": "APPROVED"},
          "agent": {"name": "reviewer"},
          "permission_mode": "bypassPermissions",
          "context_window": {"total_input_tokens": 45000, "context_window_size": 200000,
                             "used_percentage": 23},
          "thinking": {"enabled": true},
          "cost": {"total_cost_usd": 0.42, "total_duration_ms": 65000,
                   "total_lines_added": 3, "total_lines_removed": 3}
        }"#,
    ),
    ("Partial input (cwd only)", r#"{"cwd": "{CWD}"}"#),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_cfg() -> Config {
        Config {
            color: Some(false),
            ..Config::default()
        }
    }

    #[test]
    fn empty_input_does_not_crash() {
        let input: Input = serde_json::from_str("{}").unwrap_or_default();
        let out = build(&input, &plain_cfg(), None);
        assert!(!out.contains("panic"));
    }

    #[test]
    fn garbage_input_neither_crashes_nor_prints_an_empty_line() {
        let input: Input = serde_json::from_str("not json").unwrap_or_default();
        let out = build(&input, &plain_cfg(), None);
        // An empty line would look like a broken status line, so the fallback
        // is the working directory.
        assert!(!out.is_empty());
        assert!(out.starts_with('/'), "expected a path, got: {out}");
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let input: Input =
            serde_json::from_str(r#"{"cwd":"/tmp","brand_new_field":{"x":1}}"#).unwrap();
        assert_eq!(input.cwd, "/tmp");
    }

    #[test]
    fn model_effort_and_path_are_shown() {
        let input = demo_input(DEMOS[0].1);
        let out = build(&input, &plain_cfg(), None);
        // The demo substitutes the real cwd, so the expected root is this
        // checkout's directory name rather than a hard-coded one.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        assert!(out.contains(&format!("{root}/src")), "path missing: {out}");
        assert!(out.contains("Opus 5"), "model missing: {out}");
        assert!(out.contains("1M"), "1M marker missing: {out}");
        assert!(out.contains("max"), "effort missing: {out}");
        assert!(out.contains("17%"), "context missing: {out}");
        assert!(out.contains("5h 29%"), "quota missing: {out}");
        assert_eq!(out.lines().count(), 2, "wrong number of lines: {out}");
    }

    #[test]
    fn a_narrow_terminal_drops_segments() {
        let input = demo_input(DEMOS[0].1);
        let cfg = plain_cfg();
        let narrow = build(&input, &cfg, Some(30));
        for line in narrow.lines() {
            assert!(
                style::visible_width(line) <= 30,
                "line too long ({}): {line}",
                style::visible_width(line)
            );
        }
        // The most important information survives even when it is tight:
        // `path` and `model` are last in the drop order, and the path gives up
        // its head before it gives up the project's name.
        assert!(narrow.contains("Opus 5"), "{narrow}");
        assert!(narrow.contains("klaude-status"), "{narrow}");
        assert!(
            narrow.lines().next().is_some_and(|l| !l.trim().is_empty()),
            "the path line vanished entirely: {narrow}"
        );
    }

    #[test]
    fn configured_max_width_overrides_the_terminal() {
        let input = demo_input(DEMOS[0].1);
        let cfg = Config {
            max_width: 40,
            ..plain_cfg()
        };
        // A wide terminal must not widen the line past the configured limit.
        let out = build(&input, &cfg, cfg.width_limit(Some(200)));
        for line in out.lines() {
            assert!(
                style::visible_width(line) <= 40,
                "line too long ({}): {line}",
                style::visible_width(line)
            );
        }
    }

    #[test]
    fn bypass_mode_shouts() {
        let input = demo_input(DEMOS[2].1);
        let out = build(&input, &plain_cfg(), None);
        assert!(out.contains("BYPASS"), "{out}");
        assert!(out.contains("PR#128"), "{out}");
        assert!(out.contains("@reviewer"), "{out}");
    }

    #[test]
    fn thinking_off_is_visible() {
        let input = demo_input(DEMOS[1].1);
        let out = build(&input, &plain_cfg(), None);
        assert!(out.contains("no-think"), "{out}");
        assert!(out.contains("fast"), "{out}");
        assert!(out.contains("Explanatory"), "{out}");
        assert!(out.contains("+extra"), "added directory missing: {out}");
    }
}
