//! ANSI styling.
//!
//! Deliberately uses the 8/16 basic colors rather than the 256-color palette:
//! those follow the terminal theme, so the same status line looks right on a
//! light and a dark background. Colors can be turned off entirely (`color:
//! false` or `NO_COLOR`), in which case these functions return the text as is.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    /// Dim gray: background information the eye skips over.
    Dim,
    /// The terminal's own foreground color, emphasized.
    Bold,
}

impl Color {
    fn code(self) -> &'static str {
        match self {
            Color::Red => "31",
            Color::Green => "32",
            Color::Yellow => "33",
            Color::Blue => "34",
            Color::Magenta => "35",
            Color::Cyan => "36",
            Color::Dim => "2",
            Color::Bold => "1",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Painter {
    enabled: bool,
}

impl Painter {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn paint(&self, text: &str, color: Color) -> String {
        if !self.enabled || text.is_empty() {
            return text.to_string();
        }
        format!("\x1b[{}m{text}\x1b[0m", color.code())
    }

    /// Emphasized color: bold + color. Used when something demands attention
    /// (bypass mode, an exhausted quota) and color alone is not enough.
    pub fn strong(&self, text: &str, color: Color) -> String {
        if !self.enabled || text.is_empty() {
            return text.to_string();
        }
        format!("\x1b[1;{}m{text}\x1b[0m", color.code())
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint(text, Color::Dim)
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint(text, Color::Bold)
    }

    /// OSC 8 hyperlink. A URL with control characters is not a link: it could
    /// end the escape early and write into the terminal.
    pub fn link(&self, text: &str, url: &str) -> String {
        if !self.enabled || url.is_empty() || url.chars().any(char::is_control) {
            return text.to_string();
        }
        format!("\x1b]8;;{url}\x1b\\{text}{LINK_END}")
    }
}

/// Closes an OSC 8 hyperlink.
pub const LINK_END: &str = "\x1b]8;;\x1b\\";

/// Consumes the rest of an escape whose ESC was just read and returns it. An
/// OSC (`ESC ]`, a hyperlink) ends at BEL or `ESC \\`, a CSI at its final letter.
pub fn take_escape(chars: &mut impl Iterator<Item = char>) -> String {
    let mut seq = String::new();
    let mut osc = false;
    for c in chars {
        seq.push(c);
        if seq.len() == 1 && c == ']' {
            osc = true;
            continue;
        }
        let done = if osc {
            c == '\x07' || seq.ends_with("\x1b\\")
        } else {
            c.is_ascii_alphabetic()
        };
        if done {
            break;
        }
    }
    seq
}

/// Visible width in cells: skips ANSI escapes and counts CJK-wide characters as
/// two. Good enough for truncating a status line; not full Unicode width.
pub fn visible_width(s: &str) -> usize {
    let mut width = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            take_escape(&mut chars);
            continue;
        }
        width += char_width(c);
    }
    width
}

fn char_width(c: char) -> usize {
    let cp = c as u32;
    // Combining marks take no space.
    if (0x0300..=0x036F).contains(&cp) {
        return 0;
    }
    // CJK, Hangul, emoji ranges: two cells.
    let wide = (0x1100..=0x115F).contains(&cp)
        || (0x2E80..=0xA4CF).contains(&cp)
        || (0xAC00..=0xD7A3).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE6F).contains(&cp)
        || (0xFF00..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x1F300..=0x1F64F).contains(&cp)
        || (0x1F900..=0x1F9FF).contains(&cp);
    if wide { 2 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_does_not_add_width() {
        let p = Painter::new(true);
        let painted = p.paint("main", Color::Green);
        assert!(painted.len() > 4);
        assert_eq!(visible_width(&painted), 4);
    }

    #[test]
    fn colors_off_returns_raw_text() {
        let p = Painter::new(false);
        assert_eq!(p.paint("main", Color::Green), "main");
        assert_eq!(p.strong("main", Color::Red), "main");
    }

    #[test]
    fn a_link_adds_no_width() {
        let p = Painter::new(true);
        let linked = p.link(
            &p.paint("PR#857", Color::Blue),
            "https://github.com/o/r/pull/857",
        );
        assert_eq!(visible_width(&linked), 6);
    }

    #[test]
    fn no_link_without_colors_or_with_a_control_character() {
        assert_eq!(Painter::new(false).link("PR#1", "https://x/1"), "PR#1");
        assert_eq!(Painter::new(true).link("PR#1", "https://x/1\x07"), "PR#1");
        assert_eq!(Painter::new(true).link("PR#1", ""), "PR#1");
    }

    #[test]
    fn wide_characters_count_as_two() {
        assert_eq!(visible_width("日本"), 4);
        assert_eq!(visible_width("ää"), 2);
    }
}
