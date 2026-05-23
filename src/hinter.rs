use std::collections::{HashMap, HashSet};
use std::io::Write;
use regex::{Regex, RegexSet};

use crate::hint::huffman;
use crate::display_width;
use crate::match_formatter::MatchFormatter;
use crate::state::State;

const CLEAR_SEQ: &str = "\x1b[H\x1b[J";
const HIDE_CURSOR_SEQ: &str = "\x1b[?25l";

pub struct PaneInput {
    pub lines: Vec<String>,
    pub pane_id: String,
    pub width: usize,
    pub tty_path: String,
}

pub struct Hinter {
    pane_inputs: Vec<PaneInput>,
    /// One compiled Regex per pattern. Keeps named groups scoped per-pattern,
    /// avoiding the duplicate-name error the `regex` crate throws when
    /// multiple patterns each contain `(?P<match>...)` and are joined with `|`.
    compiled: Vec<Regex>,
    /// Combined RegexSet for one-pass per-line membership testing. Lines that
    /// match no patterns are skipped entirely; otherwise only patterns the set
    /// reports as matching are scanned for captures.
    set: RegexSet,
    alphabet: Vec<String>,
    formatter: MatchFormatter,
    reuse_hints: bool,
    backdrop_style: String,
    target_by_hint: HashMap<String, Target>,
    /// Per-pane, per-line, resolved (non-overlapping) match list with hints assigned.
    /// Computed once on the first call to `run`; reused on every subsequent render.
    cache: Option<Vec<Vec<Vec<CachedMatch>>>>,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub text: String,
    pub offset: (usize, usize), // (line_index, col)
    pub source_pane_id: String,
}

/// One match in display order; positions are byte offsets into the source line.
struct CachedMatch {
    start: usize,
    end: usize,
    /// Byte (start, len) of the named-capture group within the full match,
    /// for patterns that use `(?P<match>...)`. None for plain patterns.
    fmt_offset: Option<(usize, usize)>,
    /// None when the assigned hint is wider than the captured text — render raw.
    hint: Option<String>,
}

impl Hinter {
    pub fn new(
        pane_inputs: Vec<PaneInput>,
        patterns: Vec<String>,
        alphabet: Vec<String>,
        formatter: MatchFormatter,
        reuse_hints: bool,
        backdrop_style: String,
        _state: &State,
    ) -> Self {
        // Validate each pattern once; keep RegexSet and Vec<Regex> aligned by index.
        // Smart-case: a pattern with no ASCII uppercase becomes case-insensitive.
        let mut compiled: Vec<Regex> = Vec::with_capacity(patterns.len());
        let mut sources: Vec<String> = Vec::with_capacity(patterns.len());
        for p in patterns {
            let src = apply_smart_case(&p);
            if let Ok(re) = Regex::new(&src) {
                compiled.push(re);
                sources.push(src);
            }
        }
        let set = RegexSet::new(&sources).expect("patterns already validated");
        Hinter {
            pane_inputs,
            compiled,
            set,
            alphabet,
            formatter,
            reuse_hints,
            backdrop_style,
            target_by_hint: HashMap::new(),
            cache: None,
        }
    }

    /// Render hint overlays for all pane inputs. Writes directly to each pane's TTY.
    /// First call runs the full regex scan and hint assignment; subsequent calls
    /// reuse the cached match data (matches do not change during a session).
    pub fn run(&mut self, state: &State) -> std::io::Result<()> {
        if self.cache.is_none() {
            self.precompute();
        }
        let cache = self.cache.as_ref().expect("cache populated by precompute");

        for (pi, input) in self.pane_inputs.iter().enumerate() {
            let mut file = std::fs::OpenOptions::new().write(true).open(&input.tty_path)?;
            write!(file, "{}{}", CLEAR_SEQ, HIDE_CURSOR_SEQ)?;

            if input.lines.is_empty() {
                file.flush()?;
                continue;
            }

            let mut visual_row = 1usize;
            for (li, line) in input.lines.iter().enumerate() {
                let rendered = self.render_line(line, &cache[pi][li], state);
                let pad_w = compute_padding(line, input.width);
                // Anchor each line to its absolute row with a CUP escape before the
                // content. Wide characters that the terminal renders wider than
                // wcwidth() predicts can no longer push subsequent lines down — each
                // CUP overrides any auto-wrap from the previous line.
                //
                // visual_row tracks the actual terminal row, not the logical line
                // index. When capture-pane -J joins wrapped lines, one logical line
                // can occupy several visual rows; using li+1 would place subsequent
                // lines inside the wrapped content of the previous one.
                write!(file, "\x1b[{};1H{}", visual_row, rendered)?;
                if pad_w > 0 {
                    write!(file, "{:1$}", "", pad_w)?;
                }
                let line_w = display_width::of_str_with_tabs(line);
                let rows_taken = if input.width > 0 {
                    ((line_w + input.width - 1) / input.width).max(1)
                } else {
                    1
                };
                visual_row += rows_taken;
            }
            // Park the cursor at home so any deferred-wrap state on the last line
            // is consumed by an escape (not a printable byte) before any future
            // write to this TTY.
            write!(file, "\x1b[1;1H")?;

            file.flush()?;
        }
        Ok(())
    }

