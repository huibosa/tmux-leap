#!/usr/bin/env bash

CURRENT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
LEAP_VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$CURRENT_DIR/Cargo.toml" | head -1)

BIN_DIR="$CURRENT_DIR/bin"
BIN="$BIN_DIR/tmux-leap"
VERSION_FILE="$BIN_DIR/version.txt"

_download_binary() {
  local os arch asset url
  os=$(uname -s | tr '[:upper:]' '[:lower:]')
  arch=$(uname -m)
  [[ "$arch" == "arm64" ]] && arch="aarch64"
  asset="tmux-leap-${os}-${arch}"
  url="https://github.com/huibosa/tmux-leap/releases/download/v${LEAP_VERSION}/${asset}"

  mkdir -p "$BIN_DIR"
  if command -v curl &>/dev/null; then
    curl -fsSL "$url" -o "$BIN"
  elif command -v wget &>/dev/null; then
    wget -q "$url" -O "$BIN"
  else
    tmux display-message "[tmux-leap] curl or wget required to download binary"
    exit 1
  fi
  chmod +x "$BIN"
  echo "$LEAP_VERSION" > "$VERSION_FILE"
}

# Download or upgrade when binary is missing or version changed.
if [[ ! -f "$BIN" ]] || [[ ! -f "$VERSION_FILE" ]] || [[ "$(cat "$VERSION_FILE")" != "$LEAP_VERSION" ]]; then
  tmux display-message "[tmux-leap] downloading v${LEAP_VERSION}..."
  _download_binary
fi

# Locate binary: prefer downloaded release, then local build, then PATH.
LEAP_BINARY=""
if [[ -f "$BIN" ]]; then
  LEAP_BINARY="$BIN"
elif [[ -f "$CURRENT_DIR/target/release/tmux-leap" ]]; then
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
