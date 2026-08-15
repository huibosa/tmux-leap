use std::collections::HashSet;
use std::path::PathBuf;
use std::process::{Command, Output};

struct TmuxServer {
    tmux: PathBuf,
    socket_name: String,
}

impl TmuxServer {
    fn start() -> Option<Self> {
        let tmux = which::which("tmux").ok()?;
        let socket_name = format!("tmux-leap-test-{}", std::process::id());
        let server = Self { tmux, socket_name };
        server.run_ok(&[
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-s",
            "repro",
            "-x",
            "160",
            "-y",
            "48",
            "cat",
        ]);
        Some(server)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.tmux)
            .arg("-L")
            .arg(&self.socket_name)
            .args(args)
            .output()
            .expect("run tmux")
    }

    fn run_ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "tmux {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("tmux stdout is UTF-8")
            .trim()
            .to_string()
    }
}

impl Drop for TmuxServer {
    fn drop(&mut self) {
        let _ = self.run(&["kill-server"]);
    }
}

#[test]
fn start_handles_eight_pane_two_by_four_window() {
    let Some(server) = TmuxServer::start() else {
        eprintln!("skipping tmux integration test: tmux is not installed");
        return;
    };
    let home = tempfile::tempdir().expect("create temporary home");

    for _ in 1..4 {
        server.run_ok(&["split-window", "-h", "-d", "-t", "repro:0", "cat"]);
        server.run_ok(&["select-layout", "-t", "repro:0", "even-horizontal"]);
    }
    let top_row_panes = server.run_ok(&["list-panes", "-t", "repro:0", "-F", "#{pane_id}"]);
    for pane_id in top_row_panes.lines() {
        server.run_ok(&[
            "split-window",
            "-v",
            "-d",
            "-l",
            "50%",
            "-t",
            pane_id,
            "cat",
        ]);
    }

    let geometry = server.run_ok(&[
        "list-panes",
        "-t",
        "repro:0",
        "-F",
        "#{pane_left} #{pane_top}",
    ]);
    let positions: Vec<(u32, u32)> = geometry
        .lines()
        .map(|line| {
            let mut fields = line
                .split_whitespace()
                .map(|field| field.parse::<u32>().expect("numeric pane position"));
            (
                fields.next().expect("pane left"),
                fields.next().expect("pane top"),
            )
        })
        .collect();
    assert_eq!(positions.len(), 8);
    assert_eq!(
        positions
            .iter()
            .map(|(left, _)| left)
            .collect::<HashSet<_>>()
            .len(),
        4
    );
    assert_eq!(
        positions
            .iter()
            .map(|(_, top)| top)
            .collect::<HashSet<_>>()
            .len(),
        2
    );

    let source_pane = server.run_ok(&["display-message", "-p", "-t", "repro:0.0", "#{pane_id}"]);
    let tmux_environment = server.run_ok(&[
        "display-message",
        "-p",
        "-t",
        "repro:0",
        "#{socket_path},#{pid},0",
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_tmux-leap"))
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join(".cache"))
        .env("TMUX", tmux_environment)
        .args(["start", &source_pane, "--mode", "benchmark"])
        .output()
        .expect("run tmux-leap");

    assert!(
        output.status.success(),
        "tmux-leap failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
