# tmux-leap

Overlay regex-matched text in your tmux panes with selectable letter hints. Press a hint to copy, paste, open, or jump to the match — without leaving the keyboard.

```
Visit https://example.com or check /etc/hosts
SHA: deadbeef1234567 UUID: 12345678-1234-1234-1234-123456789abc
```
becomes:
```
Visit jttps://example.com or check vetc/hosts
SHA: xeadbeef1234567 UUID: z2345678-1234-1234-1234-123456789abc
```
Press `j` → `https://example.com` is in your clipboard.

## Features

- **Multi-pane** — hints span every pane in the active window simultaneously
- **Huffman-coded hints** — shortest possible key sequences for the most common matches
- **Built-in patterns** — URLs, paths, SHAs, UUIDs, IPs, hex values, git status, diff output
- **Custom patterns** — add your own via `@leap-pattern-NAME` tmux options
- **Modifier keys** — main / Shift / Ctrl / Alt each trigger a different action
- **Jump mode** — move the cursor to a match's location in copy-mode
- **CJK / emoji aware** — hint overlay columns align correctly with wide characters

## Requirements

- tmux ≥ 3.0
- Rust / Cargo (to build from source)

## Installation

### TPM

```tmux
set -g @plugin 'huibosa/tmux-leap'
```

Run `prefix + I` to install.

### Manual

```bash
git clone https://github.com/huibosa/tmux-leap ~/.tmux/plugins/tmux-leap
cd ~/.tmux/plugins/tmux-leap
cargo build --release
```

Add to `~/.tmux.conf`:

```tmux
run-shell ~/.tmux/plugins/tmux-leap/tmux-leap.tmux
```

Reload: `tmux source-file ~/.tmux.conf`

## Usage

| Action | Key |
|---|---|
| Enter hint mode | `prefix + f` (default) |
| Enter jump mode | `prefix + j` (default) |
| Select a match | type its hint letters |
| Toggle multi-select mode | `Tab` |
| Finish multi-select mode | `Tab` again |
| Exit without selecting | `q` / `Escape` / `Ctrl-c` |

### Modifier keys (inside hint mode)

| Key | Default action |
|---|---|
| `<hint>` | copy to clipboard |
| `Shift + <hint>` | paste into active pane |
| `Ctrl + <hint>` | open with `xdg-open` / `open` |
| `Alt + <hint>` | custom shell command |

## Configuration

Set options in `~/.tmux.conf` before the `run-shell` line.

| Option | Default | Description |
|---|---|---|
| `@leap-key` | `f` | Key to enter hint mode (`prefix + <key>`) |
| `@leap-jump-key` | `j` | Key to enter jump mode |
| `@leap-keyboard-layout` | `qwerty` | Hint alphabet layout (see below) |
| `@leap-main-action` | `:copy:` | Action for unmodified hint |
| `@leap-shift-action` | `:paste:` | Action for Shift + hint |
| `@leap-ctrl-action` | `:open:` | Action for Ctrl + hint |
| `@leap-alt-action` | _(none)_ | Action for Alt + hint |
| `@leap-hint-style` | `fg=green,bold` | ANSI style for hint text |
| `@leap-highlight-style` | `fg=yellow` | ANSI style for matched text |
| `@leap-selected-hint-style` | `fg=blue,bold` | Style for selected hint (multi-mode) |
| `@leap-backdrop-style` | _(none)_ | Style applied to non-matched text |
| `@leap-hint-position` | `left` | `left` or `right` — hint placement |
| `@leap-use-system-clipboard` | `1` | Copy to system clipboard via pbcopy / wl-copy / xclip |

### Keyboard layouts

`@leap-keyboard-layout` controls which characters are used for hints:

`qwerty` · `qwerty-homerow` · `qwerty-left-hand` · `qwerty-right-hand` ·
`azerty` · `azerty-homerow` · `dvorak` · `dvorak-homerow` · `colemak` · `colemak-homerow` · …

### Custom actions

