use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const BUILTIN_PATTERNS: &[(&str, &str)] = &[
    ("ip",    r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}"),
    ("uuid",  r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"),
    ("sha",   r"[0-9a-f]{7,128}"),
    ("digit", r"[0-9]{4,}"),
    ("url",   r#"((https?://|git@|git://|ssh://|ftp://|file:///)[^\s()"']+)"#),
    ("path",  r"(([.\w\-~\$@]+)?(/[.\w\-@]+)+/?)"),
    ("hex",   r"(0x[0-9a-fA-F]+)"),
    ("git-status",        r"(modified|deleted|deleted by us|new file): +(?P<match>.+)"),
    ("git-status-branch", r"Your branch is up to date with '(?P<match>.*)'\."),
    ("diff",              r"(---|[+][+][+]) [ab]/(?P<match>.*)"),
];

pub const ALPHABET_MAP: &[(&str, &str)] = &[
    ("qwerty",             "asdfqwerzxcvjklmiuopghtybn"),
    ("qwerty-homerow",     "asdfjklgh"),
    ("qwerty-left-hand",   "asdfqwerzcxv"),
    ("qwerty-right-hand",  "jkluiopmyhn"),
    ("azerty",             "qsdfazerwxcvjklmuiopghtybn"),
    ("azerty-homerow",     "qsdfjkmgh"),
    ("azerty-left-hand",   "qsdfazerwxcv"),
    ("azerty-right-hand",  "jklmuiophyn"),
    ("qwertz",             "asdfqweryxcvjkluiopmghtzbn"),
    ("qwertz-homerow",     "asdfghjkl"),
    ("qwertz-left-hand",   "asdfqweryxcv"),
    ("qwertz-right-hand",  "jkluiopmhzn"),
    ("dvorak",             "aoeuqjkxpyhtnsgcrlmwvzfidb"),
    ("dvorak-homerow",     "aoeuhtnsid"),
    ("dvorak-left-hand",   "aoeupqjkyix"),
    ("dvorak-right-hand",  "htnsgcrlmwvz"),
    ("colemak",            "arstqwfpzxcvneioluymdhgjbk"),
    ("colemak-homerow",    "arstneiodh"),
    ("colemak-left-hand",  "arstqwfpzxcv"),
    ("colemak-right-hand", "neioluymjhk"),
];

pub const DISALLOWED_HINT_CHARS: &[char] = &['c', 'i', 'm', 'q', 'n'];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub key: String,
    pub jump_key: String,
    #[serde(default = "default_words_key")]
    pub words_key: String,
    #[serde(default = "default_jump_words_key")]
    pub jump_words_key: String,
    pub keyboard_layout: String,
    pub alphabet: Vec<String>,
    pub patterns: HashMap<String, String>,
    pub main_action: String,
    pub ctrl_action: String,
    pub alt_action: String,
    pub shift_action: String,
    pub use_system_clipboard: bool,
    pub hint_position: String,
    pub hint_style: String,
    pub selected_hint_style: String,
    pub highlight_style: String,
    pub selected_highlight_style: String,
    pub backdrop_style: String,
    pub tmux_version: String,
    pub enabled_builtin_patterns: String,
    pub enable_bindings: bool,
}

impl Default for Config {
    fn default() -> Self {
        use crate::tmux::style::parse_style;
        Config {
            key: "f".into(),
            jump_key: "j".into(),
            words_key: "F".into(),
            jump_words_key: "J".into(),
            keyboard_layout: "qwerty".into(),
            alphabet: vec![],
            patterns: HashMap::new(),
            main_action: ":copy:".into(),
            ctrl_action: ":open:".into(),
            alt_action: String::new(),
            shift_action: ":paste:".into(),
            use_system_clipboard: true,
            hint_position: "left".into(),
            hint_style: parse_style("fg=green,bold"),
            selected_hint_style: parse_style("fg=blue,bold"),
            highlight_style: parse_style("fg=yellow"),
            selected_highlight_style: parse_style("fg=blue"),
            backdrop_style: String::new(),
            tmux_version: "3.1".into(),
            enabled_builtin_patterns: "all".into(),
            enable_bindings: true,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if let Ok(f) = std::fs::File::open(&path) {
            if let Ok(cfg) = serde_json::from_reader(f) {
                return cfg;
            }
        }
        Config::default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let f = std::fs::File::create(&path)?;
        serde_json::to_writer(f, self)?;
        Ok(())
    }

    pub fn alphabet_for_layout(layout: &str) -> Vec<String> {
        let chars = ALPHABET_MAP
            .iter()
            .find(|(k, _)| *k == layout)
            .map(|(_, v)| *v)
            .unwrap_or("asdfqwerzxcvjklmiuopghtybn");
        chars
            .chars()
            .filter(|c| !DISALLOWED_HINT_CHARS.contains(c))
            .map(|c| c.to_string())
            .collect()
    }
}

fn default_words_key() -> String {
    "F".into()
}

fn default_jump_words_key() -> String {
    "J".into()
}

pub fn config_path() -> std::path::PathBuf {
    cache_dir().join("config.json")
}

pub fn cache_dir() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("tmux-leap")
}

pub fn socket_path() -> std::path::PathBuf {
    cache_dir().join("leap.sock")
}
