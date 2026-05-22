use std::collections::{HashMap, HashSet};
use std::io::Write;
use regex::Regex;

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
    alphabet: Vec<String>,
    formatter: MatchFormatter,
    reuse_hints: bool,
    backdrop_style: String,
    target_by_hint: HashMap<String, Target>,
    target_by_text: HashMap<String, Target>,
    hints: Option<Vec<String>>,
    current_pane_id: String,
    current_width: usize,
    current_tty: Option<std::fs::File>,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub text: String,
    pub hint: String,
    pub offset: (usize, usize), // (line_index, col)
    pub source_pane_id: String,
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
        let compiled: Vec<Regex> = patterns
            .into_iter()
            .filter_map(|p| Regex::new(&p).ok())
            .collect();
        Hinter {
            pane_inputs,
            compiled,
            alphabet,
            formatter,
            reuse_hints,
            backdrop_style,
            target_by_hint: HashMap::new(),
            target_by_text: HashMap::new(),
            hints: None,
            current_pane_id: String::new(),
            current_width: 0,
            current_tty: None,
        }
    }

    /// Render hint overlays for all pane inputs. Writes directly to each pane's TTY.
    pub fn run(&mut self, state: &State) -> std::io::Result<()> {
        self.regenerate_hints();

        // Move pane_inputs out so we can iterate by reference while still calling
        // &mut self methods (process_line, compute_padding). Restored on exit.
        let pane_inputs = std::mem::take(&mut self.pane_inputs);

        for input in &pane_inputs {
            let mut file = std::fs::OpenOptions::new().write(true).open(&input.tty_path)?;
            write!(file, "{}{}", CLEAR_SEQ, HIDE_CURSOR_SEQ)?;

            if input.lines.is_empty() {
                file.flush()?;
                continue;
            }

            self.current_pane_id = input.pane_id.clone();
            self.current_width   = input.width;
            self.current_tty     = Some(file);

            let last = input.lines.len() - 1;
            for (idx, line) in input.lines.iter().enumerate() {
                let ending  = if idx < last { "\n" } else { "" };
                let rendered = self.process_line(line, idx, state);
                let pad_w = self.compute_padding(line, input.width);
                let padding = if pad_w > 0 { " ".repeat(pad_w) } else { String::new() };
                if let Some(ref mut tty) = self.current_tty {
                    write!(tty, "{}{}{}", rendered, padding, ending)?;
                }
            }

            if let Some(ref mut tty) = self.current_tty { tty.flush()?; }
            self.current_tty = None;
        }

        self.pane_inputs = pane_inputs;
        Ok(())
    }

    pub fn lookup(&self, hint: &str) -> Option<&Target> {
        self.target_by_hint.get(hint)
    }

    // ---- private -----------------------------------------------------------

    fn regenerate_hints(&mut self) {
        let n = self.count_matches();
        self.hints = Some(huffman::generate_hints(&self.alphabet, n));
        self.target_by_hint.clear();
        self.target_by_text.clear();
    }

    fn count_matches(&self) -> usize {
        if self.reuse_hints { self.count_unique() } else { self.count_all() }
    }

    fn count_all(&self) -> usize {
        let mut total = 0;
        for input in &self.pane_inputs {
            for line in &input.lines {
                for re in &self.compiled {
                    total += re.find_iter(line).count();
                }
            }
        }
        total
    }

    fn count_unique(&self) -> usize {
        let mut set: HashSet<String> = HashSet::new();
        for input in &self.pane_inputs {
            for line in &input.lines {
                for re in &self.compiled {
                    for cap in re.captures_iter(line) {
                        set.insert(primary_text(&cap).to_string());
                    }
                }
            }
        }
        set.len()
    }

    fn process_line(&mut self, line: &str, line_index: usize, state: &State) -> String {
        // Phase 1: collect raw match info from every pattern (no mutation of self).
        // Borrow slices into `line` rather than allocating per match.
        struct RawMatch<'a> {
            start: usize,
            end:   usize,
            text:  &'a str,                 // captured group or full match
            offset: Option<(usize, usize)>, // char-offset of named group within `full`
        }

        let mut raw: Vec<RawMatch> = Vec::new();
        for re in &self.compiled {
            for cap in re.captures_iter(line) {
                let m = cap.get(0).unwrap();
                let (text, offset) = extract_capture_info(&cap, m.start());
                raw.push(RawMatch {
                    start: m.start(),
                    end:   m.end(),
                    text,
                    offset,
                });
            }
        }

        // Sort by start position; for equal start, prefer the LONGEST match so a full
        // UUID/URL wins over a sha/digit prefix that begins at the same byte.
        raw.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| b.end.cmp(&a.end)));

        // Phase 2: build rendered line, processing non-overlapping matches.
        let mut result = String::new();
        let mut last_end = 0usize;

        for m in raw {
            if m.start < last_end { continue; } // overlapping — skip
            result.push_str(&line[last_end..m.start]);
            let full = &line[m.start..m.end];
            let replacement = self.format_match(
                full, m.text, m.offset, line_index, m.start, state,
            );
            result.push_str(&replacement);
            last_end = m.end;
        }
        result.push_str(&line[last_end..]);

        let styled = format!("{}{}", self.backdrop_style, result);
        expand_tabs(&styled)
    }

    fn format_match(
        &mut self,
        full_text: &str,
        captured_text: &str,
        relative_offset: Option<(usize, usize)>,
        line_index: usize,
        col_offset: usize,
        state: &State,
    ) -> String {
        let absolute_offset = (
            line_index,
            col_offset + relative_offset.map(|(s, _)| s).unwrap_or(0),
        );

        let hint = self.hint_for(captured_text, state);

        // Hint wider than the match can't be inlined — put it back and return raw.
        if display_width::of_str(&hint) > display_width::of_str(captured_text) {
            if let Some(ref mut h) = self.hints { h.push(hint); }
            return full_text.to_string();
        }

        let target = Target {
            text: captured_text.to_string(),
            hint: hint.clone(),
            offset: absolute_offset,
            source_pane_id: self.current_pane_id.clone(),
        };
        self.target_by_hint.insert(hint.clone(), target.clone());
        self.target_by_text.insert(captured_text.to_string(), target);

        if !state.input.is_empty() && !hint.starts_with(&state.input) {
            return full_text.to_string();
        }

        let fmt_offset = relative_offset.map(|(start_ch, len_ch)| {
            let byte_start = char_pos_to_byte(full_text, start_ch);
            let byte_len   = char_pos_to_byte(&full_text[byte_start..], len_ch);
            (byte_start, byte_len)
        });

        self.formatter.format(
            &hint,
            full_text,
            state.selected_hints.contains(&hint),
            fmt_offset,
        )
    }

    fn hint_for(&mut self, text: &str, _state: &State) -> String {
        if self.reuse_hints {
            if let Some(t) = self.target_by_text.get(text) {
                return t.hint.clone();
            }
        }
        self.hints
            .as_mut()
            .and_then(|h| h.pop())
            .unwrap_or_else(|| "?".to_string())
    }

    fn compute_padding(&self, raw_line: &str, width: usize) -> usize {
        let display_w = display_width::of_str(raw_line);
        let tab_extra: usize = raw_line.chars().filter(|&c| c == '\t').count() * 7;
        width.saturating_sub(display_w + tab_extra)
    }
}

/// Return the text of the named `match` capture group if present, else the whole match.
fn primary_text<'a>(cap: &regex::Captures<'a>) -> &'a str {
    if let Some(m) = cap.name("match") { m.as_str() } else { cap.get(0).unwrap().as_str() }
}

/// Extract (captured_text, char_offset_within_full_match).
/// `match_abs_start` is the byte offset of the full match in the original string,
/// needed to compute the relative offset of the named group.
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
