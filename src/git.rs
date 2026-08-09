//! Git state via gitoxide, without a `git` subprocess.
//!
//! The status line runs after every turn (300 ms debounce), so forking is off
//! the table: `git status` costs hundreds of milliseconds in a large repo.
//!
//! Untracked files do not set the dirty flag: detecting them requires walking
//! the whole working tree, which is exactly the slow part being avoided.
//!
//! The dirty check is the only unbounded operation in the program, so it runs
//! against a deadline. See [`dirty_state`].

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gix::remote::Direction;

/// Walk ceiling: a large divergence is shown as `999+` so that a pathological
/// case (thousands of commits behind) cannot slow the status line down.
const WALK_LIMIT: usize = 999;

/// Whether the working tree differs from `HEAD`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Dirty {
    #[default]
    Clean,
    Modified,
    /// The check hit its deadline before reaching a verdict. Reported honestly
    /// rather than guessed: in a large repo, "clean" and "not checked" look the
    /// same from the outside and mean very different things.
    Unknown,
}

#[derive(Debug, Default)]
pub struct GitInfo {
    /// Short branch name, or `None` when HEAD is detached.
    pub branch: Option<String>,
    /// Short SHA when HEAD is detached.
    pub detached_at: Option<String>,
    pub dirty: Dirty,
    pub ahead: usize,
    pub behind: usize,
}

/// Collect git state for a directory. `None` if the directory is not in a repo.
pub fn collect(dir: &Path, dirty_budget: Option<Duration>) -> Option<GitInfo> {
    let repo = gix::discover(dir).ok()?;
    let mut info = GitInfo {
        dirty: dirty_state(&repo, dirty_budget),
        ..Default::default()
    };

    match repo.head_name().ok().flatten() {
        Some(name) => {
            info.branch = Some(name.shorten().to_string());
            if let Some((ahead, behind)) = divergence(&repo, &name) {
                info.ahead = ahead;
                info.behind = behind;
            }
        }
        None => {
            info.detached_at = repo
                .head_id()
                .ok()
                .map(|id| id.to_hex_with_len(7).to_string());
        }
    }

    Some(info)
}

/// Is the working tree dirty, answered within `budget`.
///
/// `gix::Repository::is_dirty()` does the right thing but has no deadline, and
/// the cost is driven by the size of the working tree, not by the size of the
/// change: comparing the index against the worktree has to stat every tracked
/// file, and a *clean* repo is the worst case because nothing stops the scan
/// early. Measured on a 51k-file repo that is ~120 ms warm and over a second
/// cold, which is not something to do after every keystroke.
///
/// So a watchdog thread flips an interrupt flag once the budget is gone, and
/// the answer becomes [`Dirty::Unknown`] rather than a guess. The watchdog is
/// detached and never joined: a fast repo pays only the thread spawn, and the
/// process exits without waiting for it. `budget` of `None` skips the watchdog
/// and runs to completion, which is always correct and occasionally slow.
///
/// The two comparisons keep the order `is_dirty()` uses, because staged changes
/// are found without a single `stat` and that is the likeliest way to be dirty.
/// The deadline covers both, but only the second one can actually be cut short
/// mid-flight, so the floor is whatever it costs to read the index once (~60 ms
/// at 51k files, a few hundred microseconds in a normal repo).
fn dirty_state(repo: &gix::Repository, budget: Option<Duration>) -> Dirty {
    let interrupt = Arc::new(AtomicBool::new(false));
    if let Some(budget) = budget {
        let watchdog = Arc::clone(&interrupt);
        std::thread::spawn(move || {
            std::thread::sleep(budget);
            watchdog.store(true, Ordering::Relaxed);
        });
    }

    if staged_changes(repo) {
        return Dirty::Modified;
    }
    // Out of budget already: the worktree scan is the expensive half, so
    // starting it now would blow past the deadline for no reason.
    if interrupt.load(Ordering::Relaxed) {
        return Dirty::Unknown;
    }

    match worktree_changes(repo, Arc::clone(&interrupt)) {
        Some(true) => Dirty::Modified,
        // The flag is the only way to tell "scanned everything, found nothing"
        // apart from "ran out of time".
        Some(false) if interrupt.load(Ordering::Relaxed) => Dirty::Unknown,
        Some(false) => Dirty::Clean,
        None => Dirty::Unknown,
    }
}

