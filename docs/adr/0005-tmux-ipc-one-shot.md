# ADR 0005: tmux IPC is one-shot Command::new, not a persistent shell

## Status

Accepted.

## Context

The Crystal implementation kept a persistent `/bin/sh` child process open and wrote tmux commands to its stdin, reading stdout back through a channel. This avoided the fork+exec overhead for each tmux call and let the shell expand variables.

In the Rust rewrite, we have a choice:

- **(a) Persistent shell.** Same as Crystal: open a `/bin/sh` at startup, pipe all tmux commands through it.
- **(b) One-shot Command::new.** Each tmux call is a separate `Command::new("tmux").args(...).output()` invocation.

## Decision

We picked **(b)** — one-shot `Command::new` per tmux call.

## Rationale

- **A fingers session makes ~30 tmux calls.** Fork+exec on Linux is O(1ms) per call; 30 × 1ms = 30ms — well within the 100ms cold-start budget.
- **No IPC framing needed.** The persistent-shell approach requires a sentinel (`echo cmd-end`) to delimit command output. That's a hidden protocol; bugs there surface as subtle hangs or corrupted output.
- **No threaded reader.** Crystal's persistent shell spawned a fiber to read output; Rust's sync model would need a thread or async runtime. Either adds complexity we don't need.
- **Simpler stdin passing.** For `load-buffer` (copy to clipboard), we need to write to tmux's stdin anyway. With one-shot `Command`, we just use `Stdio::piped()`. The persistent-shell approach has to frame the data differently.
- **tokio stays out.** Persistent IPC is the main driver for wanting an async runtime. Without it, `tmux-fingers` remains fully synchronous and avoids tokio's binary footprint and compile time.

## Consequences

- `tmux/mod.rs` exposes `exec(args)` and `exec_stdin(args, bytes)` as the only IPC surface.
- Any future batch optimisation (e.g. joining layout setup calls with `;`) can be done as a single `exec` call with a compound argument — no architectural change needed.
- If profiling shows fork cost is significant on a specific target (unlikely), the persistent-shell approach can be added behind a feature flag without touching the rest of the codebase.
