/// Parse a tmux-style string like "fg=green,bold" and return ANSI SGR sequences.
///
/// Supported colors: black(0) red(1) green(2) yellow(3) blue(4) magenta(5) cyan(6) white(7)
/// and colour<N> for 256-color.
/// Supported attributes: bold, dim, underscore, reverse, italics, bright (alias for bold).
pub fn parse_style(input: &str) -> String {
    let mut out = String::new();
    for token in input.split(|c| c == ' ' || c == ',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some(rest) = token.strip_prefix("fg=") {
            out.push_str(&color_sgr(rest, false));
        } else if let Some(rest) = token.strip_prefix("bg=") {
            out.push_str(&color_sgr(rest, true));
        } else {
            out.push_str(&attr_sgr(token));
        }
    }
    out
}

fn color_sgr(color: &str, bg: bool) -> String {
    let base = if bg { 40 } else { 30 };
    if color == "default" {
        return format!("\x1b[{}m", base + 9);
    }
    if let Some(n) = color.strip_prefix("colour").or_else(|| color.strip_prefix("color")) {
        if let Ok(n) = n.parse::<u8>() {
            let layer = if bg { 48 } else { 38 };
            return format!("\x1b[{};5;{}m", layer, n);
        }
    }
    let named: &[(&str, u8)] = &[
        ("black", 0), ("red", 1), ("green", 2), ("yellow", 3),
        ("blue", 4), ("magenta", 5), ("cyan", 6), ("white", 7),
    ];
    for (name, code) in named {
        if color == *name {
            return format!("\x1b[{}m", base + code);
        }
    }
    String::new()
}

fn attr_sgr(attr: &str) -> String {
    match attr {
        "bold" | "bright" => "\x1b[1m".into(),
        "dim"             => "\x1b[2m".into(),
        "underscore"      => "\x1b[4m".into(),
        "reverse"         => "\x1b[7m".into(),
        "italics"         => "\x1b[3m".into(),
        _                 => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_green() {
        let s = parse_style("fg=green,bold");
        assert!(s.contains("\x1b[32m"), "expected fg green SGR");
        assert!(s.contains("\x1b[1m"), "expected bold SGR");
    }

    #[test]
    fn fg_yellow() {
        assert_eq!(parse_style("fg=yellow"), "\x1b[33m");
    }

    #[test]
    fn bg_blue() {
        assert_eq!(parse_style("bg=blue"), "\x1b[44m");
    }

    #[test]
    fn colour256() {
        assert_eq!(parse_style("fg=colour200"), "\x1b[38;5;200m");
    }

    #[test]
    fn empty() {
        assert_eq!(parse_style(""), "");
    }
}
