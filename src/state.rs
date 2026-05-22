use crate::hinter::Target;

#[derive(Default)]
pub struct State {
    pub input: String,
    pub modifier: String,
    pub selected_hints: Vec<String>,
    pub multi_matches: Vec<String>,
    pub result: String,
    pub exiting: bool,
    pub multi_mode: bool,
    pub matched_target: Option<Target>,
}
