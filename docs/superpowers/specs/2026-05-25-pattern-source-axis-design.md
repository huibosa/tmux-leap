# Pattern-source axis: 2×2 binding matrix

Date: 2026-05-25
Status: Approved (design)

## Summary

Add a second axis to tmux-leap's root bindings so users can choose pattern
source (configured patterns vs. all whitespace-delimited tokens) independently
of action mode (copy/paste/open vs. jump cursor). Result: a 2×2 matrix of
4 root bindings, all configurable.

## Motivation

Today the pattern source is fused with the action mode:

- `prefix+F` (default mode) hints **configured patterns** and acts as
  copy/paste/open.
- `prefix+J` (jump mode) hints **all words** (`\S+`) and jumps the cursor.

The two combinations the user cannot reach are:
- words + copy: hint any whitespace token and copy it.
- patterns + jump: hint a URL/path/etc. and jump the cursor to it.

Both are useful. Decoupling the two axes adds them with minimal new code.

## Design

### Surface

CLI: one new boolean flag on `start`.

```
tmux-leap start [--mode default|jump] [--words] [--patterns NAMES] <pane>
```

Tmux options (all configurable; defaults shown):

| Option                  | Default | Effect                                      |
|-------------------------|---------|---------------------------------------------|
| `@leap-key`             | `f`     | `start`                          (patterns + copy) |
| `@leap-jump-key`        | `j`     | `start --mode jump`              (patterns + jump) |
| `@leap-words-key`       | `F`     | `start --words`                  (words + copy)    |
| `@leap-jump-words-key`  | `J`     | `start --mode jump --words`      (words + jump)    |

Existing options `@leap-key` and `@leap-jump-key` are kept. Their **defaults
change** from `F`/`J` to `f`/`j`. Two new options `@leap-words-key` and
`@leap-jump-words-key` are added with defaults `F`/`J`.

### Internals

`src/cli.rs` — `StartArgs` gains:

```rust
#[arg(long)]
pub words: bool,
```

`src/cmd/start.rs` — pattern selection:

```rust
let patterns = if args.words {
    vec![r"\S+".to_string()]
} else if let Some(p) = &args.patterns {
    patterns_from_option(p, &config)
} else {
    config.patterns.values().cloned().collect()
};
```

This **removes** the current branch `if args.mode == "jump" { vec![r"\S+"] }`.
Words are now driven exclusively by `--words`.

Mode-keyed logic that stays as-is:
- `let join = mode != "jump"` — capture-pane joining (required for copy-mode
  cursor coordinates in jump mode).
- `mode != "jump"` value passed as the `reuse_hints` argument to
  `Hinter::new` (default mode dedupes hints across identical matched text;
  jump mode gives every match its own hint because the cursor lands on a
  specific position).
- Action routing in `teardown` (jump restores cursor at the matched offset;
  default restores the previous pane focus).

`src/config.rs` — `Config` gains two fields with defaults:

```rust
pub words_key: String,        // default "F"
pub jump_words_key: String,   // default "J"
```

And the existing defaults change:

```rust
key: "f".into(),        // was "F"
jump_key: "j".into(),   // was "J"
```

`src/cmd/load_config.rs`:
- Add two match arms: `"words_key"` and `"jump_words_key"`.
- `setup_root_bindings` emits **4** bindings instead of 2:

```text
bind-key {key}             run-shell -b 'cli start "#{pane_id}"'
bind-key {jump_key}        run-shell -b 'cli start --mode jump "#{pane_id}"'
bind-key {words_key}       run-shell -b 'cli start --words "#{pane_id}"'
bind-key {jump_words_key}  run-shell -b 'cli start --mode jump --words "#{pane_id}"'
```

### Data flow

Unchanged. The flag flows: tmux binding → CLI parse → `start::run` → patterns
vector → `Hinter`. No socket protocol change. No `State` struct change. No
action_runner change.

### Error handling

- `--words` combined with `--patterns`: `--words` wins (silent). User-facing
  combinations come from load_config bindings, which never emit both.
- `\S+` is a compile-time constant; cannot fail.
- Existing pattern compile errors keep their current behavior.

## Testing

1. **Unit, `cli.rs`** — clap parses `--words` into `StartArgs.words` correctly.
2. **Unit, `load_config.rs`** — extend bindings tests:
   - For default config, the 4 emitted root bindings have keys `f`/`F`/`j`/`J`
     and command tails `start`, `start --words`, `start --mode jump`,
     `start --mode jump --words`.
   - The current `leap_mode_bindings` tests stay untouched.
3. **Manual** (live tmux required):
   - prefix+f: configured patterns hint, copy/paste/open via modifiers.
   - prefix+F: `\S+` hint, copy/paste/open via modifiers.
   - prefix+j: configured patterns hint, jump cursor to match.
   - prefix+J: `\S+` hint, jump cursor to match.
   - Verify zoom preservation and multi-pane each still work for all 4.

## Backward compatibility

This is a **breaking config change** for users on default keys:
- `prefix+F` semantics change from patterns+copy → words+copy.
- `prefix+J` semantics are unchanged (still words+jump).
- Users who set `@leap-key="F"` explicitly are unaffected.

Documented in CHANGELOG / release notes. No automatic migration; no aliasing.

If a user sets `@leap-key="F"` and leaves `@leap-words-key="F"` at default,
the bindings collide and tmux's last-wins semantics apply. We do not detect
the collision; the README note covers it.

## Out of scope

- Renaming or aliasing old options.
- Making the `\S+` pattern itself configurable (e.g., choose between `\S+` and
  `\w+`).
- Any change to socket protocol, `State`, `Hinter`, `ActionRunner`, or
  teardown logic.

## Files touched

- `src/cli.rs` — 1 new field on `StartArgs`.
- `src/cmd/start.rs` — pattern selection rewritten (~5 lines).
- `src/config.rs` — 2 new fields, 2 default changes.
- `src/cmd/load_config.rs` — 2 new option arms, `setup_root_bindings` emits
  4 bindings, plus a new test.
- `CHANGELOG` / release notes — breaking-change entry.

Estimated total: ~30–50 lines changed across 4 source files plus tests.
