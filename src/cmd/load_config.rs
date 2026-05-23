use crate::config::{Config, BUILTIN_PATTERNS};
use crate::tmux::pane;
use crate::tmux::style::parse_style;

pub fn run() {
    let mut config = Config::default();

    // Read tmux version
    config.tmux_version = pane::tmux_version();

    // Collect all @leap-* options
    let raw = crate::tmux::exec(&["show-options", "-g"]);
    let leap_opts: Vec<(String, String)> = raw
        .lines()
        .filter(|l| l.starts_with("@leap"))
        .filter_map(|l| {
            let mut parts = l.splitn(2, ' ');
            let key = parts.next()?.to_string();
            let val = parts.next().unwrap_or("").trim_matches('"').to_string();
            Some((key, val))
        })
        .collect();

    let mut user_patterns: Vec<(String, String)> = Vec::new();

    for (option, value) in &leap_opts {
        let method = option_to_method(option);
        match method.as_str() {
            "key"                       => config.key = value.clone(),
            "jump_key"                  => config.jump_key = value.clone(),
            "keyboard_layout"           => config.keyboard_layout = value.clone(),
            "main_action"               => config.main_action = value.clone(),
            "ctrl_action"               => config.ctrl_action = value.clone(),
            "alt_action"                => config.alt_action = value.clone(),
            "shift_action"              => config.shift_action = value.clone(),
            "use_system_clipboard"      => config.use_system_clipboard = value == "1",
            "hint_position"             => config.hint_position = value.clone(),
            "hint_style"                => config.hint_style = parse_style(value),
            "selected_hint_style"       => config.selected_hint_style = parse_style(value),
            "highlight_style"           => config.highlight_style = parse_style(value),
            "selected_highlight_style"  => config.selected_highlight_style = parse_style(value),
            "backdrop_style"            => config.backdrop_style = parse_style(value),
            "enabled_builtin_patterns"  => config.enabled_builtin_patterns = value.clone(),
            "enable_bindings"           => config.enable_bindings = value == "1",
            _ => {}
        }

        if method.starts_with("pattern_") && !value.is_empty() {
            if let Err(e) = regex::Regex::new(value) {
                eprintln!("[tmux-leap] Invalid pattern '{option}': {e}");
                std::process::exit(1);
            }
            let name = method.trim_start_matches("pattern_").to_string();
            user_patterns.push((name, value.clone()));
        }
    }

    // User patterns
    for (name, pattern) in user_patterns {
        config.patterns.insert(name, pattern);
    }

    // Built-in patterns
    let builtin_names: Vec<&str> = if config.enabled_builtin_patterns == "all" {
        BUILTIN_PATTERNS.iter().map(|(k, _)| *k).collect()
    } else {
        config.enabled_builtin_patterns
            .split(',')
            .map(|s| s.trim())
            .collect()
    };
    for name in builtin_names {
        if let Some(&(_, pat)) = BUILTIN_PATTERNS.iter().find(|(k, _)| *k == name) {
            config.patterns.entry(name.to_string()).or_insert_with(|| pat.to_string());
        }
    }

    // Compute alphabet
    config.alphabet = Config::alphabet_for_layout(&config.keyboard_layout);

    // Save config JSON
    if let Err(e) = config.save() {
        eprintln!("[tmux-leap] Failed to save config: {e}");
        std::process::exit(1);
    }

    // Install bindings
    let cli = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("tmux-leap"))
        .to_string_lossy()
        .into_owned();

    if config.enable_bindings {
        setup_root_bindings(&cli, &config);
    }
    setup_leap_mode_bindings(&cli);
    crate::tmux::exec(&["set-option", "-g", "@leap-cli", &cli]);
}

