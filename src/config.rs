//! Configuration.
//!
//! The layout is read from `~/.claude/klaude-status.json` if that file exists,
//! otherwise the default is used. The file describes lines as lists of segment
//! names, so the number and order of lines is up to the user without a
//! recompile.
//!
//! ```json
//! {
//!   "lines": [
//!     ["path", "git", "session"],
//!     ["model", "effort", "flags", "context", "limits", "cost"]
//!   ],
//!   "color": true,
//!   "max_width": 0,
//!   "bar_width": 8,
//!   "git_timeout_ms": 250
//! }
//! ```

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// One inner list = one line of the status line.
    pub lines: Vec<Vec<String>>,
    /// `None` = decide automatically (on unless `NO_COLOR` is set).
    pub color: Option<bool>,
    /// 0 = detect from the terminal, anything else is a fixed upper bound.
    pub max_width: usize,
    pub bar_width: usize,
    /// Deadline for the "is the working tree dirty" check, in milliseconds.
    /// The cost of that check scales with the size of the working tree, so in a
    /// large repo it is the one thing that can blow the frame budget; past this
    /// deadline the `git` segment reports `?` instead of stalling. 0 removes
    /// the deadline: always correct, occasionally slow.
    pub git_timeout_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lines: vec![
                vec!["path".into(), "git".into(), "session".into()],
                vec![
                    "model".into(),
                    "effort".into(),
                    "flags".into(),
                    "context".into(),
                    "limits".into(),
                    "cost".into(),
                ],
            ],
            color: None,
            max_width: 0,
            bar_width: 8,
            git_timeout_ms: 250,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        // A broken config must not leave the status line empty: fall back to
        // the default silently.
        serde_json::from_str(&text).unwrap_or_default()
    }

    fn path() -> Option<PathBuf> {
        if let Some(explicit) = std::env::var_os("KLAUDE_STATUS_CONFIG") {
            return Some(PathBuf::from(explicit));
        }
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".claude/klaude-status.json"))
    }

    /// Width budget for one line. A configured `max_width` wins over whatever
    /// the terminal reports, which is how you pin the layout in a terminal that
    /// lies about its size (or in a screenshot).
    pub fn width_limit(&self, detected: Option<usize>) -> Option<usize> {
        if self.max_width > 0 {
            Some(self.max_width)
        } else {
            detected
        }
    }

    /// `None` = no deadline at all.
    pub fn git_timeout(&self) -> Option<std::time::Duration> {
        match self.git_timeout_ms {
            0 => None,
            ms => Some(std::time::Duration::from_millis(ms)),
        }
    }

    /// Colors on unless the config or `NO_COLOR` says otherwise.
    pub fn color_enabled(&self) -> bool {
        self.color
            .unwrap_or_else(|| std::env::var_os("NO_COLOR").is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_width_wins_over_the_terminal() {
        let cfg = Config {
            max_width: 60,
            ..Config::default()
        };
        assert_eq!(cfg.width_limit(Some(200)), Some(60));
        assert_eq!(cfg.width_limit(None), Some(60));
    }

    #[test]
    fn zero_timeout_means_no_deadline() {
        let cfg = Config {
            git_timeout_ms: 0,
            ..Config::default()
        };
        assert_eq!(cfg.git_timeout(), None);
        assert!(Config::default().git_timeout().is_some());
    }

    #[test]
    fn zero_width_means_detect() {
        let cfg = Config::default();
        assert_eq!(cfg.width_limit(Some(120)), Some(120));
        assert_eq!(cfg.width_limit(None), None);
    }
}
