//! Assembling lines and making them fit.
//!
//! When a line does not fit the terminal, segments are dropped least-important
//! first instead of cutting the line in half: losing the cost readout beats
//! seeing half a directory name. Truncation is the last resort.

use crate::segments::Ctx;
use crate::style::{Painter, visible_width};

/// Separator between segments.
const OUTER: &str = " \u{2502} ";

/// Drop order when space runs out, least important first. A segment that is not
/// listed here is dropped last.
const DROP_ORDER: &[&str] = &[
    "version", "api", "repo", "cost", "session", "limits", "context", "flags", "effort", "git",
    "model", "path",
];

pub fn render(ctx: &Ctx, width: Option<usize>) -> String {
    ctx.cfg
        .lines
        .iter()
        .map(|names| render_line(ctx, names, width))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_line(ctx: &Ctx, names: &[String], width: Option<usize>) -> String {
    let mut cells: Vec<(&str, String)> = names
        .iter()
        .filter_map(|name| ctx.segment(name).map(|text| (name.as_str(), text)))
        .collect();

    let Some(max) = width else {
        return join(&cells, ctx.p);
    };

    while cells.len() > 1 && visible_width(&join(&cells, ctx.p)) > max {
        let Some(victim) = weakest(&cells) else { break };
        cells.remove(victim);
    }

    let line = join(&cells, ctx.p);
    if visible_width(&line) > max {
        truncate_visible(&line, max)
    } else {
        line
    }
}

fn join(cells: &[(&str, String)], p: Painter) -> String {
    let sep = p.dim(OUTER);
    cells
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join(&sep)
}

/// Index of the segment to drop next.
fn weakest(cells: &[(&str, String)]) -> Option<usize> {
    cells
        .iter()
        .enumerate()
        .min_by_key(|(_, (name, _))| {
            DROP_ORDER
                .iter()
                .position(|d| d == name)
                .unwrap_or(DROP_ORDER.len())
        })
        .map(|(i, _)| i)
}

/// Truncate by visible width, keeping ANSI escapes intact and closing any style
/// that was left open.
fn truncate_visible(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let budget = max.saturating_sub(1);
    let mut out = String::with_capacity(s.len());
    let mut width = 0usize;
    let mut styled = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            styled = true;
            out.push(c);
            for e in chars.by_ref() {
                out.push(e);
                if e.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        let w = visible_width(&c.to_string());
        if width + w > budget {
            break;
        }
        out.push(c);
        width += w;
    }
    out.push('\u{2026}');
    // Only reset if something was actually opened: with colors off the output
    // must stay free of escape sequences.
    if styled {
        out.push_str("\x1b[0m");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_weakest_segment_goes_first() {
        let cells = vec![
            ("path", "p".to_string()),
            ("cost", "c".to_string()),
            ("model", "m".to_string()),
        ];
        assert_eq!(weakest(&cells), Some(1));
    }

    #[test]
    fn an_unknown_segment_gets_high_priority() {
        let cells = vec![("mine", "x".to_string()), ("cost", "c".to_string())];
        assert_eq!(weakest(&cells), Some(1));
    }

    #[test]
    fn truncation_respects_the_width() {
        let out = truncate_visible("abcdefghij", 5);
        assert_eq!(visible_width(&out), 5);
    }

    #[test]
    fn truncation_does_not_break_escapes() {
        let painted = format!("\x1b[32m{}\x1b[0m", "abcdefghij");
        let out = truncate_visible(&painted, 4);
        assert_eq!(visible_width(&out), 4);
        assert!(out.starts_with("\x1b[32m"));
        assert!(out.ends_with("\x1b[0m"));
    }

    #[test]
    fn truncation_stays_plain_when_the_input_is_plain() {
        let out = truncate_visible("abcdefghij", 5);
        assert!(!out.contains('\x1b'), "unexpected escape in {out:?}");
    }
}
