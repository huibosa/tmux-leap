use crate::action_runner::ActionRunner;
use crate::cli::StartArgs;
use crate::config::Config;
use crate::hinter::{Hinter, PaneInput, Target};
use crate::input_socket::InputSocket;
use crate::match_formatter::MatchFormatter;
use crate::state::State;
use crate::tmux::pane::{self, Pane, Window};
use crate::tmux;
use crate::view::View;
use std::collections::HashMap;

pub fn run(args: StartArgs) {
    let mut config = Config::load();
    // Bootstrap: if load-config has never run, alphabet and patterns will be empty.
    if config.alphabet.is_empty() {
        config.alphabet = Config::alphabet_for_layout(&config.keyboard_layout);
    }
    if config.patterns.is_empty() {
        for (name, pat) in crate::config::BUILTIN_PATTERNS {
            config.patterns.insert(name.to_string(), pat.to_string());
        }
    }
    let config = config;

    let (active_pane, target_pane, window_panes) = resolve_pane(&args.pane_id);

    let patterns = if args.words {
        vec![r"\S+".to_string()]
    } else if let Some(p) = &args.patterns {
        patterns_from_option(p, &config)
    } else {
        config.patterns.values().cloned().collect()
    };

    // Capture tmux state before we take over key bindings
    let saved = capture_tmux_state();

    // Build and display the hints window
    let mut session = HintsSession::build(
        &target_pane,
        &active_pane,
        window_panes,
        &config,
        patterns,
        &args.mode,
    );

    session.render_all();

    if args.mode == "benchmark" {
        teardown(session, &saved, &active_pane, &args.mode, None);
        return;
    }

    // Enter leap key table and accept input
    let socket = InputSocket::new().expect("failed to create input socket");
    tmux::exec_batch(&[
        &["set-option", "-g", "prefix", "None"],
        &["set-option", "-g", "prefix2", "None"],
        &["set-window-option", "key-table", "leap"],
        &["switch-client", "-T", "leap"],
    ]);

    let matched = session.input_loop(&socket, &config);

    socket.close();

    // Run the action if a match was made
    if let Some((target, result)) = &matched {
        let src_pane = pane::find_pane(&target.source_pane_id)
            .unwrap_or_else(|| active_pane.clone());
        ActionRunner {
            modifier: &session.state.modifier,
            match_text: result,
            hint: &session.state.input,
            active_pane: &active_pane,
            source_pane: &src_pane,
            offset: Some(target.offset),
            mode: &args.mode,
            main_action: args.main_action.as_deref(),
            ctrl_action: args.ctrl_action.as_deref(),
            alt_action: args.alt_action.as_deref(),
            shift_action: args.shift_action.as_deref(),
            config: &config,
        }
        .run();
    }

    teardown(session, &saved, &active_pane, &args.mode, matched.as_ref().map(|(t, _)| t));
}

struct SavedState {
    last_pane_id: String,
    last_key_table: String,
    prefix: String,
    prefix2: String,
}

fn capture_tmux_state() -> SavedState {
    let out = tmux::exec(&[
        "display-message", "-t", "{last}", "-p",
        "#{pane_id};#{client_key_table};#{prefix};#{prefix2}",
    ]);
    let parts: Vec<&str> = out.splitn(4, ';').collect();
    SavedState {
        last_pane_id: parts.get(0).unwrap_or(&"").to_string(),
        last_key_table: {
            let t = parts.get(1).unwrap_or(&"");
            if t.is_empty() { "root".to_string() } else { t.to_string() }
        },
        prefix: parts.get(2).unwrap_or(&"").to_string(),
        prefix2: parts.get(3).unwrap_or(&"").to_string(),
    }
}

