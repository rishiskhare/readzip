#!/usr/bin/env sh
# readzip installer — fetches the latest pre-built binary from GitHub Releases.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/rishiskhare/readzip/main/install.sh | sh
#
# Override install location:
#   READZIP_INSTALL_DIR=/usr/local/bin curl -fsSL ... | sh
#
# Pin a version:
#   READZIP_VERSION=v0.1.0 curl -fsSL ... | sh

set -eu

REPO="rishiskhare/readzip"
INSTALL_DIR="${READZIP_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${READZIP_VERSION:-}"

err() { printf "readzip: %s\n" "$1" >&2; exit 1; }
info() { printf "\033[38;5;75m==>\033[0m %s\n" "$1"; }

need() { command -v "$1" >/dev/null 2>&1 || err "missing required tool: $1"; }
need curl
need tar
need uname

os=$(uname -s)
arch=$(uname -m)

case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-gnu" ;;
  *)      err "unsupported OS: $os (Linux and macOS only)" ;;
esac

case "$arch" in
  x86_64|amd64) arch_part="x86_64" ;;
  arm64|aarch64) arch_part="aarch64" ;;
  *) err "unsupported architecture: $arch" ;;
esac

target="${arch_part}-${os_part}"

if [ -z "$VERSION" ]; then
  info "resolving latest release"
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name":' \
    | head -1 \
    | sed -E 's/.*"tag_name":[[:space:]]*"([^"]+)".*/\1/')
  [ -n "$VERSION" ] || err "could not resolve latest release tag"
fi

tarball="readzip-${target}.tar.gz"
url="https://github.com/${REPO}/releases/download/${VERSION}/${tarball}"

info "downloading ${tarball} (${VERSION})"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

curl -fsSL "$url" -o "$tmp/$tarball" || err "download failed: $url"
tar -xzf "$tmp/$tarball" -C "$tmp" || err "extraction failed"

[ -f "$tmp/readzip" ] || err "binary 'readzip' not found in tarball"
chmod +x "$tmp/readzip"

mkdir -p "$INSTALL_DIR"
mv "$tmp/readzip" "$INSTALL_DIR/readzip"

info "installed readzip ${VERSION} → ${INSTALL_DIR}/readzip"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    printf "\n\033[38;5;111mnote:\033[0m %s is not on your PATH.\n" "$INSTALL_DIR"
    printf "      add this line to your shell profile, then re-open the terminal:\n\n"
    printf "        export PATH=\"%s:\$PATH\"\n\n" "$INSTALL_DIR"
    ;;
esac

printf "\n  next:  \033[38;5;75mreadzip init\033[0m       wire up Claude Code + any installed MCP agents\n"
printf "         \033[38;5;75mreadzip demo\033[0m       see compression on a sample file\n"
printf "         \033[38;5;75mreadzip eval <dir>\033[0m measure savings on your own codebase\n\n"