    pub fn lookup(&self, hint: &str) -> Option<&Target> {
        self.target_by_hint.get(hint)
    }

    // ---- private -----------------------------------------------------------

    /// One-pass scan: find all matches in every line of every pane, resolve
    /// overlaps, generate hints, assign targets. Populates `self.cache` and
    /// `self.target_by_hint`.
    fn precompute(&mut self) {
        // Phase 1: per-pane, per-line, collect non-overlapping matches.
        struct PreMatch {
            start: usize,
            end: usize,
            captured_text: String,
            captured_offset: Option<(usize, usize)>,
        }

        let mut all: Vec<Vec<Vec<PreMatch>>> = Vec::with_capacity(self.pane_inputs.len());

        for input in &self.pane_inputs {
            let mut per_line: Vec<Vec<PreMatch>> = Vec::with_capacity(input.lines.len());
            for line in &input.lines {
                // One DFA pass tells us which patterns can possibly match this line.
                // Most lines match nothing → we skip the per-pattern capture scan entirely.
                let set_matches = self.set.matches(line);
                if !set_matches.matched_any() {
                    per_line.push(Vec::new());
                    continue;
                }

                let mut raw: Vec<(usize, usize, String, Option<(usize, usize)>)> = Vec::new();
                for idx in set_matches.iter() {
                    let re = &self.compiled[idx];
                    for cap in re.captures_iter(line) {
                        let m = cap.get(0).unwrap();
                        let (text, offset) = extract_capture_info(&cap, m.start());
                        // Trim trailing punctuation only when the user's regex didn't
                        // pin the match shape with a named (?P<match>...) group.
                        let (end, text) = if offset.is_some() {
                            (m.end(), text.to_string())
                        } else {
                            let trimmed = trim_trailing(text);
                            (m.start() + trimmed.len(), trimmed.to_string())
                        };
                        if end <= m.start() { continue; }
                        raw.push((m.start(), end, text, offset));
                    }
                }
                // start ASC, end DESC — longest wins at same start position.
                raw.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));

                let mut resolved = Vec::with_capacity(raw.len());
                let mut last_end = 0usize;
                for (start, end, text, offset) in raw {
                    if start < last_end { continue; }
                    last_end = end;
                    resolved.push(PreMatch { start, end, captured_text: text, captured_offset: offset });
                }
                per_line.push(resolved);
            }
            all.push(per_line);
        }

        // Phase 2: count for hint generation.
        let n = if self.reuse_hints {
            let mut set: HashSet<&str> = HashSet::new();
            for pane in &all {
                for line in pane {
                    for m in line {
                        set.insert(m.captured_text.as_str());
                    }
                }
            }
            set.len()
        } else {
            all.iter().flatten().map(|line| line.len()).sum()
        };

        // Huffman hints, file-cached. Sorted shortest-first; pop yields longest.
        let mut hint_pool = huffman::generate_hints(&self.alphabet, n);

        // Phase 3: assign hints in iteration order, building cache and target table.
        let mut hint_by_text: HashMap<String, String> = HashMap::new();
        let mut cache: Vec<Vec<Vec<CachedMatch>>> = Vec::with_capacity(all.len());

        for (pi, pane_matches) in all.into_iter().enumerate() {
            let pane_id = self.pane_inputs[pi].pane_id.clone();
            let mut pane_cache: Vec<Vec<CachedMatch>> = Vec::with_capacity(pane_matches.len());

            for (li, line_matches) in pane_matches.into_iter().enumerate() {
                let line_str = &self.pane_inputs[pi].lines[li];
                let mut cached: Vec<CachedMatch> = Vec::with_capacity(line_matches.len());

                for m in line_matches {
                    let (was_popped, hint) = if self.reuse_hints {
                        if let Some(h) = hint_by_text.get(&m.captured_text) {
                            (false, h.clone())
                        } else {
                            (true, hint_pool.pop().unwrap_or_else(|| "?".into()))
                        }
                    } else {
                        (true, hint_pool.pop().unwrap_or_else(|| "?".into()))
                    };

                    let cap_w = display_width::of_str(&m.captured_text);
                    let hint_w = display_width::of_str(&hint);

                    if hint_w > cap_w {
                        // Hint too wide — return it to the pool and render raw.
                        if was_popped {
                            hint_pool.push(hint);
                        }
                        cached.push(CachedMatch {
                            start: m.start, end: m.end,
                            fmt_offset: None, hint: None,
                        });
                        continue;
                    }

                    // Convert character-based capture offset to byte offset within full match.
                    let full = &line_str[m.start..m.end];
                    let fmt_offset = m.captured_offset.map(|(start_ch, len_ch)| {
                        let byte_start = char_pos_to_byte(full, start_ch);
                        let byte_len = char_pos_to_byte(&full[byte_start..], len_ch);
                        (byte_start, byte_len)
                    });

                    let abs_offset = (li, m.start + m.captured_offset.map(|(s, _)| s).unwrap_or(0));
                    let target = Target {
                        text: m.captured_text.clone(),
                        offset: abs_offset,
                        source_pane_id: pane_id.clone(),
                    };
                    self.target_by_hint.insert(hint.clone(), target);

                    if self.reuse_hints && was_popped {
                        hint_by_text.insert(m.captured_text, hint.clone());
                    }

                    cached.push(CachedMatch {
                        start: m.start, end: m.end,
                        fmt_offset, hint: Some(hint),
                    });
                }
                pane_cache.push(cached);
            }
            cache.push(pane_cache);
        }

        self.cache = Some(cache);
    }

    /// Render one line by walking the precomputed matches. Pure read of cache;
    /// no regex, no allocation of Targets, no mutation of self.
    fn render_line(&self, line: &str, line_matches: &[CachedMatch], state: &State) -> String {
        let mut result = String::new();
        let mut last_end = 0usize;

        for m in line_matches {
            result.push_str(&line[last_end..m.start]);
            let full = &line[m.start..m.end];

            let replacement = match &m.hint {
                None => full.to_string(),
                Some(hint) => {
                    if !state.input.is_empty() && !hint.starts_with(&state.input) {
                        full.to_string()
                    } else {
                        self.formatter.format(
                            hint,
                            full,
                            state.selected_hints.contains(hint),
                            m.fmt_offset,
                        )
                    }
                }
            };
            result.push_str(&replacement);
            last_end = m.end;
        }
        result.push_str(&line[last_end..]);

        let styled = format!("{}{}", self.backdrop_style, result);
        expand_tabs(&styled)
    }
}

