pub mod pane;
pub mod style;

use std::process::Command;

/// Run a single tmux subcommand and return trimmed stdout.
pub fn exec(args: &[&str]) -> String {
    let out = Command::new("tmux")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("tmux exec failed: {e}"));
    finalize_stdout(out.stdout)
}

/// Run several tmux subcommands in a single fork, separated by `;` arguments.
/// Stdout from all commands is concatenated and returned trimmed.
pub fn exec_batch(commands: &[&[&str]]) -> String {
    let total: usize = commands.iter().map(|c| c.len()).sum::<usize>() + commands.len();
    let mut args: Vec<&str> = Vec::with_capacity(total);
    for (i, cmd) in commands.iter().enumerate() {
        if i > 0 {
            args.push(";");
        }
        args.extend_from_slice(cmd);
    }
    exec(&args)
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
    finalize_stdout(out.stdout)
}

/// Convert a tmux stdout buffer to a String, dropping trailing newlines in place.
/// Avoids the extra alloc that `from_utf8_lossy(...).to_string()` would introduce.
fn finalize_stdout(bytes: Vec<u8>) -> String {
    let mut s = String::from_utf8(bytes)
        .unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned());
    let trimmed_len = s.trim_end_matches('\n').len();
    s.truncate(trimmed_len);
    s
}

