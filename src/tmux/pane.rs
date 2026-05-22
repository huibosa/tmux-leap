#![allow(dead_code)]
use super::exec;

/// tmux -F format string for list-panes / display-message.
const PANE_FMT: &str = r##"{"pane_id":"#{pane_id}","window_id":"#{window_id}","pane_width":#{pane_width},"pane_height":#{pane_height},"pane_left":#{pane_left},"pane_top":#{pane_top},"pane_current_path":"#{pane_current_path}","pane_in_mode":#{?pane_in_mode,true,false},"scroll_position":#{?scroll_position,#{scroll_position},null},"window_zoomed_flag":#{?window_zoomed_flag,true,false},"pane_tty":"#{pane_tty}"}"##;

const WINDOW_FMT: &str = r##"{"window_id":"#{window_id}","window_width":#{window_width},"window_height":#{window_height},"pane_id":"#{pane_id}","pane_tty":"#{pane_tty}"}"##;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Pane {
    pub pane_id: String,
    pub window_id: String,
    pub pane_width: u32,
    pub pane_height: u32,
    pub pane_left: u32,
    pub pane_top: u32,
    pub pane_current_path: String,
    pub pane_in_mode: bool,
    pub scroll_position: Option<i32>,
    pub window_zoomed_flag: bool,
    pub pane_tty: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Window {
    pub window_id: String,
    pub window_width: u32,
    pub window_height: u32,
    pub pane_id: String,
    pub pane_tty: String,
}

pub fn list_panes(target: Option<&str>, filter: Option<&str>) -> Vec<Pane> {
    let mut args = vec!["list-panes", "-F", PANE_FMT];
    if let Some(t) = target {
        args.extend_from_slice(&["-t", t]);
    } else {
        args.push("-a");
    }
    if let Some(f) = filter {
        args.extend_from_slice(&["-f", f]);
    }
    let out = exec(&args);
    out.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("parse pane: {e}\n{l}")))
        .collect()
}

pub fn list_panes_in_window(window_id: &str) -> Vec<Pane> {
    let args = vec!["list-panes", "-F", PANE_FMT, "-t", window_id];
    let out = exec(&args);
    out.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("parse pane: {e}\n{l}")))
        .collect()
}

pub fn find_pane(id: &str) -> Option<Pane> {
    let out = exec(&["display-message", "-t", id, "-F", PANE_FMT, "-p"]);
    if out.is_empty() {
        None
    } else {
        serde_json::from_str(&out).ok()
    }
}

pub fn find_active_pane_in_window(window_id: &str) -> Option<Pane> {
    let out = exec(&["list-panes", "-F", PANE_FMT, "-t", window_id, "-f", "#{pane_active}"]);
    out.lines()
        .find(|l| !l.is_empty())
        .and_then(|l| serde_json::from_str(l).ok())
}

pub fn capture_pane(pane: &Pane, join: bool) -> String {
    let pane_id = &pane.pane_id;
    if pane.pane_in_mode {
        if let Some(scroll) = pane.scroll_position {
            let start = (-scroll).to_string();
            let end = (pane.pane_height as i32 - scroll - 1).to_string();
            let mut args = vec!["capture-pane", "-p", "-t", pane_id, "-S", &start, "-E", &end];
            if join { args.insert(2, "-J"); }
            return exec(&args);
        }
    }
    let mut args = vec!["capture-pane", "-p", "-t", pane_id];
    if join { args.insert(2, "-J"); }
    exec(&args)
}

pub fn create_window(name: &str, cmd: &str) -> Window {
    let fmt = WINDOW_FMT;
    let out = exec(&[
        "new-window",
        "-c", "#{pane_current_path}",
        "-P", "-d",
        "-n", name,
        "-F", fmt,
        cmd,
    ]);
    serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("parse window: {e}\n{out}"))
}

pub fn split_window(window_id: &str) -> Pane {
    let out = exec(&["split-window", "-t", window_id, "-d", "-P", "-F", PANE_FMT, "cat"]);
    serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("parse pane: {e}\n{out}"))
}

pub fn swap_panes(src: &str, dst: &str, zoomed: bool) {
    if zoomed {
        exec(&["swap-pane", "-d", "-Z", "-s", src, "-t", dst]);
    } else {
        exec(&["swap-pane", "-d", "-s", src, "-t", dst]);
    }
}

/// Run several swap-pane operations in a single tmux invocation.
pub fn swap_panes_batch(swaps: &[(&str, &str)], zoomed: bool) {
    match swaps.len() {
        0 => return,
        1 => return swap_panes(swaps[0].0, swaps[0].1, zoomed),
        _ => {}
    }
    let mut args: Vec<&str> = Vec::with_capacity(swaps.len() * 8);
    for (i, (src, dst)) in swaps.iter().enumerate() {
        if i > 0 { args.push(";"); }
        args.push("swap-pane");
        args.push("-d");
        if zoomed { args.push("-Z"); }
        args.push("-s");
        args.push(src);
        args.push("-t");
        args.push(dst);
    }
    exec(&args);
}

pub fn kill_pane(id: &str) {
    exec(&["kill-pane", "-t", id]);
}

pub fn kill_window(id: &str) {
    exec(&["kill-window", "-t", id]);
}

pub fn resize_window(window_id: &str, w: u32, h: u32) {
    let ws = w.to_string();
    let hs = h.to_string();
    exec(&["resize-window", "-t", window_id, "-x", &ws, "-y", &hs]);
}

pub fn window_layout(window_id: &str) -> String {
    exec(&["display-message", "-t", window_id, "-p", "#{window_layout}"])
}

pub fn select_layout(window_id: &str, layout: &str) {
    exec(&["select-layout", "-t", window_id, layout]);
}

pub fn resize_pane(pane_id: &str, w: u32, h: u32) {
    let ws = w.to_string();
    let hs = h.to_string();
    exec(&["resize-pane", "-t", pane_id, "-x", &ws, "-y", &hs]);
}

pub fn select_pane(id: &str, zoomed: bool) {
    if zoomed {
        exec(&["select-pane", "-Z", "-t", id]);
    } else {
        exec(&["select-pane", "-t", id]);
    }
}

pub fn zoom_pane(id: &str) {
    exec(&["resize-pane", "-Z", "-t", id]);
}

pub fn set_buffer(value: &str) {
    super::exec_stdin(&["load-buffer", "-w", "-"], value.as_bytes());
}

pub fn set_buffer_no_clipboard(value: &str) {
    super::exec_stdin(&["load-buffer", "-"], value.as_bytes());
}

pub fn set_global_option(name: &str, value: &str) {
    exec(&["set-option", "-g", name, value]);
}

pub fn get_global_option(name: &str) -> String {
    exec(&["show", "-gqv", name])
}

pub fn set_key_table(table: &str) {
    exec(&["set-window-option", "key-table", table]);
    exec(&["switch-client", "-T", table]);
}

pub fn disable_prefix() {
    set_global_option("prefix", "None");
    set_global_option("prefix2", "None");
}

pub fn display_message(msg: &str, delay_ms: u32) {
    let d = delay_ms.to_string();
    exec(&["display-message", "-d", &d, msg]);
}

pub fn tmux_version() -> String {
    let out = exec(&["-V"]);
    out.split_whitespace().last().unwrap_or("3.1").to_string()
}