/// Return (captured_text, char-offset (start, len) of named group within full match).
fn extract_capture_info<'h>(
    cap: &regex::Captures<'h>,
    match_abs_start: usize,
) -> (&'h str, Option<(usize, usize)>) {
    let full = cap.get(0).unwrap().as_str();
    if let Some(named) = cap.name("match") {
        let text = named.as_str();
        let start_bytes = named.start() - match_abs_start;
        let start_chars = full[..start_bytes].chars().count();
        let len_chars   = text.chars().count();
        (text, Some((start_chars, len_chars)))
    } else {
        (full, None)
    }
}

fn compute_padding(raw_line: &str, width: usize) -> usize {
    if width == 0 { return 0; }
    let line_w = display_width::of_str_with_tabs(raw_line);
    if line_w >= width {
        // Wrapped line: pad only the last partial row.
        let remainder = line_w % width;
        if remainder == 0 { 0 } else { width - remainder }
    } else {
        width - line_w
    }
}

fn expand_tabs(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut col = 0usize;
    for c in s.chars() {
        if c == '\t' {
            let spaces = 8 - (col % 8);
            out.extend(std::iter::repeat(' ').take(spaces));
            col += spaces;
        } else {
            out.push(c);
            col += display_width::of_char(c);
        }
    }
    out
}

fn char_pos_to_byte(s: &str, char_pos: usize) -> usize {
    s.char_indices().nth(char_pos).map(|(b, _)| b).unwrap_or(s.len())
}

