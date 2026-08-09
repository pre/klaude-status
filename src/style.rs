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
}

/// Visible width in cells: skips ANSI escapes and counts CJK-wide characters as
/// two. Good enough for truncating a status line; not full Unicode width.
pub fn visible_width(s: &str) -> usize {
    let mut width = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip the CSI sequence up to its final letter.
            for e in chars.by_ref() {
                if e.is_ascii_alphabetic() {
                    break;
                }
            }
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
    fn wide_characters_count_as_two() {
        assert_eq!(visible_width("日本"), 4);
        assert_eq!(visible_width("ää"), 2);
    }
}