fn setup_root_bindings(cli: &str, config: &Config) {
    let log_path = crate::config::cache_dir().join("leap.log").to_string_lossy().into_owned();
    crate::tmux::exec(&[
        "bind-key", &config.key,
        "run-shell", "-b",
        &format!("{cli} start \"#{{pane_id}}\" >>{log_path} 2>&1"),
    ]);
    crate::tmux::exec(&[
        "bind-key", &config.jump_key,
        "run-shell", "-b",
        &format!("{cli} start --mode jump \"#{{pane_id}}\" >>{log_path} 2>&1"),
    ]);
}

fn setup_leap_mode_bindings(cli: &str) {
    for (key, cmd) in leap_mode_bindings(cli) {
        leap_bind(&key, &cmd);
    }
}

fn leap_mode_bindings(cli: &str) -> Vec<(String, String)> {
    const DISALLOWED: &[char] = &['c', 'i', 'm', 'q', 'n'];
    let mut bindings = Vec::new();

    for c in 'a'..='z' {
        if DISALLOWED.contains(&c) {
            continue;
        }
        let lower = c.to_string();
        let upper = c.to_uppercase().to_string();
        let ctrl = format!("C-{c}");
        let alt = format!("M-{c}");
        bindings.push((lower.clone(), format!("{cli} send-input hint:{lower}:main")));
        bindings.push((upper, format!("{cli} send-input hint:{lower}:shift")));
        bindings.push((ctrl, format!("{cli} send-input hint:{lower}:ctrl")));
        bindings.push((alt, format!("{cli} send-input hint:{lower}:alt")));
    }

    bindings.push(("Tab".into(), format!("{cli} send-input toggle-multi-mode")));
    bindings.push(("C-c".into(), format!("{cli} send-input exit")));
    bindings.push(("q".into(), format!("{cli} send-input exit")));
    bindings.push(("Escape".into(), format!("{cli} send-input exit")));
    bindings.push(("Enter".into(), format!("{cli} send-input noop")));
    bindings.push(("Any".into(), format!("{cli} send-input noop")));
    bindings
}

fn leap_bind(key: &str, cmd: &str) {
    let args = leap_bind_args(key, cmd);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    crate::tmux::exec(&args);
}

fn leap_bind_args(key: &str, cmd: &str) -> Vec<String> {
    // Do not use `run-shell -b` here. Leap inputs are stateful: `Tab` must be
    // processed before the following hint key, and multi-character hints must
    // arrive in order. A foreground `run-shell` keeps tmux's command queue
    // ordered while `tmux-leap send-input` runs (normally just a Unix socket write).
    vec![
        "bind-key".into(),
        "-Tleap".into(),
        key.into(),
        "run-shell".into(),
        cmd.into(),
    ]
}

fn option_to_method(option: &str) -> String {
    option
        .trim_start_matches("@leap-")
        .replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_mode_binds_tab_to_multi_select_toggle() {
        let bindings = leap_mode_bindings("tmux-leap");

        assert!(
            bindings
                .iter()
                .any(|(key, cmd)| key == "Tab" && cmd == "tmux-leap send-input toggle-multi-mode"),
            "Tab must toggle multi-select mode instead of falling through to Any/noop"
        );
    }

    #[test]
    fn leap_mode_binds_tab_before_any_noop() {
        let bindings = leap_mode_bindings("tmux-leap");
        let tab_pos = bindings.iter().position(|(key, _)| key == "Tab").unwrap();
        let any_pos = bindings.iter().position(|(key, _)| key == "Any").unwrap();

        assert!(
            tab_pos < any_pos,
            "Tab binding should be installed before Any/noop"
        );
    }

    #[test]
    fn leap_input_bindings_run_synchronously_to_preserve_order() {
        let args = leap_bind_args("Tab", "tmux-leap send-input toggle-multi-mode");

        assert_eq!(
            args,
            vec![
                "bind-key",
                "-Tleap",
                "Tab",
                "run-shell",
                "tmux-leap send-input toggle-multi-mode"
            ]
        );
        assert!(
            !args.iter().any(|arg| arg == "-b"),
            "leap input bindings must not use run-shell -b; background jobs can race Tab after the next hint key"
        );
    }
}