/// Returns (active_pane, target_pane, all_panes_in_target_window).
/// The pane list is fetched in one fork (`list-panes -t <pane_id>` resolves to
/// the containing window) and reused by HintsSession::build to avoid a second
/// list-panes call.
fn resolve_pane(pane_target: &str) -> (Pane, Pane, Vec<Pane>) {
    if pane_target.starts_with('%') {
        let panes = pane::list_panes_in_window(pane_target);
        let active = panes
            .iter()
            .find(|p| p.pane_id == pane_target)
            .cloned()
            .unwrap_or_else(|| panic!("pane not found: {pane_target}"));
        (active.clone(), active, panes)
    } else {
        let pane_id = tmux::exec(&["display-message", "-t", pane_target, "-p", "#{pane_id}"]);
        let panes = pane::list_panes_in_window(&pane_id);
        let target = panes
            .iter()
            .find(|p| p.pane_id == pane_id)
            .cloned()
            .unwrap_or_else(|| panic!("pane not found: {pane_id}"));
        let active = panes.first().cloned().unwrap_or_else(|| target.clone());
        (active, target, panes)
    }
}

fn patterns_from_option(option: &str, config: &Config) -> Vec<String> {
    option
        .split(',')
        .filter_map(|name| {
            if let Some(p) = config.patterns.get(name.trim()) {
                Some(p.clone())
            } else {
                eprintln!("[tmux-leap] unknown pattern: {name}");
                None
            }
        })
        .collect()
}

// ---- HintsSession holds the leap window and per-pane state ----------------

struct HintsSession {
    leap_window: Window,
    pane_pairs: Vec<(Pane, Pane)>, // (source, leap)
    hinter: Hinter,
    pub state: State,
    mode: String,
    zoomed: bool,
}

impl HintsSession {
    fn build(
        target_pane: &Pane,
        active_pane: &Pane,
        window_panes: Vec<Pane>,
        config: &Config,
        patterns: Vec<String>,
        mode: &str,
    ) -> HintsSession {
        let zoomed = target_pane.window_zoomed_flag;

        // ADR 0001/0002: If zoomed, only the active pane; otherwise all panes in window.
        let source_panes: Vec<Pane> = if zoomed {
            vec![active_pane.clone()]
        } else {
            window_panes
        };

        // Create leap window with one pane
        let fw = pane::create_window("[leap]", "cat");

        // Split to match source pane count (already have 1)
        for _ in 1..source_panes.len() {
            pane::split_window(&fw.window_id);
        }

        // Resize leap window to match source window dimensions
        if !source_panes.is_empty() {
            let source_w = source_panes.iter().map(|p| p.pane_left + p.pane_width).max().unwrap_or(80);
            let source_h = source_panes.iter().map(|p| p.pane_top + p.pane_height).max().unwrap_or(24);
            pane::resize_window(&fw.window_id, source_w, source_h);
        }

        // Mirror layout for multi-pane (ADR 0004)
        if source_panes.len() > 1 {
            let layout = tmux::exec(&["display-message", "-t", &target_pane.window_id, "-p", "#{window_layout}"]);
            pane::select_layout(&fw.window_id, &layout);
        }

        // Pair source ↔ leap panes by (top, left)
        let leap_panes = pane::list_panes_in_window(&fw.window_id);
        let pairs = pair_by_position(&source_panes, &leap_panes);

        // Repair any rounding mismatches. resize_pane does not change pane_id or
        // pane_tty, so the pairs are still valid afterward — no re-list needed.
        let resizes: Vec<(&str, u32, u32)> = pairs
            .iter()
            .filter(|(src, fng)| src.pane_width != fng.pane_width || src.pane_height != fng.pane_height)
            .map(|(src, fng)| (fng.pane_id.as_str(), src.pane_width, src.pane_height))
            .collect();
        pane::resize_pane_batch(&resizes);

        // Build PaneInputs
        // Jump offsets are copy-mode grid coordinates. tmux copy-mode
        // cursor-down moves by physical rows, including soft-wrap
        // continuations, so jump mode must keep capture-pane's default
        // unjoined output. Other modes join wrapped lines for nicer matching.
        let join = mode != "jump";
        let pane_inputs: Vec<PaneInput> = pairs
            .iter()
            .map(|(src, fng)| {
                let content = pane::capture_pane(src, join);
                let lines: Vec<String> = content.lines().map(str::to_string).collect();
                PaneInput {
                    lines,
                    pane_id: src.pane_id.clone(),
                    width: src.pane_width as usize,
                    tty_path: fng.pane_tty.clone(),
                }
            })
            .collect();

        let formatter = MatchFormatter::new(
            config.hint_style.clone(),
            config.selected_hint_style.clone(),
            config.highlight_style.clone(),
            config.selected_highlight_style.clone(),
            config.backdrop_style.clone(),
            config.hint_position.clone(),
        );

        let hinter = Hinter::new(
            pane_inputs,
            patterns,
            config.alphabet.clone(),
            formatter,
            mode != "jump",
            config.backdrop_style.clone(),
            &State::default(),
        );

        HintsSession {
            leap_window: fw,
            pane_pairs: pairs,
            hinter,
            state: State::default(),
            mode: mode.to_string(),
            zoomed,
        }
    }

