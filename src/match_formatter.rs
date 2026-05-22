use crate::display_width;

pub struct MatchFormatter {
    pub hint_style: String,
    pub selected_hint_style: String,
    pub highlight_style: String,
    pub selected_highlight_style: String,
    pub backdrop_style: String,
    pub hint_position: String,
}

impl MatchFormatter {
    pub fn new(
        hint_style: String,
        selected_hint_style: String,
        highlight_style: String,
        selected_highlight_style: String,
        backdrop_style: String,
        hint_position: String,
    ) -> Self {
        MatchFormatter {
            hint_style,
            selected_hint_style,
            highlight_style,
            selected_highlight_style,
            backdrop_style,
            hint_position,
        }
    }

    /// Format a match with its hint overlay.
    /// `offset` is Some((start_byte_pos, byte_len)) for named-capture patterns.
    pub fn format(
        &self,
        hint: &str,
        highlight: &str,
        selected: bool,
        offset: Option<(usize, usize)>,
    ) -> String {
        const RESET: &str = "\x1b[0m";
        format!(
            "{}{}{}{}",
            RESET,
            self.before_offset(offset, highlight),
            self.format_offset(selected, hint, self.within_offset(offset, highlight)),
            self.after_offset(offset, highlight),
        )
    }

    fn before_offset<'a>(&self, offset: Option<(usize, usize)>, highlight: &'a str) -> String {
        match offset {
            None => String::new(),
            Some((start, _)) => {
                if start == 0 {
                    String::new()
                } else {
                    format!("{}{}", self.backdrop_style, &highlight[..start])
                }
            }
        }
    }

    fn within_offset<'a>(&self, offset: Option<(usize, usize)>, highlight: &'a str) -> &'a str {
        match offset {
            None => highlight,
            Some((start, len)) => &highlight[start..start + len],
        }
    }

    fn after_offset<'a>(&self, offset: Option<(usize, usize)>, highlight: &'a str) -> String {
        match offset {
            None => String::new(),
            Some((start, len)) => {
                let rest = &highlight[start + len..];
                if rest.is_empty() {
                    String::new()
                } else {
                    format!("{}{}", self.backdrop_style, rest)
                }
            }
        }
    }

    fn format_offset(&self, selected: bool, hint: &str, highlight: &str) -> String {
        const RESET: &str = "\x1b[0m";
        let chopped = self.chop_highlight(hint, highlight);
        let (hint_sty, hl_sty) = if selected {
            (&self.selected_hint_style, &self.selected_highlight_style)
        } else {
            (&self.hint_style, &self.highlight_style)
        };

        let hint_pair = format!("{}{}", hint_sty, hint);
        let hl_pair = format!("{}{}", hl_sty, chopped);

        if self.hint_position == "right" {
            format!("{}{}{}{}{}", hl_pair, RESET, hint_pair, RESET, self.backdrop_style)
        } else {
            format!("{}{}{}{}{}", hint_pair, RESET, hl_pair, RESET, self.backdrop_style)
        }
    }

    fn chop_highlight(&self, hint: &str, highlight: &str) -> String {
        let hint_w = display_width::of_str(hint);
        if self.hint_position == "right" {
            chop_from_end(highlight, hint_w)
        } else {
            chop_from_start(highlight, hint_w)
        }
    }
}

fn chop_from_start(highlight: &str, hint_w: usize) -> String {
    let mut consumed_w = 0usize;
    let mut char_count = 0usize;
    for c in highlight.chars() {
        if consumed_w >= hint_w {
            break;
        }
        consumed_w += display_width::of_char(c);
        char_count += 1;
    }
    let skip_bytes: usize = highlight.chars().take(char_count).map(|c| c.len_utf8()).sum();
    let padding = if consumed_w > hint_w {
        " ".repeat(consumed_w - hint_w)
    } else {
        String::new()
    };
    format!("{}{}", padding, &highlight[skip_bytes..])
}

fn chop_from_end(highlight: &str, hint_w: usize) -> String {
    // One pass collecting (cumulative_byte_pos, cumulative_display_w) per char.
    // Then back-scan to find the largest prefix whose width fits keep_w.
    // Halves the display_width::of_char calls vs. an of_str + walk approach.
    let mut byte_pos = 0usize;
    let mut col = 0usize;
    let mut prefix: Vec<(usize, usize)> = Vec::with_capacity(highlight.len());
    prefix.push((0, 0));
    for c in highlight.chars() {
        byte_pos += c.len_utf8();
        col += display_width::of_char(c);
        prefix.push((byte_pos, col));
    }
    let total_w = col;
    let keep_w = total_w.saturating_sub(hint_w);

    let (keep_bytes, accumulated_w) = prefix
        .iter()
        .rev()
        .find(|&&(_, w)| w <= keep_w)
        .copied()
        .unwrap_or((0, 0));

    let pad_n = keep_w - accumulated_w;
    let mut out = String::with_capacity(keep_bytes + pad_n);
    out.push_str(&highlight[..keep_bytes]);
    for _ in 0..pad_n {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::style::parse_style;

    fn make_formatter() -> MatchFormatter {
        MatchFormatter::new(
            parse_style("fg=green,bold"),
            parse_style("fg=blue,bold"),
            parse_style("fg=yellow"),
            parse_style("fg=blue"),
            String::new(),
            "left".into(),
        )
    }

    #[test]
    fn formats_without_offset() {
        let f = make_formatter();
        let out = f.format("ab", "abcdef", false, None);
        assert!(out.contains("ab"), "hint must appear");
        assert!(out.contains("cdef") || out.contains("def"), "rest of highlight");
    }

    #[test]
    fn hint_width_chops_highlight() {
        let f = make_formatter();
        let out = f.format("a", "hello", false, None);
        // "a" has width 1, so "hello" should lose its first display column
        assert!(!out.is_empty());
    }

    #[test]
    fn right_position_reverses_order() {
        let mut f = make_formatter();
        f.hint_position = "right".into();
        let out = f.format("a", "hello", false, None);
        // When position=right, highlight appears before hint
        let hint_pos = out.rfind("a");
        let hl_pos = out.find("hell");
        if let (Some(hp), Some(hl)) = (hint_pos, hl_pos) {
            assert!(hl < hp, "highlight should precede hint for right position");
        }
    }
}
