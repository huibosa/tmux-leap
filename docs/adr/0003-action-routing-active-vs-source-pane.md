# ADR 0003: Action routing splits between active pane and source pane

## Status

Accepted.

## Context

In single-pane mode, there was one pane: the user's active pane was also the pane the match came from. Actions (paste, jump, shell) all operated on that single pane via `original_pane`.

In multi-pane mode (ADR 0001), the two diverge. Consider: the user is in pane A, presses prefix+F, picks a hint that's visually in pane B. Now there are two candidate panes for actions:

- **Active pane** — pane A, where the user was working.
- **Source pane** — pane B, where the matched text lives.

Different actions have different "natural targets." We had to decide per action.

## Decision

| Action | Pane | Reasoning |
|---|---|---|
| Copy (`:copy:`) | (none) | Just clipboard. No pane involved. |
| Paste (`:paste:`) | **Active** | Result lands where the user was working. |
| Jump (`:jump:`) | **Source** | Cursor lands at the match's location, in the pane that holds it. |
| Shell action `chdir` (`:open:`, custom commands) | **Source** | Match's context is the pane it came from; relative paths resolve against that pane's CWD. |
| `expanded_match` (path-tilde expansion) | **Source** | Same as shell `chdir` reasoning. |

`ActionRunner`'s constructor takes both `active_pane` and `source_pane` and dispatches by action type. The `Target` struct carries `source_pane_id`, populated by `Hinter` when each hint is generated.

## Rationale

- **Paste tracks the user.** The mental model is "I see a SHA in the build pane and want it pasted into my editor." Pasting into the build pane would inject characters into a process the user isn't focused on.
- **Jump and shell track the match.** Jump means "show me where this is" — only meaningful in the pane that holds the match. Shell `chdir` means "what's the relevant working directory for this match" — also the pane that holds it.
- We considered making everything use the same pane (either always active or always source). Both are wrong: always-active breaks jump (would jump to the active pane regardless of where the match is); always-source breaks paste (text would be injected into a non-focused process).
- The asymmetry mirrors a real semantic distinction (deliver-to-user vs. operate-on-thing-in-place), so it's clear once you see it.

## Consequences

- `Target` gains a `source_pane_id : String` field — this propagates through the full pipeline (Hinter → state.matched_target → Start#process_result → ActionRunner).
- `ActionRunner.original_pane` is split into `active_pane` and `source_pane`. The constructor signature changes; any callers must be updated.
- `jump` prepends `tmux select-pane -t <source_pane_id>` so copy-mode opens in the right pane.
- New behavior to document: "if you pick a hint from a different pane, paste lands in your starting pane, but jump moves your cursor to the match's pane."
