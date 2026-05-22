use std::path::PathBuf;
use std::process::{Command, Stdio};
use crate::tmux::pane::{self, Pane};
use crate::config::Config;

pub struct ActionRunner<'a> {
    pub modifier: &'a str,
    pub match_text: &'a str,
    pub hint: &'a str,
    pub active_pane: &'a Pane,
    pub source_pane: &'a Pane,
    pub offset: Option<(usize, usize)>,
    pub mode: &'a str,
    pub main_action: Option<&'a str>,
    pub ctrl_action: Option<&'a str>,
    pub alt_action: Option<&'a str>,
    pub shift_action: Option<&'a str>,
    pub config: &'a Config,
}

impl<'a> ActionRunner<'a> {
    pub fn run(&self) {
        // Always load the match into tmux buffer first.
        if self.config.use_system_clipboard {
            pane::set_buffer(self.match_text);
        } else {
            pane::set_buffer_no_clipboard(self.match_text);
        }

        let cmd = if self.mode == "jump" {
            self.jump_command()
        } else {
            self.action_command()
        };

        let cmd = match cmd {
            Some(c) if !c.is_empty() => c,
            _ => return,
        };

        let expanded = self.expanded_match();
        let env = [("MODIFIER", self.modifier), ("HINT", self.hint)];
        let cwd = if self.source_pane.pane_current_path.is_empty() {
            None
        } else {
            Some(PathBuf::from(&self.source_pane.pane_current_path))
        };

        // Parse shell command string into program + args
        let parts = shell_split(&cmd);
        if parts.is_empty() {
            return;
        }

        let mut child = Command::new(&parts[0]);
        child.args(&parts[1..]);
        child.stdin(Stdio::piped());
        child.stdout(Stdio::null());
        child.envs(env);
        if let Some(dir) = cwd {
            child.current_dir(dir);
        }

        if let Ok(mut c) = child.spawn() {
            if let Some(mut stdin) = c.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(expanded.as_bytes());
            }
        }
    }

    fn action(&self) -> Option<&str> {
        match self.modifier {
            "main"  => self.main_action.or(Some(&self.config.main_action)),
            "shift" => self.shift_action.or(Some(&self.config.shift_action)),
            "alt"   => self.alt_action.or(Some(&self.config.alt_action)),
            "ctrl"  => self.ctrl_action.or(Some(&self.config.ctrl_action)),
            _       => None,
        }
    }

    fn action_command(&self) -> Option<String> {
        match self.action()? {
            ":copy:"  => self.system_copy_command(),
            ":open:"  => self.system_open_command(),
            ":paste:" => Some(self.paste_command()),
            other     => Some(other.to_string()),
        }
    }

    fn paste_command(&self) -> String {
        if self.active_pane.pane_in_mode {
            format!(
                "tmux send-keys -t {} -X cancel ; tmux paste-buffer -t {}",
                self.active_pane.pane_id, self.active_pane.pane_id
            )
        } else {
            format!("tmux paste-buffer -t {}", self.active_pane.pane_id)
        }
    }

    fn jump_command(&self) -> Option<String> {
        let (line, col) = self.offset?;
        let src = &self.source_pane.pane_id;
        Some(format!(
            "tmux select-pane -t {src} ; \
             tmux copy-mode -t {src} ; \
             tmux send-keys -t {src} -X top-line ; \
             tmux send-keys -t {src} -N {line} -X cursor-down ; \
             tmux send-keys -t {src} -N {col} -X cursor-right",
        ))
    }

    fn system_copy_command(&self) -> Option<String> {
        if !self.config.use_system_clipboard {
            return None;
        }
        if program_exists("pbcopy") {
            Some("pbcopy".into())
        } else if program_exists("wl-copy") {
            Some("wl-copy".into())
        } else if program_exists("xclip") {
            Some("xclip -selection clipboard".into())
        } else if program_exists("xsel") {
            Some("xsel -i --clipboard".into())
        } else {
            None
        }
    }

    fn system_open_command(&self) -> Option<String> {
        if program_exists("xdg-open") {
            Some("xargs xdg-open".into())
        } else if program_exists("open") {
            Some("xargs open".into())
        } else {
            None
        }
    }

    fn expanded_match(&self) -> String {
        if self.action() == Some(":open:") && self.match_text.starts_with('~') {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
            let without_tilde = self.match_text.trim_start_matches('~');
            let mut path = home;
            if !without_tilde.is_empty() {
                path.push(without_tilde.trim_start_matches('/'));
            }
            path.to_string_lossy().into_owned()
        } else {
            self.match_text.to_string()
        }
    }
}

fn program_exists(name: &str) -> bool {
    which::which(name).is_ok()
}

fn shell_split(s: &str) -> Vec<String> {
    // Simple whitespace split; handles quoted strings via a minimal parser.
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for c in s.chars() {
        match c {
            '\'' if !in_double => { in_single = !in_single; }
            '"' if !in_single  => { in_double = !in_double; }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}
