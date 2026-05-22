/// Display column count per UAX #11 East Asian Width.
/// Wide (CJK, fullwidth, emoji) → 2; control/combining → 0; else → 1.
/// Delegates to the `unicode-width` crate, with special handling for VS-16 upgrade.
use unicode_width::UnicodeWidthChar;

pub fn of_char(c: char) -> usize {
    let cp = c as u32;
    if cp == 0x09 { return 1; } // tab: 1 column for raw line; tab expansion adds the rest
    if cp < 0x20 { return 0; }
    if cp >= 0x7F && cp < 0xA0 { return 0; }
    c.width().unwrap_or(1)
}

/// Display width of a string, with VS-16 (U+FE0F) upgrading the previous character from 1→2.
pub fn of_str(s: &str) -> usize {
    of_str_inner(s, false)
}

/// Like `of_str`, but tabs expand to the next 8-column stop relative to the running
/// column count (matches a terminal's tab handling on a freshly-positioned line).
/// Use this when predicting padding for a line that will be tab-expanded on output.
pub fn of_str_with_tabs(s: &str) -> usize {
    of_str_inner(s, true)
}

fn of_str_inner(s: &str, tab_aligned: bool) -> usize {
    let mut total = 0usize;
    let mut prev_w = usize::MAX;
    for c in s.chars() {
        if c as u32 == 0xFE0F && prev_w == 1 {
            total += 1;
            prev_w = 2;
            continue;
        }
        let w = if tab_aligned && c == '\t' {
            8 - (total % 8)
        } else {
            of_char(c)
        };
        total += w;
        prev_w = w;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width() {
        assert_eq!(of_str("hello"), 5);
    }

    #[test]
    fn cjk_is_wide() {
        assert_eq!(of_char('中'), 2);
        assert_eq!(of_str("你好"), 4);
    }

    #[test]
    fn tab_is_one() {
        assert_eq!(of_char('\t'), 1);
    }

    #[test]
    fn control_is_zero() {
        assert_eq!(of_char('\x01'), 0);
    }

    #[test]
    fn mixed() {
        assert_eq!(of_str("ab中cd"), 2 + 2 + 2);
    }

    #[test]
    fn vs16_upgrades_preceding_one_col_char() {
        assert_eq!(of_str("⚠\u{FE0F}"), 2);
    }

    #[test]
    fn of_str_with_tabs_uses_8_col_stops() {
        // tab at col 3 -> 5 spaces to reach col 8, then "def" -> col 11.
        assert_eq!(of_str_with_tabs("abc\tdef"), 11);
        // tab at col 0 -> 8 spaces to reach col 8.
        assert_eq!(of_str_with_tabs("\t"), 8);
        // tab at col 8 -> 8 spaces to reach col 16.
        assert_eq!(of_str_with_tabs("12345678\t"), 16);
    }

    #[test]
    fn of_str_with_tabs_handles_vs16() {
        assert_eq!(of_str_with_tabs("⚠\u{FE0F}\t"), 8);
    }
}
