mod cli;
mod cmd;
mod config;
mod action_runner;
mod display_width;
mod hinter;
mod hint;
mod input_socket;
mod match_formatter;
mod state;
mod tmux;
mod view;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Start(args) => cmd::start::run(args),
        Commands::LoadConfig => cmd::load_config::run(),
        Commands::SendInput { command } => cmd::send_input::run(&command),
        Commands::Version => cmd::version::run(),
    }
}
