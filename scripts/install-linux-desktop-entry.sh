#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 WarpCoreDev
# SPDX-License-Identifier: GPL-3.0-or-later

# Installs a desktop entry and icons for the DEVELOPMENT build.
#
# Why this is needed: under Wayland a window cannot set its own icon. The
# compositor matches the window's app_id against an installed .desktop file and
# takes the icon from there — with no match you get the generic placeholder.
# Packaged builds (.deb/.AppImage/rpm) ship their own entry, but `tauri dev`
# doesn't, so a developer's window is always the placeholder until this runs.
#
# `enableGTKAppId: true` in tauri.conf.json pins the app_id to the bundle
# identifier, so the entry below must be named after that identifier.
#
# Usage:  ./scripts/install-linux-desktop-entry.sh [--uninstall]
set -euo pipefail

APP_ID="com.voodooscribe.app"
APP_NAME="VoodooScribe"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICON_SRC="$REPO_ROOT/src-tauri/icons"
BIN="$REPO_ROOT/src-tauri/target/debug/voodooscribe"

DESKTOP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$DESKTOP_DIR/$APP_ID.desktop"
  for size in 32 128 256 512; do
    rm -f "$ICON_DIR/${size}x${size}/apps/$APP_ID.png"
  done
  gtk-update-icon-cache -qtf "$ICON_DIR" 2>/dev/null || true
  update-desktop-database -q "$DESKTOP_DIR" 2>/dev/null || true
  echo "Removed $APP_ID desktop entry and icons."
  exit 0
fi

# Icon names must match the Icon= key below, which in turn matches the app_id.
install -Dm644 "$ICON_SRC/32x32.png"       "$ICON_DIR/32x32/apps/$APP_ID.png"
install -Dm644 "$ICON_SRC/128x128.png"     "$ICON_DIR/128x128/apps/$APP_ID.png"
install -Dm644 "$ICON_SRC/128x128@2x.png"  "$ICON_DIR/256x256/apps/$APP_ID.png"
install -Dm644 "$ICON_SRC/icon.png"        "$ICON_DIR/512x512/apps/$APP_ID.png"

mkdir -p "$DESKTOP_DIR"
cat > "$DESKTOP_DIR/$APP_ID.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=Offline audio and video transcription
Exec=$BIN
Icon=$APP_ID
Terminal=false
Categories=AudioVideo;Audio;Utility;
# X11 matches on WM_CLASS; KWin also consults this under Wayland.
StartupWMClass=$APP_ID
EOF

gtk-update-icon-cache -qtf "$ICON_DIR" 2>/dev/null || true
update-desktop-database -q "$DESKTOP_DIR" 2>/dev/null || true

echo "Installed $DESKTOP_DIR/$APP_ID.desktop"
echo "Icons under $ICON_DIR/*/apps/$APP_ID.png"
echo "Restart the app for the compositor to pick it up."