/// Differences between `HEAD^{tree}` and the index, i.e. staged changes.
/// Compares two indices in memory and never touches the working tree.
fn staged_changes(repo: &gix::Repository) -> bool {
    let (Ok(head_tree), Ok(index)) = (repo.head_tree_id_or_empty(), repo.index_or_empty()) else {
        return false;
    };
    let mut staged = false;
    let _ = repo.tree_index_status(
        &head_tree,
        &index,
        None,
        gix::status::tree_index::TrackRenames::Disabled,
        |_, _, _| {
            staged = true;
            Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Break(()))
        },
    );
    staged
}

/// Differences between the index and the working tree, i.e. unstaged edits.
/// This is the half that has to `stat` every tracked file, so it is the half
/// that honors `interrupt`. `None` if the scan could not be started at all.
fn worktree_changes(repo: &gix::Repository, interrupt: Arc<AtomicBool>) -> Option<bool> {
    let iter = repo
        .status(gix::progress::Discard)
        .ok()?
        .should_interrupt_owned(interrupt)
        .index_worktree_rewrites(None)
        .index_worktree_submodules(gix::status::Submodule::AsConfigured { check_dirty: true })
        .index_worktree_options_mut(|opts| {
            // No directory walk: untracked files are deliberately not part of
            // the dirty flag, and the walk is the expensive half.
            opts.dirwalk_options = None;
        })
        .into_index_worktree_iter(Vec::new())
        .ok()?;
    // An interrupted scan surfaces as an error item, which `is_ok` stops on.
    Some(iter.take_while(Result::is_ok).next().is_some())
}

/// Number of commits that exist only locally (ahead) and only on the tracking
/// branch (behind).
fn divergence(repo: &gix::Repository, branch: &gix::refs::FullName) -> Option<(usize, usize)> {
    let upstream = repo
        .branch_remote_tracking_ref_name(branch.as_ref(), Direction::Fetch)?
        .ok()?;
    let remote_id = repo
        .find_reference(upstream.as_ref())
        .ok()?
        .into_fully_peeled_id()
        .ok()?
        .detach();
    let local_id = repo.head_id().ok()?.detach();
    if local_id == remote_id {
        return Some((0, 0));
    }
    Some((
        count_reachable(repo, local_id, remote_id),
        count_reachable(repo, remote_id, local_id),
    ))
}

/// How many commits are reachable from `from` without going through `hidden`.
fn count_reachable(repo: &gix::Repository, from: gix::ObjectId, hidden: gix::ObjectId) -> usize {
    repo.rev_walk([from])
        .with_hidden([hidden])
        .all()
        .map(|walk| walk.take_while(Result::is_ok).take(WALK_LIMIT + 1).count())
        .unwrap_or(0)
}

impl GitInfo {
    /// The ref to display: branch name or detached SHA.
    pub fn head_label(&self) -> Option<&str> {
        self.branch.as_deref().or(self.detached_at.as_deref())
    }

    pub fn is_detached(&self) -> bool {
        self.branch.is_none() && self.detached_at.is_some()
    }

    /// Format a count with respect to the walk ceiling.
    pub fn fmt_count(n: usize) -> String {
        if n > WALK_LIMIT {
            format!("{WALK_LIMIT}+")
        } else {
            n.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_above_the_ceiling_are_marked() {
        assert_eq!(GitInfo::fmt_count(3), "3");
        assert_eq!(GitInfo::fmt_count(WALK_LIMIT), "999");
        assert_eq!(GitInfo::fmt_count(WALK_LIMIT + 1), "999+");
    }

    #[test]
    fn a_directory_outside_a_repo_yields_nothing() {
        assert!(collect(Path::new("/"), Some(Duration::from_millis(250))).is_none());
    }

    /// A zero budget means the watchdog has already fired by the time the scan
    /// starts, which must degrade to `Unknown` rather than to a wrong answer.
    #[test]
    fn an_exhausted_budget_reports_unknown_not_clean() {
        let Ok(repo) = gix::discover(Path::new(env!("CARGO_MANIFEST_DIR"))) else {
            return; // not built from a checkout; nothing to assert against
        };
        assert_eq!(dirty_state(&repo, Some(Duration::ZERO)), Dirty::Unknown);
    }

    /// The whole point of the deadline: a slow repo must not stall the line.
    #[test]
    fn the_dirty_check_respects_its_budget() {
        let Ok(repo) = gix::discover(Path::new(env!("CARGO_MANIFEST_DIR"))) else {
            return;
        };
        let start = std::time::Instant::now();
        let _ = dirty_state(&repo, Some(Duration::from_millis(50)));
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "dirty check overran its budget by far: {:?}",
            start.elapsed()
        );
    }
}
