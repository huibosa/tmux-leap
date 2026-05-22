use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tmux-leap", about = "tmux copy/paste with hint labels")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show hints over pane content and handle selection
    Start(StartArgs),
    /// Parse tmux options, write config JSON, install key bindings
    LoadConfig,
    /// Send an input command to the running leap session socket
    SendInput {
        /// Command string (e.g. "hint:a:main", "exit")
        command: String,
    },
    /// Print version
    Version,
}

#[derive(Args)]
pub struct StartArgs {
    /// Pane ID or tmux target-pane token (e.g. %3 or {last})
    pub pane_id: String,

    /// Operating mode: "default" or "jump"
    #[arg(long, default_value = "default")]
    pub mode: String,

    /// Comma-separated pattern names to use (overrides config)
    #[arg(long)]
    pub patterns: Option<String>,

    /// Action when a hint is selected with no modifier
    #[arg(long)]
    pub main_action: Option<String>,

    /// Action when a hint is selected while holding Ctrl
    #[arg(long)]
    pub ctrl_action: Option<String>,

    /// Action when a hint is selected while holding Alt
    #[arg(long)]
    pub alt_action: Option<String>,

    /// Action when a hint is selected while holding Shift
    #[arg(long)]
    pub shift_action: Option<String>,
}
