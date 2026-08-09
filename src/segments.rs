//! Segments: one named piece of information on the status line.
//!
//! Every segment returns `None` when it has nothing to say (no git, no quota
//! data, no session name). That keeps the lines tight and avoids leaving empty
//! separators behind.

use crate::config::Config;
use crate::git::{Dirty, GitInfo};
use crate::input::Input;
use crate::style::{Color, Painter};

/// Separator inside a segment: parts of the same topic.
const INNER: &str = " · ";

pub struct Ctx<'a> {
    pub input: &'a Input,
    pub git: Option<GitInfo>,
    pub p: Painter,
    pub cfg: &'a Config,
    /// Unix seconds, read once so that every countdown agrees with the others.
    pub now: i64,
}

impl Ctx<'_> {
    pub fn segment(&self, name: &str) -> Option<String> {
        match name {
            "path" => self.path(),
            "git" => self.git_status(),
            "session" => self.session(),
            "model" => self.model(),
            "effort" => self.effort(),
            "flags" => self.flags(),
            "context" => self.context(),
            "limits" => self.limits(),
            "cost" => self.cost(),
            "api" => self.api_share(),
            "repo" => self.repo(),
            "version" => self.version(),
            _ => None,
        }
    }

    /// Where we are. The project root is emphasized and the path inside it is
    /// dimmed, because the root says *which* project and the rest only says
    /// *where* inside it.
    fn path(&self) -> Option<String> {
        let cwd = self.input.current_dir();
        if cwd.is_empty() {
            return None;
        }

        let mut out = match self.input.project_dir().and_then(|p| relative_to(cwd, p)) {
            Some((project, rel)) => {
                let mut s = self.p.bold(basename(project));
                if !rel.is_empty() {
                    s.push_str(&self.p.dim(&format!("/{rel}")));
                }
                s
            }
            None => self.p.bold(&shorten_path(cwd)),
        };

        // Added directories (`/add-dir`) widen what Claude can see, so their
        // existence belongs on the status line.
        let added = self
            .input
            .workspace
            .as_ref()
            .map(|w| w.added_dirs.len())
            .unwrap_or(0);
        if added > 0 {
            out.push_str(&self.p.dim(&format!(" +{added}d")));
        }
        Some(out)
    }

    fn git_status(&self) -> Option<String> {
        let git = self.git.as_ref();
        // The branch name comes from git, but Claude Code's worktree info is a
        // fallback: the session may sit in a worktree whose .git link could not
        // be opened.
        let head = git.and_then(|g| g.head_label()).or_else(|| {
            self.input
                .worktree
                .as_ref()
                .and_then(|w| w.branch.as_deref())
        })?;

        let dirty = git.map(|g| g.dirty).unwrap_or_default();
        let head_color = if git.is_some_and(GitInfo::is_detached) {
            Color::Magenta
        } else {
            match dirty {
                Dirty::Modified => Color::Yellow,
                Dirty::Clean => Color::Green,
                // Gray reads as "no information", which is exactly what it is.
                Dirty::Unknown => Color::Dim,
            }
        };

        let mut out = self.p.dim("\u{2387} ");
        out.push_str(&self.p.paint(head, head_color));
        match dirty {
            Dirty::Modified => out.push_str(&self.p.paint("*", Color::Yellow)),
            Dirty::Unknown => out.push_str(&self.p.dim("?")),
            Dirty::Clean => {}
        }
        if let Some(g) = git {
            if g.ahead > 0 {
                out.push_str(&self.p.paint(
                    &format!(" \u{2191}{}", GitInfo::fmt_count(g.ahead)),
                    Color::Green,
                ));
            }
            if g.behind > 0 {
                out.push_str(&self.p.paint(
                    &format!(" \u{2193}{}", GitInfo::fmt_count(g.behind)),
                    Color::Red,
                ));
            }
        }

        // The worktree name arrives ready-made from Claude Code, so it is not
        // looked up in git again.
        if let Some(name) = self.worktree_name() {
            out.push_str(&self.p.paint(&format!(" \u{29c9}{name}"), Color::Cyan));
        }
        Some(out)
    }

    fn worktree_name(&self) -> Option<&str> {
        if let Some(wt) = self.input.worktree.as_ref().filter(|w| !w.name.is_empty()) {
            return Some(&wt.name);
        }
        self.input
            .workspace
            .as_ref()
            .and_then(|w| w.git_worktree.as_deref())
            .map(basename)
    }

    /// The session name is what tells parallel sessions apart: same repo,
    /// different task. An unnamed session shows the start of its id, which is
    /// enough to find the window's transcript.
    fn session(&self) -> Option<String> {
        match self.input.session_name.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => Some(self.p.dim(&truncate(name, 42))),
            _ => {
                let id = self.input.session_id.get(..4)?;
                Some(self.p.dim(&format!("#{id}")))
            }
        }
    }

    /// `owner/repo`, for people whose project name repeats across several
    /// organizations. Not on the default lines; add it in the config if needed.
    fn repo(&self) -> Option<String> {
        let repo = self.input.workspace.as_ref()?.repo.as_ref()?;
        if repo.owner.is_empty() || repo.name.is_empty() {
            return None;
        }
        Some(self.p.dim(&format!("{}/{}", repo.owner, repo.name)))
    }

    /// How much of the session's wall clock went to waiting for the model. A
    /// high number means the session is API-bound, not work-bound.
    fn api_share(&self) -> Option<String> {
        let c = self.input.cost.as_ref()?;
        if c.total_duration_ms == 0 || c.total_api_duration_ms == 0 {
            return None;
        }
        let share = (c.total_api_duration_ms as f64 / c.total_duration_ms as f64) * 100.0;
        Some(self.p.dim(&format!("api {:.0}%", share.clamp(0.0, 100.0))))
    }

    fn model(&self) -> Option<String> {
        let m = self.input.model.as_ref()?;
        let raw = if m.display_name.is_empty() {
            &m.id
        } else {
            &m.display_name
        };
        let (name, million) = split_context_suffix(raw);
        if name.is_empty() {
            return None;
        }
        let mut out = self.p.paint(name, Color::Cyan);
        // Flagging the 1M context matters: it changes both the context ceiling
        // and the pricing past 200k.
        if million {
            out.push_str(&self.p.dim(" 1M"));
        }
        Some(out)
    }

    fn effort(&self) -> Option<String> {
        let level = self.input.effort.as_ref()?.level.as_str();
        if level.is_empty() {
            return None;
        }
        let color = match level {
            "max" | "xhigh" => Color::Magenta,
            "high" => Color::Blue,
            _ => Color::Dim,
        };
        Some(self.p.paint(level, color))
    }

    /// Anything that is not the default setting and could explain why Claude is
    /// behaving unexpectedly.
    fn flags(&self) -> Option<String> {
        let mut flags: Vec<String> = Vec::new();

        if self.input.fast_mode {
            flags.push(self.p.paint("\u{26a1}fast", Color::Cyan));
        }
        if let Some(t) = &self.input.thinking
            && !t.enabled
        {
            flags.push(self.p.paint("no-think", Color::Red));
        }
        if self.input.exceeds_200k_tokens {
            flags.push(self.p.dim("200k+"));
        }
        if let Some(style) = self
            .input
            .output_style
            .as_ref()
            .filter(|s| !s.name.is_empty() && s.name != "default")
        {
            flags.push(self.p.paint(&style.name, Color::Blue));
        }
        if let Some(agent) = self.input.agent.as_ref().filter(|a| !a.name.is_empty()) {
            flags.push(self.p.paint(&format!("@{}", agent.name), Color::Magenta));
        }
        if let Some(vim) = self.input.vim.as_ref().filter(|v| !v.mode.is_empty()) {
            flags.push(self.p.dim(&vim.mode.chars().take(1).collect::<String>()));
        }
        if let Some(pr) = &self.input.pr {
            let mut s = format!("PR#{}", pr.number);
            if let Some(state) = pr.review_state.as_deref() {
                s.push_str(&format!(" {}", state.to_lowercase()));
            }
            flags.push(self.p.paint(&s, Color::Blue));
        }
        // Bypass mode is a safety question, so it shouts if it ever does appear
        // in the input.
        if let Some(mode) = self.input.permission_mode.as_deref() {
            match mode {
                "bypassPermissions" => flags.push(self.p.strong("BYPASS", Color::Red)),
                "plan" => flags.push(self.p.paint("plan", Color::Blue)),
                "acceptEdits" => flags.push(self.p.dim("auto-edit")),
                _ => {}
            }
        }

        if flags.is_empty() {
            None
        } else {
            Some(flags.join(" "))
        }
    }

    /// How full the context is. The single most important number: it predicts
    /// when the conversation gets compacted and quality dips.
    fn context(&self) -> Option<String> {
        let cw = self.input.context_window.as_ref()?;
        if cw.context_window_size == 0 {
            return None;
        }
        let pct = cw.used_percentage.clamp(0.0, 100.0);
        let color = match pct {
            p if p >= 80.0 => Color::Red,
            p if p >= 60.0 => Color::Yellow,
            _ => Color::Green,
        };
        let bar = bar(pct, self.cfg.bar_width);
        Some(format!(
            "{} {}{}",
            self.p.paint(&bar, color),
            self.p.paint(&format!("{pct:.0}%"), color),
            self.p.dim(&format!(
                " {}/{}",
                compact_tokens(cw.total_input_tokens),
                compact_tokens(cw.context_window_size)
            ))
        ))
    }

    /// Subscription quotas straight from Claude Code: fresher than any separate
    /// polling, and free (no network call).
    fn limits(&self) -> Option<String> {
        let rl = self.input.rate_limits.as_ref()?;
        let mut parts = Vec::new();
        for (label, window) in [("5h", &rl.five_hour), ("7d", &rl.seven_day)] {
            let Some(w) = window else { continue };
            let used = w.used_percentage.clamp(0.0, 100.0);
            let color = match used {
                u if u >= 90.0 => Color::Red,
                u if u >= 75.0 => Color::Yellow,
                _ => Color::Green,
            };
            let mut cell = format!(
                "{} {}",
                self.p.dim(label),
                self.p.paint(&format!("{used:.0}%"), color)
            );
            if w.resets_at > self.now {
                cell.push_str(
                    &self
                        .p
                        .dim(&format!(" {}", fmt_short(w.resets_at - self.now))),
                );
            }
            parts.push(cell);
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(INNER))
        }
    }

    fn cost(&self) -> Option<String> {
        let c = self.input.cost.as_ref()?;
        let mut parts = Vec::new();
        if c.total_cost_usd > 0.0 {
            parts.push(self.p.dim(&format!("${:.2}", c.total_cost_usd)));
        }
        if c.total_lines_added > 0 || c.total_lines_removed > 0 {
            parts.push(format!(
                "{}{}",
                self.p
                    .paint(&format!("+{}", c.total_lines_added), Color::Green),
                self.p
                    .paint(&format!("/-{}", c.total_lines_removed), Color::Red)
            ));
        }
        if c.total_duration_ms >= 1000 {
            parts.push(self.p.dim(&fmt_short((c.total_duration_ms / 1000) as i64)));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    }

    fn version(&self) -> Option<String> {
        let v = self.input.version.as_deref()?;
        Some(self.p.dim(&format!("cc{v}")))
    }
}

