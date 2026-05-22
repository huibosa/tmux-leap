# ADR 0004: Render multi-pane via mirrored layout + N swap-panes (not display-popup)

## Status

Accepted.

## Context

Multi-pane mode (ADR 0001) needs to render hint overlays over every pane in the window, in place, preserving each pane's geometry. Three rendering strategies were considered:

- **(1) Mirror the source layout in a hidden fingers window, swap N panes.** Generalization of the existing single-pane swap-pane trick. Capture each source pane, build a fingers window with the same layout via `select-layout <source_layout_string>`, render each pane's overlay into the corresponding fingers pane, then `swap-pane` each (source, fingers) pair.
- **(2) Single popup overlay.** Use `display-popup -E -w 100% -h 100%` (tmux 3.2+) to draw a composite buffer that places each pane's content at its layout-derived coordinates. One swap, one cleanup.
- **(3) Direct write to each pane's TTY (no swap).** Rejected — concurrent shell output corrupts the overlay; reverse-restoring color/cursor state without the swap trick is impossible.

## Decision

We picked **(1)**. Approach (3) is unsafe; approach (2) was rejected on tmux-version and visual grounds.

## Rationale

For (1) over (2):

- **tmux version floor.** tmux-fingers supports tmux 3.0+. `display-popup` requires 3.2+. Picking (2) would force a version bump and cut off users on older systems.
- **Visual fidelity.** `display-popup` has its own border and styling that either eats screen real estate or has to be styled to disappear. It's a popup *over* the window, not the window itself. The mirrored-swap approach renders inside the actual window, with hints exactly where the real content is.
- **Reuses existing rendering.** The single-pane Hinter pipeline (regex match → Huffman hint → styled output → write to pane TTY) works unchanged per pane in (1). In (2) we'd need a new "composite buffer" rendering path that maps `(line, col)` of each pane into the popup's flat grid.
- **Coordinate computation.** (2) requires us to parse tmux layout strings ourselves and compute pane positions. (1) lets tmux do that work via `select-layout`.

For (1) over (3):

- A hidden fingers window with rendered overlays + swap-pane is the only way to put text in front of an active pane without racing against the shell process running in it. The single-pane code already proved this. Don't break it.

## Consequences

- The rendering pipeline scales linearly with pane count: capture × N, render × N, swap × N. For typical windows (2–4 panes) the overhead is invisible.
- Layout reconstruction relies on `select-layout <layout-string>` reproducing the source geometry exactly. tmux may round pane sizes by ±1 — handled by reading back the fingers pane geometry and calling `resize-pane` to repair any mismatches.
- N swap-pane calls are not atomic. Between swap 1 and swap N (a few ms), the user briefly sees a window with mixed real and overlaid panes. Acceptable.
- A crash mid-render leaves a window with some panes overlaid. Pre-existing single-pane bug too, just N times wider. Manual recovery via `tmux kill-pane`/`kill-window`. A swap-manifest + recover subcommand is left as future work.
- Stays compatible with tmux 3.0+.
