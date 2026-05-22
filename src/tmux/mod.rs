pub mod pane;
pub mod style;

use std::process::Command;

/// Run a single tmux subcommand and return trimmed stdout.
pub fn exec(args: &[&str]) -> String {
    let out = Command::new("tmux")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("tmux exec failed: {e}"));
    String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string()
}

/// Run a tmux subcommand, writing `input` to its stdin. Returns trimmed stdout.
pub fn exec_stdin(args: &[&str], input: &[u8]) -> String {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("tmux")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("tmux spawn failed: {e}"));
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).ok();
    }
    let out = child.wait_with_output().expect("tmux wait failed");
    String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string()
}