/// Prefix `(?i)` to a pattern that contains no ASCII-uppercase letter, so that
/// `URL`/`Sha`/etc. match alongside their lowercase form. Patterns the user
/// wrote with explicit capitals are left case-sensitive.
fn apply_smart_case(p: &str) -> String {
    if p.bytes().any(|b| b.is_ascii_uppercase()) {
        p.to_string()
    } else {
        format!("(?i){}", p)
    }
}

/// Strip trailing sentence punctuation and unbalanced closing brackets from a
/// match. Operates byte-wise on ASCII; non-ASCII bytes (UTF-8 continuations,
/// CJK, etc.) never match a trim target so multi-byte sequences are preserved.
fn trim_trailing(text: &str) -> &str {
    let mut s = text;
    loop {
        let bytes = s.as_bytes();
        let Some(&last) = bytes.last() else { break };
        let opener = match last {
            b'.' | b',' | b';' | b':' | b'?' | b'!' => {
                s = &s[..s.len() - 1];
                continue;
            }
            b')' => b'(',
            b']' => b'[',
            b'}' => b'{',
            b'>' => b'<',
            _ => break,
        };
        let head = &bytes[..bytes.len() - 1];
        let opens = head.iter().filter(|&&b| b == opener).count();
        let closes = head.iter().filter(|&&b| b == last).count();
        if closes >= opens {
            s = &s[..s.len() - 1];
        } else {
            break;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_case_lowers_only_pattern() {
        assert_eq!(apply_smart_case("foo"), "(?i)foo");
        assert_eq!(apply_smart_case(r"[0-9a-f]+"), "(?i)[0-9a-f]+");
    }

    #[test]
    fn smart_case_preserves_pattern_with_uppercase() {
        assert_eq!(apply_smart_case(r"[a-fA-F]+"), r"[a-fA-F]+");
        assert_eq!(apply_smart_case("Foo"), "Foo");
    }

    #[test]
    fn smart_case_compiles_and_matches() {
        let re = Regex::new(&apply_smart_case("modified")).unwrap();
        assert!(re.is_match("Modified: foo.rs"));
        assert!(re.is_match("modified"));
    }

    #[test]
    fn trim_strips_trailing_sentence_punct() {
        assert_eq!(trim_trailing("https://x.com/a."), "https://x.com/a");
        assert_eq!(trim_trailing("foo,"), "foo");
        assert_eq!(trim_trailing("end!?."), "end");
    }

    #[test]
    fn trim_strips_unbalanced_closer() {
        assert_eq!(trim_trailing("https://x.com/a)"), "https://x.com/a");
        assert_eq!(trim_trailing("path/foo]"), "path/foo");
    }

    #[test]
    fn trim_keeps_balanced_closer() {
        assert_eq!(trim_trailing("foo(bar)"), "foo(bar)");
        assert_eq!(trim_trailing("[a]"), "[a]");
    }

    #[test]
    fn trim_keeps_clean_text() {
        assert_eq!(trim_trailing("https://x.com/a"), "https://x.com/a");
        assert_eq!(trim_trailing(""), "");
    }

    #[test]
    fn compute_padding_vs16_emoji() {
        // "⚠️" = U+26A0 (neutral, 1 col) + U+FE0F (VS-16 upgrade → 2 cols).
        // A line "ab⚠️" has actual display width 4; padding to width 6 should be 2.
        assert_eq!(compute_padding("ab⚠️", 6), 2);
    }

    #[test]
    fn compute_padding_tab_uses_column_relative_stops() {
        // "abc\tdef": tab at col 3 expands to 5 spaces (next stop col 8), then
        // "def" → col 11. Padding to width 15 = 4. The previous fixed-8 model
        // would have over-counted the line as col 14, returning 1.
        assert_eq!(compute_padding("abc\tdef", 15), 4);
    }

    #[test]
    fn compute_padding_wrapped_line_pads_last_row() {
        // A 200-char line in an 80-col pane fills rows 1-2 fully and puts 40 chars
        // in row 3. Padding should be 40 (= 80 - 40) to fill the last partial row.
        let long_line: String = "a".repeat(200);
        assert_eq!(compute_padding(&long_line, 80), 40);
    }

    #[test]
    fn compute_padding_exactly_full_rows_needs_no_pad() {
        // 160 chars in an 80-col pane fills exactly 2 rows; no padding needed.
        let full_line: String = "a".repeat(160);
        assert_eq!(compute_padding(&full_line, 80), 0);
    }

    #[test]
    fn trim_preserves_non_ascii_tail() {
        // A trailing wide character must not be split by byte-wise trim.
        let s = "https://x.com/日本語";
        assert_eq!(trim_trailing(s), s);
    }
}
