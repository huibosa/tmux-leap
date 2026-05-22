use crate::input_socket;

pub fn run(command: &str) {
    if let Err(e) = input_socket::send(command) {
        eprintln!("tmux-leap: send-input failed: {e}");
        std::process::exit(1);
    }
}