    fn render_all(&mut self) {
        let swaps: Vec<(&str, &str)> = self.pane_pairs
            .iter()
            .map(|(src, fng)| (fng.pane_id.as_str(), src.pane_id.as_str()))
            .collect();
        // ADR 0002/0004: zoomed → swap first, then render
        if self.zoomed {
            pane::swap_panes_batch(&swaps, true);
            let _ = self.hinter.run(&self.state);
        } else {
            let _ = self.hinter.run(&self.state);
            pane::swap_panes_batch(&swaps, false);
        }
    }

    fn input_loop(
        &mut self,
        socket: &InputSocket,
        _config: &Config,
    ) -> Option<(Target, String)> {
        loop {
            match socket.recv() {
                Ok(input) => {
                    let mut view = View::new(&mut self.hinter, &mut self.state, self.mode.clone());
                    view.process_input(&input);
                    if self.state.exiting {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("tmux-leap: socket error: {e}");
                    break;
                }
            }
        }
        if !self.state.result.is_empty() {
            let target = self.state.matched_target.clone()?;
            Some((target, self.state.result.clone()))
        } else {
            None
        }
    }
}

fn teardown(
    session: HintsSession,
    saved: &SavedState,
    active_pane: &Pane,
    mode: &str,
    matched_target: Option<&Target>,
) {
    let zoomed = session.zoomed;

    // Swap each pair back (reverse order).
    // Pass zoomed so swap-pane -Z preserves the zoom state (ADR 0002).
    let swaps: Vec<(&str, &str)> = session.pane_pairs
        .iter()
        .rev()
        .map(|(src, fng)| (fng.pane_id.as_str(), src.pane_id.as_str()))
        .collect();
    pane::swap_panes_batch(&swaps, zoomed);

    pane::kill_window(&session.leap_window.window_id);

    // Restore pane focus (ADR 0003 jump mode vs default).
    // select-pane -Z preserves zoom state on each focus change.
    // When zoomed we skip the last_pane selection: zoom mode has only one visible
    // pane, so cycling through last_pane would briefly render the unzoomed layout.
    if mode == "jump" {
        if let Some(target) = matched_target {
            let src = pane::find_pane(&target.source_pane_id)
                .unwrap_or_else(|| active_pane.clone());
            pane::select_pane(&active_pane.pane_id, zoomed);
            pane::select_pane(&src.pane_id, zoomed);
        } else {
            if !zoomed {
                pane::select_pane(&saved.last_pane_id, false);
            }
            pane::select_pane(&active_pane.pane_id, zoomed);
        }
    } else {
        if !zoomed {
            pane::select_pane(&saved.last_pane_id, false);
        }
        pane::select_pane(&active_pane.pane_id, zoomed);
    }

    // Restore key table and prefixes
    tmux::exec_batch(&[
        &["set-window-option", "key-table", &saved.last_key_table],
        &["switch-client", "-T", &saved.last_key_table],
        &["set-option", "-g", "prefix", &saved.prefix],
        &["set-option", "-g", "prefix2", &saved.prefix2],
    ]);
}

fn pair_by_position(source: &[Pane], leap: &[Pane]) -> Vec<(Pane, Pane)> {
    let leap_by_pos: HashMap<(u32, u32), &Pane> = leap
        .iter()
        .map(|p| ((p.pane_top, p.pane_left), p))
        .collect();

    source
        .iter()
        .filter_map(|src| {
            leap_by_pos
                .get(&(src.pane_top, src.pane_left))
                .map(|lp| (src.clone(), (*lp).clone()))
        })
        .collect()
}
