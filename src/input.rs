//! The JSON Claude Code feeds to a statusLine command on stdin.
//!
//! The schema was read straight out of Claude Code 2.1.226's status line
//! builder rather than from documentation. Every field the builder adds
//! conditionally is an `Option`, and unknown fields are ignored: a newer Claude
//! Code may add fields without breaking this binary.

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Input {
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub session_id: String,
    pub session_name: Option<String>,
    pub model: Option<Model>,
    pub workspace: Option<Workspace>,
    pub version: Option<String>,
    pub output_style: Option<Named>,
    pub cost: Option<Cost>,
    pub context_window: Option<ContextWindow>,
    #[serde(default)]
    pub exceeds_200k_tokens: bool,
    #[serde(default)]
    pub fast_mode: bool,
    pub effort: Option<Effort>,
    pub thinking: Option<Thinking>,
    pub rate_limits: Option<RateLimits>,
    pub vim: Option<Vim>,
    pub agent: Option<Named>,
    pub pr: Option<Pr>,
    pub worktree: Option<Worktree>,
    /// Not present in the 2.1.226 status line input (the builder leaves it
    /// undefined), but the hook payload base produces it elsewhere - read it in
    /// case it ever shows up.
    pub permission_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Model {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct Workspace {
    #[serde(default)]
    pub current_dir: String,
    #[serde(default)]
    pub project_dir: String,
    #[serde(default)]
    pub added_dirs: Vec<String>,
    pub git_worktree: Option<String>,
    pub repo: Option<Repo>,
}

#[derive(Debug, Deserialize)]
pub struct Repo {
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Named {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Cost {
    #[serde(default)]
    pub total_cost_usd: f64,
    #[serde(default)]
    pub total_duration_ms: u64,
    #[serde(default)]
    pub total_api_duration_ms: u64,
    #[serde(default)]
    pub total_lines_added: u64,
    #[serde(default)]
    pub total_lines_removed: u64,
}

#[derive(Debug, Deserialize)]
pub struct ContextWindow {
    #[serde(default)]
    pub total_input_tokens: u64,
    #[serde(default)]
    pub context_window_size: u64,
    #[serde(default)]
    pub used_percentage: f64,
}

#[derive(Debug, Deserialize)]
pub struct Effort {
    #[serde(default)]
    pub level: String,
}

#[derive(Debug, Deserialize)]
pub struct Thinking {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct RateLimits {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
}

#[derive(Debug, Deserialize)]
pub struct Window {
    #[serde(default)]
    pub used_percentage: f64,
    /// Unix seconds.
    #[serde(default)]
    pub resets_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct Vim {
    #[serde(default)]
    pub mode: String,
}

#[derive(Debug, Deserialize)]
pub struct Pr {
    #[serde(default)]
    pub number: u64,
    pub review_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Worktree {
    #[serde(default)]
    pub name: String,
    pub branch: Option<String>,
}

impl Input {
    /// The current directory. `workspace.current_dir` and `cwd` come from the
    /// same source, but either one may be missing from a partial input.
    pub fn current_dir(&self) -> &str {
        self.workspace
            .as_ref()
            .map(|w| w.current_dir.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.cwd)
    }

    pub fn project_dir(&self) -> Option<&str> {
        self.workspace
            .as_ref()
            .map(|w| w.project_dir.as_str())
            .filter(|s| !s.is_empty())
    }
}
