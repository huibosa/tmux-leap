use crate::hinter::Hinter;
use crate::state::State;

pub struct View<'a> {
    hinter: &'a mut Hinter,
    state: &'a mut State,
    mode: String,
}

impl<'a> View<'a> {
    pub fn new(hinter: &'a mut Hinter, state: &'a mut State, mode: String) -> Self {
        View { hinter, state, mode }
    }

    pub fn render(&mut self) {
        if let Err(e) = self.hinter.run(self.state) {
            eprintln!("tmux-leap: render error: {e}");
            self.state.exiting = true;
        }
    }

    pub fn process_input(&mut self, input: &str) {
        let parts: Vec<&str> = input.splitn(3, ':').collect();
        match parts.first().copied().unwrap_or("") {
            "hint" => {
                if parts.len() >= 3 {
                    let char = parts[1];
                    let modifier = parts[2];
                    self.process_hint(char, modifier);
                }
            }
            "exit" => self.request_exit(),
            "toggle-multi-mode" => self.process_multimode(),
            _ => {}
        }
    }

    fn process_hint(&mut self, char: &str, modifier: &str) {
        self.state.input.push_str(char);
        self.state.modifier = modifier.to_string();

        if let Some(target) = self.hinter.lookup(&self.state.input).cloned() {
            let text = target.text.clone();
            self.state.matched_target = Some(target);
            self.handle_match(text);
        } else {
            self.render();
        }
    }

    fn process_multimode(&mut self) {
        if self.mode == "jump" {
            return;
        }
        let prev = self.state.multi_mode;
        self.state.multi_mode = !prev;
        if prev && !self.state.multi_mode {
            // toggle off: collect results
            self.state.result = self.state.multi_matches.join(" ");
            self.request_exit();
        }
    }

    fn handle_match(&mut self, text: String) {
        if self.state.multi_mode {
            self.state.multi_matches.push(text);
            self.state.selected_hints.push(self.state.input.clone());
            self.state.input.clear();
            self.render();
        } else {
            self.state.result = text;
            self.request_exit();
        }
    }

    fn request_exit(&mut self) {
        self.state.exiting = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hinter::PaneInput;
    use crate::match_formatter::MatchFormatter;

    fn test_hinter() -> Hinter {
        Hinter::new(
            vec![PaneInput {
                lines: vec!["foo".into()],
                pane_id: "%1".into(),
                width: 80,
                tty_path: "/dev/null".into(),
            }],
            vec!["foo".into()],
            vec!["a".into()],
            MatchFormatter::new(
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                "left".into(),
            ),
            true,
            String::new(),
            &State::default(),
        )
    }

    #[test]
    fn tab_then_hint_keeps_session_open_and_selects_match() {
        let mut hinter = test_hinter();
        let mut state = State::default();
        hinter.run(&state).unwrap();

        let mut view = View::new(&mut hinter, &mut state, "default".into());
        view.process_input("toggle-multi-mode");
        view.process_input("hint:a:main");

        assert!(view.state.multi_mode);
        assert!(!view.state.exiting, "multi-select should not exit after first selected hint");
        assert_eq!(view.state.multi_matches, vec!["foo"]);
        assert_eq!(view.state.selected_hints, vec!["a"]);
        assert!(view.state.input.is_empty());
        assert!(view.state.result.is_empty());
    }

    #[test]
    fn second_tab_finishes_multi_select_with_joined_result() {
        let mut hinter = test_hinter();
        let mut state = State::default();
        hinter.run(&state).unwrap();

        let mut view = View::new(&mut hinter, &mut state, "default".into());
        view.process_input("toggle-multi-mode");
        view.process_input("hint:a:main");
        view.process_input("toggle-multi-mode");

        assert!(!view.state.multi_mode);
        assert!(view.state.exiting);
        assert_eq!(view.state.result, "foo");
    }
}
