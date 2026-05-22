# tmux-leap — codebase guide

Rust rewrite of tmux-fingers. Single binary that overlays regex-matched text in
tmux panes with selectable Huffman-coded letter hints.

## Build & test

```bash
cargo build --release          # produces target/release/tmux-leap
cargo test                     # 20 unit tests, no tmux required
cargo build                    # dev build for quick iteration
```

The entry point for tmux is `tmux-leap.tmux` (bash), which calls
`tmux-leap load-config` on tmux startup.

## Key commands

```bash
tmux-leap load-config          # parse @leap-* options, write config JSON, install bindings
tmux-leap start <pane_id>      # core flow — run hint session on a pane
tmux-leap send-input <cmd>     # write one line to the running session's Unix socket
tmux-leap version              # print version
```

## Source layout

```
src/
  main.rs            CLI dispatch (clap)
  cli.rs             Subcommand definitions (StartArgs, etc.)
  config.rs          Config struct (serde JSON), BUILTIN_PATTERNS, ALPHABET_MAP,
                     cache/socket path helpers (~/.cache/tmux-leap/)
  cmd/
    start.rs         Core session flow: capture panes → create [leap] window →
                     render hints → input loop → action → teardown
    load_config.rs   Parse @leap-* tmux options, save config.json, install bindings
    send_input.rs    Connect to Unix socket and write one message
    version.rs       Print CARGO_PKG_VERSION
  tmux/
    mod.rs           exec() / exec_stdin() — one-shot Command::new("tmux") per call
    pane.rs          Pane / Window structs (serde JSON from tmux -F), all tmux operations
    style.rs         SGR encoder: "fg=green,bold" → "\x1b[32m\x1b[1m"
  hint/
    huffman.rs       Huffman tree hint generator; file-based cache in ~/.cache/tmux-leap/
  hinter.rs          Per-pane regex scan → hint assignment → TTY render
  match_formatter.rs Hint overlay formatting with offset / position logic
  display_width.rs   UAX #11 display width via unicode-width crate; VS-16 upgrade
  state.rs           Session state (input, modifier, multi-mode, matched target)
  view.rs            Input command dispatch: hint:<char>:<modifier>, exit, toggle-multi-mode
  input_socket.rs    UnixListener accept loop + send() client
  action_runner.rs   Modifier-key dispatch; copy/paste/open/jump; clipboard detection
docs/
  CONTEXT.md         Domain glossary (Match, Hint, Target, Active pane, Source pane, …)
  adr/               Architectural decision records 0001–0006
```

## Architecture notes

**tmux IPC** — every tmux call is a separate `Command::new("tmux")` fork (ADR 0005).
No persistent shell, no async runtime. ~30 calls per session at ~1ms each.

**Hint rendering** — a hidden `[leap]` window is created, given the same layout as the
source window via `select-layout`, then each (source, leap) pane pair is linked with
`swap-pane`. The hinter writes ANSI-escaped hint overlays directly to the leap pane TTYs.
After selection the swap is reversed and `[leap]` is killed (ADR 0004).

**Multi-pane** — always on; `target_panes` = all panes in the active window, except when
the window is zoomed, in which case `target_panes = [active_pane]` (ADR 0001, 0002).

**Overlap resolution** — each pattern gets its own compiled `Regex`. Per-line matches from
all patterns are merged and sorted `(start ASC, end DESC)`. The longest match wins at any
given byte position; shorter sub-matches at the same start are skipped.

**Zoom preservation** — `swap-pane -Z` and `select-pane -Z` are passed whenever the
source session was zoomed, so fingers mode does not unzoom the pane (ADR 0002).

**Action routing** — paste goes to the active pane (where the user was), jump/open/shell
go to the source pane (where the match lives) (ADR 0003).

## Configuration options

All options are `@leap-*` tmux user options (read by `load_config.rs`).
Parsed option names drop the `@leap-` prefix and replace `-` with `_`
(e.g. `@leap-hint-style` → `hint_style`).

Config is persisted as JSON to `~/.cache/tmux-leap/config.json`.
The Unix socket lives at `~/.cache/tmux-leap/leap.sock`.
The key table is named `leap`.
The overlay window is named `[leap]`.
The global option `@leap-cli` is set to the binary path after load-config.

## Testing

Unit tests cover: Huffman generator, display width, match formatter, SGR encoder,
UUID/overlap regression. Run with `cargo test`.

For live testing:
```bash
# In a tmux session:
~/dev/tmux-leap/target/release/tmux-leap start %0

# Drive it from another pane:
~/dev/tmux-leap/target/release/tmux-leap send-input "hint:a:main"
~/dev/tmux-leap/target/release/tmux-leap send-input "exit"

# Check what was copied:
tmux show-buffer
```

## Adding a built-in pattern

In `src/config.rs`, add an entry to `BUILTIN_PATTERNS`. If the pattern uses a named
capture, name it `match` (e.g. `(?P<match>...)`). Each pattern is compiled independently
so duplicate group names across patterns are fine.
