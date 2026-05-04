#!/usr/bin/env bash
set -euo pipefail

# FastZIP Linux install script
# Installs the GUI binary, CLI binary, desktop entry, and icon.
#
# Usage:
#   sudo ./install-fastzip-linux.sh [--prefix /usr/local]
#
# Without --prefix, defaults to /usr/local (binaries → /usr/local/bin,
# desktop → /usr/local/share/applications, icon → /usr/local/share/icons).

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR=""
DESKTOP_DIR=""
ICON_DIR=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--prefix /usr/local]"
            exit 1
            ;;
    esac
done

BIN_DIR="$PREFIX/bin"
DESKTOP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor/256x256/apps"

if [[ "$(id -u)" -ne 0 ]]; then
    echo "This script must be run as root (sudo)."
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== FastZIP Linux Install ==="
echo "Prefix: $PREFIX"
echo ""

# ── Binaries ──────────────────────────────────────────────────────

echo "Installing binaries to $BIN_DIR ..."
mkdir -p "$BIN_DIR"

if [[ -f "$REPO_ROOT/target/release/fastzip" ]]; then
    cp "$REPO_ROOT/target/release/fastzip" "$BIN_DIR/fastzip"
    chmod 755 "$BIN_DIR/fastzip"
    echo "  → fastzip (GUI)"
else
    echo "  ⚠ fastzip GUI binary not found (build with: cargo build --release)"
fi

if [[ -f "$REPO_ROOT/target/release/fastzip-cli" ]]; then
    cp "$REPO_ROOT/target/release/fastzip-cli" "$BIN_DIR/fastzip-cli"
    chmod 755 "$BIN_DIR/fastzip-cli"
    echo "  → fastzip-cli"
else
    echo "  ⚠ fastzip-cli binary not found (build with: cargo build --release --bin fastzip-cli)"
fi

# ── Desktop entry ─────────────────────────────────────────────────

echo "Installing desktop entry to $DESKTOP_DIR ..."
mkdir -p "$DESKTOP_DIR"
cp "$REPO_ROOT/assets/fastzip.desktop" "$DESKTOP_DIR/fastzip.desktop"
chmod 644 "$DESKTOP_DIR/fastzip.desktop"
echo "  → fastzip.desktop"

# ── Icon ──────────────────────────────────────────────────────────

echo "Installing icon to $ICON_DIR ..."
mkdir -p "$ICON_DIR"
if [[ -f "$REPO_ROOT/assets/fastzip-icon.png" ]]; then
    cp "$REPO_ROOT/assets/fastzip-icon.png" "$ICON_DIR/fastzip.png"
    chmod 644 "$ICON_DIR/fastzip.png"
    echo "  → fastzip.png"
fi

# ── Done ──────────────────────────────────────────────────────────

echo ""
echo "Installation complete."
echo ""
echo "You can now run:"
echo "  fastzip          # launch GUI"
echo "  fastzip-cli      # CLI mode"
echo ""
echo "To uninstall, run:"
echo "  sudo rm -f $BIN_DIR/fastzip $BIN_DIR/fastzip-cli"
echo "  sudo rm -f $DESKTOP_DIR/fastzip.desktop"
echo "  sudo rm -f $ICON_DIR/fastzip.png"
echo "  sudo update-desktop-database 2>/dev/null || true"