/// Return `(project root, relative part)` if `cwd` is inside the project. The
/// prefix comparison happens on a component boundary so `/a/bc` does not match
/// `/a/b`.
fn relative_to<'a>(cwd: &'a str, project: &'a str) -> Option<(&'a str, &'a str)> {
    let rest = cwd.strip_prefix(project)?;
    if rest.is_empty() {
        return Some((project, ""));
    }
    let rest = rest.strip_prefix('/')?;
    Some((project, rest))
}

fn basename(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
}

/// Shorten an absolute path: home becomes `~`, and a long path collapses to its
/// last two components.
fn shorten_path(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let shown = if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    };
    let parts: Vec<&str> = shown.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 3 {
        return shown;
    }
    format!("\u{2026}/{}", parts[parts.len() - 2..].join("/"))
}

/// `"Opus 5 (1M context)"` -> `("Opus 5", true)`.
fn split_context_suffix(name: &str) -> (&str, bool) {
    match name.find(" (1M context)") {
        Some(idx) => (&name[..idx], true),
        None => (name, name.contains("[1m]")),
    }
}

fn bar(pct: f64, width: usize) -> String {
    let width = width.max(1);
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    // At least one block as soon as there is any usage: zero blocks in a busy
    // session would look broken.
    let filled = if filled == 0 && pct > 0.0 { 1 } else { filled };
    let mut s = String::with_capacity(width * 3);
    for i in 0..width {
        s.push(if i < filled { '\u{2589}' } else { '\u{2591}' });
    }
    s
}

