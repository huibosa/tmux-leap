# Context: tmux-fingers

A tmux plugin that overlays regex-matched substrings of pane content with selectable letter hints. Press a hint, the underlying match is copied / pasted / opened.

## Glossary

### Match
A substring of pane content that matches one of the configured patterns. Matches are the things hints get attached to.

### Pattern
A regex that defines what counts as a match. Built-in patterns cover paths, SHAs, IPs, UUIDs, etc. Users can add custom patterns.

### Hint
A short letter sequence (Huffman-coded over the configured alphabet) assigned to a match for selection. Hints are unique within a single fingers-mode session and shared across all panes (one hint pool per window-wide session).

### Target
The data structure that pairs a match with its assigned hint and its **source pane**. Carries `text`, `hint`, `offset`, `source_pane_id`. See `src/fingers/types.cr`.

### Active pane
The pane the user was in when they invoked fingers mode. Used as the destination for actions that "deliver to the user" — primarily **paste**.

### Source pane
The pane whose content held the matched text. Used as the context for actions that "operate on the match in place" — **jump**, **shell action chdir**.

In single-pane mode (pre-multi-pane), active and source were always the same. The distinction matters because in multi-pane mode they can differ: the user can be in pane A (active) and pick a hint that's in pane B (source).

### Fingers window / fingers pane
A hidden tmux window (named `[fingers]`) created at session start, holding the rendered hint overlay. Swapped with the source window's panes via `swap-pane` so the overlay appears in place of the real content. Killed at session end.

### Multi-pane mode
The current operating mode (since the multi-pane patch). prefix+F invokes fingers mode across **every pane in the active window simultaneously**. Replaces the previous single-pane-only behavior. There is no opt-out.

### Multi-select / multi-mode (`state.multi_mode`)
**Distinct from multi-pane.** A separate, pre-existing feature: pressing `Tab` inside fingers mode lets the user pick multiple hints in a single session, joining their text with spaces on exit. Orthogonal to whether the session spans one pane or many.

### Display width
The number of terminal columns a string occupies. Distinct from `String#size` (codepoint count) and `String#bytesize` (UTF-8 byte count). Used for column alignment of hint overlays. Computed via UAX #11 East Asian Width — see `src/fingers/display_width.cr`.
