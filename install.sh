#!/usr/bin/env bash
# install.sh — Quick installer for wifimon on Linux/macOS
# Usage: curl -sSf https://raw.githubusercontent.com/cumulus13/wifimon/main/install.sh | bash
set -euo pipefail

REPO="cumulus13/wifimon"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
info() { echo -e "${CYAN}${BOLD}[wifimon]${RESET} $*"; }
ok()   { echo -e "${GREEN}[wifimon]${RESET} $*"; }
err()  { echo -e "${RED}[wifimon] ERROR:${RESET} $*" >&2; exit 1; }

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  linux)
    case "$ARCH" in
      x86_64)        ARTIFACT="wifimon-linux-x86_64.tar.gz" ;;
      aarch64|arm64) ARTIFACT="wifimon-linux-aarch64.tar.gz" ;;
      armv7l)        ARTIFACT="wifimon-linux-armv7.tar.gz" ;;
      *) err "Unsupported arch: $ARCH" ;;
    esac ;;
  darwin)
    case "$ARCH" in
      x86_64) ARTIFACT="wifimon-macos-x86_64.tar.gz" ;;
      arm64)  ARTIFACT="wifimon-macos-aarch64.tar.gz" ;;
      *) err "Unsupported arch: $ARCH" ;;
    esac ;;
  *) err "Unsupported OS: $OS. Download from https://github.com/$REPO/releases" ;;
esac

info "Fetching latest release tag…"
TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
      | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')
[ -z "$TAG" ] && err "Could not determine latest release."
info "Latest: $TAG"

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
info "Downloading $ARTIFACT…"
curl -fsSL "https://github.com/$REPO/releases/download/$TAG/$ARTIFACT" -o "$TMP/$ARTIFACT"
tar xzf "$TMP/$ARTIFACT" -C "$TMP"

info "Installing to $INSTALL_DIR/wifimon…"
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP/wifimon" "$INSTALL_DIR/wifimon"
else
  sudo mv "$TMP/wifimon" "$INSTALL_DIR/wifimon"
fi
chmod +x "$INSTALL_DIR/wifimon"

ok "Installed wifimon $TAG"
"$INSTALL_DIR/wifimon" --version
