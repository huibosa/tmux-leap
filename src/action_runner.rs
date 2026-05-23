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
                "tmux send-keys -t {} -X cancel ; paste-buffer -t {}",
                self.active_pane.pane_id, self.active_pane.pane_id
            )
        } else {
            format!("tmux paste-buffer -t {}", self.active_pane.pane_id)
        }
    }

    fn jump_command(&self) -> Option<String> {
        let (line, col) = self.offset?;
        let src = &self.source_pane.pane_id;
        let mut cmd = format!(
            "tmux select-pane -t {src} ; \
             copy-mode -t {src} ; \
             send-keys -t {src} -X top-line ; \
             send-keys -t {src} -X start-of-line",
        );
        if line > 0 {
            cmd.push_str(&format!(" ; send-keys -t {src} -N {line} -X cursor-down"));
        }
        if col > 0 {
            cmd.push_str(&format!(" ; send-keys -t {src} -N {col} -X cursor-right"));
        }
        Some(cmd)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, in_mode: bool) -> Pane {
        Pane {
            pane_id: id.to_string(),
            window_id: "@1".into(),
            pane_width: 80,
            pane_height: 24,
            pane_left: 0,
            pane_top: 0,
            pane_current_path: "/tmp".into(),
            pane_in_mode: in_mode,
            scroll_position: None,
            window_zoomed_flag: false,
            pane_tty: "/dev/null".into(),
        }
    }

    fn runner<'a>(
        active_pane: &'a Pane,
        source_pane: &'a Pane,
        config: &'a Config,
        mode: &'a str,
        offset: Option<(usize, usize)>,
    ) -> ActionRunner<'a> {
        ActionRunner {
            modifier: "main",
            match_text: "match",
            hint: "a",
            active_pane,
            source_pane,
            offset,
            mode,
            main_action: None,
            ctrl_action: None,
            alt_action: None,
            shift_action: None,
            config,
        }
    }

    #[test]
    fn jump_command_is_one_tmux_batch_without_nested_tmux_commands() {
        let active = pane("%1", false);
        let source = pane("%2", false);
        let config = Config::default();
        let command = runner(&active, &source, &config, "jump", Some((2, 3)))
            .jump_command()
            .unwrap();

        let parts = shell_split(&command);
        assert_eq!(parts.iter().filter(|p| p.as_str() == "tmux").count(), 1);
        assert!(!parts.windows(2).any(|w| w[0] == ";" && w[1] == "tmux"));
        assert!(parts.windows(5).any(|w| w == ["copy-mode", "-t", "%2", ";", "send-keys"]));
        assert!(parts.windows(6).any(|w| w == ["-N", "2", "-X", "cursor-down", ";", "send-keys"]));
        assert!(parts.windows(4).any(|w| w == ["-N", "3", "-X", "cursor-right"]));
    }

    #[test]
    fn jump_command_omits_zero_repeat_counts() {
        let active = pane("%1", false);
        let source = pane("%2", false);
        let config = Config::default();
        let command = runner(&active, &source, &config, "jump", Some((0, 0)))
            .jump_command()
            .unwrap();

        assert!(!shell_split(&command).windows(2).any(|w| w == ["-N", "0"]));
        assert!(command.contains("copy-mode"));
        assert!(command.contains("top-line"));
        assert!(command.contains("start-of-line"));
    }

    #[test]
    fn paste_from_copy_mode_is_one_tmux_batch_without_nested_tmux_commands() {
        let active = pane("%1", true);
        let source = pane("%2", false);
        let config = Config::default();
        let command = runner(&active, &source, &config, "default", None).paste_command();

        let parts = shell_split(&command);
        assert_eq!(parts.iter().filter(|p| p.as_str() == "tmux").count(), 1);
        assert!(!parts.windows(2).any(|w| w[0] == ";" && w[1] == "tmux"));
        assert!(parts.windows(4).any(|w| w == [";", "paste-buffer", "-t", "%1"]));
    }
}
