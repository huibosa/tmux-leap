# ADR 0001: Multi-pane mode replaces single-pane mode

## Status

Accepted.

## Context

Fingers mode originally operated only on the active pane. Users requested the ability to select matches across all panes in the current window in a single keystroke.

Three options were on the table:

- **(a) Replace.** Make multi-pane the only mode. prefix+F always shows hints across the whole window.
- **(b) Coexist.** Keep prefix+F single-pane; add a new key (e.g. `@fingers-all-panes-key`) for multi-pane.
- **(c) Default + opt-out.** Multi-pane becomes default; old single-pane available behind a flag.

## Decision

We picked **(a) — replace**. There is no separate keybinding. prefix+F always shows hints across every pane in the window. (Zoomed pane is the one degenerate case — see ADR 0002.)

## Rationale

- Multi-pane is a strict superset of single-pane in capability: a window with one pane behaves identically to the old single-pane mode (the rendering pipeline iterates over `target_panes`, which collapses to `[active_pane]`).
- A second keybinding (option b) splits the mental model and the documentation, and forces users to remember two keys for variants of the same feature.
- A breaking default with an opt-out (option c) has the same migration cost as a clean replacement, but with permanent code-path duplication.

## Consequences

- Existing user muscle memory keeps working — same keybinding, same key table inside the mode.
- Users who genuinely want hints scoped to one pane can either zoom that pane (which falls back to single-pane behavior, ADR 0002) or pre-filter via `@fingers-patterns` to narrow what gets highlighted.
- The single-pane code path is removed, not preserved behind a flag. There's no second code path to maintain.
- The `Start` command's `pane_id` argument now means "the pane the user was in when invoking fingers" (the **active pane**) rather than "the only pane to scan" (the old **target pane**). The window is always derived from this pane.