fn compact_tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}k", n / 1_000),
        _ => {
            let m = n as f64 / 1_000_000.0;
            if m < 10.0 && (m.fract() * 10.0).round() > 0.0 {
                format!("{m:.1}M")
            } else {
                format!("{m:.0}M")
            }
        }
    }
}

/// Compact duration: `3d4h`, `2h14m`, `48m`, `30s`.
fn fmt_short(secs: i64) -> String {
    let s = secs.max(0);
    let (d, h, m) = (s / 86_400, (s % 86_400) / 3_600, (s % 3_600) / 60);
    if d > 0 {
        if h > 0 {
            format!("{d}d{h}h")
        } else {
            format!("{d}d")
        }
    } else if h > 0 {
        format!("{h}h{m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}\u{2026}", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_scales() {
        assert_eq!(bar(0.0, 8), "░░░░░░░░");
        assert_eq!(bar(100.0, 8), "▉▉▉▉▉▉▉▉");
        assert_eq!(bar(50.0, 8), "▉▉▉▉░░░░");
        // Small but nonzero usage is always visible.
        assert_eq!(bar(0.5, 8), "▉░░░░░░░");
    }

    #[test]
    fn tokens_get_compact() {
        assert_eq!(compact_tokens(950), "950");
        assert_eq!(compact_tokens(168_570), "168k");
        assert_eq!(compact_tokens(1_000_000), "1M");
        assert_eq!(compact_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn durations_get_short() {
        assert_eq!(fmt_short(45), "45s");
        assert_eq!(fmt_short(8_040), "2h14m");
        assert_eq!(fmt_short(2_880), "48m");
        assert_eq!(fmt_short(280_800), "3d6h");
    }

    #[test]
    fn the_1m_model_is_recognized() {
        assert_eq!(
            split_context_suffix("Opus 5 (1M context)"),
            ("Opus 5", true)
        );
        assert_eq!(split_context_suffix("Sonnet 5"), ("Sonnet 5", false));
        assert_eq!(
            split_context_suffix("claude-opus-5[1m]"),
            ("claude-opus-5[1m]", true)
        );
    }

    #[test]
    fn paths_stay_on_a_component_boundary() {
        assert_eq!(relative_to("/a/b/src", "/a/b"), Some(("/a/b", "src")));
        assert_eq!(relative_to("/a/b", "/a/b"), Some(("/a/b", "")));
        // /a/bc is not inside /a/b.
        assert_eq!(relative_to("/a/bc", "/a/b"), None);
    }

    #[test]
    fn a_long_path_collapses() {
        assert_eq!(
            shorten_path("/Users/you/dev/klaude-status"),
            "…/dev/klaude-status"
        );
        assert_eq!(shorten_path("/a/b"), "/a/b");
    }

    #[test]
    fn a_name_truncates_cleanly() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }
}
