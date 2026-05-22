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
    const DISALLOWED: &[char] = &['c', 'i', 'm', 'q', 'n'];
    for c in 'a'..='z' {
        if DISALLOWED.contains(&c) {
            continue;
        }
        let lower = c.to_string();
        let upper = c.to_uppercase().to_string();
        let ctrl = format!("C-{c}");
        let alt = format!("M-{c}");
        leap_bind(&lower, &format!("{cli} send-input hint:{lower}:main"));
        leap_bind(&upper, &format!("{cli} send-input hint:{lower}:shift"));
        leap_bind(&ctrl,  &format!("{cli} send-input hint:{lower}:ctrl"));
        leap_bind(&alt,   &format!("{cli} send-input hint:{lower}:alt"));
    }
    leap_bind("C-c",    &format!("{cli} send-input exit"));
    leap_bind("q",      &format!("{cli} send-input exit"));
    leap_bind("Escape", &format!("{cli} send-input exit"));
    leap_bind("Enter",  &format!("{cli} send-input noop"));
    leap_bind("Any",    &format!("{cli} send-input noop"));
}

fn leap_bind(key: &str, cmd: &str) {
    crate::tmux::exec(&["bind-key", "-Tleap", key, "run-shell", "-b", cmd]);
}

fn option_to_method(option: &str) -> String {
    option
        .trim_start_matches("@leap-")
        .replace('-', "_")
}
