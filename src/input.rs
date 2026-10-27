//! The JSON Claude Code feeds to a statusLine command on stdin.
//!
//! The schema was read straight out of Claude Code 2.1.226's status line
//! builder rather than from documentation. Every field the builder adds
//! conditionally is an `Option`, unknown fields are ignored, and every field
//! tolerates a `null` or a wrong type: a field that has no value yet, or whose
//! shape changed in a newer Claude Code, degrades to its default on its own.
//! One bad value must never blank the whole line.

use serde::{Deserialize, Deserializer};

/// Accept the field's declared type or fall back to its default. A plain
/// `#[serde(default)]` only covers a *missing* key; an explicit `null` (or any
/// other unexpected type) would otherwise abort the whole parse, and the render
/// would fall back to a bare path even though the payload was full of data.
fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or_default())
}

#[derive(Debug, Default, Deserialize)]
pub struct Input {
    #[serde(default, deserialize_with = "lenient")]
    pub cwd: String,
    #[serde(default, deserialize_with = "lenient")]
    pub session_id: String,
    #[serde(default, deserialize_with = "lenient")]
    pub session_name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub model: Option<Model>,
    #[serde(default, deserialize_with = "lenient")]
    pub workspace: Option<Workspace>,
    #[serde(default, deserialize_with = "lenient")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub output_style: Option<Named>,
    #[serde(default, deserialize_with = "lenient")]
    pub cost: Option<Cost>,
    #[serde(default, deserialize_with = "lenient")]
    pub context_window: Option<ContextWindow>,
    #[serde(default, deserialize_with = "lenient")]
    pub exceeds_200k_tokens: bool,
    #[serde(default, deserialize_with = "lenient")]
    pub fast_mode: bool,
    #[serde(default, deserialize_with = "lenient")]
    pub effort: Option<Effort>,
    #[serde(default, deserialize_with = "lenient")]
    pub thinking: Option<Thinking>,
    #[serde(default, deserialize_with = "lenient")]
    pub rate_limits: Option<RateLimits>,
    #[serde(default, deserialize_with = "lenient")]
    pub vim: Option<Vim>,
    #[serde(default, deserialize_with = "lenient")]
    pub agent: Option<Named>,
    #[serde(default, deserialize_with = "lenient")]
    pub pr: Option<Pr>,
    #[serde(default, deserialize_with = "lenient")]
    pub worktree: Option<Worktree>,
    /// Not present in the 2.1.226 status line input (the builder leaves it
    /// undefined), but the hook payload base produces it elsewhere - read it in
    /// case it ever shows up.
    #[serde(default, deserialize_with = "lenient")]
    pub permission_mode: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Model {
    #[serde(default, deserialize_with = "lenient")]
    pub id: String,
    #[serde(default, deserialize_with = "lenient")]
    pub display_name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Workspace {
    #[serde(default, deserialize_with = "lenient")]
    pub current_dir: String,
    #[serde(default, deserialize_with = "lenient")]
    pub project_dir: String,
    #[serde(default, deserialize_with = "lenient")]
    pub added_dirs: Vec<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub git_worktree: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub repo: Option<Repo>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Repo {
    #[serde(default, deserialize_with = "lenient")]
    pub owner: String,
    #[serde(default, deserialize_with = "lenient")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Named {
    #[serde(default, deserialize_with = "lenient")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Cost {
    #[serde(default, deserialize_with = "lenient")]
    pub total_cost_usd: f64,
    #[serde(default, deserialize_with = "lenient")]
    pub total_duration_ms: u64,
    #[serde(default, deserialize_with = "lenient")]
    pub total_api_duration_ms: u64,
    #[serde(default, deserialize_with = "lenient")]
    pub total_lines_added: u64,
    #[serde(default, deserialize_with = "lenient")]
    pub total_lines_removed: u64,
}

#[derive(Debug, Default, Deserialize)]
pub struct ContextWindow {
    #[serde(default, deserialize_with = "lenient")]
    pub total_input_tokens: u64,
    #[serde(default, deserialize_with = "lenient")]
    pub context_window_size: u64,
    #[serde(default, deserialize_with = "lenient")]
    pub used_percentage: f64,
}

#[derive(Debug, Default, Deserialize)]
pub struct Effort {
    #[serde(default, deserialize_with = "lenient")]
    pub level: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Thinking {
    #[serde(default, deserialize_with = "lenient")]
    pub enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct RateLimits {
    #[serde(default, deserialize_with = "lenient")]
    pub five_hour: Option<Window>,
    #[serde(default, deserialize_with = "lenient")]
    pub seven_day: Option<Window>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Window {
    #[serde(default, deserialize_with = "lenient")]
    pub used_percentage: f64,
    /// Unix seconds.
    #[serde(default, deserialize_with = "lenient")]
    pub resets_at: i64,
}

#[derive(Debug, Default, Deserialize)]
pub struct Vim {
    #[serde(default, deserialize_with = "lenient")]
    pub mode: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Pr {
    #[serde(default, deserialize_with = "lenient")]
    pub number: u64,
    #[serde(default, deserialize_with = "lenient")]
    pub url: String,
    #[serde(default, deserialize_with = "lenient")]
    pub review_state: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Worktree {
    #[serde(default, deserialize_with = "lenient")]
    pub name: String,
    #[serde(default, deserialize_with = "lenient")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_in_one_field_does_not_blank_the_rest() {
        let i: Input = serde_json::from_str(
            r#"{"cwd":"/tmp",
                "model":{"id":null,"display_name":"Fable 5"},
                "cost":{"total_cost_usd":null,"total_lines_added":3},
                "context_window":null,
                "exceeds_200k_tokens":null}"#,
        )
        .expect("lenient fields must keep the parse alive");
        assert_eq!(i.cwd, "/tmp");
        let model = i.model.as_ref().expect("model survives its null id");
        assert_eq!(model.display_name, "Fable 5");
        assert_eq!(model.id, "");
        let cost = i.cost.as_ref().expect("cost survives its null usd");
        assert_eq!(cost.total_cost_usd, 0.0);
        assert_eq!(cost.total_lines_added, 3);
        assert!(i.context_window.is_none());
        assert!(!i.exceeds_200k_tokens);
    }

    #[test]
    fn wrong_shaped_subobject_degrades_to_none() {
        let i: Input =
            serde_json::from_str(r#"{"cwd":"/tmp","workspace":"nope"}"#).expect("must parse");
        assert_eq!(i.cwd, "/tmp");
        assert!(i.workspace.is_none());
    }
}
