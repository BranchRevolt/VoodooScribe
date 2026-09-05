#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 WarpCoreDev
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Fetches static, LGPL FFmpeg binaries for every platform Tauri bundles, and
# names each one with its Rust target triple so Tauri picks it up as a sidecar
# (externalBin "binaries/ffmpeg" -> binaries/ffmpeg-<triple>[.exe]).
#
# Binaries are intentionally git-ignored (tens of MB each). Run this once after
# cloning, and in CI before `tauri build`. Re-running overwrites existing files.
#
# LGPL builds (no --enable-gpl) are preferred: the app only decodes audio, so the
# GPL-only components are dead weight. Linux and Windows come from BtbN's official
# LGPL builds; BtbN publishes no LGPL static build for macOS, see below.
#
# Usage:
#   ./fetch-ffmpeg.sh            # fetch only the host platform (fast, for dev)
#   ./fetch-ffmpeg.sh --all      # fetch every release target (for CI/release)

set -euo pipefail
cd "$(dirname "$0")"

BTBN="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest"

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing tool: $1" >&2; exit 1; }; }
need curl; need tar; need unzip; need xz

# Download $1 (BtbN archive base name), extract bin/ffmpeg, save as binaries/$2
fetch_btbn() {
  local archive="$1" out="$2"
  local tmp; tmp="$(mktemp -d)"
  echo "→ $out  ($archive)"
  case "$archive" in
    *.zip)
      curl -fL --retry 3 -o "$tmp/a.zip" "$BTBN/$archive"
      unzip -qo "$tmp/a.zip" -d "$tmp"
      cp "$tmp"/*/bin/ffmpeg.exe "$out"
      ;;
    *.tar.xz)
      curl -fL --retry 3 -o "$tmp/a.tar.xz" "$BTBN/$archive"
      tar -xJf "$tmp/a.tar.xz" -C "$tmp"
      cp "$tmp"/*/bin/ffmpeg "$out"
      chmod +x "$out"
      ;;
  esac
  rm -rf "$tmp"
}

fetch_linux_x64()  { fetch_btbn "ffmpeg-master-latest-linux64-lgpl.tar.xz"   "ffmpeg-x86_64-unknown-linux-gnu"; }
fetch_linux_arm()  { fetch_btbn "ffmpeg-master-latest-linuxarm64-lgpl.tar.xz" "ffmpeg-aarch64-unknown-linux-gnu"; }
fetch_win_x64()    { fetch_btbn "ffmpeg-master-latest-win64-lgpl.zip"        "ffmpeg-x86_64-pc-windows-msvc.exe"; }
fetch_win_arm()    { fetch_btbn "ffmpeg-master-latest-winarm64-lgpl.zip"     "ffmpeg-aarch64-pc-windows-msvc.exe"; }

# macOS: BtbN publishes no LGPL static build, so these come from osxexperts.net,
# which ships static notarized FFmpeg for both arches. They are GPL builds, which
# is compatible with this app's own GPL-3.0-or-later licence; either way a binary
# release must ship FFmpeg's corresponding source or a written offer for it.
# osxexperts puts the FFmpeg version in the file name (ffmpeg80intel.zip,
# ffmpeg9arm.zip), and drops the old file when a new one is published, so a
# hard-coded link breaks on every release. Read the current name off the index
# page instead, falling back to a known one if the page cannot be parsed.
osxexperts_url() {
  local pattern="$1" fallback="$2" name
  name="$(curl -fsL --retry 3 https://www.osxexperts.net/ \
          | grep -oE "ffmpeg[0-9]+${pattern}\.zip" | head -n1)" || true
  if [ -n "$name" ]; then
    echo "https://www.osxexperts.net/$name"
  else
    echo "$fallback"
  fi
}

fetch_mac() {
  local out="$1" url="$2"
  echo "→ $out  ($url)"
  curl -fL --retry 3 -o "$out.zip" "$url"
  unzip -qo "$out.zip" -d . && rm -f "$out.zip"
  [ -f ffmpeg ] && mv -f ffmpeg "$out"
  chmod +x "$out"
}

fetch_mac_arm() {
  fetch_mac ffmpeg-aarch64-apple-darwin \
    "$(osxexperts_url arm https://www.osxexperts.net/ffmpeg9arm.zip)"
}
fetch_mac_x64() {
  fetch_mac ffmpeg-x86_64-apple-darwin \
    "$(osxexperts_url intel https://www.osxexperts.net/ffmpeg80intel.zip)"
}

if [ "${1:-}" = "--all" ]; then
  fetch_linux_x64; fetch_linux_arm; fetch_win_x64; fetch_win_arm; fetch_mac_arm; fetch_mac_x64
  echo "done: all targets"
  exit 0
fi

# Host platform only (dev default)
host="$(uname -s)-$(uname -m)"
case "$host" in
  Linux-x86_64)  fetch_linux_x64 ;;
  Linux-aarch64) fetch_linux_arm ;;
  Darwin-arm64)  fetch_mac_arm ;;
  Darwin-x86_64) fetch_mac_x64 ;;
  *) echo "unknown host '$host' — use --all or fetch manually" >&2; exit 1 ;;
esac
echo "done: $host"
