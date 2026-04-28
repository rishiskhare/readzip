#!/usr/bin/env sh
# readzip uninstaller — companion to install.sh.
# Reverses everything install.sh did:
#   1. Calls `readzip uninstall` (removes Claude Code hook +
#      Codex AGENTS hint + cache)
#   2. Deletes the binary at ${READZIP_INSTALL_DIR:-$HOME/.local/bin}/readzip
#   3. Deletes the config dir at ${HOME}/.config/readzip
#   4. Reports what it did
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/rishiskhare/readzip/main/uninstall.sh | sh
#
# Override binary location:
#   READZIP_INSTALL_DIR=/usr/local/bin curl -fsSL ... | sh
#
# Keep ~/.cache/readzip/stats.tsv:
#   READZIP_KEEP_STATS=1 curl -fsSL ... | sh

set -eu

INSTALL_DIR="${READZIP_INSTALL_DIR:-$HOME/.local/bin}"
KEEP_STATS="${READZIP_KEEP_STATS:-}"

info() { printf "\033[38;5;75m==>\033[0m %s\n" "$1"; }
warn() { printf "\033[38;5;111mnote:\033[0m %s\n" "$1" >&2; }

binary="${INSTALL_DIR}/readzip"

# 1. Drive the in-binary uninstall first (Claude hook, Codex hint, cache).
if [ -x "$binary" ]; then
  info "running ${binary} uninstall"
  if [ -n "$KEEP_STATS" ]; then
    "$binary" uninstall --keep-cache || warn "readzip uninstall returned non-zero; continuing"
  else
    "$binary" uninstall || warn "readzip uninstall returned non-zero; continuing"
  fi
else
  warn "readzip binary not found at ${binary}; skipping in-binary cleanup"
fi

# 2. Delete the binary itself.
if [ -e "$binary" ]; then
  info "removing ${binary}"
  rm -f "$binary"
fi

# 3. Delete the config dir.
config_dir="${HOME}/.config/readzip"
if [ -d "$config_dir" ]; then
  info "removing ${config_dir}"
  rm -rf "$config_dir"
fi

# 4. Stats wipe (the in-binary uninstall already did this unless --keep-cache;
#    this is the belt-and-suspenders path for the curl-piped flow).
if [ -z "$KEEP_STATS" ] && [ -d "${HOME}/.cache/readzip" ]; then
  info "removing ${HOME}/.cache/readzip"
  rm -rf "${HOME}/.cache/readzip"
fi

printf "\nreadzip is fully uninstalled. Restart your AI tool to clear its in-memory hook reference.\n"