Actions can be a built-in keyword or any shell command. The matched text is
written to the command's stdin; `$MODIFIER` and `$HINT` are set as environment
variables.

```tmux
# Open URLs with Firefox
set -g @leap-ctrl-action 'xargs firefox'

# Copy to a named register with a custom script
set -g @leap-alt-action '~/.scripts/store-match.sh'
```

Built-in keywords: `:copy:` · `:paste:` · `:open:`

### Custom patterns

```tmux
# Match Jira ticket IDs
set -g @leap-pattern-jira '[A-Z]+-[0-9]+'

# Match Docker image digests
set -g @leap-pattern-digest 'sha256:[a-f0-9]{64}'
```

### Enabled built-in patterns

```tmux
# Only enable a subset of built-ins
set -g @leap-enabled-builtin-patterns 'url,path,sha,uuid'
```

Available: `ip` · `uuid` · `sha` · `digit` · `number` · `url` · `path` · `hex` ·
`git-status` · `git-status-branch` · `diff`

## How it works

1. `tmux-leap.tmux` sources on tmux startup and calls `tmux-leap load-config`, which reads `@leap-*` options, writes a config JSON cache, and installs key bindings.
2. On `prefix + f`, tmux runs `tmux-leap start #{pane_id}` in the background.
3. The binary captures every pane in the active window, creates a hidden `[leap]` window with the same layout, renders Huffman-coded hint overlays into it, and swaps the overlay into place via `swap-pane`.
4. Key presses are routed through a `leap` key table to `tmux-leap send-input`, which writes to a Unix socket the `start` process is listening on.
5. On selection, the match is loaded into the tmux buffer (optionally also the system clipboard) and the configured action runs. The `[leap]` window is killed and the original layout restored.

See `docs/CONTEXT.md` for the full domain glossary and `docs/adr/` for architectural decisions.

## TODO

Ideas drawn from the hint / quickselect implementations in alacritty, kitty, and wezterm.

### Planned

- **Filter-as-you-type with prefix strip + auto-pick on unique** — render only the *remaining* characters of each label, fire as soon as one label has the typed prefix. (kitty `kittens/hints/main.go`, alacritty `display/hint.rs`)
- **Extend match dedup to jump mode** — copy/paste already gives identical text one shared hint via `reuse_hints`; jump deliberately doesn't. Decide if a unified hint per unique text is the better default. (wezterm `mux/src/localpane.rs`)
- **Replace Huffman labels with alphabet-split labels** — same constant-prefix property, no decoder tree, no disk cache at `~/.cache/tmux-leap/`. Behavior change: slightly less optimal label lengths. (alacritty `display/hint.rs`, `HINT_SPLIT_PERCENTAGE`)

### Deferred

- **Atomic-update bracketing** (DECSET 2026) around per-pane TTY writes — small visible benefit since the swap-pane model already isolates rendering. (kitty)
- **OSC 8 hyperlink wrap** of each hint for mouse-clickable selection — blocked on tmux mouse-event routing. (kitty)
- **Lazy per-pattern compile** — only force patterns the `RegexSet` actually hits. Marginal at today's pattern counts. (alacritty `config/ui_config.rs`, `LazyRegex`)
- **Bounded scrollback search** — `viewport ± N lines` cap on regex scope. Defer until scrollback capture (`capture-pane -S -`) is added. (wezterm `quickselect.rs`, `scope_lines`)
- **`MatchFormatter` rewrite** — single pre-allocated `String` + `write!` instead of a `format!` chain, paired with a match-cursor walk. Revisit if profiling flags it. (alacritty `display/content.rs`, `HintMatches::advance`)

### Known caveats

- **Soft-wrapped matches are split.** A URL wrapping across two visible lines is currently two strings to the regex scanner. Real fix needs `tmux capture-pane -J` plus a byte→cell coordinate map.
- **Tab columns past column 0 are approximated** in `compute_padding` (8 columns flat instead of `8 - col % 8`). Pre-existing.

## License

MIT
