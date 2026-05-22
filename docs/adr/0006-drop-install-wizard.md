# ADR 0006: Drop the install wizard and self-update

## Status

Accepted.

## Context

The Crystal implementation shipped an interactive `install-wizard.sh` that:
1. Detected the user's OS and compiled the Crystal binary.
2. Ran on first invocation (or on version mismatch) from `tmux-fingers.tmux`.
3. Had a documented bug (`845d2c8`) where version-mismatch detection could loop indefinitely.

The wizard also drove a `self-update` flow: if the binary version didn't match `shard.yml`, it triggered a recompile.

For the Rust rewrite, we have a choice:

- **(a) Port the wizard.** Replicate the Crystal wizard in bash/Rust, adapting it for `cargo`.
- **(b) Drop it entirely.** Ship a binary; users who build from source run `cargo install --path .` once.

## Decision

We picked **(b)** — no install wizard, no self-update.

## Rationale

- **Leaner first-run UX.** The wizard was the primary source of support issues in the Crystal version (OS detection edge cases, network failures mid-install, the infinite-loop bug). Removing it eliminates an entire failure category.
- **Static binaries.** GitHub Actions builds `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu`, and `aarch64-apple-darwin` binaries. TPM users download a single file; no compilation required.
- **cargo is the wizard.** For users who clone the repo, `cargo install --path .` is a single command that works everywhere Rust is installed. It's simpler and more reliable than a custom bash wizard.
- **No version drift hazard.** The self-update flow was needed because Crystal binaries aren't portable and the wizard had to recompile. Rust static binaries are portable; the binary just works after download.

## Consequences

- `tmux-fingers.tmux` no longer calls `install-wizard.sh`. If no binary is found, it prints a single tmux message: `"[tmux-fingers] run 'cargo install --path .' or download a release binary"`.
- The `install-wizard.sh` file is not present in this repo.
- `info` subcommand (which the wizard used to check installed version) is also dropped — version is available via `tmux-fingers version`.
- Migration: Crystal users switching to the Rust version remove the old binary from `$PATH` or `bin/`, then follow the one-liner install.
