# ADR 0002: Zoomed pane degrades to single-pane (no auto-unzoom)

## Status

Accepted.

## Context

Multi-pane mode (ADR 0001) renders hint overlays across every pane in the window. But a **zoomed** pane fills the entire window — the other panes exist in the layout but are not visible. Rendering hints into invisible panes produces hints the user can't see, which is broken.

Three behaviors were considered:

- **(i) Auto-unzoom.** Unzoom on entry, render multi-pane, rezoom on exit.
- **(ii) Stay zoomed, render single pane.** If `window_zoomed_flag` is set, the only pane in `target_panes` is the zoomed pane.
- **(iii) Refuse.** Display an error when invoked in a zoomed pane.

## Decision

We picked **(ii)**. When fingers mode is invoked in a zoomed pane, `target_panes = [active_pane]`. The rest of the rendering pipeline runs unchanged.

## Rationale

- Zoom is an explicit user action meaning "I want this pane to fill the screen." Auto-unzooming surprises the user and undoes their intent.
- The auto-unzoom path adds real reliability problems: race conditions if the user resizes the terminal mid-toggle; the `swap-pane -Z` logic that already exists for zoom interacts in non-obvious ways; if the Crystal process dies mid-mode, the user finds their window unzoomed for unrelated reasons.
- Refusing to enter the mode is rude and unhelpful — the user has matches they want in the visible pane.
- Implementation cost is near-zero: the multi-pane code path already iterates over `target_panes`. The zoom case sets the iteration set to one element. Same code, different size of input.

## Consequences

- Documented behavior: "while a pane is zoomed, fingers mode operates only on that pane."
- No special-case rendering code; the zoomed branch just constructs a one-element pane list at the top of `show_hints`.
- Users wanting multi-pane hints from a zoomed state must unzoom first (`prefix+z`).
