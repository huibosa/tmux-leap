#!/usr/bin/env bash

CURRENT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Locate the binary: prefer release build in repo, fall back to PATH.
if [[ -f "$CURRENT_DIR/target/release/tmux-leap" ]]; then
  LEAP_BINARY="$CURRENT_DIR/target/release/tmux-leap"
elif command -v tmux-leap &>/dev/null; then
  LEAP_BINARY="tmux-leap"
fi

if [[ -z "$LEAP_BINARY" ]]; then
  tmux display-message "[tmux-leap] run 'cargo install --path .' or download a release binary"
  exit 0
fi

if [[ "$TERM" == "dumb" ]]; then
  LEAP_TERM=$(tmux show-option -gqv default-terminal)
else
  LEAP_TERM="$TERM"
fi

tmux run "TERM=$LEAP_TERM $LEAP_BINARY load-config"
exit $?
